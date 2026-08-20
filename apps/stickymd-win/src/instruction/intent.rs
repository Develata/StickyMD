//! Minimal Phase 3 editor intents.
//!
//! plan_ref: docs/plan/03_system_architecture.md#instruction-interface

use stickymd_core::{EditKind, Generation, Selection};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppIntent {
    Edit {
        expected_generation: Generation,
        selection: Selection,
        inserted: String,
        kind: EditKind,
        timestamp_ms: u64,
    },
    Undo,
    Redo,
    CopySelection {
        expected_generation: Generation,
        selection: Selection,
    },
    CutSelection {
        expected_generation: Generation,
        selection: Selection,
        timestamp_ms: u64,
    },
    PasteText {
        expected_generation: Generation,
        selection: Selection,
        timestamp_ms: u64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaveReason {
    Manual,
    FocusLoss,
}

/// Non-editor instructions emitted by the shell. Persistence and lifecycle
/// coordination consume these before any execution-domain request is made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistenceIntent {
    SaveNow(SaveReason),
    ResolvePrimary,
    ResolveSecondary,
    RequestQuit,
}
