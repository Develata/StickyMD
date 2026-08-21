//! Runtime configuration DTO and durable TOML boundary.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

mod coordinator;
mod runtime;

pub use coordinator::{
    ConfigAck, ConfigCoordinator, ConfigPersistRequest, ConfigRevision, ConfigRevisionExhausted,
};

pub use runtime::{
    ConfigStorageError, ConfigWarning, DockEdge, RuntimeConfig, ThemeMode, ViewMode, load_config,
    save_config,
};
