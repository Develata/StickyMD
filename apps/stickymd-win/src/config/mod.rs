//! Runtime configuration DTO and durable TOML boundary.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

mod runtime;

pub use runtime::{
    ConfigStorageError, ConfigWarning, RuntimeConfig, ThemeMode, ViewMode, load_config, save_config,
};
