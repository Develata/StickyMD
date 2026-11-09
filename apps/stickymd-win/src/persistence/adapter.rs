//! Windows execution of the storage ports.
//!
//! plan_ref: docs/plan/05_document_persistence.md#atomic-save

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use stickymd_app::config::RuntimeConfig;
use stickymd_app::execution::IoPorts;
use stickymd_app::ports::{
    AssetPasteCompletion, AssetPasteFailure, AssetPasteRequest, AssetReconcileMode,
    AssetReconcileReport, AssetSyncRequest, ConfigStorage, ExportCompletion, ExportRequest,
    ExportSink, FailureKind, ManagedAssetStore, NoteStorage, PersistRequest, PersistResult,
    PreserveOutcome, StorageFailure,
};
use stickymd_core::{ExternalFileState, PreserveExpectation};

use crate::assets::{
    AssetPasteError, AssetStorage, AssetStorageError, prepare_and_store_paste,
    reconcile_safe_boundary,
};
use crate::config::{ConfigStorageError, save_config};
use crate::export::{ExportError, export_snapshot};
use crate::platform::windows::program_dir::RuntimePaths;

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

/// The managed images of this program directory.
pub struct WinAssetStore {
    pub images: PathBuf,
    pub trash: PathBuf,
    pub note_file: PathBuf,
}

impl ManagedAssetStore for WinAssetStore {
    fn paste(
        &mut self,
        request: AssetPasteRequest,
    ) -> Result<AssetPasteCompletion, AssetPasteFailure> {
        let expected_generation = request.expected_generation;
        let storage =
            AssetStorage::open(&self.images, &self.trash).map_err(|error| AssetPasteFailure {
                expected_generation,
                error: AssetPasteError::Storage(error).into(),
                rollback: Default::default(),
            })?;
        prepare_and_store_paste(&storage, request).map_err(|failure| AssetPasteFailure {
            expected_generation: failure.expected_generation,
            error: failure.error.into(),
            rollback: failure.rollback,
        })
    }

    fn sync(&mut self, request: &AssetSyncRequest) -> Result<AssetReconcileReport, StorageFailure> {
        let storage = AssetStorage::open(&self.images, &self.trash)?;
        for effect in &request.effects {
            storage.apply_effect(effect)?;
        }
        Ok(match (request.reconcile, request.safe_expected) {
            (Some(AssetReconcileMode::SafeBoundary), Some(expected)) => {
                reconcile_safe_boundary(&storage, &self.note_file, expected, &request.references)?
            }
            (Some(mode), _) => storage.reconcile(&request.references, mode)?,
            (None, _) => AssetReconcileReport::default(),
        })
    }
}

pub struct WinExportSink {
    pub note_dir: PathBuf,
}

impl ExportSink for WinExportSink {
    fn export(&mut self, request: ExportRequest) -> Result<ExportCompletion, StorageFailure> {
        Ok(export_snapshot(crate::export::ExportRequest {
            snapshot: request.snapshot,
            note_dir: self.note_dir.clone(),
            target: request.target,
        })?)
    }
}

pub struct WinConfigStorage {
    pub target: PathBuf,
    pub temporary: PathBuf,
}

impl ConfigStorage for WinConfigStorage {
    fn save(&mut self, config: &RuntimeConfig) -> Result<(), StorageFailure> {
        Ok(save_config(&self.target, &self.temporary, config)?)
    }
}

/// The capabilities of this program directory, for the shared I/O worker.
pub fn win_io_ports(
    paths: &RuntimePaths,
) -> IoPorts<WinNoteStorage, WinAssetStore, WinExportSink, WinConfigStorage> {
    IoPorts {
        note: WinNoteStorage {
            note_file: paths.note_file.clone(),
            note_tmp: paths.note_tmp.clone(),
            note_dir: paths.note_dir.clone(),
        },
        assets: WinAssetStore {
            images: paths.images_dir.clone(),
            trash: paths.trash_dir.clone(),
            note_file: paths.note_file.clone(),
        },
        export: WinExportSink {
            note_dir: paths.note_dir.clone(),
        },
        config: WinConfigStorage {
            target: paths.config_file.clone(),
            temporary: paths.config_tmp.clone(),
        },
    }
}

fn asset_failure_kind(error: &AssetStorageError) -> FailureKind {
    match error {
        AssetStorageError::Directory(_) | AssetStorageError::ChangedDuringProof(_) => {
            FailureKind::Unavailable
        }
        AssetStorageError::ReparseDirectory(_)
        | AssetStorageError::OwnershipNotProven(_)
        | AssetStorageError::TooLarge
        | AssetStorageError::Io(_)
        | AssetStorageError::NameCollisionExhausted
        | AssetStorageError::MissingAsset(_)
        | AssetStorageError::AmbiguousCollision(_)
        | AssetStorageError::InvalidRootLayout => FailureKind::Failed,
    }
}

impl From<AssetStorageError> for StorageFailure {
    fn from(error: AssetStorageError) -> Self {
        Self::new(asset_failure_kind(&error), error.to_string())
    }
}

impl From<AssetPasteError> for StorageFailure {
    fn from(error: AssetPasteError) -> Self {
        let kind = match &error {
            AssetPasteError::Read(_) => FailureKind::Unavailable,
            AssetPasteError::Storage(storage) => asset_failure_kind(storage),
            AssetPasteError::MixedOrUnsupportedFiles
            | AssetPasteError::Image(_)
            | AssetPasteError::BatchTooLarge { .. }
            | AssetPasteError::NotImage => FailureKind::Failed,
        };
        Self::new(kind, error.to_string())
    }
}

impl From<ExportError> for StorageFailure {
    fn from(error: ExportError) -> Self {
        Self::new(FailureKind::Failed, error.to_string())
    }
}

impl From<ConfigStorageError> for StorageFailure {
    fn from(error: ConfigStorageError) -> Self {
        Self::new(FailureKind::Failed, error.to_string())
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

    /// A wrapper adds context to the message and keeps the class of what it wraps.
    #[test]
    fn phase15_a_wrapped_storage_error_keeps_its_class() {
        let unavailable =
            AssetPasteError::Storage(AssetStorageError::ChangedDuringProof("asset.png".into()));
        let message = unavailable.to_string();
        let failure = StorageFailure::from(unavailable);
        assert_eq!(failure.kind(), FailureKind::Unavailable);
        assert_eq!(failure.to_string(), message);
        assert!(message.starts_with("managed image publication failed: "));
        let failed = AssetPasteError::Storage(AssetStorageError::TooLarge);
        assert_eq!(StorageFailure::from(failed).kind(), FailureKind::Failed);
    }

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
