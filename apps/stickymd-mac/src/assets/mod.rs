//! Managed-image storage and transaction coordination.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#macos-clipboard-and-assets

mod path;
mod storage;
mod transaction;

pub use path::local_image_destination;
#[cfg(test)]
pub use path::resolve_local_image;
pub use storage::{AssetStorage, AssetStorageError};
pub use transaction::{AssetPasteError, prepare_and_store_paste};
