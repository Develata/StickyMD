//! Startup loading after single-instance and writable-directory checks.
//!
//! plan_ref: docs/plan/05_document_persistence.md#startup-sequence

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use stickymd_core::{DocumentState, LineEnding, PreserveExpectation, RecoveryCandidate};
use thiserror::Error;

use crate::config::{load_config, save_config};
use crate::persistence::{inspect_note, persist_note, quarantine_temporary, remove_temporary};
use crate::platform::windows::ArboardClipboard;
use crate::platform::windows::program_dir::RuntimePaths;
use stickymd_app::config::RuntimeConfig;
use stickymd_app::execution::PersistenceWorker;
use stickymd_app::flow::{
    NoteDocumentPlan, NoteRefusal, NoteStartupStep, document_from_fact, invalid_temporary_warning,
    oversized_temporary_warning, plan_note_startup,
};
use stickymd_app::ports::{PersistMode, PersistRequest, PersistResult};
use stickymd_app::session::{AppSession, InstantClock, SessionParts};

/// The line ending of a note this host creates.
const NEW_NOTE_LINE_ENDING: LineEnding = LineEnding::Crlf;

pub struct BootstrapOutcome {
    pub document: DocumentState,
    pub config: RuntimeConfig,
    pub config_persistence_allowed: bool,
    pub recovery: Option<RecoveryCandidate>,
    /// The unusable canonical observation a restore must preserve first (plan 05 recovery 5).
    pub recovery_canonical_to_preserve: Option<PreserveExpectation>,
    pub warnings: Vec<String>,
}

impl BootstrapOutcome {
    /// The session this startup found, with its clock at `started`, and the warning that goes on
    /// the status line.
    pub fn into_session(
        self,
        worker: PersistenceWorker,
        started: Instant,
    ) -> (AppSession, Option<String>) {
        let session = AppSession::new(SessionParts {
            document: self.document,
            clipboard: Box::new(ArboardClipboard::new()),
            config: self.config,
            config_persistence_allowed: self.config_persistence_allowed,
            recovery: self.recovery,
            recovery_canonical_to_preserve: self.recovery_canonical_to_preserve,
            worker,
            clock: Box::new(InstantClock::starting_at(started)),
        });
        (session, self.warnings.last().cloned())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapMilestone {
    ConfigReady,
}

#[cfg(test)]
pub fn bootstrap(paths: &RuntimePaths) -> Result<BootstrapOutcome, StartupError> {
    bootstrap_observed(paths, |_| {})
}

pub fn bootstrap_observed(
    paths: &RuntimePaths,
    mut observe: impl FnMut(BootstrapMilestone),
) -> Result<BootstrapOutcome, StartupError> {
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
    observe(BootstrapMilestone::ConfigReady);

    let canonical = inspect_note(&paths.note_file).map_err(StartupError::NoteStorage)?;
    let temporary = inspect_note(&paths.note_tmp).map_err(StartupError::TemporaryStorage)?;
    let plan = plan_note_startup(canonical.as_ref(), temporary.as_ref(), NEW_NOTE_LINE_ENDING);

    if let Some(oversized) = plan.oversized_temporary {
        let preserved =
            quarantine_temporary(&paths.note_tmp).map_err(StartupError::QuarantineTemporary)?;
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
            remove_temporary(&paths.note_tmp).map_err(StartupError::RemoveRedundantTemporary)?;
            (document_from_fact(loaded), None, None)
        }
        NoteStartupStep::QuarantineInvalidTemporary(next) => {
            let preserved =
                quarantine_temporary(&paths.note_tmp).map_err(StartupError::QuarantineTemporary)?;
            warnings.push(invalid_temporary_warning(&preserved));
            (open_document(paths, next)?, None, None)
        }
        NoteStartupStep::Open(next) => (open_document(paths, next)?, None, None),
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
    paths: &RuntimePaths,
    plan: NoteDocumentPlan,
) -> Result<DocumentState, StartupError> {
    match plan {
        NoteDocumentPlan::Load(fact) => Ok(document_from_fact(fact)),
        NoteDocumentPlan::CreateEmpty => create_empty_note(paths),
        NoteDocumentPlan::Refuse(refusal) => Err(refusal.into()),
    }
}

fn create_empty_note(paths: &RuntimePaths) -> Result<DocumentState, StartupError> {
    let request = PersistRequest {
        generation: stickymd_core::Generation::initial(),
        text: Arc::from(""),
        line_ending: NEW_NOTE_LINE_ENDING,
        mode: PersistMode::Guarded { expected: None },
    };
    match persist_note(&paths.note_file, &paths.note_tmp, &request)
        .map_err(StartupError::NoteStorage)?
    {
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
    Config(crate::config::ConfigStorageError),
    #[error("note bootstrap failed: {0}")]
    NoteStorage(crate::persistence::NoteStorageError),
    #[error("temporary-note inspection failed: {0}")]
    TemporaryStorage(crate::persistence::NoteStorageError),
    #[error("note.md is not valid UTF-8; StickyMD will not overwrite it")]
    NoteInvalidUtf8,
    #[error("note.md is too large for automatic loading ({0} bytes)")]
    NoteTooLarge(u64),
    #[error(
        "note.md.tmp is too large for safe automatic recovery ({bytes} bytes); preserved at {preserved}"
    )]
    TemporaryTooLarge { bytes: u64, preserved: PathBuf },
    #[error("cannot preserve unusable note.md.tmp evidence: {0}")]
    QuarantineTemporary(crate::persistence::NoteStorageError),
    #[error("note.md disappeared during recovery inspection")]
    CanonicalMissingDuringRecovery,
    #[error("a note appeared during first creation; startup stopped to avoid overwrite")]
    FirstCreateConflict,
    #[error("cannot remove a redundant note.md.tmp: {0}")]
    RemoveRedundantTemporary(crate::persistence::NoteStorageError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::windows::program_dir::RuntimePaths;
    use crate::test_support::unique_temp_path;
    use std::fs;
    use std::path::PathBuf;

    fn fixture() -> (PathBuf, RuntimePaths) {
        let root = unique_temp_path("bootstrap");
        fs::create_dir(&root).unwrap();
        let executable = root.join("StickyMD.exe");
        fs::write(&executable, b"").unwrap();
        let paths = RuntimePaths::from_executable(&executable).unwrap();
        paths.ensure_layout().unwrap();
        (root, paths)
    }

    #[test]
    fn first_launch_creates_portable_layout_and_durable_defaults() {
        let (root, paths) = fixture();
        let outcome = bootstrap(&paths).unwrap();
        assert!(outcome.document.text().is_empty());
        assert!(!outcome.document.is_dirty());
        assert!(paths.note_file.is_file());
        assert!(paths.config_file.is_file());
        assert!(paths.images_dir.is_dir());
        assert!(paths.trash_dir.is_dir());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn existing_bom_note_loads_clean_and_preserves_crlf_metadata() {
        let (root, paths) = fixture();
        fs::write(&paths.note_file, b"\xef\xbb\xbfA\r\nB").unwrap();
        let outcome = bootstrap(&paths).unwrap();
        assert_eq!(outcome.document.text(), "A\nB");
        assert_eq!(outcome.document.line_ending(), LineEnding::Crlf);
        assert!(!outcome.document.is_dirty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_canonical_note_blocks_startup_without_overwrite() {
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
    fn different_temporary_note_enters_recovery_without_deleting_evidence() {
        let (root, paths) = fixture();
        fs::write(&paths.note_file, b"old").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(&paths.note_tmp, b"new").unwrap();
        let outcome = bootstrap(&paths).unwrap();
        assert!(outcome.recovery.is_some());
        assert!(paths.note_tmp.exists());
        fs::remove_dir_all(root).unwrap();
    }

    /// Phase 15 byte compatibility on a data copy: a no-op start rewrites nothing, and saving the
    /// unchanged document reproduces the canonical bytes and leaves the config and images alone.
    #[test]
    fn phase15_a_no_op_start_and_save_leave_the_data_byte_identical() {
        let (root, paths) = fixture();
        let note = "第一行 mixed\r\n公式 $x=1$\r\n\r\n末行没有换行".as_bytes();
        let config = b"version = 1\nopacity = 83\ntheme = \"dark\"\n";
        let image = [0x89, b'P', b'N', b'G', 0, 1, 2, 3];
        let image_path = paths.images_dir.join("stickymd-0.png");
        fs::write(&paths.note_file, note).unwrap();
        fs::write(&paths.config_file, config).unwrap();
        fs::write(&image_path, image).unwrap();

        let outcome = bootstrap(&paths).unwrap();
        assert!(outcome.recovery.is_none());
        assert_eq!(fs::read(&paths.note_file).unwrap(), note);
        assert_eq!(fs::read(&paths.config_file).unwrap(), config);

        let snapshot = outcome.document.snapshot();
        let result = persist_note(
            &paths.note_file,
            &paths.note_tmp,
            &PersistRequest {
                generation: snapshot.generation,
                text: snapshot.text,
                line_ending: snapshot.line_ending,
                mode: PersistMode::Guarded {
                    expected: outcome.document.base_disk_hash(),
                },
            },
        )
        .unwrap();
        assert!(matches!(result, PersistResult::Saved { .. }));
        assert_eq!(fs::read(&paths.note_file).unwrap(), note);
        assert!(!paths.note_tmp.exists());
        assert_eq!(fs::read(&paths.config_file).unwrap(), config);
        assert_eq!(fs::read(&image_path).unwrap(), image);
        fs::remove_dir_all(root).unwrap();
    }

    /// A failed save can leave newer text in a temp older than an external save: a restart offers
    /// it as a recovery choice and keeps both files.
    #[test]
    fn an_older_different_temporary_survives_a_restart_as_a_recovery_choice() {
        let (root, paths) = fixture();
        fs::write(&paths.note_tmp, "本地未保存").unwrap();
        fs::write(&paths.note_file, "外部保存").unwrap();
        let older = std::time::SystemTime::now() - std::time::Duration::from_secs(3600);
        let temporary = fs::OpenOptions::new()
            .write(true)
            .open(&paths.note_tmp)
            .unwrap();
        temporary.set_modified(older).unwrap();
        drop(temporary);
        let candidate = bootstrap(&paths).unwrap().recovery.unwrap();
        assert_eq!(candidate.temporary.text, "本地未保存");
        assert!(!candidate.temporary_is_newer);
        assert_eq!(fs::read_to_string(&paths.note_tmp).unwrap(), "本地未保存");
        assert_eq!(fs::read_to_string(&paths.note_file).unwrap(), "外部保存");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn identical_temporary_note_is_removed_only_after_canonical_load() {
        let (root, paths) = fixture();
        fs::write(&paths.note_file, b"same").unwrap();
        fs::write(&paths.note_tmp, b"same").unwrap();
        let outcome = bootstrap(&paths).unwrap();
        assert_eq!(outcome.document.text(), "same");
        assert!(outcome.recovery.is_none());
        assert!(!paths.note_tmp.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_note_with_valid_temporary_is_an_explicit_recovery_candidate() {
        let (root, paths) = fixture();
        fs::write(&paths.note_tmp, "临时内容").unwrap();
        let outcome = bootstrap(&paths).unwrap();
        assert!(outcome.document.text().is_empty());
        assert_eq!(outcome.recovery.unwrap().temporary.text, "临时内容");
        assert!(outcome.recovery_canonical_to_preserve.is_none());
        assert!(!paths.note_file.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn valid_temporary_never_silently_overwrites_invalid_canonical_note() {
        let (root, paths) = fixture();
        fs::write(&paths.note_file, [0xff]).unwrap();
        fs::write(&paths.note_tmp, b"recoverable").unwrap();
        let outcome = bootstrap(&paths).unwrap();
        assert!(outcome.recovery.is_some());
        assert_eq!(
            outcome.recovery_canonical_to_preserve,
            Some(PreserveExpectation::Content(stickymd_core::hash_bytes(&[
                0xff
            ])))
        );
        assert_eq!(fs::read(&paths.note_file).unwrap(), [0xff]);
        assert_eq!(fs::read(&paths.note_tmp).unwrap(), b"recoverable");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn oversized_temporary_is_preserved_and_never_decoded_as_empty_recovery() {
        let (root, paths) = fixture();
        fs::write(&paths.note_file, b"canonical").unwrap();
        let temporary = fs::File::create(&paths.note_tmp).unwrap();
        temporary
            .set_len(crate::persistence::MAX_NOTE_LOAD + 1)
            .unwrap();
        let outcome = bootstrap(&paths).unwrap();
        assert_eq!(outcome.document.text(), "canonical");
        assert!(outcome.recovery.is_none());
        assert!(!paths.note_tmp.exists());
        assert_eq!(
            fs::read_dir(&paths.note_dir)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("note.invalid-tmp-"))
                .count(),
            1
        );
        assert!(
            outcome
                .warnings
                .iter()
                .any(|warning| warning.contains("安全载入上限"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    fn preserved_temporaries(paths: &RuntimePaths) -> Vec<PathBuf> {
        fs::read_dir(&paths.note_dir)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("note.invalid-tmp-")
            })
            .map(|entry| entry.path())
            .collect()
    }

    /// Moving the temporary aside must not end in an empty note standing in for it.
    #[test]
    fn oversized_temporary_without_a_note_stops_after_moving_it_aside() {
        let (root, paths) = fixture();
        let bytes = crate::persistence::MAX_NOTE_LOAD + 1;
        fs::File::create(&paths.note_tmp)
            .unwrap()
            .set_len(bytes)
            .unwrap();
        let error = bootstrap(&paths).err().unwrap();
        assert!(
            matches!(error, StartupError::TemporaryTooLarge { bytes: reported, .. } if reported == bytes),
            "{error}"
        );
        assert!(!paths.note_file.exists());
        assert!(!paths.note_tmp.exists());
        assert_eq!(preserved_temporaries(&paths).len(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_temporary_is_moved_aside_before_an_unusable_note_is_refused() {
        let (root, paths) = fixture();
        fs::write(&paths.note_file, [0xfe]).unwrap();
        fs::write(&paths.note_tmp, [0xff]).unwrap();
        let error = bootstrap(&paths).err().unwrap();
        assert!(matches!(error, StartupError::NoteInvalidUtf8), "{error}");
        assert_eq!(fs::read(&paths.note_file).unwrap(), [0xfe]);
        assert!(!paths.note_tmp.exists());
        let preserved = preserved_temporaries(&paths);
        assert_eq!(preserved.len(), 1);
        assert_eq!(fs::read(&preserved[0]).unwrap(), [0xff]);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_temporary_without_canonical_is_quarantined_before_empty_create() {
        let (root, paths) = fixture();
        fs::write(&paths.note_tmp, [0xff]).unwrap();
        let outcome = bootstrap(&paths).unwrap();
        assert!(outcome.document.text().is_empty());
        assert_eq!(fs::read(&paths.note_file).unwrap(), b"");
        assert!(!paths.note_tmp.exists());
        let preserved = fs::read_dir(&paths.note_dir)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| {
                entry
                    .file_name()
                    .to_string_lossy()
                    .starts_with("note.invalid-tmp-")
            })
            .unwrap();
        assert_eq!(fs::read(preserved.path()).unwrap(), [0xff]);
        fs::remove_dir_all(root).unwrap();
    }
}
