//! Runtime configuration model and coordinator. The durable TOML file is a host capability.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

mod coordinator;
mod runtime;

pub use coordinator::{
    ConfigAck, ConfigCoordinator, ConfigPersistRequest, ConfigRevision, ConfigRevisionExhausted,
};

pub use runtime::{
    CONFIG_VERSION, ContentZoomPercent, DockEdge, MIN_WINDOW_HEIGHT_DIP, MIN_WINDOW_WIDTH_DIP,
    RuntimeConfig, ThemeMode, ViewMode, WindowConfig,
};
