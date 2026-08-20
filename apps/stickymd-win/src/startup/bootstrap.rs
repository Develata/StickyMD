//! Startup loading after single-instance and writable-directory checks.
//!
//! plan_ref: docs/plan/05_document_persistence.md#startup-sequence

use std::path::PathBuf;
use std::sync::Arc;

use stickymd_core::{
    DocumentState, ExternalFileState, LineEnding, RecoveryCandidate, RecoveryDisposition,
    inspect_recovery,
};
use thiserror::Error;

use crate::config::{ConfigWarning, RuntimeConfig, load_config, save_config};
use crate::persistence::{
    PersistMode, PersistRequest, PersistResult, inspect_note, persist_note, quarantine_temporary,
    remove_temporary,
};
use crate::platform::windows::program_dir::RuntimePaths;

pub struct BootstrapOutcome {
    pub document: DocumentState,
    pub config: RuntimeConfig,
    pub config_persistence_allowed: bool,
    pub recovery: Option<RecoveryCandidate>,
    pub recovery_canonical_requires_preserve: bool,
    pub warnings: Vec<String>,
}

pub fn bootstrap(paths: &RuntimePaths) -> Result<BootstrapOutcome, StartupError> {
    let mut warnings = Vec::new();
    let config_outcome = load_config(&paths.config_file).map_err(StartupError::Config)?;
    if let Some(warning) = &config_outcome.warning {
        warnings.push(config_warning_message(warning));
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

    let canonical = inspect_note(&paths.note_file).map_err(StartupError::NoteStorage)?;
    let temporary = inspect_note(&paths.note_tmp).map_err(StartupError::TemporaryStorage)?;
    let recovery = inspect_recovery(
        canonical
            .as_ref()
            .and_then(crate::persistence::NoteObservation::recovery_inspection),
        temporary
            .as_ref()
            .and_then(crate::persistence::NoteObservation::recovery_inspection),
    );

    if temporary
        .as_ref()
        .is_some_and(|observation| matches!(observation.state, ExternalFileState::TooLarge { .. }))
    {
        let preserved =
            quarantine_temporary(&paths.note_tmp).map_err(StartupError::QuarantineTemporary)?;
        if canonical.is_none() {
            let bytes = temporary.as_ref().map_or(0, |observation| {
                if let ExternalFileState::TooLarge { bytes } = observation.state {
                    bytes
                } else {
                    0
                }
            });
            return Err(StartupError::TemporaryTooLarge { bytes, preserved });
        }
        warnings.push(format!(
            "note.md.tmp 超过安全载入上限；已隔离保留为 {}。",
            preserved.display()
        ));
    }

    match recovery {
        RecoveryDisposition::Candidate(candidate) => {
            let recovery_canonical_requires_preserve =
                canonical.as_ref().is_some_and(|observation| {
                    matches!(
                        observation.state,
                        ExternalFileState::InvalidUtf8 { .. } | ExternalFileState::TooLarge { .. }
                    )
                });
            let document = candidate.canonical.as_ref().map_or_else(
                || DocumentState::empty(LineEnding::Crlf),
                |loaded| {
                    DocumentState::loaded(
                        &loaded.text,
                        loaded.line_ending,
                        Some(loaded.fingerprint),
                    )
                },
            );
            Ok(BootstrapOutcome {
                document,
                config: config_outcome.config,
                config_persistence_allowed: config_outcome.persistence_allowed,
                recovery: Some(candidate),
                recovery_canonical_requires_preserve,
                warnings,
            })
        }
        RecoveryDisposition::RedundantTemporary | RecoveryDisposition::StaleTemporary => {
            let loaded = load_existing_canonical(canonical.as_ref())?;
            remove_temporary(&paths.note_tmp).map_err(StartupError::RemoveRedundantTemporary)?;
            Ok(BootstrapOutcome {
                document: document_from_external(loaded),
                config: config_outcome.config,
                config_persistence_allowed: config_outcome.persistence_allowed,
                recovery: None,
                recovery_canonical_requires_preserve: false,
                warnings,
            })
        }
        RecoveryDisposition::InvalidTemporary => {
            let preserved =
                quarantine_temporary(&paths.note_tmp).map_err(StartupError::QuarantineTemporary)?;
            warnings.push(format!(
                "note.md.tmp 不是有效 UTF-8；已隔离保留为 {}。",
                preserved.display()
            ));
            let document = match canonical.as_ref() {
                Some(observation) => document_from_external(extract_present(observation)?),
                None => create_empty_note(paths)?,
            };
            Ok(BootstrapOutcome {
                document,
                config: config_outcome.config,
                config_persistence_allowed: config_outcome.persistence_allowed,
                recovery: None,
                recovery_canonical_requires_preserve: false,
                warnings,
            })
        }
        RecoveryDisposition::NoTemporary => {
            let document = match canonical.as_ref() {
                Some(observation) => document_from_external(extract_present(observation)?),
                None => create_empty_note(paths)?,
            };
            Ok(BootstrapOutcome {
                document,
                config: config_outcome.config,
                config_persistence_allowed: config_outcome.persistence_allowed,
                recovery: None,
                recovery_canonical_requires_preserve: false,
                warnings,
            })
        }
    }
}

fn create_empty_note(paths: &RuntimePaths) -> Result<DocumentState, StartupError> {
    let request = PersistRequest {
        generation: stickymd_core::Generation::initial(),
        text: Arc::from(""),
        line_ending: LineEnding::Crlf,
        mode: PersistMode::Guarded { expected: None },
    };
    match persist_note(&paths.note_file, &paths.note_tmp, &request)
        .map_err(StartupError::NoteStorage)?
    {
        PersistResult::Saved { fingerprint, .. } => Ok(DocumentState::loaded(
            "",
            LineEnding::Crlf,
            Some(fingerprint),
        )),
        PersistResult::Conflict { .. } => Err(StartupError::FirstCreateConflict),
    }
}

fn load_existing_canonical(
    canonical: Option<&crate::persistence::NoteObservation>,
) -> Result<stickymd_core::ExternalFileFact, StartupError> {
    canonical
        .ok_or(StartupError::CanonicalMissingDuringRecovery)
        .and_then(extract_present)
}

fn extract_present(
    observation: &crate::persistence::NoteObservation,
) -> Result<stickymd_core::ExternalFileFact, StartupError> {
    match &observation.state {
        ExternalFileState::Present(fact) => Ok(fact.clone()),
        ExternalFileState::InvalidUtf8 { .. } => Err(StartupError::NoteInvalidUtf8),
        ExternalFileState::TooLarge { bytes } => Err(StartupError::NoteTooLarge(*bytes)),
        ExternalFileState::Missing => Err(StartupError::CanonicalMissingDuringRecovery),
    }
}

fn document_from_external(fact: stickymd_core::ExternalFileFact) -> DocumentState {
    DocumentState::loaded(&fact.text, fact.line_ending, Some(fact.fingerprint))
}

fn config_warning_message(warning: &ConfigWarning) -> String {
    match warning {
        ConfigWarning::CorruptPreserved(_) => {
            "config.toml 已损坏；原文件已保留，当前使用默认配置。".into()
        }
        ConfigWarning::CorruptCouldNotBePreserved => {
            "config.toml 已损坏且无法改名保留；不会覆盖它，当前使用默认配置。".into()
        }
        ConfigWarning::UnsupportedNewerVersion(version) => {
            format!("config.toml 版本 {version} 高于当前程序；文件已保留，当前使用默认配置。")
        }
        ConfigWarning::ReadFailed(error) => {
            format!("config.toml 无法读取；不会覆盖它，当前使用默认配置：{error}")
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
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture() -> (PathBuf, RuntimePaths) {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("stickymd-bootstrap-{}-{nonce}", std::process::id()));
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
        assert!(!outcome.recovery_canonical_requires_preserve);
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
        assert!(outcome.recovery_canonical_requires_preserve);
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
