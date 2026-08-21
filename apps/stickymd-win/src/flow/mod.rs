//! Flow coordination for editor intents and external capabilities.
//!
//! plan_ref: docs/plan/03_system_architecture.md#flow-coordination

mod clipboard;
mod editor;
mod persistence;
mod preferences;
mod preview;
mod reconciliation;
mod recovery;
mod save;
pub mod window;

pub use clipboard::{ClipboardError, ClipboardPaste, ClipboardPort, PendingAssetPaste};
pub use editor::{AppEffect, EditorCoordinator};
pub use persistence::{PersistenceCoordinator, ReconciliationAction};
pub use preferences::{WindowPreferenceEffect, coordinate_window_preference};
pub use preview::{
    PreviewAction, PreviewAdmission, PreviewCoordinator, PreviewEffect, PreviewVisibility,
};
pub use reconciliation::{ExternalDecision, decide_external_change};
pub use recovery::{CanonicalRecoveryPlan, RecoveryCoordinator, RecoveryOperation};
pub use save::{AutosaveAction, AutosaveScheduler, SaveTrigger};
