//! Managed images on macOS.
//!
//! POSIX cannot tie a content check to a rename or a delete of the same file, so this store does
//! what plan 08 allows when ownership cannot be proven at the moment of the operation: unreferenced
//! images stay where they are and nothing is ever deleted. Only a restore from `.trash` moves a
//! file, and it verifies what it moved afterwards.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#macos-clipboard-and-assets

use std::collections::HashMap;
use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use sha2::{Digest, Sha256};
use stickymd_app::ports::{AssetReconcileReport, StoredAsset, StoredAssetDisposition};
use stickymd_core::{AssetEffect, ManagedAssetLocation, ManagedAssetName};
use stickymd_render::image::{MAX_ENCODED_IMAGE_BYTES, PreparedImage};
use thiserror::Error;

use crate::platform::macos::file::{DirectoryHandle, FileObservation, full_sync};

static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Error)]
pub enum AssetStorageError {
    #[error("asset directory is unavailable: {0}")]
    Directory(io::Error),
    #[error("asset path is not a proven StickyMD-owned regular file: {}", .0.display())]
    OwnershipNotProven(PathBuf),
    #[error("asset payload exceeds the 64 MiB bound")]
    TooLarge,
    #[error("asset I/O failed: {0}")]
    Io(io::Error),
    #[error("all managed hash-prefix names collided with untrusted content")]
    NameCollisionExhausted,
    #[error("asset changed while ownership was being verified: {}", .0.display())]
    ChangedDuringProof(PathBuf),
    #[error("managed asset is missing from both active and trash locations: {}", .0)]
    MissingAsset(ManagedAssetName),
}

#[derive(Debug)]
pub struct AssetStorage {
    images_path: PathBuf,
    trash_path: PathBuf,
    images: DirectoryHandle,
    trash: DirectoryHandle,
}

struct ProvenAsset {
    digest: stickymd_core::Hash32,
    observation: FileObservation,
}

impl AssetStorage {
    #[cfg(test)]
    pub fn open(images: &Path, trash: &Path) -> Result<Self, AssetStorageError> {
        Ok(Self {
            images_path: images.to_owned(),
            trash_path: trash.to_owned(),
            images: DirectoryHandle::open(images).map_err(AssetStorageError::Directory)?,
            trash: DirectoryHandle::open(trash).map_err(AssetStorageError::Directory)?,
        })
    }

    /// Finds both directories inside `note`, wherever it is now: of `images` and `trash` only
    /// the last component counts.
    pub fn open_in(
        note: &DirectoryHandle,
        images: &Path,
        trash: &Path,
    ) -> Result<Self, AssetStorageError> {
        let base = note.current_path().map_err(AssetStorageError::Directory)?;
        let child = |path: &Path| {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| io::Error::other("the directory has no usable name"))?;
            Ok((base.join(name), note.open_child(name)?))
        };
        let (images_path, images) = child(images).map_err(AssetStorageError::Directory)?;
        let (trash_path, trash) = child(trash).map_err(AssetStorageError::Directory)?;
        Ok(Self {
            images_path,
            trash_path,
            images,
            trash,
        })
    }

    fn handle(&self, location: ManagedAssetLocation) -> &DirectoryHandle {
        match location {
            ManagedAssetLocation::Images => &self.images,
            ManagedAssetLocation::Trash => &self.trash,
        }
    }

    fn path(&self, name: &str, location: ManagedAssetLocation) -> PathBuf {
        match location {
            ManagedAssetLocation::Images => self.images_path.join(name),
            ManagedAssetLocation::Trash => self.trash_path.join(name),
        }
    }

    fn occupied(
        &self,
        name: &str,
        location: ManagedAssetLocation,
    ) -> Result<bool, AssetStorageError> {
        match fs::symlink_metadata(self.path(name, location)) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(AssetStorageError::Io(error)),
        }
    }

    pub fn store(&self, image: &PreparedImage) -> Result<StoredAsset, AssetStorageError> {
        let full_hash = image.hash().to_hex();
        for prefix in [20_usize, 32, 64] {
            let Some(name) =
                ManagedAssetName::from_hash_prefix(&full_hash, prefix, image.extension())
            else {
                continue;
            };
            if self.occupied(name.as_str(), ManagedAssetLocation::Images)? {
                if self
                    .prove(&name, ManagedAssetLocation::Images)
                    .is_ok_and(|proven| proven.digest == image.hash())
                {
                    return Ok(StoredAsset {
                        name,
                        disposition: StoredAssetDisposition::Reused,
                    });
                }
                continue;
            }
            if self.occupied(name.as_str(), ManagedAssetLocation::Trash)? {
                if let Ok(proven) = self.prove(&name, ManagedAssetLocation::Trash)
                    && proven.digest == image.hash()
                {
                    self.restore(&name, &proven)?;
                    return Ok(StoredAsset {
                        name,
                        disposition: StoredAssetDisposition::Restored,
                    });
                }
                continue;
            }
            self.publish_new(name.as_str(), image.bytes())?;
            return Ok(StoredAsset {
                name,
                disposition: StoredAssetDisposition::Created,
            });
        }
        Err(AssetStorageError::NameCollisionExhausted)
    }

    /// Only a move back from `.trash` happens; the opposite direction is left undone (plan 08).
    pub fn apply_effect(&self, effect: &AssetEffect) -> Result<(), AssetStorageError> {
        if effect.to != ManagedAssetLocation::Images || effect.from != ManagedAssetLocation::Trash {
            return Ok(());
        }
        let name = effect.name.as_str();
        if !self.occupied(name, ManagedAssetLocation::Trash)? {
            if self.occupied(name, ManagedAssetLocation::Images)? {
                self.prove(&effect.name, ManagedAssetLocation::Images)?;
                return Ok(());
            }
            return Err(AssetStorageError::MissingAsset(effect.name.clone()));
        }
        let proven = self.prove(&effect.name, ManagedAssetLocation::Trash)?;
        if self.occupied(name, ManagedAssetLocation::Images)? {
            // Already active. The duplicate in `.trash` is left alone: nothing is deleted.
            self.prove(&effect.name, ManagedAssetLocation::Images)?;
            return Ok(());
        }
        self.restore(&effect.name, &proven)
    }

    /// Reconciles references without moving anything out of `images/` or deleting anything.
    pub fn reconcile(
        &self,
        references: &HashMap<ManagedAssetName, usize>,
    ) -> Result<AssetReconcileReport, AssetStorageError> {
        let mut report = AssetReconcileReport::default();
        for name in references.keys() {
            if self.prove(name, ManagedAssetLocation::Images).is_ok() {
                continue;
            }
            match self.prove(name, ManagedAssetLocation::Trash) {
                Ok(proven) => {
                    self.restore(name, &proven)?;
                    report.restored += 1;
                }
                Err(_) => report.missing_references.push(name.clone()),
            }
        }
        for location in [ManagedAssetLocation::Images, ManagedAssetLocation::Trash] {
            let root = match location {
                ManagedAssetLocation::Images => &self.images_path,
                ManagedAssetLocation::Trash => &self.trash_path,
            };
            for entry in fs::read_dir(root).map_err(AssetStorageError::Io)? {
                let entry = entry.map_err(AssetStorageError::Io)?;
                let Some(name) = entry.file_name().to_str().and_then(ManagedAssetName::parse)
                else {
                    continue;
                };
                if references.get(&name).copied().unwrap_or(0) > 0 {
                    continue;
                }
                report.skipped_untrusted += 1;
                if location == ManagedAssetLocation::Trash {
                    report.physical_delete_deferred = true;
                }
            }
        }
        report.missing_references.sort();
        Ok(report)
    }

    fn prove(
        &self,
        name: &ManagedAssetName,
        location: ManagedAssetLocation,
    ) -> Result<ProvenAsset, AssetStorageError> {
        let path = self.path(name.as_str(), location);
        let mut file = self
            .handle(location)
            .open_read(name.as_str())
            .map_err(|_| AssetStorageError::OwnershipNotProven(path.clone()))?;
        let before = FileObservation::of(&file).map_err(AssetStorageError::Io)?;
        if before.size() > MAX_ENCODED_IMAGE_BYTES as u64 {
            return Err(AssetStorageError::TooLarge);
        }
        let digest = hash_bounded(&mut file)?;
        if !digest.to_hex().starts_with(name.hash_prefix()) {
            return Err(AssetStorageError::OwnershipNotProven(path));
        }
        let after = FileObservation::of(&file).map_err(AssetStorageError::Io)?;
        if before != after {
            return Err(AssetStorageError::ChangedDuringProof(path));
        }
        Ok(ProvenAsset {
            digest,
            observation: after,
        })
    }

    /// Moves a proven file from `.trash` to `images/`, then checks that what moved is what was
    /// proven, and undoes the move if it is not.
    fn restore(
        &self,
        name: &ManagedAssetName,
        proven: &ProvenAsset,
    ) -> Result<(), AssetStorageError> {
        let leaf = name.as_str();
        self.trash
            .rename_no_replace_into(leaf, &self.images, leaf)
            .map_err(AssetStorageError::Io)?;
        let matches = self
            .prove(name, ManagedAssetLocation::Images)
            .is_ok_and(|moved| {
                moved.digest == proven.digest && moved.observation.same_file_as(&proven.observation)
            });
        if !matches {
            self.images
                .rename_no_replace_into(leaf, &self.trash, leaf)
                .map_err(AssetStorageError::Io)?;
            return Err(AssetStorageError::ChangedDuringProof(
                self.path(leaf, ManagedAssetLocation::Trash),
            ));
        }
        self.images.sync().map_err(AssetStorageError::Io)
    }

    fn publish_new(&self, target: &str, bytes: &[u8]) -> Result<(), AssetStorageError> {
        if bytes.len() > MAX_ENCODED_IMAGE_BYTES {
            return Err(AssetStorageError::TooLarge);
        }
        let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let temporary = format!(".stickymd-asset-{}-{sequence}.tmp", std::process::id());
        self.publish_through(&temporary, target, bytes)
    }

    /// A name already taken is not ours: only a file this call created is cleaned up, and only
    /// while the name still leads to it.
    fn publish_through(
        &self,
        temporary: &str,
        target: &str,
        bytes: &[u8],
    ) -> Result<(), AssetStorageError> {
        let mut file = self
            .images
            .create_new(temporary)
            .map_err(AssetStorageError::Io)?;
        let created = FileObservation::of(&file).map_err(AssetStorageError::Io)?;
        let result = (|| {
            file.write_all(bytes).map_err(AssetStorageError::Io)?;
            full_sync(&file).map_err(AssetStorageError::Io)?;
            self.images
                .rename_no_replace(temporary, target)
                .map_err(AssetStorageError::Io)?;
            self.images.sync().map_err(AssetStorageError::Io)
        })();
        if result.is_err() {
            let still_ours = self
                .images
                .open_read(temporary)
                .and_then(|file| FileObservation::of(&file))
                .is_ok_and(|now| now.same_file_as(&created));
            if still_ours {
                let _ = self.images.remove_file(temporary);
            }
        }
        result
    }
}

fn hash_bounded(file: &mut fs::File) -> Result<stickymd_core::Hash32, AssetStorageError> {
    let mut digest = Sha256::new();
    let mut remaining = MAX_ENCODED_IMAGE_BYTES as u64 + 1;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining > 0 {
        let wanted = buffer.len().min(remaining as usize);
        let read = file
            .read(&mut buffer[..wanted])
            .map_err(AssetStorageError::Io)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
        remaining -= read as u64;
    }
    if remaining == 0 {
        return Err(AssetStorageError::TooLarge);
    }
    Ok(stickymd_core::Hash32::new(digest.finalize().into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use stickymd_render::image::prepare_rgba_image;

    fn fixture(label: &str) -> (PathBuf, AssetStorage) {
        let root = unique_temp_path(label);
        fs::create_dir_all(root.join("images")).unwrap();
        fs::create_dir(root.join(".trash")).unwrap();
        let storage = AssetStorage::open(&root.join("images"), &root.join(".trash")).unwrap();
        (root, storage)
    }

    fn image(color: u8) -> PreparedImage {
        prepare_rgba_image(2, 2, vec![color; 16]).unwrap()
    }

    #[test]
    fn a_moved_note_directory_keeps_its_images_reachable_by_path() {
        let root = unique_temp_path("assets-moved-note");
        let note = root.join("note");
        fs::create_dir_all(note.join("images")).unwrap();
        fs::create_dir(note.join(".trash")).unwrap();
        let handle = DirectoryHandle::open(&note).unwrap();
        let moved = root.join("moved");
        fs::rename(&note, &moved).unwrap();
        fs::create_dir(&note).unwrap();
        let storage =
            AssetStorage::open_in(&handle, &note.join("images"), &note.join(".trash")).unwrap();
        assert_eq!(storage.images_path, moved.join("images"));
        assert_eq!(storage.trash_path, moved.join(".trash"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_temporary_name_already_taken_is_left_alone() {
        let (root, storage) = fixture("assets-taken-temporary");
        let taken = root.join("images/.stickymd-asset-taken.tmp");
        fs::write(&taken, b"user").unwrap();
        storage
            .publish_through(".stickymd-asset-taken.tmp", "target.png", b"new")
            .unwrap_err();
        assert_eq!(fs::read(&taken).unwrap(), b"user");
        assert!(!root.join("images/target.png").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_failed_publish_removes_its_own_temporary_and_nothing_else() {
        let (root, storage) = fixture("assets-own-temporary");
        fs::write(root.join("images/target.png"), b"old").unwrap();
        storage
            .publish_through(".stickymd-asset-own.tmp", "target.png", b"new")
            .unwrap_err();
        assert!(!root.join("images/.stickymd-asset-own.tmp").exists());
        assert_eq!(fs::read(root.join("images/target.png")).unwrap(), b"old");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn storing_creates_then_reuses() {
        let (root, storage) = fixture("asset-store");
        let picture = image(1);
        let first = storage.store(&picture).unwrap();
        assert_eq!(first.disposition, StoredAssetDisposition::Created);
        assert_eq!(
            fs::read(root.join("images").join(first.name.as_str())).unwrap(),
            picture.bytes()
        );
        let second = storage.store(&picture).unwrap();
        assert_eq!(second.disposition, StoredAssetDisposition::Reused);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_trashed_copy_is_restored_instead_of_written_again() {
        let (root, storage) = fixture("asset-restore");
        let picture = image(2);
        let stored = storage.store(&picture).unwrap();
        fs::rename(
            root.join("images").join(stored.name.as_str()),
            root.join(".trash").join(stored.name.as_str()),
        )
        .unwrap();
        let again = storage.store(&picture).unwrap();
        assert_eq!(again.disposition, StoredAssetDisposition::Restored);
        assert!(root.join("images").join(again.name.as_str()).is_file());
        assert!(!root.join(".trash").join(again.name.as_str()).exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_name_taken_by_other_bytes_is_not_trusted() {
        let (root, storage) = fixture("asset-collision");
        let picture = image(3);
        let hash = picture.hash().to_hex();
        let short = ManagedAssetName::from_hash_prefix(&hash, 20, picture.extension()).unwrap();
        fs::write(root.join("images").join(short.as_str()), b"user bytes").unwrap();
        let stored = storage.store(&picture).unwrap();
        assert_ne!(stored.name, short);
        assert_eq!(
            fs::read(root.join("images").join(short.as_str())).unwrap(),
            b"user bytes"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_unreferenced_image_is_neither_moved_nor_deleted() {
        let (root, storage) = fixture("asset-deferred");
        let picture = image(4);
        let stored = storage.store(&picture).unwrap();
        let active = root.join("images").join(stored.name.as_str());
        storage
            .apply_effect(&AssetEffect {
                name: stored.name.clone(),
                from: ManagedAssetLocation::Images,
                to: ManagedAssetLocation::Trash,
            })
            .unwrap();
        assert!(active.is_file());
        let report = storage.reconcile(&HashMap::new()).unwrap();
        assert!(active.is_file());
        assert_eq!(report.moved_to_trash, 0);
        assert_eq!(report.deleted_from_trash, 0);
        assert_eq!(report.skipped_untrusted, 1);
        assert!(!report.physical_delete_deferred);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn an_unreferenced_trash_entry_stays_and_says_deletion_was_deferred() {
        let (root, storage) = fixture("asset-trash-stays");
        let picture = image(5);
        let stored = storage.store(&picture).unwrap();
        let trashed = root.join(".trash").join(stored.name.as_str());
        fs::rename(root.join("images").join(stored.name.as_str()), &trashed).unwrap();
        let report = storage.reconcile(&HashMap::new()).unwrap();
        assert!(trashed.is_file());
        assert!(report.physical_delete_deferred);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_referenced_image_in_trash_comes_back_and_a_missing_one_is_reported() {
        let (root, storage) = fixture("asset-referenced");
        let picture = image(6);
        let stored = storage.store(&picture).unwrap();
        fs::rename(
            root.join("images").join(stored.name.as_str()),
            root.join(".trash").join(stored.name.as_str()),
        )
        .unwrap();
        let missing =
            ManagedAssetName::from_hash_prefix(&"a".repeat(64), 20, picture.extension()).unwrap();
        let references = HashMap::from([(stored.name.clone(), 1), (missing.clone(), 1)]);
        let report = storage.reconcile(&references).unwrap();
        assert_eq!(report.restored, 1);
        assert_eq!(report.missing_references, vec![missing]);
        assert!(root.join("images").join(stored.name.as_str()).is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_user_file_with_a_managed_name_is_never_restored() {
        let (root, storage) = fixture("asset-user-file");
        let name =
            ManagedAssetName::from_hash_prefix(&"b".repeat(64), 20, image(7).extension()).unwrap();
        fs::write(root.join(".trash").join(name.as_str()), b"not the hash").unwrap();
        let references = HashMap::from([(name.clone(), 1)]);
        let report = storage.reconcile(&references).unwrap();
        assert_eq!(report.restored, 0);
        assert_eq!(report.missing_references, vec![name.clone()]);
        assert!(root.join(".trash").join(name.as_str()).is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_restore_with_nothing_to_restore_reports_it_missing() {
        let (root, storage) = fixture("asset-effect-missing");
        let name =
            ManagedAssetName::from_hash_prefix(&"c".repeat(64), 20, image(8).extension()).unwrap();
        let outcome = storage.apply_effect(&AssetEffect {
            name,
            from: ManagedAssetLocation::Trash,
            to: ManagedAssetLocation::Images,
        });
        assert!(matches!(outcome, Err(AssetStorageError::MissingAsset(_))));
        fs::remove_dir_all(root).unwrap();
    }
}
