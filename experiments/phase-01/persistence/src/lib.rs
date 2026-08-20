//! Deletable Phase 1 portable-persistence verification.
//!
//! plan_ref: docs/plan/05_document_persistence.md#atomic-save

use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

#[cfg(windows)]
pub mod windows_adapter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    Crlf,
    Lf,
}

pub fn detect_line_ending(text: &str) -> LineEnding {
    let bytes = text.as_bytes();
    let mut crlf = 0usize;
    let mut lf = 0usize;
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            crlf += 1;
            index += 2;
        } else {
            if bytes[index] == b'\n' {
                lf += 1;
            }
            index += 1;
        }
    }
    if crlf >= lf {
        LineEnding::Crlf
    } else {
        LineEnding::Lf
    }
}

pub fn to_internal(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn to_disk(text: &str, line_ending: LineEnding) -> String {
    match line_ending {
        LineEnding::Crlf => text.replace('\n', "\r\n"),
        LineEnding::Lf => text.to_owned(),
    }
}

pub fn strip_utf8_bom(bytes: &[u8]) -> &[u8] {
    bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes)
}

fn digest(bytes: &[u8]) -> [u8; 32] {
    Sha256::digest(bytes).into()
}

pub fn directory_identity(canonical_path: &str) -> String {
    let normalized = canonical_path
        .trim_end_matches(['\\', '/'])
        .replace('/', "\\")
        .to_lowercase();
    let hash = digest(normalized.as_bytes());
    let mut hex = String::with_capacity(64);
    for byte in hash {
        use fmt::Write as _;
        let _ = write!(hex, "{byte:02x}");
    }
    format!("Local\\StickyMD.{hex}")
}

#[derive(Debug, Clone, Copy)]
pub struct FileCandidate<'a> {
    pub bytes: &'a [u8],
    pub modified: SystemTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryDecision {
    None,
    InvalidTemp,
    CleanRedundantTemp,
    KeepCurrentStaleTemp,
    OfferRecovery,
}

pub fn decide_recovery(
    temp: Option<FileCandidate<'_>>,
    note: Option<FileCandidate<'_>>,
) -> RecoveryDecision {
    let Some(temp) = temp else {
        return RecoveryDecision::None;
    };
    let temp_bytes = strip_utf8_bom(temp.bytes);
    if std::str::from_utf8(temp_bytes).is_err() {
        return RecoveryDecision::InvalidTemp;
    }
    let Some(note) = note else {
        return RecoveryDecision::OfferRecovery;
    };
    if strip_utf8_bom(note.bytes) == temp_bytes {
        return RecoveryDecision::CleanRedundantTemp;
    }
    if temp.modified > note.modified {
        RecoveryDecision::OfferRecovery
    } else {
        RecoveryDecision::KeepCurrentStaleTemp
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalChangeDecision {
    IgnoreOwnWrite,
    ReloadCleanBuffer,
    EnterConflict,
}

pub fn decide_external_change(
    observed_hash: [u8; 32],
    last_saved_hash: Option<[u8; 32]>,
    document_dirty: bool,
) -> ExternalChangeDecision {
    if last_saved_hash == Some(observed_hash) {
        ExternalChangeDecision::IgnoreOwnWrite
    } else if document_dirty {
        ExternalChangeDecision::EnterConflict
    } else {
        ExternalChangeDecision::ReloadCleanBuffer
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AtomicStage {
    BeforeTempCreate,
    WritingOrFlushingTemp,
    AfterTempFlushedBeforeReplace,
    ReplacingTarget,
}

#[derive(Debug)]
pub struct AtomicWriteError {
    pub stage: AtomicStage,
    pub source: io::Error,
    pub recoverable_temp: Option<PathBuf>,
}

impl fmt::Display for AtomicWriteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "atomic write failed at {:?}: {}",
            self.stage, self.source
        )
    }
}

impl std::error::Error for AtomicWriteError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

fn fail(stage: AtomicStage, source: io::Error, temp: Option<PathBuf>) -> AtomicWriteError {
    AtomicWriteError {
        stage,
        source,
        recoverable_temp: temp,
    }
}

pub fn atomic_write_with_hook(
    directory: &Path,
    filename: &str,
    bytes: &[u8],
    mut hook: impl FnMut(AtomicStage) -> io::Result<()>,
) -> Result<(), AtomicWriteError> {
    if Path::new(filename).components().count() != 1 {
        return Err(fail(
            AtomicStage::BeforeTempCreate,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "filename must be one path component",
            ),
            None,
        ));
    }
    hook(AtomicStage::BeforeTempCreate)
        .map_err(|error| fail(AtomicStage::BeforeTempCreate, error, None))?;
    let target = directory.join(filename);
    let temp = directory.join(format!("{filename}.tmp"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temp)
        .map_err(|error| fail(AtomicStage::BeforeTempCreate, error, None))?;
    if let Err(error) = file
        .write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
    {
        return Err(fail(AtomicStage::WritingOrFlushingTemp, error, Some(temp)));
    }
    drop(file);
    hook(AtomicStage::AfterTempFlushedBeforeReplace).map_err(|error| {
        fail(
            AtomicStage::AfterTempFlushedBeforeReplace,
            error,
            Some(temp.clone()),
        )
    })?;

    #[cfg(windows)]
    let replace_result = windows_adapter::replace_flushed_temp(&target, &temp);
    #[cfg(not(windows))]
    let replace_result = fs::rename(&temp, &target);

    replace_result.map_err(|error| fail(AtomicStage::ReplacingTarget, error, Some(temp)))
}

pub fn atomic_write(
    directory: &Path,
    filename: &str,
    bytes: &[u8],
) -> Result<(), AtomicWriteError> {
    atomic_write_with_hook(directory, filename, bytes, |_| Ok(()))
}

pub fn writable_check(directory: &Path) -> io::Result<()> {
    fs::create_dir_all(directory)?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let probe = directory.join(format!(
        ".stickymd-write-test-{}-{nonce}",
        std::process::id()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&probe)?;
    let write_result = file
        .write_all(b"ok")
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all());
    drop(file);
    let cleanup_result = fs::remove_file(&probe);
    match (write_result, cleanup_result) {
        (Err(write_error), _) => Err(write_error),
        (Ok(()), Err(cleanup_error)) => Err(cleanup_error),
        (Ok(()), Ok(())) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Duration;

    static NEXT: AtomicU64 = AtomicU64::new(0);

    struct TempDir(PathBuf);

    impl TempDir {
        fn new(label: &str) -> Self {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "stickymd-phase1-{label}-{}-{id}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn mixed_newlines_use_majority_and_tie_prefers_crlf() {
        assert_eq!(detect_line_ending("a\r\nb\r\nc\n"), LineEnding::Crlf);
        assert_eq!(detect_line_ending("a\nb\nc\r\n"), LineEnding::Lf);
        assert_eq!(detect_line_ending("a\r\nb\n"), LineEnding::Crlf);
    }

    #[test]
    fn recovery_requires_newer_different_valid_temp() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(20);
        let old = SystemTime::UNIX_EPOCH + Duration::from_secs(10);
        let note = FileCandidate {
            bytes: b"note",
            modified: now,
        };
        assert_eq!(
            decide_recovery(
                Some(FileCandidate {
                    bytes: b"temp",
                    modified: now + Duration::from_secs(1)
                }),
                Some(note)
            ),
            RecoveryDecision::OfferRecovery
        );
        assert_eq!(
            decide_recovery(
                Some(FileCandidate {
                    bytes: b"temp",
                    modified: old
                }),
                Some(note)
            ),
            RecoveryDecision::KeepCurrentStaleTemp
        );
        assert_eq!(
            decide_recovery(
                Some(FileCandidate {
                    bytes: b"note",
                    modified: old
                }),
                Some(note)
            ),
            RecoveryDecision::CleanRedundantTemp
        );
        assert_eq!(
            decide_recovery(
                Some(FileCandidate {
                    bytes: &[0xff],
                    modified: now
                }),
                Some(note)
            ),
            RecoveryDecision::InvalidTemp
        );
    }

    #[test]
    fn failure_before_temp_creation_preserves_original() {
        let dir = TempDir::new("before-temp");
        fs::write(dir.0.join("note.md"), b"original").expect("seed original");
        let error = atomic_write_with_hook(&dir.0, "note.md", b"new", |stage| {
            if stage == AtomicStage::BeforeTempCreate {
                Err(io::Error::other("injected"))
            } else {
                Ok(())
            }
        })
        .expect_err("injected failure must propagate");
        assert_eq!(error.stage, AtomicStage::BeforeTempCreate);
        assert_eq!(
            fs::read(dir.0.join("note.md")).expect("read original"),
            b"original"
        );
        assert!(!dir.0.join("note.md.tmp").exists());
    }

    #[test]
    fn failure_after_flush_preserves_original_and_recoverable_temp() {
        let dir = TempDir::new("after-flush");
        fs::write(dir.0.join("note.md"), b"original").expect("seed original");
        let error = atomic_write_with_hook(&dir.0, "note.md", b"new", |stage| {
            if stage == AtomicStage::AfterTempFlushedBeforeReplace {
                Err(io::Error::other("injected"))
            } else {
                Ok(())
            }
        })
        .expect_err("injected failure must propagate");
        assert_eq!(error.stage, AtomicStage::AfterTempFlushedBeforeReplace);
        assert_eq!(
            fs::read(dir.0.join("note.md")).expect("read original"),
            b"original"
        );
        assert_eq!(
            fs::read(dir.0.join("note.md.tmp")).expect("read temp"),
            b"new"
        );
    }

    #[test]
    fn first_create_and_existing_replace_land_complete_content() {
        let dir = TempDir::new("success");
        atomic_write(&dir.0, "note.md", b"first").expect("first atomic create");
        assert_eq!(
            fs::read(dir.0.join("note.md")).expect("read first"),
            b"first"
        );
        atomic_write(&dir.0, "note.md", b"second complete").expect("atomic replace");
        assert_eq!(
            fs::read(dir.0.join("note.md")).expect("read second"),
            b"second complete"
        );
        assert!(!dir.0.join("note.md.tmp").exists());
    }

    #[test]
    fn unresolved_stale_temp_is_not_overwritten() {
        let dir = TempDir::new("stale-temp");
        fs::write(dir.0.join("note.md"), b"original").expect("seed original");
        fs::write(dir.0.join("note.md.tmp"), b"recover me").expect("seed temp");
        let error = atomic_write(&dir.0, "note.md", b"new").expect_err("must refuse stale temp");
        assert_eq!(error.stage, AtomicStage::BeforeTempCreate);
        assert_eq!(
            fs::read(dir.0.join("note.md")).expect("read original"),
            b"original"
        );
        assert_eq!(
            fs::read(dir.0.join("note.md.tmp")).expect("read temp"),
            b"recover me"
        );
    }

    #[test]
    fn external_change_decision_respects_dirty_state() {
        let own = digest(b"own");
        let external = digest(b"external");
        assert_eq!(
            decide_external_change(own, Some(own), true),
            ExternalChangeDecision::IgnoreOwnWrite
        );
        assert_eq!(
            decide_external_change(external, Some(own), false),
            ExternalChangeDecision::ReloadCleanBuffer
        );
        assert_eq!(
            decide_external_change(external, Some(own), true),
            ExternalChangeDecision::EnterConflict
        );
    }

    #[test]
    fn writable_check_removes_its_probe() {
        let dir = TempDir::new("writable-check");
        writable_check(&dir.0).expect("writable directory should pass");
        let remaining = fs::read_dir(&dir.0)
            .expect("read test directory")
            .collect::<Result<Vec<_>, _>>()
            .expect("enumerate test directory");
        assert!(remaining.is_empty());
    }

    #[test]
    fn directory_identity_is_stable_for_case_and_separator_variants() {
        assert_eq!(
            directory_identity(r"\\?\C:\Notes\StickyMD\"),
            directory_identity("//?/c:/notes/stickymd")
        );
    }
}
