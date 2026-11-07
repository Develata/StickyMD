//! Managed-image storage and transaction coordination.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#managed-vs-user-asset

mod path;
mod safe_boundary;
mod storage;
mod transaction;

pub(crate) use crate::platform::windows::local_image_file::open as open_local_image;
pub use path::resolve_local_image;
pub use safe_boundary::reconcile_safe_boundary;
pub use storage::{AssetStorage, AssetStorageError};
pub use transaction::{AssetPasteError, prepare_and_store_paste};
