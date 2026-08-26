//! Runtime configuration DTO and durable TOML boundary.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

mod coordinator;
mod runtime;
mod storage;

pub use coordinator::{
    ConfigAck, ConfigCoordinator, ConfigPersistRequest, ConfigRevision, ConfigRevisionExhausted,
};

pub use runtime::{
    ContentZoomPercent, DockEdge, MIN_WINDOW_HEIGHT_DIP, MIN_WINDOW_WIDTH_DIP, RuntimeConfig,
    ThemeMode, ViewMode,
};
pub use storage::{ConfigStorageError, ConfigWarning, load_config, save_config};
