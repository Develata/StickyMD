//! The startup asset boundary: when it runs, what it asks the store for, and what it reports.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#state-changes

use stickymd_core::{DocumentState, LineEnding, ManagedAssetName, hash_bytes};

use super::reconcile_at_startup;
use crate::ports::{
    AssetPasteCompletion, AssetPasteFailure, AssetPasteRequest, AssetReconcileMode,
    AssetReconcileReport, AssetSyncRequest, FailureKind, ManagedAssetStore, StorageFailure,
};

const ASSET: &str = "stickymd-0123456789abcdef0123.png";

/// Records each sync request and answers with `reply`.
struct RecordingStore {
    requests: Vec<AssetSyncRequest>,
    reply: Result<AssetReconcileReport, StorageFailure>,
}

impl RecordingStore {
    fn answering(reply: Result<AssetReconcileReport, StorageFailure>) -> Self {
        Self {
            requests: Vec::new(),
            reply,
        }
    }
}

impl ManagedAssetStore for RecordingStore {
    fn paste(
        &mut self,
        _request: AssetPasteRequest,
    ) -> Result<AssetPasteCompletion, AssetPasteFailure> {
        unreachable!("startup does not paste")
    }

    fn sync(&mut self, request: &AssetSyncRequest) -> Result<AssetReconcileReport, StorageFailure> {
        self.requests.push(request.clone());
        self.reply.clone()
    }
}

fn document() -> DocumentState {
    let base = Some(hash_bytes(b"canonical"));
    DocumentState::loaded(&format!("![](images/{ASSET})\n"), LineEnding::Lf, base)
}

#[test]
fn a_startup_that_offers_recovery_leaves_the_assets_to_the_recovery_choice() {
    let mut store = RecordingStore::answering(Ok(AssetReconcileReport::default()));
    assert_eq!(reconcile_at_startup(&mut store, &document(), true), None);
    assert!(store.requests.is_empty());
}

#[test]
fn a_normal_startup_runs_one_safe_boundary_over_the_loaded_references() {
    let document = document();
    let mut store = RecordingStore::answering(Ok(AssetReconcileReport::default()));
    assert_eq!(reconcile_at_startup(&mut store, &document, false), None);
    let [request] = store.requests.as_slice() else {
        panic!("one request, got {:?}", store.requests);
    };
    assert!(request.effects.is_empty());
    assert_eq!(request.reconcile, Some(AssetReconcileMode::SafeBoundary));
    assert_eq!(request.safe_expected, Some(document.base_disk_hash()));
    assert_eq!(&request.references, document.managed_ref_counts());
    assert_eq!(
        request
            .references
            .get(&ManagedAssetName::parse(ASSET).unwrap()),
        Some(&1)
    );
}

#[test]
fn missing_references_and_a_failed_boundary_go_on_the_status_line() {
    let missing = AssetReconcileReport {
        missing_references: vec![ManagedAssetName::parse(ASSET).unwrap()],
        ..AssetReconcileReport::default()
    };
    let mut store = RecordingStore::answering(Ok(missing));
    assert_eq!(
        reconcile_at_startup(&mut store, &document(), false).as_deref(),
        Some("有 1 个受管图片引用缺少文件；Markdown 未修改。")
    );
    let failure = StorageFailure::new(FailureKind::Unavailable, "asset directory is unavailable");
    let mut store = RecordingStore::answering(Err(failure));
    assert_eq!(
        reconcile_at_startup(&mut store, &document(), false).as_deref(),
        Some("启动图片整理未完成；用户文件与文档均未删除：asset directory is unavailable")
    );
}
