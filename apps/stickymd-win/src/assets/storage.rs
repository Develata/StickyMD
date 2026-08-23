//! Filesystem ownership proof and serial managed-image lifecycle operations.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#managed-vs-user-asset

use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::windows::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use stickymd_core::{AssetEffect, ManagedAssetLocation, ManagedAssetName, hash_bytes};
use stickymd_render::image::{MAX_ENCODED_IMAGE_BYTES, PreparedImage};
use thiserror::Error;

use crate::platform::windows::file_identity::{OpenFileObservation, observe_open_file};
use crate::platform::windows::managed_file::{
    create_new_for_managed_publish, delete_open_file, open_for_managed_mutation,
    open_managed_directory, rename_open_file,
};

const FILE_ATTRIBUTE_REPARSE_POINT_VALUE: u32 = 0x400;
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct AssetStorage {
    images: PathBuf,
    trash: PathBuf,
    canonical_images: PathBuf,
    canonical_trash: PathBuf,
    note: PathBuf,
    canonical_note: PathBuf,
    note_observation: OpenFileObservation,
    images_observation: OpenFileObservation,
    trash_observation: OpenFileObservation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetLocationProof {
    Images,
    Trash,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredAssetDisposition {
    Created,
    Reused,
    Restored,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetReconcileMode {
    Runtime,
    SafeBoundary,
}

struct ProvenAsset {
    file: File,
    digest: stickymd_core::Hash32,
    observation: OpenFileObservation,
}

struct BoundAssetRoot {
    _note: File,
    directory: File,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAsset {
    pub name: ManagedAssetName,
    pub disposition: StoredAssetDisposition,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AssetReconcileReport {
    pub restored: usize,
    pub moved_to_trash: usize,
    pub deleted_from_trash: usize,
    pub missing_references: Vec<ManagedAssetName>,
    pub skipped_untrusted: usize,
    pub physical_delete_deferred: bool,
}

#[derive(Debug, Error)]
pub enum AssetStorageError {
    #[error("asset directory is unavailable: {0}")]
    Directory(std::io::Error),
    #[error("asset directory is a reparse point: {}", .0.display())]
    ReparseDirectory(PathBuf),
    #[error("asset path is not a proven StickyMD-owned regular file: {}", .0.display())]
    OwnershipNotProven(PathBuf),
    #[error("asset payload exceeds the 64 MiB bound")]
    TooLarge,
    #[error("asset I/O failed: {0}")]
    Io(std::io::Error),
    #[error("all managed hash-prefix names collided with untrusted content")]
    NameCollisionExhausted,
    #[error("asset changed while ownership was being verified: {}", .0.display())]
    ChangedDuringProof(PathBuf),
    #[error("managed asset is missing from both active and trash locations: {}", .0)]
    MissingAsset(ManagedAssetName),
    #[error("active and trash files share a managed prefix but have different full hashes: {}", .0)]
    AmbiguousCollision(ManagedAssetName),
    #[error("managed asset roots do not form note/images and note/.trash siblings")]
    InvalidRootLayout,
}

impl AssetStorage {
    pub fn open(images: &Path, trash: &Path) -> Result<Self, AssetStorageError> {
        let note = images
            .parent()
            .ok_or(AssetStorageError::InvalidRootLayout)?;
        if trash.parent() != Some(note)
            || images.file_name() != Some(std::ffi::OsStr::new("images"))
            || trash.file_name() != Some(std::ffi::OsStr::new(".trash"))
        {
            return Err(AssetStorageError::InvalidRootLayout);
        }
        ensure_plain_directory(note)?;
        ensure_plain_directory(images)?;
        ensure_plain_directory(trash)?;
        let canonical_note = fs::canonicalize(note).map_err(AssetStorageError::Directory)?;
        let canonical_images = fs::canonicalize(images).map_err(AssetStorageError::Directory)?;
        let canonical_trash = fs::canonicalize(trash).map_err(AssetStorageError::Directory)?;
        if canonical_images.parent() != Some(canonical_note.as_path())
            || canonical_trash.parent() != Some(canonical_note.as_path())
        {
            return Err(AssetStorageError::InvalidRootLayout);
        }
        let note_observation =
            observe_open_file(&open_managed_directory(note).map_err(AssetStorageError::Directory)?)
                .map_err(AssetStorageError::Directory)?;
        let images_observation = observe_open_file(
            &open_managed_directory(images).map_err(AssetStorageError::Directory)?,
        )
        .map_err(AssetStorageError::Directory)?;
        let trash_observation = observe_open_file(
            &open_managed_directory(trash).map_err(AssetStorageError::Directory)?,
        )
        .map_err(AssetStorageError::Directory)?;
        Ok(Self {
            images: images.to_owned(),
            trash: trash.to_owned(),
            canonical_images,
            canonical_trash,
            note: note.to_owned(),
            canonical_note,
            note_observation,
            images_observation,
            trash_observation,
        })
    }

    pub fn store(&self, image: &PreparedImage) -> Result<StoredAsset, AssetStorageError> {
        self.verify_root(ManagedAssetLocation::Images)?;
        self.verify_root(ManagedAssetLocation::Trash)?;
        let full_hash = image.hash().to_hex();
        for prefix in [20usize, 32, 64] {
            let Some(name) =
                ManagedAssetName::from_hash_prefix(&full_hash, prefix, image.extension())
            else {
                continue;
            };
            let active = self.images.join(name.as_str());
            if active.try_exists().map_err(AssetStorageError::Io)? {
                if self
                    .prove_open(&name, ManagedAssetLocation::Images)
                    .is_ok_and(|proven| proven.digest == image.hash())
                {
                    return Ok(StoredAsset {
                        name,
                        disposition: StoredAssetDisposition::Reused,
                    });
                }
                continue;
            }
            let trashed = self.trash.join(name.as_str());
            if trashed.try_exists().map_err(AssetStorageError::Io)? {
                if let Ok(proven) = self.prove_open(&name, ManagedAssetLocation::Trash)
                    && proven.digest == image.hash()
                {
                    let target_root = self.open_root(ManagedAssetLocation::Images)?;
                    rename_open_file(&proven.file, &target_root.directory, &active)
                        .map_err(AssetStorageError::Io)?;
                    return Ok(StoredAsset {
                        name,
                        disposition: StoredAssetDisposition::Restored,
                    });
                }
                continue;
            }
            self.publish_new(&active, image.bytes())?;
            return Ok(StoredAsset {
                name,
                disposition: StoredAssetDisposition::Created,
            });
        }
        Err(AssetStorageError::NameCollisionExhausted)
    }

    pub fn apply_effect(&self, effect: &AssetEffect) -> Result<(), AssetStorageError> {
        let source_location = effect.from;
        let target_location = effect.to;
        let source = self.path(&effect.name, source_location);
        let target = self.path(&effect.name, target_location);
        if !source.try_exists().map_err(AssetStorageError::Io)? {
            if target.try_exists().map_err(AssetStorageError::Io)? {
                self.prove(&effect.name, target_location)?;
                return Ok(());
            }
            return Err(AssetStorageError::MissingAsset(effect.name.clone()));
        }
        let source = self.prove_open(&effect.name, source_location)?;
        if target.try_exists().map_err(AssetStorageError::Io)? {
            let target = self.prove_open(&effect.name, target_location)?;
            require_equal_digest(&effect.name, source.digest, target.digest)?;
            // The full digests match, so removing the duplicate cannot discard
            // a distinct prefix-collision payload.
            delete_open_file(source.file).map_err(AssetStorageError::Io)?;
            return Ok(());
        }
        let target_root = self.open_root(target_location)?;
        rename_open_file(&source.file, &target_root.directory, &target)
            .map_err(AssetStorageError::Io)
    }

    pub fn prove(
        &self,
        name: &ManagedAssetName,
        location: ManagedAssetLocation,
    ) -> Result<AssetLocationProof, AssetStorageError> {
        self.prove_open(name, location)?;
        Ok(match location {
            ManagedAssetLocation::Images => AssetLocationProof::Images,
            ManagedAssetLocation::Trash => AssetLocationProof::Trash,
        })
    }

    fn prove_open(
        &self,
        name: &ManagedAssetName,
        location: ManagedAssetLocation,
    ) -> Result<ProvenAsset, AssetStorageError> {
        let _root = self.open_root(location)?;
        let path = self.path(name, location);
        let mut file = open_for_managed_mutation(&path).map_err(AssetStorageError::Io)?;
        let metadata = file.metadata().map_err(AssetStorageError::Io)?;
        if !metadata.file_type().is_file() || is_reparse(&metadata) {
            return Err(AssetStorageError::OwnershipNotProven(path));
        }
        let before = observe_open_file(&file).map_err(AssetStorageError::Io)?;
        let bytes = read_bounded_file(&mut file)?;
        let digest = hash_bytes(&bytes);
        if !digest.to_hex().starts_with(name.hash_prefix()) {
            return Err(AssetStorageError::OwnershipNotProven(path));
        }
        let after = observe_open_file(&file).map_err(AssetStorageError::Io)?;
        if before != after {
            return Err(AssetStorageError::ChangedDuringProof(path));
        }
        Ok(ProvenAsset {
            file,
            digest,
            observation: after,
        })
    }

    pub fn remove_proven(
        &self,
        name: &ManagedAssetName,
        location: ManagedAssetLocation,
    ) -> Result<(), AssetStorageError> {
        let proven = self.prove_open(name, location)?;
        let current = observe_open_file(&proven.file).map_err(AssetStorageError::Io)?;
        if current != proven.observation {
            return Err(AssetStorageError::ChangedDuringProof(
                self.path(name, location),
            ));
        }
        delete_open_file(proven.file).map_err(AssetStorageError::Io)
    }

    pub fn reconcile(
        &self,
        references: &HashMap<ManagedAssetName, usize>,
        mode: AssetReconcileMode,
    ) -> Result<AssetReconcileReport, AssetStorageError> {
        let mut report = AssetReconcileReport::default();
        for name in references.keys() {
            if self.prove(name, ManagedAssetLocation::Images).is_ok() {
                continue;
            }
            if self.prove(name, ManagedAssetLocation::Trash).is_ok() {
                self.apply_effect(&AssetEffect {
                    name: name.clone(),
                    from: ManagedAssetLocation::Trash,
                    to: ManagedAssetLocation::Images,
                })?;
                report.restored += 1;
            } else {
                report.missing_references.push(name.clone());
            }
        }
        self.reconcile_directory(ManagedAssetLocation::Images, references, mode, &mut report)?;
        self.reconcile_directory(ManagedAssetLocation::Trash, references, mode, &mut report)?;
        report.missing_references.sort();
        Ok(report)
    }

    fn reconcile_directory(
        &self,
        location: ManagedAssetLocation,
        references: &HashMap<ManagedAssetName, usize>,
        mode: AssetReconcileMode,
        report: &mut AssetReconcileReport,
    ) -> Result<(), AssetStorageError> {
        let root = match location {
            ManagedAssetLocation::Images => &self.images,
            ManagedAssetLocation::Trash => &self.trash,
        };
        let entries = fs::read_dir(root)
            .map_err(AssetStorageError::Io)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AssetStorageError::Io)?;
        for entry in entries {
            let Some(name) = entry.file_name().to_str().and_then(ManagedAssetName::parse) else {
                continue;
            };
            if self.prove(&name, location).is_err() {
                report.skipped_untrusted += 1;
                continue;
            }
            let referenced = references.get(&name).copied().unwrap_or(0) > 0;
            match (location, referenced) {
                (ManagedAssetLocation::Images, false) => {
                    self.apply_effect(&AssetEffect {
                        name,
                        from: ManagedAssetLocation::Images,
                        to: ManagedAssetLocation::Trash,
                    })?;
                    report.moved_to_trash += 1;
                }
                (ManagedAssetLocation::Trash, true) => {
                    self.apply_effect(&AssetEffect {
                        name,
                        from: ManagedAssetLocation::Trash,
                        to: ManagedAssetLocation::Images,
                    })?;
                    report.restored += 1;
                }
                (ManagedAssetLocation::Trash, false)
                    if mode == AssetReconcileMode::SafeBoundary =>
                {
                    self.remove_proven(&name, ManagedAssetLocation::Trash)?;
                    report.deleted_from_trash += 1;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn publish_new(&self, target: &Path, bytes: &[u8]) -> Result<(), AssetStorageError> {
        let target_root = self.open_root(ManagedAssetLocation::Images)?;
        if bytes.len() > MAX_ENCODED_IMAGE_BYTES {
            return Err(AssetStorageError::TooLarge);
        }
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = self.images.join(format!(
            ".stickymd-asset-{}-{sequence}.tmp",
            std::process::id()
        ));
        let mut file = create_new_for_managed_publish(&temporary).map_err(AssetStorageError::Io)?;
        let result = (|| {
            file.write_all(bytes).map_err(AssetStorageError::Io)?;
            file.sync_all().map_err(AssetStorageError::Io)?;
            rename_open_file(&file, &target_root.directory, target).map_err(AssetStorageError::Io)
        })();
        if result.is_err() {
            // Cleanup is bound to the same create-new handle. A concurrent
            // pathname replacement can therefore never make us delete an
            // object we did not create; inability to prove cleanup leaves the
            // temporary as recoverable evidence.
            let _ = delete_open_file(file);
        }
        result
    }

    fn verify_root(&self, location: ManagedAssetLocation) -> Result<(), AssetStorageError> {
        self.open_root(location).map(|_| ())
    }

    fn open_root(
        &self,
        location: ManagedAssetLocation,
    ) -> Result<BoundAssetRoot, AssetStorageError> {
        let (path, canonical) = match location {
            ManagedAssetLocation::Images => (&self.images, &self.canonical_images),
            ManagedAssetLocation::Trash => (&self.trash, &self.canonical_trash),
        };
        ensure_plain_directory(&self.note)?;
        let current_note = fs::canonicalize(&self.note).map_err(AssetStorageError::Directory)?;
        if current_note != self.canonical_note {
            return Err(AssetStorageError::ReparseDirectory(self.note.clone()));
        }
        let note = open_managed_directory(&self.note).map_err(AssetStorageError::Directory)?;
        if !observe_open_file(&note)
            .map_err(AssetStorageError::Directory)?
            .same_identity(self.note_observation)
        {
            return Err(AssetStorageError::ReparseDirectory(self.note.clone()));
        }
        ensure_plain_directory(path)?;
        let current = fs::canonicalize(path).map_err(AssetStorageError::Directory)?;
        if &current != canonical {
            return Err(AssetStorageError::ReparseDirectory(path.clone()));
        }
        let directory = open_managed_directory(path).map_err(AssetStorageError::Directory)?;
        let expected = match location {
            ManagedAssetLocation::Images => self.images_observation,
            ManagedAssetLocation::Trash => self.trash_observation,
        };
        if !observe_open_file(&directory)
            .map_err(AssetStorageError::Directory)?
            .same_identity(expected)
        {
            return Err(AssetStorageError::ReparseDirectory(path.clone()));
        }
        Ok(BoundAssetRoot {
            _note: note,
            directory,
        })
    }

    fn path(&self, name: &ManagedAssetName, location: ManagedAssetLocation) -> PathBuf {
        match location {
            ManagedAssetLocation::Images => self.images.join(name.as_str()),
            ManagedAssetLocation::Trash => self.trash.join(name.as_str()),
        }
    }
}

fn ensure_plain_directory(path: &Path) -> Result<(), AssetStorageError> {
    let metadata = fs::symlink_metadata(path).map_err(AssetStorageError::Directory)?;
    if !metadata.file_type().is_dir() || is_reparse(&metadata) {
        Err(AssetStorageError::ReparseDirectory(path.to_owned()))
    } else {
        Ok(())
    }
}

fn is_reparse(metadata: &fs::Metadata) -> bool {
    metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT_VALUE != 0
}

fn require_equal_digest(
    name: &ManagedAssetName,
    source: stickymd_core::Hash32,
    target: stickymd_core::Hash32,
) -> Result<(), AssetStorageError> {
    if source == target {
        Ok(())
    } else {
        Err(AssetStorageError::AmbiguousCollision(name.clone()))
    }
}
fn read_bounded_file(file: &mut File) -> Result<Vec<u8>, AssetStorageError> {
    let mut bytes = Vec::new();
    file.take((MAX_ENCODED_IMAGE_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(AssetStorageError::Io)?;
    if bytes.len() > MAX_ENCODED_IMAGE_BYTES {
        Err(AssetStorageError::TooLarge)
    } else {
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use stickymd_render::image::prepare_rgba_image;

    fn fixture() -> (PathBuf, AssetStorage) {
        let root = unique_temp_path("assets");
        let images = root.join("images");
        let trash = root.join(".trash");
        fs::create_dir_all(&images).unwrap();
        fs::create_dir(&trash).unwrap();
        let storage = AssetStorage::open(&images, &trash).unwrap();
        (root, storage)
    }
    fn prepared(color: u8) -> PreparedImage {
        prepare_rgba_image(2, 2, vec![color; 16]).unwrap()
    }

    #[test]
    fn content_addressed_store_deduplicates_and_proves_hash() {
        let (root, storage) = fixture();
        let image = prepared(7);
        let first = storage.store(&image).unwrap();
        let second = storage.store(&image).unwrap();
        assert_eq!(first.name, second.name);
        assert_eq!(first.disposition, StoredAssetDisposition::Created);
        assert_eq!(second.disposition, StoredAssetDisposition::Reused);
        storage
            .prove(&first.name, ManagedAssetLocation::Images)
            .unwrap();
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn managed_looking_wrong_hash_is_never_moved_or_deleted() {
        let (root, storage) = fixture();
        let name = ManagedAssetName::parse("stickymd-00000000000000000000.png").unwrap();
        let path = storage.images.join(name.as_str());
        fs::write(&path, b"user bytes").unwrap();
        let mut refs = HashMap::new();
        refs.insert(name.clone(), 1);
        let report = storage
            .reconcile(&refs, AssetReconcileMode::SafeBoundary)
            .unwrap();
        assert!(path.exists());
        assert_eq!(fs::read(path).unwrap(), b"user bytes");
        assert_eq!(report.skipped_untrusted, 1);
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn unreferenced_proven_asset_moves_then_deletes_in_order() {
        let (root, storage) = fixture();
        let asset = storage.store(&prepared(9)).unwrap();
        let report = storage
            .reconcile(&HashMap::new(), AssetReconcileMode::SafeBoundary)
            .unwrap();
        assert_eq!(report.moved_to_trash, 1);
        assert_eq!(report.deleted_from_trash, 1);
        assert!(!storage.images.join(asset.name.as_str()).exists());
        assert!(!storage.trash.join(asset.name.as_str()).exists());
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn user_named_file_is_untouched() {
        let (root, storage) = fixture();
        let path = storage.images.join("my-photo.png");
        fs::write(&path, b"user").unwrap();
        storage
            .reconcile(&HashMap::new(), AssetReconcileMode::SafeBoundary)
            .unwrap();
        assert_eq!(fs::read(path).unwrap(), b"user");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phase7_hash_collision_falls_back_from_20_to_32_hex() {
        let (root, storage) = fixture();
        let image = prepared(11);
        let hash = image.hash().to_hex();
        let short = ManagedAssetName::from_hash_prefix(&hash, 20, image.extension()).unwrap();
        fs::write(storage.images.join(short.as_str()), b"untrusted collision").unwrap();
        let stored = storage.store(&image).unwrap();
        assert_eq!(stored.name.hash_prefix().len(), 32);
        assert_eq!(
            fs::read(storage.images.join(short.as_str())).unwrap(),
            b"untrusted collision"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phase7_reconcile_restores_referenced_trash_and_preserves_user_trash() {
        let (root, storage) = fixture();
        let asset = storage.store(&prepared(13)).unwrap();
        storage
            .apply_effect(&AssetEffect {
                name: asset.name.clone(),
                from: ManagedAssetLocation::Images,
                to: ManagedAssetLocation::Trash,
            })
            .unwrap();
        fs::write(storage.trash.join("user-important.png"), b"user").unwrap();
        let mut references = HashMap::new();
        references.insert(asset.name.clone(), 1);
        let report = storage
            .reconcile(&references, AssetReconcileMode::SafeBoundary)
            .unwrap();
        assert_eq!(report.restored, 1);
        assert!(storage.images.join(asset.name.as_str()).is_file());
        assert_eq!(
            fs::read(storage.trash.join("user-important.png")).unwrap(),
            b"user"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn phase7_duplicate_proven_trash_copy_is_removed_but_mismatch_is_preserved() {
        let (root, storage) = fixture();
        let asset = storage.store(&prepared(17)).unwrap();
        let active = storage.images.join(asset.name.as_str());
        let trash = storage.trash.join(asset.name.as_str());
        fs::copy(&active, &trash).unwrap();
        let mut references = HashMap::new();
        references.insert(asset.name.clone(), 1);
        storage
            .reconcile(&references, AssetReconcileMode::SafeBoundary)
            .unwrap();
        assert!(active.is_file());
        assert!(!trash.exists());
        fs::write(&trash, b"not owned despite managed-looking name").unwrap();
        storage
            .reconcile(&references, AssetReconcileMode::SafeBoundary)
            .unwrap();
        assert_eq!(
            fs::read(&trash).unwrap(),
            b"not owned despite managed-looking name"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn full_digest_mismatch_is_never_treated_as_a_duplicate() {
        let name = ManagedAssetName::parse("stickymd-00000000000000000000.png").unwrap();
        assert!(matches!(
            require_equal_digest(
                &name,
                stickymd_core::Hash32::new([1; 32]),
                stickymd_core::Hash32::new([2; 32]),
            ),
            Err(AssetStorageError::AmbiguousCollision(_))
        ));
    }

    #[test]
    fn effect_missing_from_both_locations_is_not_reported_as_success() {
        let (root, storage) = fixture();
        let name = ManagedAssetName::parse("stickymd-0123456789abcdef0123.png").unwrap();
        let result = storage.apply_effect(&AssetEffect {
            name: name.clone(),
            from: ManagedAssetLocation::Images,
            to: ManagedAssetLocation::Trash,
        });
        assert!(matches!(result, Err(AssetStorageError::MissingAsset(found)) if found == name));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn runtime_reconcile_never_physically_deletes_unreferenced_trash() {
        let (root, storage) = fixture();
        let asset = storage.store(&prepared(19)).unwrap();
        storage
            .apply_effect(&AssetEffect {
                name: asset.name.clone(),
                from: ManagedAssetLocation::Images,
                to: ManagedAssetLocation::Trash,
            })
            .unwrap();
        let report = storage
            .reconcile(&HashMap::new(), AssetReconcileMode::Runtime)
            .unwrap();
        assert_eq!(report.deleted_from_trash, 0);
        assert!(storage.trash.join(asset.name.as_str()).is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn storage_rejects_a_replaced_note_root_before_mutation() {
        let (root, storage) = fixture();
        let original_note = root.clone();
        let moved_note = root.with_extension("moved-note");
        fs::rename(&original_note, &moved_note).unwrap();
        fs::create_dir(&original_note).unwrap();
        fs::create_dir(original_note.join("images")).unwrap();
        fs::create_dir(original_note.join(".trash")).unwrap();
        let image = prepared(31);
        assert!(matches!(
            storage.store(&image),
            Err(AssetStorageError::ReparseDirectory(_))
        ));
        assert_eq!(
            fs::read_dir(original_note.join("images")).unwrap().count(),
            0
        );
        fs::remove_dir_all(original_note).unwrap();
        fs::remove_dir_all(moved_note).unwrap();
    }
}
