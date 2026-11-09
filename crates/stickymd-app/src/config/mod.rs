//! Runtime configuration model, coordinator and the rules of its text. Reading and publishing the
//! durable TOML file is a host capability.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

mod coordinator;
mod document;
mod runtime;

pub use coordinator::{
    ConfigAck, ConfigCoordinator, ConfigPersistRequest, ConfigRevision, ConfigRevisionExhausted,
};

pub use document::{
    ConfigLoadOutcome, ConfigSerializeError, ConfigWarning, PRESERVED_CONFIG_SUFFIXES, config_text,
    preserved_config_name,
};

pub use runtime::{
    CONFIG_VERSION, ContentZoomPercent, DockEdge, MAX_WINDOW_DIP, MIN_WINDOW_HEIGHT_DIP,
    MIN_WINDOW_WIDTH_DIP, RuntimeConfig, ThemeMode, ViewMode, WindowConfig,
};
