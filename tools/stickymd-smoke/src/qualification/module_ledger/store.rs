//! Clone-wide storage for qualification last-success records.
//!
//! Every linked worktree of one clone resolves the same git common directory, so a
//! fresh release worktree sees successes recorded in any earlier worktree. Records are
//! keyed by their input fingerprint: concurrent writers with different inputs never
//! replace each other, and lookup is a direct path, not a scan.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::{json, receipt};

/// Records kept per ledger key beyond the recent-write grace window.
pub(in crate::qualification) const RETAINED_RECORDS: usize = 8;
/// Records and archives younger than this are never pruned, so a concurrent writer that
/// has archived evidence but not yet published its record cannot lose it.
const RECENT_GRACE_SECONDS: u64 = 24 * 60 * 60;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct LedgerStore {
    root: PathBuf,
}

impl LedgerStore {
    /// Resolve the store from a repository root. Fingerprinting already requires git,
    /// so a missing or broken checkout fails closed instead of using a private store.
    pub(in crate::qualification) fn for_repository(repository: &Path) -> Result<Self, String> {
        let output = Command::new("git")
            .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
            .current_dir(repository)
            .output()
            .map_err(|error| format!("cannot start git rev-parse for the ledger store: {error}"))?;
        if !output.status.success() {
            return Err("git rev-parse --git-common-dir failed; the qualification ledger requires a git checkout".to_owned());
        }
        let text = String::from_utf8(output.stdout)
            .map_err(|error| format!("git common directory is not UTF-8: {error}"))?;
        let common = PathBuf::from(text.trim_end_matches(['\r', '\n']));
        if !common.is_absolute() || !common.is_dir() {
            return Err(format!(
                "git common directory is not an existing absolute path: {}",
                common.display()
            ));
        }
        Ok(Self::at(
            common.join("stickymd").join("qualification-ledger"),
        ))
    }

    pub(in crate::qualification) fn at(root: PathBuf) -> Self {
        Self { root }
    }

    pub(in crate::qualification) fn root(&self) -> &Path {
        &self.root
    }

    fn key_directory(&self, kind: &str, key: &str) -> Result<PathBuf, String> {
        validate_key(key)?;
        Ok(self.root.join(kind).join(key))
    }

    /// The single record path for `(module, fingerprint)`.
    pub(in crate::qualification) fn module_record(
        &self,
        module: &str,
        fingerprint: &str,
    ) -> Result<PathBuf, String> {
        receipt::validate_sha256(fingerprint, "module input fingerprint")?;
        Ok(self
            .key_directory("modules", module)?
            .join(format!("{fingerprint}.json")))
    }

    pub(in crate::qualification) fn has_module_records(
        &self,
        module: &str,
    ) -> Result<bool, String> {
        let directory = self.key_directory("modules", module)?;
        Ok(record_files(&directory)?.next().is_some())
    }

    /// Content-addressed module evidence archive, `<module>-<sha256>.json`.
    pub(in crate::qualification) fn evidence(&self, file_name: &str) -> Result<PathBuf, String> {
        let (module, rest) = file_name
            .rsplit_once('-')
            .ok_or_else(|| format!("invalid ledger evidence name {file_name}"))?;
        let digest = rest
            .strip_suffix(".json")
            .ok_or_else(|| format!("invalid ledger evidence name {file_name}"))?;
        validate_key(module)?;
        receipt::validate_sha256(digest, "ledger evidence SHA-256")?;
        Ok(self.root.join("evidence").join(file_name))
    }

    /// Content-addressed binary companion evidence (for example G5 screenshots).
    pub(in crate::qualification) fn artifact(&self, sha256: &str) -> Result<PathBuf, String> {
        receipt::validate_sha256(sha256, "ledger artifact SHA-256")?;
        Ok(self.root.join("artifacts").join(sha256))
    }

    /// Keep the newest records of one module plus everything inside the grace window,
    /// then delete module evidence that no remaining record references. Pruning is
    /// best effort: a published success stays valid even if cleanup fails.
    pub(in crate::qualification) fn prune_module(&self, module: &str, keep: &Path) {
        if let Err(error) = self.try_prune_module(module, keep) {
            eprintln!("LEDGER_PRUNE_WARNING={module}: {error}");
        }
    }

    fn try_prune_module(&self, module: &str, keep: &Path) -> Result<(), String> {
        let directory = self.key_directory("modules", module)?;
        let now = unix_seconds();
        let mut records = record_files(&directory)?
            .map(|path| {
                let recorded = receipt::read_receipt(&path)
                    .ok()
                    .and_then(|document| json::u64_field(&document, "recorded_at_unix").ok())
                    .unwrap_or(0);
                (recorded, path)
            })
            .collect::<Vec<_>>();
        records.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
        let mut referenced = Vec::new();
        for (index, (recorded, path)) in records.iter().enumerate() {
            let recent = now.saturating_sub(*recorded) < RECENT_GRACE_SECONDS;
            if index < RETAINED_RECORDS || recent || path == keep {
                if let Ok(document) = receipt::read_receipt(path)
                    && let Ok(name) = json::string_field(&document, "evidence_file")
                {
                    referenced.push(name);
                }
            } else {
                let _ = fs::remove_file(path);
            }
        }
        let prefix = format!("{module}-");
        let evidence_directory = self.root.join("evidence");
        let Ok(entries) = fs::read_dir(&evidence_directory) else {
            return Ok(());
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with(&prefix)
                && name.ends_with(".json")
                && !referenced.iter().any(|kept| kept == &name)
                && older_than_grace(&entry.path(), now)
            {
                let _ = fs::remove_file(entry.path());
            }
        }
        Ok(())
    }

    /// Remove content-addressed artifacts that no caller-listed evidence still references.
    pub(in crate::qualification) fn prune_artifacts(&self, referenced: &[String]) {
        let now = unix_seconds();
        let Ok(entries) = fs::read_dir(self.root.join("artifacts")) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !referenced.iter().any(|kept| kept == &name) && older_than_grace(&entry.path(), now)
            {
                let _ = fs::remove_file(entry.path());
            }
        }
    }

    /// Evidence documents of every retained record of one module.
    pub(in crate::qualification) fn module_evidence_documents(
        &self,
        module: &str,
    ) -> Result<Vec<String>, String> {
        let directory = self.key_directory("modules", module)?;
        let mut documents = Vec::new();
        for path in record_files(&directory)? {
            let Ok(record) = receipt::read_receipt(&path) else {
                continue;
            };
            let Ok(name) = json::string_field(&record, "evidence_file") else {
                continue;
            };
            if let Ok(evidence) = self.evidence(&name)
                && let Ok(document) = receipt::read_receipt(&evidence)
            {
                documents.push(document);
            }
        }
        Ok(documents)
    }
}

fn record_files(directory: &Path) -> Result<impl Iterator<Item = PathBuf>, String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => Some(entries),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(format!(
                "cannot list ledger records {}: {error}",
                directory.display()
            ));
        }
    };
    Ok(entries.into_iter().flatten().flatten().filter_map(|entry| {
        let path = entry.path();
        let name = path.file_name()?.to_str()?;
        // Atomic writers publish through dot-prefixed temporaries; only `<sha256>.json` counts.
        let digest = name.strip_suffix(".json")?;
        receipt::validate_sha256(digest, "record").ok()?;
        Some(path)
    }))
}

/// Ledger keys become directory names; accept only the registry's ASCII spellings.
fn validate_key(key: &str) -> Result<(), String> {
    if key.is_empty()
        || key.len() > 64
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        || key.starts_with('-')
    {
        return Err(format!("invalid qualification ledger key `{key}`"));
    }
    Ok(())
}

pub(in crate::qualification) fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

fn older_than_grace(path: &Path, now: u64) -> bool {
    fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .is_some_and(|modified| now.saturating_sub(modified.as_secs()) >= RECENT_GRACE_SECONDS)
}
