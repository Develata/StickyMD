//! Bounded, deterministic undo/redo history.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#undo-grouping

use std::collections::VecDeque;

use crate::{AssetEffect, EditKind, TextDelta};

pub const MERGE_WINDOW_MS: u64 = 750;
pub const MAX_UNDO_ENTRIES: usize = 256;
pub const MAX_UNDO_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct UndoEntry {
    pub(crate) delta: TextDelta,
    kind: EditKind,
    timestamp_ms: u64,
    pub(crate) asset_effects: Vec<AssetEffect>,
}

impl UndoEntry {
    pub(crate) fn new(
        delta: TextDelta,
        kind: EditKind,
        timestamp_ms: u64,
        asset_effects: Vec<AssetEffect>,
    ) -> Self {
        Self {
            delta,
            kind,
            timestamp_ms,
            asset_effects,
        }
    }

    fn approx_bytes(&self) -> usize {
        self.asset_effects
            .iter()
            .fold(self.delta.approx_bytes(), |total, effect| {
                total.saturating_add(effect.approx_bytes())
            })
    }

    fn try_absorb(&mut self, newer: &Self) -> bool {
        if self.kind != newer.kind
            || !self.kind.is_groupable()
            || newer.timestamp_ms < self.timestamp_ms
            || newer.timestamp_ms - self.timestamp_ms > MERGE_WINDOW_MS
            || self.delta.cursor_after != newer.delta.cursor_before
        {
            return false;
        }

        match self.kind {
            EditKind::Typing => self.absorb_typing(newer),
            EditKind::Backspace => self.absorb_backspace(newer),
            EditKind::DeleteForward => self.absorb_delete_forward(newer),
            _ => false,
        }
    }

    fn absorb_typing(&mut self, newer: &Self) -> bool {
        if !self.delta.deleted.is_empty()
            || !newer.delta.deleted.is_empty()
            || self.delta.inserted.contains('\n')
            || newer.delta.inserted.contains('\n')
            || newer.delta.range.start != self.delta.range.start + self.delta.inserted.len()
            || !self.delta.cursor_before.selection.is_collapsed()
            || !newer.delta.cursor_before.selection.is_collapsed()
        {
            return false;
        }

        let mut inserted =
            String::with_capacity(self.delta.inserted.len() + newer.delta.inserted.len());
        inserted.push_str(&self.delta.inserted);
        inserted.push_str(&newer.delta.inserted);
        self.delta.inserted = inserted.into();
        self.delta.range.end = self.delta.range.start;
        self.delta.cursor_after = newer.delta.cursor_after;
        self.timestamp_ms = newer.timestamp_ms;
        self.asset_effects
            .extend(newer.asset_effects.iter().cloned());
        true
    }

    fn absorb_backspace(&mut self, newer: &Self) -> bool {
        if !self.delta.inserted.is_empty()
            || !newer.delta.inserted.is_empty()
            || newer.delta.range.end != self.delta.range.start
            || !self.delta.cursor_before.selection.is_collapsed()
            || !newer.delta.cursor_before.selection.is_collapsed()
        {
            return false;
        }

        let mut deleted =
            String::with_capacity(newer.delta.deleted.len() + self.delta.deleted.len());
        deleted.push_str(&newer.delta.deleted);
        deleted.push_str(&self.delta.deleted);
        self.delta.deleted = deleted.into();
        self.delta.range.start = newer.delta.range.start;
        self.delta.cursor_after = newer.delta.cursor_after;
        self.timestamp_ms = newer.timestamp_ms;
        self.asset_effects
            .extend(newer.asset_effects.iter().cloned());
        true
    }

    fn absorb_delete_forward(&mut self, newer: &Self) -> bool {
        if !self.delta.inserted.is_empty()
            || !newer.delta.inserted.is_empty()
            || newer.delta.range.start != self.delta.range.start
            || !self.delta.cursor_before.selection.is_collapsed()
            || !newer.delta.cursor_before.selection.is_collapsed()
        {
            return false;
        }

        let mut deleted =
            String::with_capacity(self.delta.deleted.len() + newer.delta.deleted.len());
        deleted.push_str(&self.delta.deleted);
        deleted.push_str(&newer.delta.deleted);
        self.delta.deleted = deleted.into();
        self.delta.range.end += newer.delta.deleted.len();
        self.delta.cursor_after = newer.delta.cursor_after;
        self.timestamp_ms = newer.timestamp_ms;
        self.asset_effects
            .extend(newer.asset_effects.iter().cloned());
        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RecordOutcome {
    pub(crate) recorded: bool,
    pub(crate) grouped: bool,
}

/// Combined undo and redo history. Entries move between stacks without being
/// double-counted; the shared payload budget never exceeds 4 MiB.
#[derive(Debug, Default)]
pub(crate) struct UndoManager {
    undo: VecDeque<UndoEntry>,
    redo: Vec<UndoEntry>,
    history_bytes: usize,
}

impl UndoManager {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn record(&mut self, entry: UndoEntry) -> RecordOutcome {
        self.clear_redo();

        if entry.approx_bytes() > MAX_UNDO_BYTES {
            self.clear();
            return RecordOutcome {
                recorded: false,
                grouped: false,
            };
        }

        if let Some(top) = self.undo.back() {
            let mut merged = top.clone();
            if merged.try_absorb(&entry) {
                let old_bytes = top.approx_bytes();
                let new_bytes = merged.approx_bytes();
                if new_bytes > MAX_UNDO_BYTES {
                    self.clear();
                    return RecordOutcome {
                        recorded: false,
                        grouped: false,
                    };
                }
                if let Some(top) = self.undo.back_mut() {
                    *top = merged;
                }
                self.history_bytes = self.history_bytes - old_bytes + new_bytes;
                self.enforce_bounds();
                return RecordOutcome {
                    recorded: true,
                    grouped: true,
                };
            }
        }

        self.history_bytes += entry.approx_bytes();
        self.undo.push_back(entry);
        self.enforce_bounds();
        RecordOutcome {
            recorded: true,
            grouped: false,
        }
    }

    pub(crate) fn peek_undo(&self) -> Option<&UndoEntry> {
        self.undo.back()
    }

    pub(crate) fn commit_undo(&mut self) {
        if let Some(entry) = self.undo.pop_back() {
            self.redo.push(entry);
        }
    }

    pub(crate) fn peek_redo(&self) -> Option<&UndoEntry> {
        self.redo.last()
    }

    pub(crate) fn commit_redo(&mut self) {
        if let Some(entry) = self.redo.pop() {
            self.undo.push_back(entry);
        }
    }

    pub(crate) fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub(crate) fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    #[cfg(test)]
    pub(crate) fn undo_len(&self) -> usize {
        self.undo.len()
    }

    #[cfg(test)]
    pub(crate) fn redo_len(&self) -> usize {
        self.redo.len()
    }

    #[cfg(test)]
    pub(crate) fn history_bytes(&self) -> usize {
        self.history_bytes
    }

    pub(crate) fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.history_bytes = 0;
    }

    fn clear_redo(&mut self) {
        for entry in self.redo.drain(..) {
            self.history_bytes -= entry.approx_bytes();
        }
    }

    fn enforce_bounds(&mut self) {
        while self.undo.len() > MAX_UNDO_ENTRIES || self.history_bytes > MAX_UNDO_BYTES {
            let Some(oldest) = self.undo.pop_front() else {
                break;
            };
            self.history_bytes -= oldest.approx_bytes();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::{CursorSnapshot, Selection, TextDelta};

    fn delta(
        range: std::ops::Range<usize>,
        deleted: &str,
        inserted: &str,
        before: usize,
        after: usize,
    ) -> TextDelta {
        TextDelta::new(
            range,
            Arc::from(deleted),
            Arc::from(inserted),
            CursorSnapshot::caret(before),
            CursorSnapshot::caret(after),
        )
    }

    #[test]
    fn typing_within_window_groups_and_preserves_initial_cursor() {
        let mut history = UndoManager::new();
        for (index, ch) in ["a", "b", "c"].into_iter().enumerate() {
            history.record(UndoEntry::new(
                delta(index..index, "", ch, index, index + 1),
                EditKind::Typing,
                index as u64 * 100,
                Vec::new(),
            ));
        }
        let entry = history.peek_undo().unwrap();
        assert_eq!(entry.delta.inserted(), "abc");
        assert_eq!(entry.delta.cursor_before(), CursorSnapshot::caret(0));
        assert_eq!(entry.delta.cursor_after(), CursorSnapshot::caret(3));
    }

    #[test]
    fn backspace_group_preserves_original_cursor_and_deleted_order() {
        let mut history = UndoManager::new();
        for (range, ch, before, after, time) in [
            (3..4, "d", 4, 3, 0),
            (2..3, "c", 3, 2, 100),
            (1..2, "b", 2, 1, 200),
        ] {
            history.record(UndoEntry::new(
                delta(range, ch, "", before, after),
                EditKind::Backspace,
                time,
                Vec::new(),
            ));
        }
        let entry = history.peek_undo().unwrap();
        assert_eq!(entry.delta.deleted(), "bcd");
        assert_eq!(entry.delta.range(), 1..4);
        assert_eq!(entry.delta.cursor_before(), CursorSnapshot::caret(4));
        assert_eq!(entry.delta.cursor_after(), CursorSnapshot::caret(1));
    }

    #[test]
    fn delete_forward_group_preserves_deleted_order() {
        let mut history = UndoManager::new();
        for (ch, end, time) in [("b", 2, 0), ("c", 2, 100), ("d", 2, 200)] {
            history.record(UndoEntry::new(
                delta(1..end, ch, "", 1, 1),
                EditKind::DeleteForward,
                time,
                Vec::new(),
            ));
        }
        let entry = history.peek_undo().unwrap();
        assert_eq!(entry.delta.deleted(), "bcd");
        assert_eq!(entry.delta.range(), 1..4);
    }

    #[test]
    fn reverse_selection_never_groups_as_plain_typing() {
        let mut history = UndoManager::new();
        let first = TextDelta::new(
            0..2,
            Arc::from("ab"),
            Arc::from("x"),
            CursorSnapshot::new(Selection::new(2, 0)),
            CursorSnapshot::caret(1),
        );
        history.record(UndoEntry::new(first, EditKind::Typing, 0, Vec::new()));
        history.record(UndoEntry::new(
            delta(1..1, "", "y", 1, 2),
            EditKind::Typing,
            100,
            Vec::new(),
        ));
        assert_eq!(history.undo_len(), 2);
    }

    #[test]
    fn oversized_entry_clears_history_and_is_not_recorded() {
        let mut history = UndoManager::new();
        history.record(UndoEntry::new(
            delta(0..0, "", "a", 0, 1),
            EditKind::Paste,
            0,
            Vec::new(),
        ));
        let huge = "x".repeat(MAX_UNDO_BYTES);
        let result = history.record(UndoEntry::new(
            delta(0..0, "", &huge, 0, huge.len()),
            EditKind::Paste,
            1,
            Vec::new(),
        ));
        assert!(!result.recorded);
        assert_eq!(history.undo_len(), 0);
        assert_eq!(history.redo_len(), 0);
        assert_eq!(history.history_bytes(), 0);
    }

    #[test]
    fn combined_undo_redo_payload_is_counted_once() {
        let mut history = UndoManager::new();
        history.record(UndoEntry::new(
            delta(0..0, "", "abc", 0, 3),
            EditKind::Paste,
            0,
            Vec::new(),
        ));
        let bytes = history.history_bytes();
        history.commit_undo();
        assert_eq!(history.history_bytes(), bytes);
        history.commit_redo();
        assert_eq!(history.history_bytes(), bytes);
    }
}
