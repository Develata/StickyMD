//! Typed editor, persistence and read-only Preview instructions.
//!
//! plan_ref: docs/plan/03_system_architecture.md#instruction-interface

use stickymd_core::{EditKind, Generation, Selection};
use stickymd_render::preview::SpanAction;

use crate::config::ViewMode;

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
    PasteClipboard {
        expected_generation: Generation,
        selection: Selection,
        timestamp_ms: u64,
    },
    /// Clipboard-only projection effect. It never reads or mutates canonical
    /// document text and is used by read-only Preview selection.
    WriteClipboard {
        text: String,
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
    Export,
    ResolvePrimary,
    ResolveSecondary,
    RequestQuit,
}

/// Read-only Preview instructions emitted by the interaction shell.
///
/// Preview instructions never mutate canonical Markdown text. Link targets
/// still pass through Flow Coordination before the Windows adapter is called.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewIntent {
    SetViewMode(ViewMode),
    Activate(SpanAction),
}
