//! Windows execution of the note storage port.
//!
//! plan_ref: docs/plan/05_document_persistence.md#atomic-save

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use stickymd_app::ports::{
    FailureKind, NoteStorage, PersistRequest, PersistResult, PreserveOutcome, StorageFailure,
};
use stickymd_core::{ExternalFileState, PreserveExpectation};

use super::{
    NoteStorageError, inspect_note_state_with_retry, persist_note, preserve_canonical_if_unchanged,
    remove_temporary,
};

/// The one canonical note of this program directory.
pub struct WinNoteStorage {
    pub note_file: PathBuf,
    pub note_tmp: PathBuf,
    pub note_dir: PathBuf,
}

impl NoteStorage for WinNoteStorage {
    fn persist(&mut self, request: &PersistRequest) -> Result<PersistResult, StorageFailure> {
        Ok(persist_note(&self.note_file, &self.note_tmp, request)?)
    }

    fn observe(&mut self) -> Result<ExternalFileState, StorageFailure> {
        Ok(inspect_note_state_with_retry(&self.note_file)?)
    }

    fn remove_temporary(&mut self) -> Result<(), StorageFailure> {
        Ok(remove_temporary(&self.note_tmp)?)
    }

    fn preserve_canonical(
        &mut self,
        expected: PreserveExpectation,
    ) -> Result<PreserveOutcome, StorageFailure> {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        let destination = self.note_dir.join(format!("note.invalid-{stamp}.md"));
        Ok(preserve_canonical_if_unchanged(
            &self.note_file,
            &destination,
            expected,
        )?)
    }
}

/// A read or an observation can succeed on a later attempt; anything else failed outright. The
/// message crosses unchanged because diagnostics print it.
impl From<NoteStorageError> for StorageFailure {
    fn from(error: NoteStorageError) -> Self {
        let kind = match &error {
            NoteStorageError::Metadata(_)
            | NoteStorageError::Read(_)
            | NoteStorageError::ObservationChanged => FailureKind::Unavailable,
            NoteStorageError::InvalidNoteLayout
            | NoteStorageError::UnsafeNoteDirectory(_)
            | NoteStorageError::CreateNoteDirectory { .. }
            | NoteStorageError::Publish(_)
            | NoteStorageError::RemoveTemporary(_)
            | NoteStorageError::QuarantineTemporary(_)
            | NoteStorageError::PreserveCanonical(_)
            | NoteStorageError::QuarantineNameExhausted => FailureKind::Failed,
        };
        Self::new(kind, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::windows::atomic_file::AtomicPublishError;

    #[test]
    fn phase15_every_storage_error_keeps_its_message_and_gets_its_class() {
        let io = || std::io::Error::other("cause");
        let rows = [
            (NoteStorageError::Metadata(io()), FailureKind::Unavailable),
            (NoteStorageError::Read(io()), FailureKind::Unavailable),
            (
                NoteStorageError::ObservationChanged,
                FailureKind::Unavailable,
            ),
            (NoteStorageError::InvalidNoteLayout, FailureKind::Failed),
            (
                NoteStorageError::UnsafeNoteDirectory("note".into()),
                FailureKind::Failed,
            ),
            (
                NoteStorageError::CreateNoteDirectory {
                    path: "note".into(),
                    source: io(),
                },
                FailureKind::Failed,
            ),
            (
                NoteStorageError::Publish(AtomicPublishError::DifferentDirectories),
                FailureKind::Failed,
            ),
            (NoteStorageError::RemoveTemporary(io()), FailureKind::Failed),
            (
                NoteStorageError::QuarantineTemporary(io()),
                FailureKind::Failed,
            ),
            (
                NoteStorageError::PreserveCanonical(io()),
                FailureKind::Failed,
            ),
            (
                NoteStorageError::QuarantineNameExhausted,
                FailureKind::Failed,
            ),
        ];
        for (error, kind) in rows {
            let message = error.to_string();
            let failure = StorageFailure::from(error);
            assert_eq!(failure.to_string(), message);
            assert_eq!(failure.kind(), kind, "{message}");
        }
    }
}
