//! Startup loading after single-instance and writable-directory checks.
//!
//! plan_ref: docs/plan/05_document_persistence.md#startup-sequence

use std::path::PathBuf;
use std::sync::Arc;

use stickymd_app::config::RuntimeConfig;
use stickymd_app::flow::{
    NoteDocumentPlan, NoteRefusal, NoteStartupStep, document_from_fact, invalid_temporary_warning,
    oversized_temporary_warning, plan_note_startup,
};
use stickymd_app::ports::{PersistMode, PersistRequest, PersistResult};
use stickymd_core::{DocumentState, PreserveExpectation, RecoveryCandidate};
use thiserror::Error;

use crate::config::{ConfigStorageError, load_config, save_config};
use crate::paths::RuntimePaths;
use crate::persistence::{NEW_NOTE_LINE_ENDING, NoteFiles, NoteStorageError};

/// The line ending of a note this host creates (plan 05).
pub struct BootstrapOutcome {
    pub document: DocumentState,
    pub config: RuntimeConfig,
    pub config_persistence_allowed: bool,
    pub recovery: Option<RecoveryCandidate>,
    /// The unusable canonical observation a restore must preserve first (plan 05 recovery 5).
    pub recovery_canonical_to_preserve: Option<PreserveExpectation>,
    pub warnings: Vec<String>,
}

pub fn bootstrap(paths: &RuntimePaths) -> Result<BootstrapOutcome, StartupError> {
    let mut warnings = Vec::new();
    let config_outcome = load_config(&paths.config_file).map_err(StartupError::Config)?;
    if let Some(warning) = &config_outcome.warning {
        warnings.push(warning.message());
    }
    if config_outcome.should_create_default
        && let Err(error) = save_config(
            &paths.config_file,
            &paths.config_tmp,
            &config_outcome.config,
        )
    {
        warnings.push(format!("默认配置未能保存；笔记仍可使用：{error}"));
    }

    let mut files = NoteFiles::open(&paths.note_dir).map_err(StartupError::NoteStorage)?;
    let canonical = files.inspect_note().map_err(StartupError::NoteStorage)?;
    let temporary = files
        .inspect_temporary()
        .map_err(StartupError::TemporaryStorage)?;
    let plan = plan_note_startup(canonical.as_ref(), temporary.as_ref(), NEW_NOTE_LINE_ENDING);

    if let Some(oversized) = plan.oversized_temporary {
        let preserved = files
            .quarantine_temporary()
            .map_err(StartupError::QuarantineTemporary)?;
        if !oversized.canonical_exists {
            return Err(StartupError::TemporaryTooLarge {
                bytes: oversized.bytes,
                preserved,
            });
        }
        warnings.push(oversized_temporary_warning(&preserved));
    }

    let (document, recovery, recovery_canonical_to_preserve) = match plan.step {
        NoteStartupStep::Recover {
            candidate,
            canonical_to_preserve,
            document,
        } => (document, Some(candidate), canonical_to_preserve),
        NoteStartupStep::RemoveRedundantTemporary(canonical) => {
            let loaded = canonical?;
            files
                .remove_temporary()
                .map_err(StartupError::RemoveRedundantTemporary)?;
            (document_from_fact(loaded), None, None)
        }
        NoteStartupStep::QuarantineInvalidTemporary(next) => {
            let preserved = files
                .quarantine_temporary()
                .map_err(StartupError::QuarantineTemporary)?;
            warnings.push(invalid_temporary_warning(&preserved));
            (open_document(&mut files, next)?, None, None)
        }
        NoteStartupStep::Open(next) => (open_document(&mut files, next)?, None, None),
    };
    Ok(BootstrapOutcome {
        document,
        config: config_outcome.config,
        config_persistence_allowed: config_outcome.persistence_allowed,
        recovery,
        recovery_canonical_to_preserve,
        warnings,
    })
}

fn open_document(
    files: &mut NoteFiles,
    plan: NoteDocumentPlan,
) -> Result<DocumentState, StartupError> {
    match plan {
        NoteDocumentPlan::Load(fact) => Ok(document_from_fact(fact)),
        NoteDocumentPlan::CreateEmpty => create_empty_note(files),
        NoteDocumentPlan::Refuse(refusal) => Err(refusal.into()),
    }
}

fn create_empty_note(files: &mut NoteFiles) -> Result<DocumentState, StartupError> {
    let request = PersistRequest {
        generation: stickymd_core::Generation::initial(),
        text: Arc::from(""),
        line_ending: NEW_NOTE_LINE_ENDING,
        mode: PersistMode::Guarded { expected: None },
    };
    match files.persist(&request).map_err(StartupError::NoteStorage)? {
        PersistResult::Saved { fingerprint, .. } => Ok(DocumentState::loaded(
            "",
            NEW_NOTE_LINE_ENDING,
            Some(fingerprint),
        )),
        PersistResult::Conflict { .. } => Err(StartupError::FirstCreateConflict),
    }
}

impl From<NoteRefusal> for StartupError {
    fn from(refusal: NoteRefusal) -> Self {
        match refusal {
            NoteRefusal::InvalidUtf8 => Self::NoteInvalidUtf8,
            NoteRefusal::TooLarge(bytes) => Self::NoteTooLarge(bytes),
            NoteRefusal::Missing => Self::CanonicalMissingDuringRecovery,
        }
    }
}

#[derive(Debug, Error)]
pub enum StartupError {
    #[error("config bootstrap failed: {0}")]
    Config(ConfigStorageError),
    #[error("note bootstrap failed: {0}")]
    NoteStorage(NoteStorageError),
    #[error("temporary-note inspection failed: {0}")]
    TemporaryStorage(NoteStorageError),
    #[error("note.md is not valid UTF-8; StickyMD will not overwrite it")]
    NoteInvalidUtf8,
    #[error("note.md is too large for automatic loading ({0} bytes)")]
    NoteTooLarge(u64),
    #[error(
        "note.md.tmp is too large for safe automatic recovery ({bytes} bytes); preserved at {preserved}"
    )]
    TemporaryTooLarge { bytes: u64, preserved: PathBuf },
    #[error("cannot preserve unusable note.md.tmp evidence: {0}")]
    QuarantineTemporary(NoteStorageError),
    #[error("note.md disappeared during recovery inspection")]
    CanonicalMissingDuringRecovery,
    #[error("a note appeared during first creation; startup stopped to avoid overwrite")]
    FirstCreateConflict,
    #[error("cannot remove a redundant note.md.tmp: {0}")]
    RemoveRedundantTemporary(NoteStorageError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::fs;
    use stickymd_core::LineEnding;

    fn fixture() -> (PathBuf, RuntimePaths) {
        let root = unique_temp_path("bootstrap");
        fs::create_dir(&root).unwrap();
        let executable = root.join("stickymd");
        fs::write(&executable, b"").unwrap();
        let paths = RuntimePaths::from_executable(&executable).unwrap();
        paths.ensure_layout().unwrap();
        (root, paths)
    }

    #[test]
    fn first_launch_creates_the_layout_and_durable_defaults() {
        let (root, paths) = fixture();
        let outcome = bootstrap(&paths).unwrap();
        assert!(outcome.document.text().is_empty());
        assert!(!outcome.document.is_dirty());
        assert_eq!(outcome.document.line_ending(), LineEnding::Lf);
        assert!(paths.note_file.is_file());
        assert!(paths.config_file.is_file());
        fs::remove_dir_all(root).unwrap();
    }

    /// Without line breaks there is no style to keep, so the note takes this host's (plan 05).
    #[test]
    fn an_existing_note_without_line_breaks_takes_lf() {
        for bytes in [&b""[..], b"one line", b"a\r\nb\nc"] {
            let (root, paths) = fixture();
            fs::write(&paths.note_file, bytes).unwrap();
            let outcome = bootstrap(&paths).unwrap();
            assert_eq!(outcome.document.line_ending(), LineEnding::Lf, "{bytes:?}");
            fs::remove_dir_all(root).unwrap();
        }
    }

    #[test]
    fn an_existing_crlf_note_keeps_its_line_ending() {
        let (root, paths) = fixture();
        fs::write(&paths.note_file, b"\xef\xbb\xbfA\r\nB").unwrap();
        let outcome = bootstrap(&paths).unwrap();
        assert_eq!(outcome.document.text(), "A\nB");
        assert_eq!(outcome.document.line_ending(), LineEnding::Crlf);
        assert!(!outcome.document.is_dirty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_invalid_note_blocks_startup_without_overwrite() {
        let (root, paths) = fixture();
        fs::write(&paths.note_file, [0xff]).unwrap();
        assert!(matches!(
            bootstrap(&paths),
            Err(StartupError::NoteInvalidUtf8)
        ));
        assert_eq!(fs::read(&paths.note_file).unwrap(), [0xff]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_different_temporary_enters_recovery_and_keeps_both_files() {
        let (root, paths) = fixture();
        fs::write(&paths.note_file, b"old").unwrap();
        fs::write(&paths.note_tmp, b"new").unwrap();
        let outcome = bootstrap(&paths).unwrap();
        assert!(outcome.recovery.is_some());
        assert_eq!(fs::read(&paths.note_file).unwrap(), b"old");
        assert_eq!(fs::read(&paths.note_tmp).unwrap(), b"new");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_redundant_temporary_is_removed_after_the_note_loads() {
        let (root, paths) = fixture();
        fs::write(&paths.note_file, b"same").unwrap();
        fs::write(&paths.note_tmp, b"same").unwrap();
        let outcome = bootstrap(&paths).unwrap();
        assert!(outcome.recovery.is_none());
        assert!(!paths.note_tmp.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_temporary_evidence_is_quarantined_with_a_warning() {
        let (root, paths) = fixture();
        fs::write(&paths.note_file, b"note").unwrap();
        fs::write(&paths.note_tmp, [0xff]).unwrap();
        let outcome = bootstrap(&paths).unwrap();
        assert!(!paths.note_tmp.exists());
        assert_eq!(outcome.warnings.len(), 1);
        fs::remove_dir_all(root).unwrap();
    }
}
