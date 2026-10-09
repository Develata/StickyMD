//! Clone-wide storage for qualification last-success records.
//!
//! Every linked worktree of one clone resolves the same git common directory, so a
//! fresh release worktree sees successes recorded in any earlier worktree. Records are
//! keyed by their input fingerprint: writers with different inputs never replace each
//! other, and lookup is a direct path, not a scan.
//!
//! Concurrency: one OS file lock (`.lock`) serializes every mutation (archive, publish,
//! prune) and is held shared by readers while they follow a record to its evidence, so
//! cleanup can never delete something a reader or another writer is about to use. The
//! OS releases the lock when a process exits, so a crash cannot strand it.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::{json, receipt};

/// Records kept per module; older records and their unreferenced evidence are removed.
pub(in crate::qualification) const RETAINED_RECORDS: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::qualification) struct LedgerStore {
    root: PathBuf,
}

/// Holds the store lock until dropped; closing the handle releases it.
#[must_use = "the store lock is released when the guard is dropped"]
pub(in crate::qualification) struct StoreGuard {
    _file: File,
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

    /// Shared lock for following records to their evidence.
    pub(in crate::qualification) fn read_guard(&self) -> Result<StoreGuard, String> {
        let file = self.lock_file()?;
        file.lock_shared()
            .map_err(|error| format!("cannot lock ledger store for reading: {error}"))?;
        Ok(StoreGuard { _file: file })
    }

    /// Exclusive lock for archiving, publishing and pruning.
    pub(in crate::qualification) fn write_guard(&self) -> Result<StoreGuard, String> {
        let file = self.lock_file()?;
        file.lock()
            .map_err(|error| format!("cannot lock ledger store for writing: {error}"))?;
        Ok(StoreGuard { _file: file })
    }

    fn lock_file(&self) -> Result<File, String> {
        fs::create_dir_all(&self.root).map_err(|error| {
            format!(
                "cannot create ledger store {}: {error}",
                self.root.display()
            )
        })?;
        OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(self.root.join(".lock"))
            .map_err(|error| format!("cannot open ledger store lock: {error}"))
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

    /// Cheap pre-check before fingerprinting; a racing writer only delays reuse.
    pub(in crate::qualification) fn has_module_records(
        &self,
        module: &str,
    ) -> Result<bool, String> {
        Ok(!record_files(&self.key_directory("modules", module)?)?.is_empty())
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

    /// Keep the newest records of one module and delete that module's evidence that no
    /// remaining record references. Requires the write guard. Any record or directory
    /// that cannot be read stops cleanup: unknown references are never treated as unused.
    pub(in crate::qualification) fn prune_module(
        &self,
        _guard: &StoreGuard,
        module: &str,
    ) -> Result<(), String> {
        let mut records = Vec::new();
        for path in record_files(&self.key_directory("modules", module)?)? {
            let document = receipt::read_receipt(&path)?;
            records.push((
                json::u64_field(&document, "recorded_at_unix")?,
                json::string_field(&document, "evidence_file")?,
                path,
            ));
        }
        // Newest first; ties keep a stable order by path.
        records.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.2.cmp(&right.2)));
        let mut referenced = Vec::new();
        for (index, (_, evidence, path)) in records.iter().enumerate() {
            if index < RETAINED_RECORDS {
                referenced.push(evidence.clone());
            } else {
                remove(path)?;
            }
        }
        let prefix = format!("{module}-");
        for name in directory_names(&self.root.join("evidence"))? {
            if name.starts_with(&prefix) && name.ends_with(".json") && !referenced.contains(&name) {
                remove(&self.root.join("evidence").join(&name))?;
            }
        }
        Ok(())
    }

    /// Delete artifacts outside `referenced`. Requires the write guard; the caller must
    /// pass a complete reference set or not call this at all.
    pub(in crate::qualification) fn prune_artifacts(
        &self,
        _guard: &StoreGuard,
        referenced: &[String],
    ) -> Result<(), String> {
        for name in directory_names(&self.root.join("artifacts"))? {
            if !referenced.contains(&name) {
                remove(&self.root.join("artifacts").join(&name))?;
            }
        }
        Ok(())
    }

    /// Evidence documents of every retained record of one module; fails on any
    /// unreadable record so callers cannot mistake a partial scan for a complete one.
    pub(in crate::qualification) fn module_evidence_documents(
        &self,
        module: &str,
    ) -> Result<Vec<String>, String> {
        let mut documents = Vec::new();
        for path in record_files(&self.key_directory("modules", module)?)? {
            let record = receipt::read_receipt(&path)?;
            let evidence = self.evidence(&json::string_field(&record, "evidence_file")?)?;
            documents.push(receipt::read_receipt(&evidence)?);
        }
        Ok(documents)
    }
}

/// Published `<sha256>.json` records; temporaries and foreign names are ignored.
fn record_files(directory: &Path) -> Result<Vec<PathBuf>, String> {
    Ok(directory_names(directory)?
        .into_iter()
        .filter(|name| {
            name.strip_suffix(".json")
                .is_some_and(|digest| receipt::validate_sha256(digest, "record").is_ok())
        })
        .map(|name| directory.join(name))
        .collect())
}

/// Every entry name, or an error if the directory exists but cannot be fully listed.
fn directory_names(directory: &Path) -> Result<Vec<String>, String> {
    let entries = match fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(format!("cannot list {}: {error}", directory.display())),
    };
    let mut names = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|error| format!("cannot list {}: {error}", directory.display()))?;
        let name = entry.file_name().into_string().map_err(|name| {
            format!(
                "ledger store entry is not UTF-8: {}",
                name.to_string_lossy()
            )
        })?;
        // Atomic writers publish through dot-prefixed temporaries.
        if !name.starts_with('.') {
            names.push(name);
        }
    }
    names.sort();
    Ok(names)
}

fn remove(path: &Path) -> Result<(), String> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot prune {}: {error}", path.display())),
    }
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

#[cfg(test)]
mod tests;

pub(in crate::qualification) fn unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}
