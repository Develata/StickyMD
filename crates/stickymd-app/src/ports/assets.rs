//! The managed asset port and its receipts.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#managed-vs-user-asset

use std::collections::{BTreeSet, HashMap};

use stickymd_core::{
    AssetEffect, Generation, Hash32, ManagedAssetLocation, ManagedAssetName, Selection,
};

use super::StorageFailure;
use crate::flow::ClipboardPaste;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetReconcileMode {
    Runtime,
    SafeBoundary,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoredAssetDisposition {
    Created,
    Reused,
    Restored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAsset {
    pub name: ManagedAssetName,
    pub disposition: StoredAssetDisposition,
}

#[derive(Debug, Clone)]
pub struct AssetPasteRequest {
    pub expected_generation: Generation,
    pub selection: Selection,
    pub timestamp_ms: u64,
    pub payload: ClipboardPaste,
}

#[derive(Debug)]
pub struct AssetPasteCompletion {
    pub expected_generation: Generation,
    pub selection: Selection,
    pub timestamp_ms: u64,
    pub markdown: String,
    pub rollback: AssetPasteRollback,
}

#[derive(Debug, Default, Clone)]
pub struct AssetPasteRollback {
    pub stored: Vec<StoredAsset>,
}

impl AssetPasteRollback {
    /// Collapse rollback evidence to the latest canonical reference state.
    /// This is used when a completed paste lost its generation race: replaying
    /// the old disposition blindly could move a now-referenced asset to trash.
    pub fn convergence_effects(
        &self,
        references: &HashMap<ManagedAssetName, usize>,
    ) -> Vec<AssetEffect> {
        self.stored
            .iter()
            .map(|asset| asset.name.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|name| {
                let referenced = references.get(&name).copied().unwrap_or(0) > 0;
                let (from, to) = if referenced {
                    (ManagedAssetLocation::Trash, ManagedAssetLocation::Images)
                } else {
                    (ManagedAssetLocation::Images, ManagedAssetLocation::Trash)
                };
                AssetEffect { name, from, to }
            })
            .collect()
    }
}

/// A failed paste still returns every publication it performed. Only the coordinator can compare
/// this ledger with the latest canonical references and request a safe convergence; the worker
/// must never replay an old rollback decision by itself.
#[derive(Debug)]
pub struct AssetPasteFailure {
    pub expected_generation: Generation,
    pub error: StorageFailure,
    pub rollback: AssetPasteRollback,
}

/// One request to bring managed files to the newest references.
#[derive(Debug, Clone)]
pub struct AssetSyncRequest {
    pub request_id: u64,
    pub generation: Generation,
    pub effects: Vec<AssetEffect>,
    pub references: HashMap<ManagedAssetName, usize>,
    pub reconcile: Option<AssetReconcileMode>,
    /// For a safe boundary: the Durable Fingerprint the note must still have for trash to be
    /// deleted.
    pub safe_expected: Option<Option<Hash32>>,
}

/// Durable storage of managed images. Each call is one complete transaction on the worker thread.
pub trait ManagedAssetStore: Send + 'static {
    fn paste(
        &mut self,
        request: AssetPasteRequest,
    ) -> Result<AssetPasteCompletion, AssetPasteFailure>;

    fn sync(&mut self, request: &AssetSyncRequest) -> Result<AssetReconcileReport, StorageFailure>;
}
