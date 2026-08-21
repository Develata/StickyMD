//! Typed edit requests, canonical deltas, metadata, and outcomes.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#documentstate

use std::ops::Range;
use std::sync::Arc;

use crate::{AssetEffect, CursorSnapshot, Generation};

/// Input class used only for deterministic undo grouping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EditKind {
    Typing,
    Backspace,
    DeleteForward,
    Paste,
    ImeCommit,
    Newline,
    SelectionReplace,
    Other,
}

impl EditKind {
    pub(crate) const fn is_groupable(self) -> bool {
        matches!(self, Self::Typing | Self::Backspace | Self::DeleteForward)
    }
}

/// Deterministic grouping metadata supplied by the coordination layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditMeta {
    pub kind: EditKind,
    pub timestamp_ms: u64,
}

impl EditMeta {
    pub const fn new(kind: EditKind, timestamp_ms: u64) -> Self {
        Self { kind, timestamp_ms }
    }
}

/// Request to replace a canonical UTF-8 byte range.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditRequest {
    pub expected_generation: Generation,
    pub range: Range<usize>,
    pub inserted: String,
    pub cursor_before: CursorSnapshot,
    pub cursor_after: CursorSnapshot,
    pub meta: EditMeta,
}

impl EditRequest {
    pub fn new(
        expected_generation: Generation,
        range: Range<usize>,
        inserted: impl Into<String>,
        cursor_before: CursorSnapshot,
        cursor_after: CursorSnapshot,
        meta: EditMeta,
    ) -> Self {
        Self {
            expected_generation,
            range,
            inserted: inserted.into(),
            cursor_before,
            cursor_after,
            meta,
        }
    }
}

/// Immutable description of an edit that actually changed canonical text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextDelta {
    pub(crate) range: Range<usize>,
    pub(crate) deleted: Arc<str>,
    pub(crate) inserted: Arc<str>,
    pub(crate) cursor_before: CursorSnapshot,
    pub(crate) cursor_after: CursorSnapshot,
}

impl TextDelta {
    pub(crate) fn new(
        range: Range<usize>,
        deleted: Arc<str>,
        inserted: Arc<str>,
        cursor_before: CursorSnapshot,
        cursor_after: CursorSnapshot,
    ) -> Self {
        Self {
            range,
            deleted,
            inserted,
            cursor_before,
            cursor_after,
        }
    }

    pub fn range(&self) -> Range<usize> {
        self.range.clone()
    }

    pub fn deleted(&self) -> &str {
        &self.deleted
    }

    pub fn inserted(&self) -> &str {
        &self.inserted
    }

    pub fn cursor_before(&self) -> CursorSnapshot {
        self.cursor_before
    }

    pub fn cursor_after(&self) -> CursorSnapshot {
        self.cursor_after
    }

    pub(crate) fn inverse(&self) -> Self {
        Self {
            range: self.range.start..self.range.start + self.inserted.len(),
            deleted: Arc::clone(&self.inserted),
            inserted: Arc::clone(&self.deleted),
            cursor_before: self.cursor_after,
            cursor_after: self.cursor_before,
        }
    }

    pub(crate) fn approx_bytes(&self) -> usize {
        self.deleted
            .len()
            .saturating_add(self.inserted.len())
            .saturating_add(128)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditOutcome {
    pub generation: Generation,
    pub dirty: bool,
    pub undo_recorded: bool,
    pub grouped: bool,
    pub delta: Option<TextDelta>,
    pub asset_effects: Vec<AssetEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndoOutcome {
    pub generation: Generation,
    pub cursor: CursorSnapshot,
    pub delta: TextDelta,
    pub asset_effects: Vec<AssetEffect>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RedoOutcome {
    pub generation: Generation,
    pub cursor: CursorSnapshot,
    pub delta: TextDelta,
    pub asset_effects: Vec<AssetEffect>,
}
