//! Runtime configuration DTO and durable TOML boundary.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

mod runtime;

pub use runtime::{ConfigStorageError, ConfigWarning, RuntimeConfig, load_config, save_config};
