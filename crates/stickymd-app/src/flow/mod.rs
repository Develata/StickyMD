//! Flow coordination for editor intents and external capabilities.
//!
//! plan_ref: docs/plan/03_system_architecture.md#flow-coordination

mod asset_io;
mod asset_paste;
mod clipboard;
mod editor;
mod export;
mod export_plan;
mod note_guard;
mod note_io;
mod note_startup;
mod persistence;
mod preferences;
mod preview;
mod preview_io;
mod reconciliation;
mod recovery;
mod save;
pub mod startup;
pub mod window;

pub use asset_io::{AssetFlow, AssetIo, AssetStep, reconcile_at_startup};
pub use asset_paste::{
    AssetPasteError, MAX_IMAGE_PASTE_BATCH_BYTES, PasteFailure, PasteTarget, add_batch_bytes,
    prepare_and_store_paste,
};
pub use clipboard::{ClipboardError, ClipboardPaste, ClipboardPort, PendingAssetPaste};
pub use editor::{AppEffect, EditorCoordinator, EditorFlowError};
pub use export::{
    CopyFailure, EXPORT_ASSET_DIRECTORY_LIMIT, ExportError, copy_hashing,
    export_asset_directory_leaf, export_staging_name, export_temporary_name,
};
pub use export_plan::{CopyPlacement, ExportNameCollision, ExportPlan, export_asset_extension};
pub use note_guard::{PublishVerdict, guarded_publish_verdict};
pub use note_io::{NoteEffect, NoteEffects, NoteIo};
pub use note_startup::{
    NoteDocumentPlan, NoteObservation, NoteRefusal, NoteStartupPlan, NoteStartupStep,
    OversizedTemporary, document_from_fact, invalid_temporary_warning, oversized_temporary_warning,
    plan_note_startup,
};
pub use persistence::{PersistenceCoordinator, ReconciliationAction};
pub use preferences::{WindowPreferenceEffect, coordinate_window_preference};
pub use preview::{
    PreviewAction, PreviewAdmission, PreviewCoordinator, PreviewEffect, PreviewFlowError,
    PreviewVisibility,
};
pub use preview_io::{
    PreviewFacts, PreviewFollowUp, PreviewIo, current_preview_layout, selection_for_generation,
};
pub use reconciliation::{ExternalDecision, decide_external_change, names_the_working_note};
pub use recovery::{CanonicalRecoveryPlan, RecoveryCoordinator, RecoveryOperation};
pub use save::{AutosaveAction, AutosaveScheduler, SaveTrigger};
