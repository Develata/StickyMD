//! Durable-reference guard for destructive managed-asset reconciliation.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#state-changes

use std::collections::HashMap;
use std::io::Read;
use std::os::windows::fs::MetadataExt;
use std::path::Path;

use stickymd_core::{Hash32, LoadedDocument, ManagedAssetName, scan_managed_asset_references};

use super::{AssetReconcileMode, AssetReconcileReport, AssetStorage, AssetStorageError};
use crate::persistence::MAX_NOTE_LOAD;
use crate::platform::windows::managed_file::open_guarded_note_read;

const FILE_ATTRIBUTE_REPARSE_POINT_VALUE: u32 = 0x400;

/// Delete trash only while the exact durable note is write/replace-locked and
/// still matches the coordinator's expected fingerprint. Any uncertainty
/// degrades to non-destructive runtime reconciliation.
pub fn reconcile_safe_boundary(
    storage: &AssetStorage,
    note_file: &Path,
    expected: Option<Hash32>,
    runtime_references: &HashMap<ManagedAssetName, usize>,
) -> Result<AssetReconcileReport, AssetStorageError> {
    let Some(expected) = expected else {
        return reconcile_deferred(storage, runtime_references);
    };
    let mut note = match open_guarded_note_read(note_file) {
        Ok(note) => note,
        Err(_) => return reconcile_deferred(storage, runtime_references),
    };
    let metadata = match note.metadata() {
        Ok(metadata)
            if metadata.file_type().is_file()
                && metadata.file_attributes() & FILE_ATTRIBUTE_REPARSE_POINT_VALUE == 0 =>
        {
            metadata
        }
        _ => return reconcile_deferred(storage, runtime_references),
    };
    if metadata.len() > MAX_NOTE_LOAD {
        return reconcile_deferred(storage, runtime_references);
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    if (&mut note)
        .take(MAX_NOTE_LOAD + 1)
        .read_to_end(&mut bytes)
        .is_err()
        || bytes.len() as u64 > MAX_NOTE_LOAD
    {
        return reconcile_deferred(storage, runtime_references);
    }
    let Ok(durable) = LoadedDocument::from_durable_bytes(&bytes) else {
        return reconcile_deferred(storage, runtime_references);
    };
    if durable.fingerprint != expected {
        return reconcile_deferred(storage, runtime_references);
    }
    let mut references = runtime_references.clone();
    for (name, count) in scan_managed_asset_references(&durable.text) {
        references
            .entry(name)
            .and_modify(|current| *current = (*current).max(count))
            .or_insert(count);
    }
    // `note` stays alive through reconciliation. Its restrictive share mode
    // prevents the validated durable reference set from changing underneath
    // the physical-delete pass.
    storage.reconcile(&references, AssetReconcileMode::SafeBoundary)
}

fn reconcile_deferred(
    storage: &AssetStorage,
    references: &HashMap<ManagedAssetName, usize>,
) -> Result<AssetReconcileReport, AssetStorageError> {
    let mut report = storage.reconcile(references, AssetReconcileMode::Runtime)?;
    report.physical_delete_deferred = true;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use std::fs;
    use std::path::PathBuf;
    use stickymd_core::{AssetEffect, ManagedAssetLocation, hash_bytes};
    use stickymd_render::image::prepare_rgba_image;

    fn fixture() -> (
        PathBuf,
        PathBuf,
        AssetStorage,
        stickymd_core::ManagedAssetName,
    ) {
        let root = unique_temp_path("safe-assets");
        let note_dir = root.join("note");
        let images = note_dir.join("images");
        let trash = note_dir.join(".trash");
        fs::create_dir_all(&images).unwrap();
        fs::create_dir(&trash).unwrap();
        let storage = AssetStorage::open(&images, &trash).unwrap();
        let image = prepare_rgba_image(2, 2, vec![17; 16]).unwrap();
        let stored = storage.store(&image).unwrap();
        storage
            .apply_effect(&AssetEffect {
                name: stored.name.clone(),
                from: ManagedAssetLocation::Images,
                to: ManagedAssetLocation::Trash,
            })
            .unwrap();
        (root, note_dir.join("note.md"), storage, stored.name)
    }

    #[test]
    fn fingerprint_mismatch_defers_physical_delete_and_preserves_trash() {
        let (root, note, storage, name) = fixture();
        fs::write(&note, b"external").unwrap();
        let report = reconcile_safe_boundary(
            &storage,
            &note,
            Some(hash_bytes(b"different")),
            &HashMap::new(),
        )
        .unwrap();
        assert!(report.physical_delete_deferred);
        assert!(root.join("note/.trash").join(name.as_str()).is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn durable_and_runtime_reference_union_protects_newer_disk_references() {
        let (root, note, storage, name) = fixture();
        let durable = format!("![](images/{name})");
        fs::write(&note, durable.as_bytes()).unwrap();
        let report = reconcile_safe_boundary(
            &storage,
            &note,
            Some(hash_bytes(durable.as_bytes())),
            &HashMap::new(),
        )
        .unwrap();
        assert!(!report.physical_delete_deferred);
        assert_eq!(report.restored, 1);
        assert!(root.join("note/images").join(name.as_str()).is_file());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn matching_durable_empty_note_allows_proven_trash_cleanup() {
        let (root, note, storage, name) = fixture();
        fs::write(&note, b"").unwrap();
        let report =
            reconcile_safe_boundary(&storage, &note, Some(hash_bytes(b"")), &HashMap::new())
                .unwrap();
        assert!(!report.physical_delete_deferred);
        assert_eq!(report.deleted_from_trash, 1);
        assert!(!root.join("note/.trash").join(name.as_str()).exists());
        fs::remove_dir_all(root).unwrap();
    }
}
