//! Typed instruction boundary between the interaction shell and coordination.
//!
//! plan_ref: docs/plan/03_system_architecture.md#instruction-interface

mod intent;

pub use intent::{
    AppIntent, LiteralSearchOptions, PersistenceIntent, PreviewIntent, SaveReason,
    WindowPlatformIntent, WindowPreferenceIntent, WindowResizeEdge,
};
