//! The configuration storage port.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

use super::StorageFailure;
use crate::config::RuntimeConfig;

pub trait ConfigStorage: Send + 'static {
    /// Atomically replaces the durable configuration.
    fn save(&mut self, config: &RuntimeConfig) -> Result<(), StorageFailure>;
}
