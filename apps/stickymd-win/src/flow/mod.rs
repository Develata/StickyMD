//! Flow coordination for editor intents and external capabilities.
//!
//! plan_ref: docs/plan/03_system_architecture.md#flow-coordination

mod clipboard;
mod editor;
mod persistence;
mod reconciliation;
mod recovery;
mod save;

pub use clipboard::{ClipboardError, ClipboardPort};
pub use editor::{AppEffect, EditorCoordinator};
pub use persistence::{PersistenceCoordinator, QuitAction, ReconciliationAction};
pub use reconciliation::{ExternalDecision, decide_external_change};
pub use recovery::{CanonicalRecoveryPlan, RecoveryCoordinator, RecoveryOperation};
pub use save::{AutosaveAction, AutosaveScheduler, SaveTrigger};
