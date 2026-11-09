//! The durable TOML boundary of the macOS host. The configuration model is shared
//! (`stickymd_app::config`); writes reach this storage through the `ConfigStorage` port.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

mod storage;

pub use storage::{ConfigStorageError, load_config, save_config};
