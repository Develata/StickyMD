//! macOS execution of the storage ports.
//!
//! plan_ref: docs/plan/05_document_persistence.md#macos-storage

use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

use stickymd_app::config::RuntimeConfig;
use stickymd_app::execution::IoPorts;
use stickymd_app::ports::{
    AssetPasteCompletion, AssetPasteFailure, AssetPasteRequest, AssetReconcileReport,
    AssetSyncRequest, ConfigStorage, ExportCompletion, ExportRequest, ExportSink, FailureKind,
    ManagedAssetStore, NoteStorage, PersistRequest, PersistResult, PreserveOutcome, StorageFailure,
};
use stickymd_core::{ExternalFileState, PreserveExpectation};

use crate::assets::{AssetPasteError, AssetStorage, AssetStorageError, prepare_and_store_paste};
use crate::config::{ConfigStorageError, save_config};
use crate::export::{ExportError, export_snapshot};
use crate::paths::RuntimePaths;

use super::{NoteFiles, NoteStorageError};

/// The one canonical note of this program directory.
pub struct MacNoteStorage {
    files: NoteFiles,
}

impl NoteStorage for MacNoteStorage {
    fn persist(&mut self, request: &PersistRequest) -> Result<PersistResult, StorageFailure> {
        Ok(self.files.persist(request)?)
    }

    fn observe(&mut self) -> Result<ExternalFileState, StorageFailure> {
        Ok(self.files.observe()?)
    }

    fn remove_temporary(&mut self) -> Result<(), StorageFailure> {
        Ok(self.files.remove_temporary()?)
    }

    fn preserve_canonical(
        &mut self,
        expected: PreserveExpectation,
    ) -> Result<PreserveOutcome, StorageFailure> {
        Ok(self.files.preserve_canonical(expected)?)
    }
}

/// The Note Directory the program started with. The note keeps writing through its descriptor when
/// the directory moves; what is opened by path has to refuse a new directory at the old place, or
/// a pasted image and the note that refers to it would end up apart.
struct NoteDirectoryBinding {
    path: PathBuf,
    device: u64,
    inode: u64,
}

impl NoteDirectoryBinding {
    fn capture(path: &Path) -> io::Result<Self> {
        let metadata = fs::symlink_metadata(path)?;
        Ok(Self {
            path: path.to_owned(),
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }

    fn verify(&self) -> io::Result<()> {
        let metadata = fs::symlink_metadata(&self.path)?;
        if metadata.dev() == self.device && metadata.ino() == self.inode {
            Ok(())
        } else {
            Err(io::Error::other(
                "the note directory was replaced while the program ran",
            ))
        }
    }
}

/// The managed images of this program directory.
pub struct MacAssetStore {
    note: NoteDirectoryBinding,
    images: PathBuf,
    trash: PathBuf,
}

impl MacAssetStore {
    pub fn open(paths: &RuntimePaths) -> io::Result<Self> {
        Ok(Self {
            note: NoteDirectoryBinding::capture(&paths.note_dir)?,
            images: paths.images_dir.clone(),
            trash: paths.trash_dir.clone(),
        })
    }

    fn storage(&self) -> Result<AssetStorage, AssetStorageError> {
        self.note.verify().map_err(AssetStorageError::Directory)?;
        AssetStorage::open(&self.images, &self.trash)
    }
}

impl ManagedAssetStore for MacAssetStore {
    fn paste(
        &mut self,
        request: AssetPasteRequest,
    ) -> Result<AssetPasteCompletion, AssetPasteFailure> {
        let expected_generation = request.expected_generation;
        let storage = self.storage().map_err(|error| AssetPasteFailure {
            expected_generation,
            error: paste_failure(AssetPasteError::Storage(error)),
            rollback: Default::default(),
        })?;
        prepare_and_store_paste(&storage, request).map_err(|failure| AssetPasteFailure {
            expected_generation: failure.expected_generation,
            error: paste_failure(failure.error),
            rollback: failure.rollback,
        })
    }

    fn sync(&mut self, request: &AssetSyncRequest) -> Result<AssetReconcileReport, StorageFailure> {
        let storage = self.storage()?;
        for effect in &request.effects {
            storage.apply_effect(effect)?;
        }
        Ok(match request.reconcile {
            Some(_) => storage.reconcile(&request.references)?,
            None => AssetReconcileReport::default(),
        })
    }
}

pub struct MacExportSink {
    note_dir: PathBuf,
}

impl ExportSink for MacExportSink {
    fn export(&mut self, request: ExportRequest) -> Result<ExportCompletion, StorageFailure> {
        Ok(export_snapshot(crate::export::ExportRequest {
            snapshot: request.snapshot,
            note_dir: self.note_dir.clone(),
            target: request.target,
        })?)
    }
}

pub struct MacConfigStorage {
    note: NoteDirectoryBinding,
    target: PathBuf,
    temporary: PathBuf,
}

impl ConfigStorage for MacConfigStorage {
    fn save(&mut self, config: &RuntimeConfig) -> Result<(), StorageFailure> {
        self.note.verify().map_err(ConfigStorageError::Directory)?;
        Ok(save_config(&self.target, &self.temporary, config)?)
    }
}

/// The capabilities of this program directory, for the shared I/O worker.
pub fn mac_io_ports(
    paths: &RuntimePaths,
) -> Result<IoPorts<MacNoteStorage, MacAssetStore, MacExportSink, MacConfigStorage>, StorageFailure>
{
    Ok(IoPorts {
        note: MacNoteStorage {
            files: NoteFiles::open(&paths.note_dir)?,
        },
        assets: MacAssetStore::open(paths).map_err(AssetStorageError::Directory)?,
        export: MacExportSink {
            note_dir: paths.note_dir.clone(),
        },
        config: MacConfigStorage {
            note: NoteDirectoryBinding::capture(&paths.note_dir)
                .map_err(AssetStorageError::Directory)?,
            target: paths.config_file.clone(),
            temporary: paths.config_tmp.clone(),
        },
    })
}

fn asset_failure_kind(error: &AssetStorageError) -> FailureKind {
    match error {
        AssetStorageError::Directory(_) | AssetStorageError::ChangedDuringProof(_) => {
            FailureKind::Unavailable
        }
        AssetStorageError::OwnershipNotProven(_)
        | AssetStorageError::TooLarge
        | AssetStorageError::Io(_)
        | AssetStorageError::NameCollisionExhausted
        | AssetStorageError::MissingAsset(_) => FailureKind::Failed,
    }
}

impl From<AssetStorageError> for StorageFailure {
    fn from(error: AssetStorageError) -> Self {
        Self::new(asset_failure_kind(&error), error.to_string())
    }
}

fn paste_failure(error: AssetPasteError) -> StorageFailure {
    let kind = match &error {
        AssetPasteError::Read(_) => FailureKind::Unavailable,
        AssetPasteError::Storage(storage) => asset_failure_kind(storage),
        AssetPasteError::MixedOrUnsupportedFiles
        | AssetPasteError::Image(_)
        | AssetPasteError::BatchTooLarge { .. }
        | AssetPasteError::NotImage => FailureKind::Failed,
    };
    StorageFailure::new(kind, error.to_string())
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

/// A read or an observation can succeed on a later attempt; anything else failed outright.
impl From<NoteStorageError> for StorageFailure {
    fn from(error: NoteStorageError) -> Self {
        let kind = match &error {
            NoteStorageError::Metadata(_)
            | NoteStorageError::Read(_)
            | NoteStorageError::ObservationChanged => FailureKind::Unavailable,
            NoteStorageError::UnsafeNoteDirectory(_)
            | NoteStorageError::CreateNoteDirectory { .. }
            | NoteStorageError::Publish(_)
            | NoteStorageError::DirectorySync(_)
            | NoteStorageError::RemoveTemporary(_)
            | NoteStorageError::QuarantineTemporary(_)
            | NoteStorageError::QuarantineNameExhausted
            | NoteStorageError::PreserveCanonical(_)
            | NoteStorageError::PreserveDisplaced { .. } => FailureKind::Failed,
        };
        Self::new(kind, error.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_replaced_note_directory_is_refused_and_left_alone() {
        let root = crate::test_support::unique_temp_path("adapter-replaced");
        let note = root.join("note");
        fs::create_dir_all(&note).unwrap();
        let mut config = MacConfigStorage {
            note: NoteDirectoryBinding::capture(&note).unwrap(),
            target: note.join("config.toml"),
            temporary: note.join("config.toml.tmp"),
        };
        config.save(&RuntimeConfig::default()).unwrap();
        fs::rename(&note, root.join("moved")).unwrap();
        fs::create_dir(&note).unwrap();
        config.save(&RuntimeConfig::default()).unwrap_err();
        assert!(!note.join("config.toml").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn every_note_error_gets_its_class_and_keeps_its_message() {
        let io = || std::io::Error::other("cause");
        for (error, kind) in [
            (NoteStorageError::Metadata(io()), FailureKind::Unavailable),
            (NoteStorageError::Read(io()), FailureKind::Unavailable),
            (
                NoteStorageError::ObservationChanged,
                FailureKind::Unavailable,
            ),
            (NoteStorageError::Publish(io()), FailureKind::Failed),
            (NoteStorageError::DirectorySync(io()), FailureKind::Failed),
            (
                NoteStorageError::PreserveDisplaced {
                    path: "x".into(),
                    source: io(),
                },
                FailureKind::Failed,
            ),
        ] {
            let message = error.to_string();
            let failure = StorageFailure::from(error);
            assert_eq!(failure.kind(), kind, "{message}");
            assert_eq!(failure.to_string(), message);
        }
    }
}
