//! The durable TOML boundary of the Windows host. The configuration model is shared
//! (`stickymd_app::config`); this module forwards it, stateless, until the storage capability
//! moves behind a Capability Port (Phase 15 B).
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

mod storage;

pub use stickymd_app::config::*;
pub use storage::{ConfigStorageError, ConfigWarning, load_config, save_config};
