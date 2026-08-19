//! Bounded undo/redo with input grouping.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#undo-分组
//!
//! Rules implemented here:
//! - In-memory only; restart clears it; never written to disk.
//! - At most 256 entries or 4 MiB of undo text, whichever comes first; the oldest
//!   entry is evicted beyond the limit.
//! - Consecutive same-kind, adjacent edits within 750 ms merge into one entry.
//! - IME commit, paste, image paste, newline and selection replacement are always
//!   independent entries.
//!
//! Entries are stored differentially (`removed` / `inserted`) rather than as full
//! snapshots so the 4 MiB budget holds on large documents.

use std::collections::VecDeque;

use crate::cursor::CursorSnapshot;
use crate::text_delta::{InputKind, TextDelta};

/// Merge window for grouping consecutive edits (milliseconds).
pub const MERGE_WINDOW_MS: u64 = 750;
/// Maximum number of undo entries.
pub const MAX_UNDO_ENTRIES: usize = 256;
/// Maximum total bytes of undo text (removed + inserted across entries).
pub const MAX_UNDO_BYTES: usize = 4 * 1024 * 1024;

/// A single (possibly merged) undoable edit.
///
/// Applying this entry produced: replace `[start, start + removed.len())` in the
/// *pre-edit* text with `inserted`. Undoing reverses it; redoing re-applies it.
#[derive(Debug, Clone)]
pub struct UndoEntry {
    /// Byte offset where the edit begins.
    pub start: usize,
    /// Text that was removed by the edit.
    pub removed: String,
    /// Text that was inserted by the edit.
    pub inserted: String,
    /// The input kind that produced the edit.
    pub kind: InputKind,
    /// Caret/selection before the edit.
    pub cursor_before: CursorSnapshot,
    /// Caret/selection after the edit.
    pub cursor_after: CursorSnapshot,
    /// Monotonic edit timestamp (ms) used for the merge window.
    pub time_ms: u64,
}

impl UndoEntry {
    /// Build an entry from an applied delta plus the text it removed.
    pub fn from_delta(
        delta: &TextDelta,
        removed: impl Into<String>,
        kind: InputKind,
        cursor_before: CursorSnapshot,
        cursor_after: CursorSnapshot,
        time_ms: u64,
    ) -> Self {
        Self {
            start: delta.range.start,
            removed: removed.into(),
            inserted: delta.replacement.clone(),
            kind,
            cursor_before,
            cursor_after,
            time_ms,
        }
    }

    /// The delta that undoes this entry, valid against the *post-edit* text.
    pub fn inverse_delta(&self) -> TextDelta {
        TextDelta::new(
            self.start..self.start + self.inserted.len(),
            self.removed.clone(),
        )
    }

    /// The delta that re-applies this entry, valid against the *pre-edit* text.
    pub fn forward_delta(&self) -> TextDelta {
        TextDelta::new(
            self.start..self.start + self.removed.len(),
            self.inserted.clone(),
        )
    }

    /// Byte cost of this entry for the 4 MiB budget.
    pub fn byte_size(&self) -> usize {
        self.removed.len() + self.inserted.len()
    }

    /// Try to fold a newer, same-kind adjacent edit into `self`. Returns true on
    /// success, leaving `self` representing the combined edit.
    fn try_absorb(&mut self, newer: &UndoEntry) -> bool {
        if self.kind != newer.kind {
            return false;
        }
        match self.kind {
            InputKind::Typing => {
                if self.removed.is_empty()
                    && newer.removed.is_empty()
                    && newer.start == self.start + self.inserted.len()
                {
                    self.inserted.push_str(&newer.inserted);
                    self.cursor_after = newer.cursor_after.clone();
                    self.time_ms = newer.time_ms;
                    return true;
                }
            }
            InputKind::Backspace => {
                if self.inserted.is_empty()
                    && newer.inserted.is_empty()
                    && newer.start + newer.removed.len() == self.start
                {
                    let mut removed = newer.removed.clone();
                    removed.push_str(&self.removed);
                    self.removed = removed;
                    self.start = newer.start;
                    self.cursor_before = newer.cursor_before.clone();
                    self.time_ms = newer.time_ms;
                    return true;
                }
            }
            InputKind::Delete
                if self.inserted.is_empty()
                    && newer.inserted.is_empty()
                    && newer.start == self.start =>
            {
                self.removed.push_str(&newer.removed);
                self.cursor_after = newer.cursor_after.clone();
                self.time_ms = newer.time_ms;
                return true;
            }
            _ => {}
        }
        false
    }
}

/// Bounded undo/redo stack with grouping.
#[derive(Debug, Default)]
pub struct UndoManager {
    undo: VecDeque<UndoEntry>,
    redo: Vec<UndoEntry>,
    undo_bytes: usize,
}

impl UndoManager {
    /// An empty undo manager.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a newly applied edit, merging it into the top entry when allowed.
    /// Any pending redo history is discarded (a new edit invalidates it).
    pub fn record(&mut self, entry: UndoEntry) {
        self.redo.clear();
        let mergeable = entry.kind.is_mergeable();
        let in_window = self
            .undo
            .back()
            .is_some_and(|top| entry.time_ms.saturating_sub(top.time_ms) < MERGE_WINDOW_MS);

        if mergeable
            && in_window
            && let Some(top) = self.undo.back_mut()
            && top.try_absorb(&entry)
        {
            self.undo_bytes += entry.byte_size();
            self.enforce_bounds();
            return;
        }
        self.undo_bytes += entry.byte_size();
        self.undo.push_back(entry);
        self.enforce_bounds();
    }

    /// Pop the most recent edit to undo, if any.
    pub fn pop_undo(&mut self) -> Option<UndoEntry> {
        let entry = self.undo.pop_back()?;
        self.undo_bytes -= entry.byte_size();
        self.redo.push(entry.clone());
        Some(entry)
    }

    /// Pop the most recent undone edit to redo, if any.
    pub fn pop_redo(&mut self) -> Option<UndoEntry> {
        let entry = self.redo.pop()?;
        self.undo_bytes += entry.byte_size();
        self.undo.push_back(entry.clone());
        Some(entry)
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo_len(&self) -> usize {
        self.undo.len()
    }

    pub fn redo_len(&self) -> usize {
        self.redo.len()
    }

    pub fn undo_bytes(&self) -> usize {
        self.undo_bytes
    }

    /// Clear all history (external reload / recovery).
    pub fn clear(&mut self) {
        self.undo.clear();
        self.redo.clear();
        self.undo_bytes = 0;
    }

    fn enforce_bounds(&mut self) {
        // Never evict below one entry, so the newest edit keeps its undo even if a
        // single oversized edit exceeds the byte budget on its own.
        while self.undo.len() > 1
            && (self.undo.len() > MAX_UNDO_ENTRIES || self.undo_bytes > MAX_UNDO_BYTES)
        {
            let Some(oldest) = self.undo.pop_front() else {
                break;
            };
            self.undo_bytes -= oldest.byte_size();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generation::Generation;

    fn caret(offset: usize) -> CursorSnapshot {
        CursorSnapshot::caret(offset, Generation::initial())
    }

    fn typing_insert(start: usize, s: &str, t: u64) -> UndoEntry {
        UndoEntry {
            start,
            removed: String::new(),
            inserted: s.to_string(),
            kind: InputKind::Typing,
            cursor_before: caret(start),
            cursor_after: caret(start + s.len()),
            time_ms: t,
        }
    }

    #[test]
    fn consecutive_typing_merges() {
        let mut m = UndoManager::new();
        m.record(typing_insert(0, "a", 0));
        m.record(typing_insert(1, "b", 100));
        m.record(typing_insert(2, "c", 200));
        assert_eq!(m.undo_len(), 1);
        let top = m.undo.back().unwrap();
        assert_eq!(top.inserted, "abc");
        assert_eq!(top.start, 0);
    }

    #[test]
    fn typing_beyond_window_is_separate() {
        let mut m = UndoManager::new();
        m.record(typing_insert(0, "a", 0));
        m.record(typing_insert(1, "b", MERGE_WINDOW_MS + 1));
        assert_eq!(m.undo_len(), 2);
    }

    #[test]
    fn non_adjacent_typing_is_separate() {
        let mut m = UndoManager::new();
        m.record(typing_insert(0, "a", 0));
        m.record(typing_insert(5, "b", 10));
        assert_eq!(m.undo_len(), 2);
    }

    #[test]
    fn ime_commit_never_merges() {
        let mut m = UndoManager::new();
        m.record(typing_insert(0, "a", 0));
        let mut commit = typing_insert(1, "你", 10);
        commit.kind = InputKind::ImeCommit;
        m.record(commit);
        assert_eq!(m.undo_len(), 2);
    }

    #[test]
    fn backspace_run_merges_leftward() {
        let mut m = UndoManager::new();
        // delete 'c' at [2,3), then 'b' at [1,2), then 'a' at [0,1)
        let mk = |start: usize, ch: &str, t: u64| UndoEntry {
            start,
            removed: ch.to_string(),
            inserted: String::new(),
            kind: InputKind::Backspace,
            cursor_before: caret(start + ch.len()),
            cursor_after: caret(start),
            time_ms: t,
        };
        m.record(mk(2, "c", 0));
        m.record(mk(1, "b", 10));
        m.record(mk(0, "a", 20));
        assert_eq!(m.undo_len(), 1);
        let top = m.undo.back().unwrap();
        assert_eq!(top.removed, "abc");
        assert_eq!(top.start, 0);
    }

    #[test]
    fn entry_limit_evicts_oldest() {
        let mut m = UndoManager::new();
        for i in 0..(MAX_UNDO_ENTRIES + 10) {
            let mut e = typing_insert(0, "x", (i as u64) * 10_000); // far apart -> no merge
            e.kind = InputKind::Paste;
            m.record(e);
        }
        assert_eq!(m.undo_len(), MAX_UNDO_ENTRIES);
    }

    #[test]
    fn byte_limit_evicts_oldest() {
        let mut m = UndoManager::new();
        let chunk = "x".repeat(1024 * 1024); // 1 MiB each
        for i in 0..6 {
            let mut e = typing_insert(0, &chunk, (i as u64) * 10_000);
            e.kind = InputKind::Paste;
            m.record(e);
        }
        assert!(m.undo_bytes() <= MAX_UNDO_BYTES);
        assert!(m.undo_len() <= 4);
    }

    #[test]
    fn new_edit_clears_redo() {
        let mut m = UndoManager::new();
        m.record(typing_insert(0, "a", 0));
        m.pop_undo();
        assert!(m.can_redo());
        m.record(typing_insert(0, "z", 5000));
        assert!(!m.can_redo());
    }

    #[test]
    fn clear_resets_all() {
        let mut m = UndoManager::new();
        m.record(typing_insert(0, "a", 0));
        m.clear();
        assert!(!m.can_undo());
        assert!(!m.can_redo());
        assert_eq!(m.undo_bytes(), 0);
    }
}
