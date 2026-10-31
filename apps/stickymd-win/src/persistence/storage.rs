//! Guarded whole-note storage built over the Windows atomic publisher.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use std::fs;
use std::io::Read;

use sha2::{Digest, Sha256};
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use stickymd_core::{
    DiskFingerprint, ExternalFileState, Generation, LoadedDocument, NoteDecodeError,
    RecoveryInspection, hash_bytes,
};
use thiserror::Error;

use crate::platform::windows::atomic_file::{
    AtomicPublishError, move_file_no_replace, prepare_temporary, publish_prepared_existing,
    publish_prepared_new,
};
use crate::platform::windows::file_identity::{OpenFileObservation, observe_open_file};

pub const MAX_NOTE_LOAD: u64 = 16 * 1024 * 1024;
const FILE_ATTRIBUTE_REPARSE_POINT_VALUE: u32 = 0x400;

#[derive(Debug, Clone)]
pub struct NoteObservation {
    pub state: ExternalFileState,
    pub recovery: RecoveryInspection,
}

impl NoteObservation {
    /// Recovery decoding is meaningful only when the file bytes were actually
    /// loaded. An over-limit file deliberately has no synthetic empty body.
    pub fn recovery_inspection(&self) -> Option<&RecoveryInspection> {
        (!matches!(self.state, ExternalFileState::TooLarge { .. })).then_some(&self.recovery)
    }
}

pub fn remove_temporary(path: &Path) -> Result<(), NoteStorageError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(NoteStorageError::RemoveTemporary(error)),
    }
}

/// Move unusable recovery evidence away from the fixed transaction temp name
/// before normal saving can reuse that name.
pub fn quarantine_temporary(path: &Path) -> Result<PathBuf, NoteStorageError> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    quarantine_temporary_at(path, stamp)
}

fn quarantine_temporary_at(path: &Path, stamp: u64) -> Result<PathBuf, NoteStorageError> {
    for suffix in 0..1000u16 {
        let filename = if suffix == 0 {
            format!("note.invalid-tmp-{stamp}.md")
        } else {
            format!("note.invalid-tmp-{stamp}-{suffix}.md")
        };
        let destination = path.with_file_name(filename);
        match move_file_no_replace(path, &destination) {
            Ok(()) => return Ok(destination),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(NoteStorageError::QuarantineTemporary(error)),
        }
    }
    Err(NoteStorageError::QuarantineNameExhausted)
}

pub fn preserve_canonical(source: &Path, destination: &Path) -> Result<(), NoteStorageError> {
    for suffix in 0..1000u16 {
        let candidate = suffixed_path(destination, suffix);
        match move_file_no_replace(source, &candidate) {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(NoteStorageError::PreserveCanonical(error)),
        }
    }
    Err(NoteStorageError::PreserveCanonical(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "all canonical recovery preservation names are occupied",
    )))
}

/// Result of preserving the canonical note before a recovery restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreserveOutcome {
    Preserved,
    /// The canonical note is no longer the observation that required preserving it; it was
    /// left in place.
    Changed(ExternalFileState),
}

/// Moves the canonical note aside only while it is still `expected`, the observation that
/// required preserving it. A canonical changed while recovery waited is left in place and
/// reported, so it becomes a conflict instead of being moved away (plan 05 recovery 8). Both
/// unusable states carry a fingerprint of the full content. The check and the move are separate
/// steps; a change between them is not detected (the small TOCTOU plan 05 accepts for OCC).
pub fn preserve_canonical_if_unchanged(
    source: &Path,
    destination: &Path,
    expected: &ExternalFileState,
) -> Result<PreserveOutcome, NoteStorageError> {
    let observed = inspect_note_state_with_retry(source)?;
    if &observed != expected {
        return Ok(PreserveOutcome::Changed(observed));
    }
    preserve_canonical(source, destination).map(|()| PreserveOutcome::Preserved)
}

fn suffixed_path(path: &Path, suffix: u16) -> PathBuf {
    if suffix == 0 {
        return path.to_owned();
    }
    let mut file_name = path
        .file_stem()
        .unwrap_or_else(|| std::ffi::OsStr::new("note.invalid"))
        .to_os_string();
    file_name.push(format!("-{suffix}"));
    if let Some(extension) = path.extension() {
        file_name.push(".");
        file_name.push(extension);
    }
    path.with_file_name(file_name)
}

/// SHA-256 of `prefix` followed by the rest of `file`, read in bounded chunks, and the total
/// length. Memory stays O(chunk) whatever the file size.
fn hash_streamed(file: &mut fs::File, prefix: &[u8]) -> std::io::Result<(DiskFingerprint, u64)> {
    let mut hasher = Sha256::new();
    hasher.update(prefix);
    let mut total = prefix.len() as u64;
    let mut chunk = vec![0_u8; 64 * 1024];
    loop {
        match file.read(&mut chunk) {
            Ok(0) => break,
            Ok(read) => {
                hasher.update(&chunk[..read]);
                total += read as u64;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(error) => return Err(error),
        }
    }
    Ok((DiskFingerprint::new(hasher.finalize().into()), total))
}

pub fn inspect_note(path: &Path) -> Result<Option<NoteObservation>, NoteStorageError> {
    for attempt in 0..3 {
        match inspect_note_once(path) {
            Err(NoteStorageError::ObservationChanged) if attempt < 2 => {
                std::thread::yield_now();
            }
            result => return result,
        }
    }
    Err(NoteStorageError::ObservationChanged)
}

fn inspect_note_once(path: &Path) -> Result<Option<NoteObservation>, NoteStorageError> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(NoteStorageError::Read(error)),
    };
    let before = match file.metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(NoteStorageError::Metadata(error)),
    };
    let before_identity = observe_open_file(&file).map_err(NoteStorageError::Metadata)?;
    let mut bytes = Vec::new();
    if before.len() <= MAX_NOTE_LOAD {
        bytes.reserve(before.len() as usize);
        file.by_ref()
            .take(MAX_NOTE_LOAD + 1)
            .read_to_end(&mut bytes)
            .map_err(NoteStorageError::Read)?;
    }
    if before.len() > MAX_NOTE_LOAD || bytes.len() as u64 > MAX_NOTE_LOAD {
        // Too large to load: stream the rest so the observation still names the content.
        let (fingerprint, total) =
            hash_streamed(&mut file, &bytes).map_err(NoteStorageError::Read)?;
        let after = file.metadata().map_err(NoteStorageError::Metadata)?;
        let after_identity = observe_open_file(&file).map_err(NoteStorageError::Metadata)?;
        let path_after = stable_path_metadata(path, after_identity)?;
        if before_identity != after_identity
            || before.len() != after.len()
            || before.modified().ok() != after.modified().ok()
            || after.len() != path_after.len()
            || after.modified().ok() != path_after.modified().ok()
            || total != after.len()
        {
            return Err(NoteStorageError::ObservationChanged);
        }
        return Ok(Some(NoteObservation {
            state: ExternalFileState::TooLarge {
                bytes: total,
                fingerprint,
            },
            recovery: RecoveryInspection {
                bytes: Vec::new(),
                modified: path_after.modified().ok(),
            },
        }));
    }
    let after = file.metadata().map_err(NoteStorageError::Metadata)?;
    let after_identity = observe_open_file(&file).map_err(NoteStorageError::Metadata)?;
    let path_after = stable_path_metadata(path, after_identity)?;
    if before_identity != after_identity
        || before.len() != after.len()
        || before.modified().ok() != after.modified().ok()
        || after.len() != path_after.len()
        || after.modified().ok() != path_after.modified().ok()
    {
        return Err(NoteStorageError::ObservationChanged);
    }
    let modified = path_after.modified().ok();
    let state = match LoadedDocument::from_durable_bytes(&bytes) {
        Ok(loaded) => ExternalFileState::Present(loaded.into_external_fact()),
        Err(NoteDecodeError::InvalidUtf8) => ExternalFileState::InvalidUtf8 {
            fingerprint: hash_bytes(&bytes),
        },
    };
    Ok(Some(NoteObservation {
        state,
        recovery: RecoveryInspection { bytes, modified },
    }))
}

fn stable_path_metadata(
    path: &Path,
    expected: OpenFileObservation,
) -> Result<std::fs::Metadata, NoteStorageError> {
    match fs::File::open(path) {
        Ok(file) => {
            if observe_open_file(&file).map_err(NoteStorageError::Metadata)? != expected {
                return Err(NoteStorageError::ObservationChanged);
            }
            file.metadata().map_err(NoteStorageError::Metadata)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(NoteStorageError::ObservationChanged)
        }
        Err(error) => Err(NoteStorageError::Metadata(error)),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistMode {
    Guarded { expected: Option<DiskFingerprint> },
    ForceOverwrite,
}

#[derive(Debug, Clone)]
pub struct PersistRequest {
    pub generation: Generation,
    pub text: std::sync::Arc<str>,
    pub line_ending: stickymd_core::LineEnding,
    pub mode: PersistMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistResult {
    Saved {
        generation: Generation,
        fingerprint: DiskFingerprint,
        durable_bytes: usize,
    },
    Conflict {
        generation: Generation,
        observed: ExternalFileState,
    },
}

pub fn persist_note(
    target: &Path,
    temporary: &Path,
    request: &PersistRequest,
) -> Result<PersistResult, NoteStorageError> {
    persist_note_with_before_publish(target, temporary, request, |_| {})
}

fn persist_note_with_before_publish<F>(
    target: &Path,
    temporary: &Path,
    request: &PersistRequest,
    before_publish: F,
) -> Result<PersistResult, NoteStorageError>
where
    F: FnOnce(bool),
{
    ensure_note_directory_for_save(target, temporary)?;
    let bytes = LoadedDocument::encode_runtime(&request.text, request.line_ending);
    let fingerprint = hash_bytes(&bytes);
    prepare_temporary(target, temporary, &bytes).map_err(NoteStorageError::Publish)?;

    // This is the final guard and deliberately occurs after temp write+flush,
    // immediately before the publish primitive. The watcher is only a UX hint;
    // this check is the correctness gate even when the watcher is unavailable.
    let publish_existing = if let PersistMode::Guarded { expected } = request.mode {
        let observed = inspect_note_state_with_retry(target)?;
        let observed_fingerprint = match &observed {
            ExternalFileState::Present(fact) => Some(fact.fingerprint),
            ExternalFileState::InvalidUtf8 { fingerprint } => Some(*fingerprint),
            ExternalFileState::Missing | ExternalFileState::TooLarge { .. } => None,
        };
        let allowed = match (&observed, expected) {
            (ExternalFileState::Missing, None) => true,
            (ExternalFileState::Present(_), Some(expected))
            | (ExternalFileState::InvalidUtf8 { .. }, Some(expected)) => {
                observed_fingerprint == Some(expected)
            }
            _ => false,
        };
        if !allowed {
            return Ok(PersistResult::Conflict {
                generation: request.generation,
                observed,
            });
        }
        !matches!(observed, ExternalFileState::Missing)
    } else {
        target.try_exists().map_err(NoteStorageError::Metadata)?
    };

    before_publish(publish_existing);
    if publish_existing {
        publish_prepared_existing(target, temporary).map_err(NoteStorageError::Publish)?;
    } else {
        if let Err(error) = publish_prepared_new(target, temporary) {
            if matches!(request.mode, PersistMode::Guarded { expected: None }) {
                let observed = inspect_note_state_with_retry(target)?;
                if !matches!(observed, ExternalFileState::Missing) {
                    return Ok(PersistResult::Conflict {
                        generation: request.generation,
                        observed,
                    });
                }
            }
            return Err(NoteStorageError::Publish(error));
        }
    }
    Ok(PersistResult::Saved {
        generation: request.generation,
        fingerprint,
        durable_bytes: bytes.len(),
    })
}

/// Restore an externally removed `note/` directory while refusing files and
/// reparse points. This is intentionally note-specific: atomic publication
/// itself must not grow a generic, policy-bearing directory bootstrap.
fn ensure_note_directory_for_save(target: &Path, temporary: &Path) -> Result<(), NoteStorageError> {
    let parent = target
        .parent()
        .filter(|parent| Some(*parent) == temporary.parent())
        .ok_or(NoteStorageError::InvalidNoteLayout)?;
    match fs::symlink_metadata(parent) {
        Ok(metadata) => validate_note_directory(parent, &metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(parent).map_err(|source| NoteStorageError::CreateNoteDirectory {
                path: parent.to_path_buf(),
                source,
            })?;
            let metadata = fs::symlink_metadata(parent).map_err(NoteStorageError::Metadata)?;
            validate_note_directory(parent, &metadata)
        }
        Err(error) => Err(NoteStorageError::Metadata(error)),
    }
}

fn validate_note_directory(
    path: &Path,
    metadata: &std::fs::Metadata,
) -> Result<(), NoteStorageError> {
    if metadata.file_type().is_dir()
        && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT_VALUE == 0
    {
        Ok(())
    } else {
        Err(NoteStorageError::UnsafeNoteDirectory(path.to_path_buf()))
    }
}

pub(crate) fn inspect_note_state_with_retry(
    path: &Path,
) -> Result<ExternalFileState, NoteStorageError> {
    const DELAYS_MS: [u64; 3] = [50, 150, 300];
    for delay in DELAYS_MS {
        match inspect_note(path) {
            Ok(Some(observed)) => return Ok(observed.state),
            Ok(None) => return Ok(ExternalFileState::Missing),
            Err(NoteStorageError::Read(_))
            | Err(NoteStorageError::Metadata(_))
            | Err(NoteStorageError::ObservationChanged) => {
                std::thread::sleep(std::time::Duration::from_millis(delay));
            }
            Err(error) => return Err(error),
        }
    }
    inspect_note(path).map(|value| value.map_or(ExternalFileState::Missing, |value| value.state))
}

#[derive(Debug, Error)]
pub enum NoteStorageError {
    #[error("note target and temporary paths must share a parent directory")]
    InvalidNoteLayout,
    #[error("note directory is not a plain directory: {}", .0.display())]
    UnsafeNoteDirectory(PathBuf),
    #[error("cannot recreate note directory {path}: {source}")]
    CreateNoteDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot inspect note metadata: {0}")]
    Metadata(std::io::Error),
    #[error("cannot read note: {0}")]
    Read(std::io::Error),
    #[error("note changed while it was being observed")]
    ObservationChanged,
    #[error("cannot atomically publish note: {0}")]
    Publish(AtomicPublishError),
    #[error("cannot remove temporary note: {0}")]
    RemoveTemporary(std::io::Error),
    #[error("cannot quarantine invalid temporary note: {0}")]
    QuarantineTemporary(std::io::Error),
    #[error("cannot preserve canonical note before recovery: {0}")]
    PreserveCanonical(std::io::Error),
    #[error("cannot allocate a unique quarantine name for temporary note")]
    QuarantineNameExhausted,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::windows::atomic_file::{prepare_temporary, publish_prepared};
    use crate::test_support::unique_temp_path;
    use std::path::PathBuf;
    use stickymd_core::{
        CursorSnapshot, DocumentState, EditKind, EditMeta, EditRequest, LineEnding, hash_bytes,
    };

    fn unique_dir() -> PathBuf {
        unique_temp_path("storage")
    }

    fn request(text: &str, expected: Option<DiskFingerprint>, force: bool) -> PersistRequest {
        PersistRequest {
            generation: Generation::initial(),
            text: text.into(),
            line_ending: LineEnding::Crlf,
            mode: if force {
                PersistMode::ForceOverwrite
            } else {
                PersistMode::Guarded { expected }
            },
        }
    }

    #[test]
    fn guarded_save_detects_external_change_and_never_writes() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let target = root.join("note.md");
        let temporary = root.join("note.md.tmp");
        fs::write(&target, b"external X").unwrap();
        let result = persist_note(
            &target,
            &temporary,
            &request("local B", Some(hash_bytes(b"base A")), false),
        )
        .unwrap();
        assert!(matches!(result, PersistResult::Conflict { .. }));
        assert_eq!(fs::read(&target).unwrap(), b"external X");
        assert_eq!(fs::read(&temporary).unwrap(), b"local B".to_vec());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn guarded_and_force_modes_have_distinct_authority() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let target = root.join("note.md");
        let temporary = root.join("note.md.tmp");
        fs::write(&target, b"base A").unwrap();
        let guarded = persist_note(
            &target,
            &temporary,
            &request("local B", Some(hash_bytes(b"base A")), false),
        )
        .unwrap();
        assert!(matches!(guarded, PersistResult::Saved { .. }));
        fs::write(&target, b"external X").unwrap();
        let forced = persist_note(&target, &temporary, &request("local B", None, true)).unwrap();
        assert!(matches!(forced, PersistResult::Saved { .. }));
        assert_eq!(fs::read(&target).unwrap(), b"local B");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn first_publish_fails_closed_if_target_appears() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let target = root.join("note.md");
        let temporary = root.join("note.md.tmp");
        fs::write(&target, b"appeared").unwrap();
        let result = persist_note(&target, &temporary, &request("local", None, false)).unwrap();
        assert!(matches!(result, PersistResult::Conflict { .. }));
        assert_eq!(fs::read(&target).unwrap(), b"appeared");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn first_publish_race_after_guard_is_reported_as_conflict_without_overwrite() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let target = root.join("note.md");
        let temporary = root.join("note.md.tmp");
        let result = persist_note_with_before_publish(
            &target,
            &temporary,
            &request("local", None, false),
            |publish_existing| {
                assert!(!publish_existing);
                fs::write(&target, b"external appeared after guard").unwrap();
            },
        )
        .unwrap();
        assert!(matches!(result, PersistResult::Conflict { .. }));
        assert_eq!(fs::read(&target).unwrap(), b"external appeared after guard");
        assert_eq!(fs::read(&temporary).unwrap(), b"local");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phase9_external_note_directory_delete_recreates_only_the_note_parent() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let note = root.join("note");
        let target = note.join("note.md");
        let temporary = note.join("note.md.tmp");

        let result = persist_note(
            &target,
            &temporary,
            &request("memory survives", None, false),
        )
        .unwrap();

        assert!(matches!(result, PersistResult::Saved { .. }));
        assert_eq!(fs::read(&target).unwrap(), b"memory survives");
        assert!(!temporary.exists());
        assert_eq!(fs::read_dir(&note).unwrap().count(), 1);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phase9_note_directory_replaced_by_file_fails_closed() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let note = root.join("note");
        fs::write(&note, b"external evidence").unwrap();
        let target = note.join("note.md");
        let temporary = note.join("note.md.tmp");

        let result = persist_note(&target, &temporary, &request("local", None, false));

        assert!(matches!(
            result,
            Err(NoteStorageError::UnsafeNoteDirectory(path)) if path == note
        ));
        assert_eq!(fs::read(&note).unwrap(), b"external evidence");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phase9_read_only_note_save_failure_preserves_dirty_document_authority() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let target = root.join("note.md");
        let temporary = root.join("note.md.tmp");
        fs::write(&target, b"old").unwrap();
        let base = hash_bytes(b"old");
        let mut document = DocumentState::loaded("old", LineEnding::Lf, Some(base));
        document
            .edit(EditRequest::new(
                document.generation(),
                0..3,
                "local dirty",
                CursorSnapshot::caret(3),
                CursorSnapshot::caret(11),
                EditMeta::new(EditKind::SelectionReplace, 0),
            ))
            .unwrap();
        let before = (
            document.text().to_owned(),
            document.generation(),
            document.saved_generation(),
            document.can_undo(),
        );
        let original_permissions = fs::metadata(&target).unwrap().permissions();
        let mut permissions = original_permissions.clone();
        permissions.set_readonly(true);
        fs::set_permissions(&target, permissions).unwrap();

        let snapshot = document.snapshot();
        let result = persist_note(
            &target,
            &temporary,
            &PersistRequest {
                generation: snapshot.generation,
                text: snapshot.text,
                line_ending: snapshot.line_ending,
                mode: PersistMode::Guarded {
                    expected: Some(base),
                },
            },
        );

        assert!(result.is_err());
        assert_eq!(fs::read(&target).unwrap(), b"old");
        assert_eq!(
            (
                document.text().to_owned(),
                document.generation(),
                document.saved_generation(),
                document.can_undo(),
            ),
            before
        );
        assert!(document.is_dirty());
        fs::set_permissions(&target, original_permissions).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn temporary_cleanup_is_idempotent_after_successful_publish() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let temporary = root.join("note.md.tmp");
        remove_temporary(&temporary).unwrap();
        fs::write(&temporary, b"evidence").unwrap();
        remove_temporary(&temporary).unwrap();
        remove_temporary(&temporary).unwrap();
        assert!(!temporary.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preserving_canonical_never_overwrites_existing_recovery_evidence() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let source = root.join("note.md");
        let occupied = root.join("note.invalid-42.md");
        fs::write(&source, b"new canonical evidence").unwrap();
        fs::write(&occupied, b"older recovery evidence").unwrap();

        preserve_canonical(&source, &occupied).unwrap();

        assert!(!source.exists());
        assert_eq!(fs::read(&occupied).unwrap(), b"older recovery evidence");
        assert_eq!(
            fs::read(root.join("note.invalid-42-1.md")).unwrap(),
            b"new canonical evidence"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_canonical_changed_since_the_recovery_observation_is_left_in_place() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let source = root.join("note.md");
        let destination = root.join("note.invalid-7.md");
        fs::write(&source, b"\xff invalid").unwrap();
        let expected = inspect_note_state_with_retry(&source).unwrap();
        assert!(matches!(expected, ExternalFileState::InvalidUtf8 { .. }));
        // An external program made the canonical valid while the recovery choice waited.
        fs::write(&source, b"valid external text").unwrap();
        let outcome = preserve_canonical_if_unchanged(&source, &destination, &expected).unwrap();
        assert!(matches!(
            outcome,
            PreserveOutcome::Changed(ExternalFileState::Present(_))
        ));
        assert_eq!(fs::read(&source).unwrap(), b"valid external text");
        assert!(!destination.exists());
        // Unchanged, it is moved aside.
        fs::write(&source, b"\xff invalid").unwrap();
        assert_eq!(
            preserve_canonical_if_unchanged(&source, &destination, &expected).unwrap(),
            PreserveOutcome::Preserved
        );
        assert!(!source.exists());
        assert_eq!(fs::read(&destination).unwrap(), b"\xff invalid");
        fs::remove_dir_all(root).unwrap();
    }

    /// The preserve check compares content, not the kind of state or the length: an equal-length
    /// invalid or too-large file with other bytes, or a missing file, is reported as changed.
    #[test]
    fn preserve_compares_the_content_of_unusable_canonicals() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let source = root.join("note.md");
        let destination = root.join("note.invalid-9.md");
        let too_large = MAX_NOTE_LOAD as usize + 1;
        let mut first = vec![b'a'; too_large];
        first[0] = 0xff;
        let mut second = first.clone();
        second[too_large - 1] = b'b';
        for (original, replacement) in [
            (b"\xffA".to_vec(), b"\xffB".to_vec()),
            (first.clone(), second.clone()),
        ] {
            fs::write(&source, &original).unwrap();
            let expected = inspect_note_state_with_retry(&source).unwrap();
            assert!(matches!(
                expected,
                ExternalFileState::InvalidUtf8 { .. } | ExternalFileState::TooLarge { .. }
            ));
            fs::write(&source, &replacement).unwrap();
            assert!(matches!(
                preserve_canonical_if_unchanged(&source, &destination, &expected).unwrap(),
                PreserveOutcome::Changed(_)
            ));
            assert_eq!(fs::read(&source).unwrap(), replacement);
            fs::remove_file(&source).unwrap();
            assert_eq!(
                preserve_canonical_if_unchanged(&source, &destination, &expected).unwrap(),
                PreserveOutcome::Changed(ExternalFileState::Missing)
            );
        }
        // The same too-large content is recognized and moved aside.
        fs::write(&source, &first).unwrap();
        let expected = inspect_note_state_with_retry(&source).unwrap();
        assert_eq!(
            preserve_canonical_if_unchanged(&source, &destination, &expected).unwrap(),
            PreserveOutcome::Preserved
        );
        assert_eq!(
            fs::metadata(&destination).unwrap().len(),
            first.len() as u64
        );
        assert!(!source.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn quarantining_temporary_suffixes_an_occupied_evidence_name() {
        let root = unique_dir();
        fs::create_dir(&root).unwrap();
        let temporary = root.join("note.md.tmp");
        let occupied = root.join("note.invalid-tmp-42.md");
        fs::write(&temporary, b"new temporary evidence").unwrap();
        fs::write(&occupied, b"older temporary evidence").unwrap();

        let preserved = quarantine_temporary_at(&temporary, 42).unwrap();

        assert_eq!(preserved, root.join("note.invalid-tmp-42-1.md"));
        assert_eq!(fs::read(&occupied).unwrap(), b"older temporary evidence");
        assert_eq!(fs::read(&preserved).unwrap(), b"new temporary evidence");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    #[ignore = "release-only Phase 4 durable persistence baseline"]
    fn phase4_persistence_release_baseline() {
        for size in [20 * 1024, 100 * 1024, 1024 * 1024] {
            let root = unique_dir();
            fs::create_dir(&root).unwrap();
            let target = root.join("note.md");
            let temporary = root.join("note.md.tmp");
            fs::write(&target, b"initial").unwrap();
            let seed = include_str!("../../../../tests/fixtures/performance/typical-note-seed.md");
            let mut text = String::with_capacity(size + seed.len());
            while text.len() < size {
                text.push_str(seed);
            }
            while text.len() > size {
                text.pop();
            }
            let document = stickymd_core::DocumentState::loaded(
                &text,
                LineEnding::Crlf,
                Some(hash_bytes(b"initial")),
            );
            let mut snapshot_us = Vec::new();
            let mut encode_us = Vec::new();
            let mut hash_us = Vec::new();
            let mut write_flush_us = Vec::new();
            let mut replace_us = Vec::new();
            let mut total_us = Vec::new();
            for _ in 0..40 {
                let total = std::time::Instant::now();
                let started = std::time::Instant::now();
                let snapshot = document.snapshot();
                snapshot_us.push(started.elapsed().as_micros() as u64);
                let started = std::time::Instant::now();
                let bytes = LoadedDocument::encode_runtime(&snapshot.text, snapshot.line_ending);
                encode_us.push(started.elapsed().as_micros() as u64);
                let started = std::time::Instant::now();
                let _fingerprint = hash_bytes(&bytes);
                hash_us.push(started.elapsed().as_micros() as u64);
                let started = std::time::Instant::now();
                prepare_temporary(&target, &temporary, &bytes).unwrap();
                write_flush_us.push(started.elapsed().as_micros() as u64);
                let started = std::time::Instant::now();
                publish_prepared(&target, &temporary).unwrap();
                replace_us.push(started.elapsed().as_micros() as u64);
                total_us.push(total.elapsed().as_micros() as u64);
            }
            println!(
                "PHASE4_PERSIST size={} snapshot={} encode={} hash={} write_flush={} replace={} total={}",
                size,
                stats(&mut snapshot_us),
                stats(&mut encode_us),
                stats(&mut hash_us),
                stats(&mut write_flush_us),
                stats(&mut replace_us),
                stats(&mut total_us),
            );
            fs::remove_dir_all(root).unwrap();
        }
    }

    fn stats(samples: &mut [u64]) -> String {
        samples.sort_unstable();
        let median = samples[samples.len() / 2];
        let p95 = samples[(samples.len() * 95 / 100).min(samples.len() - 1)];
        let max = samples[samples.len() - 1];
        format!("{median}/{p95}/{max}us")
    }
}
