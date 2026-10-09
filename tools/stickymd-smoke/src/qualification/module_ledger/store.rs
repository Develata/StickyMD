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

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::super::receipt;

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
        wait_for_lock(|| file.try_lock_shared(), "reading")?;
        self.ensure_no_links()?;
        Ok(StoreGuard { _file: file })
    }

    /// Exclusive lock for archiving, publishing and pruning.
    pub(in crate::qualification) fn write_guard(&self) -> Result<StoreGuard, String> {
        let file = self.lock_file()?;
        wait_for_lock(|| file.try_lock(), "writing")?;
        self.ensure_no_links()?;
        Ok(StoreGuard { _file: file })
    }

    /// Entries below the store root must be plain files and directories. A junction or
    /// symlink inside would let a path outside the reserved location alias trusted
    /// records, so the store refuses to operate instead. (The root itself may be
    /// redirected: the reserved-path check resolves the root.)
    fn ensure_no_links(&self) -> Result<(), String> {
        let mut pending = vec![self.root.clone()];
        while let Some(directory) = pending.pop() {
            let entries = fs::read_dir(&directory)
                .map_err(|error| format!("cannot list {}: {error}", directory.display()))?;
            for entry in entries {
                let entry = entry
                    .map_err(|error| format!("cannot list {}: {error}", directory.display()))?;
                // DirEntry::file_type does not follow links; junctions count as links.
                let file_type = entry.file_type().map_err(|error| {
                    format!("cannot inspect {}: {error}", entry.path().display())
                })?;
                if file_type.is_symlink() {
                    return Err(format!(
                        "ledger store contains a link and is not trusted: {}",
                        entry.path().display()
                    ));
                }
                if file_type.is_dir() {
                    pending.push(entry.path());
                }
            }
        }
        Ok(())
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
        keep: &Path,
    ) -> Result<(), String> {
        let mut records = Vec::new();
        for path in record_files(&self.key_directory("modules", module)?)? {
            let record = self.read_record(&path)?;
            records.push((record.recorded_at, record.evidence_file, path));
        }
        // The record just published is always retained, whatever its timestamp (equal
        // seconds or a clock moved backwards); the others are kept newest first.
        records.sort_by(|left, right| {
            (right.2 == keep)
                .cmp(&(left.2 == keep))
                .then_with(|| right.0.cmp(&left.0))
                .then_with(|| left.2.cmp(&right.2))
        });
        let mut referenced = Vec::new();
        for (index, (_, evidence, path)) in records.iter().enumerate() {
            if index < RETAINED_RECORDS || path == keep {
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
            let record = self.read_record(&path)?;
            let evidence = self.evidence(&record.evidence_file)?;
            let bytes = fs::read(&evidence)
                .map_err(|error| format!("cannot read {}: {error}", evidence.display()))?;
            // A damaged archive could still be valid JSON with fewer references; only
            // evidence matching its record's digest may decide what cleanup keeps.
            if receipt::sha256_bytes(&bytes)? != record.evidence_sha256 {
                return Err(format!(
                    "ledger evidence {} does not match its record digest",
                    evidence.display()
                ));
            }
            documents.push(String::from_utf8(bytes).map_err(|error| {
                format!(
                    "ledger evidence {} is not UTF-8: {error}",
                    evidence.display()
                )
            })?);
        }
        Ok(documents)
    }

    /// The fields cleanup relies on, from a strict parse of the whole record.
    fn read_record(&self, path: &Path) -> Result<RecordSummary, String> {
        let text = receipt::read_receipt(path)?;
        let parsed = crate::release::json::parse(&text)
            .map_err(|error| format!("ledger record {} is malformed: {error}", path.display()))?;
        let evidence_file = parsed.field("evidence_file")?.string()?.to_owned();
        self.evidence(&evidence_file)?;
        let evidence_sha256 = parsed.field("evidence_sha256")?.string()?.to_owned();
        receipt::validate_sha256(&evidence_sha256, "ledger record evidence SHA-256")?;
        Ok(RecordSummary {
            recorded_at: parsed.field("recorded_at_unix")?.unsigned()?,
            evidence_file,
            evidence_sha256,
        })
    }
}

struct RecordSummary {
    recorded_at: u64,
    evidence_file: String,
    evidence_sha256: String,
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

/// Longest a qualification command waits for another process's store operation.
const LOCK_WAIT: Duration = Duration::from_secs(120);

/// Bounded wait: store operations take seconds, so a holder past the limit is hung or
/// suspended and the caller gets a diagnosable error instead of blocking forever.
fn wait_for_lock(
    mut attempt: impl FnMut() -> Result<(), TryLockError>,
    purpose: &str,
) -> Result<(), String> {
    let deadline = Instant::now() + LOCK_WAIT;
    loop {
        match attempt() {
            Ok(()) => return Ok(()),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(TryLockError::WouldBlock) => {
                return Err(format!(
                    "ledger store stayed locked by another qualification process for {} s while {purpose}",
                    LOCK_WAIT.as_secs()
                ));
            }
            Err(TryLockError::Error(error)) => {
                return Err(format!("cannot lock ledger store for {purpose}: {error}"));
            }
        }
    }
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
