//! Managed-image storage and transaction coordination.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#managed-vs-user-asset

mod path;
mod safe_boundary;
mod storage;
mod transaction;

pub use path::resolve_local_image;
pub use safe_boundary::reconcile_safe_boundary;
#[cfg(test)]
pub use storage::StoredAssetDisposition;
pub use storage::{
    AssetReconcileMode, AssetReconcileReport, AssetStorage, AssetStorageError, StoredAsset,
};
pub use transaction::{
    AssetPasteCompletion, AssetPasteError, AssetPasteFailure, AssetPasteRequest,
    prepare_and_store_paste,
};
