//! Flow coordination for editor intents and external capabilities.
//!
//! plan_ref: docs/plan/03_system_architecture.md#flow-coordination

mod clipboard;
mod editor;

pub use clipboard::{ClipboardError, ClipboardPort};
pub use editor::{AppEffect, EditorCoordinator};
