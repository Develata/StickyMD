//! The runtime document authority.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#documentstate
//!
//! `DocumentState` is the **sole runtime authority** for document content. The disk
//! `note.md` is only its durable projection; external disk changes are External File
//! Facts that may enter *only* through [`DocumentState::load_external`] (the
//! reconciliation gate). UI, Preview and background tasks only ever receive immutable
//! [`DocumentSnapshot`]s — never a mutable reference (core invariant #9).

use std::sync::Arc;

use crate::cursor::CursorSnapshot;
use crate::error::{EditError, PersistAckError};
use crate::generation::Generation;
use crate::hash::Hash32;
use crate::line_ending::LineEnding;
use crate::text_delta::{InputKind, TextDelta};
use crate::text_store::{StringTextStore, TextStore};
use crate::undo::{UndoEntry, UndoManager};

/// Read-only snapshot of the document at a given generation.
///
/// plan_ref: docs/plan/04_runtime_state_model.md#previewstate
///
/// Carries the generation it was derived from; consumers must drop a result whose
/// generation no longer matches the current document (invariant #4). Snapshots are
/// immutable and never written back.
#[derive(Debug, Clone)]
pub struct DocumentSnapshot {
    /// The document text at snapshot time.
    pub text: Arc<str>,
    /// The generation this snapshot was derived from.
    pub generation: Generation,
}

/// The runtime authoritative document state.
#[derive(Debug)]
pub struct DocumentState {
    store: StringTextStore,
    generation: Generation,
    saved_generation: Generation,
    base_disk_hash: Option<Hash32>,
    line_ending: LineEnding,
    undo: UndoManager,
}

impl DocumentState {
    /// Create a document from already-loaded disk content.
    ///
    /// `text` is normalized to the internal `\n` form. A freshly loaded document is
    /// clean: its current generation equals its saved generation.
    pub fn loaded(text: &str, line_ending: LineEnding, disk_hash: Option<Hash32>) -> Self {
        Self {
            store: StringTextStore::new(LineEnding::to_internal(text)),
            generation: Generation::initial(),
            saved_generation: Generation::initial(),
            base_disk_hash: disk_hash,
            line_ending,
            undo: UndoManager::new(),
        }
    }

    /// Create an empty document with a chosen line ending.
    pub fn empty(line_ending: LineEnding) -> Self {
        Self::loaded("", line_ending, None)
    }

    // ---- read accessors ----

    pub fn text(&self) -> &str {
        self.store.as_str()
    }

    pub fn generation(&self) -> Generation {
        self.generation
    }

    pub fn saved_generation(&self) -> Generation {
        self.saved_generation
    }

    pub fn line_ending(&self) -> LineEnding {
        self.line_ending
    }

    pub fn base_disk_hash(&self) -> Option<Hash32> {
        self.base_disk_hash
    }

    /// Dirty when the current generation differs from the saved generation.
    pub fn is_dirty(&self) -> bool {
        self.generation != self.saved_generation
    }

    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }

    /// Produce an immutable snapshot for background tasks / preview.
    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            text: Arc::from(self.store.as_str()),
            generation: self.generation,
        }
    }

    /// The document text converted to its on-disk line ending style.
    pub fn text_for_disk(&self) -> String {
        self.line_ending.apply(self.store.as_str())
    }

    // ---- mutation ----

    /// Apply a text delta as a single edit.
    ///
    /// Validates char boundaries, records an undo entry (with grouping), bumps the
    /// generation and marks the document dirty. On any validation failure the store
    /// is left untouched. Returns the new generation.
    pub fn apply_delta(
        &mut self,
        delta: &TextDelta,
        kind: InputKind,
        now_ms: u64,
        cursor_before: CursorSnapshot,
        cursor_after: CursorSnapshot,
    ) -> Result<Generation, EditError> {
        let removed = delta.validate(self.store.as_str())?.to_string();
        if !cursor_before.is_valid_for(self.store.as_str()) {
            return Err(EditError::OutOfBounds);
        }
        self.store.apply(delta)?;
        if !cursor_after.is_valid_for(self.store.as_str()) {
            // Roll back to keep the store consistent with a rejected edit.
            let inverse = TextDelta::new(
                delta.range.start..delta.range.start + delta.replacement.len(),
                removed.clone(),
            );
            let _ = self.store.apply(&inverse);
            return Err(EditError::OutOfBounds);
        }
        let entry =
            UndoEntry::from_delta(delta, removed, kind, cursor_before, cursor_after, now_ms);
        self.undo.record(entry);
        self.bump();
        Ok(self.generation)
    }

    /// Undo the most recent edit group. Returns the caret to restore, if any.
    pub fn undo(&mut self) -> Result<Option<CursorSnapshot>, EditError> {
        let Some(entry) = self.undo.pop_undo() else {
            return Ok(None);
        };
        let inverse = entry.inverse_delta();
        inverse.validate(self.store.as_str())?;
        self.store.apply(&inverse)?;
        self.bump();
        Ok(Some(entry.cursor_before))
    }

    /// Redo the most recently undone edit group. Returns the caret to restore.
    pub fn redo(&mut self) -> Result<Option<CursorSnapshot>, EditError> {
        let Some(entry) = self.undo.pop_redo() else {
            return Ok(None);
        };
        let forward = entry.forward_delta();
        forward.validate(self.store.as_str())?;
        self.store.apply(&forward)?;
        self.bump();
        Ok(Some(entry.cursor_after))
    }

    // ---- persistence / reconciliation ----

    /// Acknowledge that `persisted` was atomically written to disk.
    ///
    /// Enforces invariant #7: `saved_generation` only advances to a generation that
    /// actually exists and was persisted. A stale ack (≤ saved) is a harmless no-op;
    /// an ack ahead of the current generation is rejected.
    pub fn acknowledge_persisted(
        &mut self,
        persisted: Generation,
        hash: Hash32,
    ) -> Result<(), PersistAckError> {
        if persisted > self.generation {
            return Err(PersistAckError::AheadOfDocument);
        }
        if persisted > self.saved_generation {
            self.saved_generation = persisted;
            self.base_disk_hash = Some(hash);
        }
        Ok(())
    }

    /// Reconciliation gate: load externally observed content into the authority.
    ///
    /// This is the *only* path by which an External File Fact enters DocumentState
    /// (invariant #2). It replaces the text, bumps the generation, clears undo (an
    /// external reload is not undoable) and marks the document clean at the new
    /// generation with the given disk hash.
    pub fn load_external(&mut self, text: &str, line_ending: LineEnding, hash: Hash32) {
        self.store.replace_all(LineEnding::to_internal(text));
        self.line_ending = line_ending;
        self.undo.clear();
        self.generation = self.generation.next();
        self.saved_generation = self.generation;
        self.base_disk_hash = Some(hash);
    }

    fn bump(&mut self) {
        self.generation = self.generation.next();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(b: u8) -> Hash32 {
        Hash32::new([b; 32])
    }

    fn caret(offset: usize) -> CursorSnapshot {
        CursorSnapshot::caret(offset, Generation::initial())
    }

    #[test]
    fn loaded_document_starts_clean() {
        let d = DocumentState::loaded("a\r\nb", LineEnding::Crlf, Some(hash(1)));
        assert_eq!(d.text(), "a\nb");
        assert!(!d.is_dirty());
        assert_eq!(d.generation(), Generation::initial());
        assert_eq!(d.text_for_disk(), "a\r\nb");
    }

    #[test]
    fn apply_delta_bumps_generation_and_marks_dirty() {
        let mut d = DocumentState::empty(LineEnding::Crlf);
        let g0 = d.generation();
        let delta = TextDelta::insert(0, "hi");
        d.apply_delta(&delta, InputKind::Typing, 0, caret(0), caret(2))
            .unwrap();
        assert_eq!(d.text(), "hi");
        assert!(d.generation() > g0);
        assert!(d.is_dirty());
    }

    #[test]
    fn apply_delta_rejects_non_boundary_and_leaves_text() {
        let mut d = DocumentState::loaded("héllo", LineEnding::Lf, None);
        let before = d.text().to_string();
        let bad = TextDelta::new(1..2, "x");
        assert!(
            d.apply_delta(&bad, InputKind::Typing, 0, caret(1), caret(2))
                .is_err()
        );
        assert_eq!(d.text(), before);
        assert!(!d.is_dirty());
    }

    #[test]
    fn undo_restores_original_and_redo_reapplies() {
        let mut d = DocumentState::loaded("abc", LineEnding::Lf, None);
        let delta = TextDelta::insert(3, "def");
        d.apply_delta(&delta, InputKind::Typing, 0, caret(3), caret(6))
            .unwrap();
        assert_eq!(d.text(), "abcdef");
        let c = d.undo().unwrap();
        assert_eq!(d.text(), "abc");
        assert!(c.is_some());
        let c = d.redo().unwrap();
        assert_eq!(d.text(), "abcdef");
        assert!(c.is_some());
    }

    #[test]
    fn ime_commit_is_one_undo_step() {
        let mut d = DocumentState::empty(LineEnding::Crlf);
        let commit = TextDelta::insert(0, "你好世界");
        d.apply_delta(&commit, InputKind::ImeCommit, 0, caret(0), caret(12))
            .unwrap();
        assert_eq!(d.text(), "你好世界");
        d.undo().unwrap();
        assert_eq!(d.text(), "");
    }

    #[test]
    fn persist_ack_advances_saved_generation() {
        let mut d = DocumentState::empty(LineEnding::Crlf);
        let delta = TextDelta::insert(0, "x");
        let g = d
            .apply_delta(&delta, InputKind::Typing, 0, caret(0), caret(1))
            .unwrap();
        assert!(d.is_dirty());
        d.acknowledge_persisted(g, hash(9)).unwrap();
        assert!(!d.is_dirty());
        assert_eq!(d.saved_generation(), g);
        assert_eq!(d.base_disk_hash(), Some(hash(9)));
    }

    #[test]
    fn persist_ack_never_exceeds_generation() {
        let mut d = DocumentState::empty(LineEnding::Crlf);
        let future = Generation::initial().next().next();
        assert!(d.acknowledge_persisted(future, hash(1)).is_err());
        assert_eq!(d.saved_generation(), Generation::initial());
    }

    #[test]
    fn stale_persist_ack_is_noop() {
        let mut d = DocumentState::empty(LineEnding::Crlf);
        let g = d
            .apply_delta(
                &TextDelta::insert(0, "a"),
                InputKind::Typing,
                0,
                caret(0),
                caret(1),
            )
            .unwrap();
        d.acknowledge_persisted(g, hash(1)).unwrap();
        // Acknowledging an older generation again changes nothing.
        d.acknowledge_persisted(Generation::initial(), hash(2))
            .unwrap();
        assert_eq!(d.saved_generation(), g);
        assert_eq!(d.base_disk_hash(), Some(hash(1)));
    }

    #[test]
    fn load_external_clears_undo_and_marks_clean() {
        let mut d = DocumentState::loaded("local edits", LineEnding::Lf, None);
        d.apply_delta(
            &TextDelta::insert(0, "X"),
            InputKind::Typing,
            0,
            caret(0),
            caret(1),
        )
        .unwrap();
        assert!(d.can_undo());
        assert!(d.is_dirty());
        d.load_external("external content", LineEnding::Lf, hash(7));
        assert_eq!(d.text(), "external content");
        assert!(!d.can_undo());
        assert!(!d.is_dirty());
        assert_eq!(d.base_disk_hash(), Some(hash(7)));
    }

    #[test]
    fn snapshot_carries_generation_and_is_immutable_text() {
        let mut d = DocumentState::loaded("snap", LineEnding::Lf, None);
        let s1 = d.snapshot();
        d.apply_delta(
            &TextDelta::insert(4, "!"),
            InputKind::Typing,
            0,
            caret(4),
            caret(5),
        )
        .unwrap();
        let s2 = d.snapshot();
        assert_eq!(&*s1.text, "snap");
        assert_eq!(&*s2.text, "snap!");
        assert!(s2.generation > s1.generation);
    }
}
