//! Flow coordination for editor intents and external capabilities.
//!
//! plan_ref: docs/plan/03_system_architecture.md#flow-coordination

mod asset_io;
mod clipboard;
mod editor;
mod note_guard;
mod note_io;
mod persistence;
mod preferences;
mod preview;
mod preview_io;
mod reconciliation;
mod recovery;
mod save;
pub mod window;

pub use asset_io::{AssetFlow, AssetIo, AssetStep, reconcile_at_startup};
pub use clipboard::{ClipboardError, ClipboardPaste, ClipboardPort, PendingAssetPaste};
pub use editor::{AppEffect, EditorCoordinator, EditorFlowError};
pub use note_guard::{PublishVerdict, guarded_publish_verdict};
pub use note_io::{NoteEffect, NoteEffects, NoteIo};
pub use persistence::{PersistenceCoordinator, ReconciliationAction};
pub use preferences::{WindowPreferenceEffect, coordinate_window_preference};
pub use preview::{
    PreviewAction, PreviewAdmission, PreviewCoordinator, PreviewEffect, PreviewFlowError,
    PreviewVisibility,
};
pub use preview_io::{
    PreviewFacts, PreviewFollowUp, PreviewIo, current_preview_layout, selection_for_generation,
};
pub use reconciliation::{ExternalDecision, decide_external_change};
pub use recovery::{CanonicalRecoveryPlan, RecoveryCoordinator, RecoveryOperation};
pub use save::{AutosaveAction, AutosaveScheduler, SaveTrigger};
