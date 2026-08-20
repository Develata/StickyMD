//! Runtime authority for canonical document text.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#documentstate
//!
//! Invariants:
//! 1. canonical text is valid UTF-8 and changes only through this module;
//! 2. edit ranges and cursor positions are validated before mutation;
//! 3. generation advances monotonically and exhaustion fails closed;
//! 4. stale edit requests are rejected;
//! 5. new edits clear redo, while no-op edits change no history;
//! 6. failed edit/undo/redo operations leave text, history, and generation unchanged;
//! 7. snapshots are immutable projections and cannot mutate this state;
//! 8. persisted acknowledgements cannot refer to future generations.

use std::ops::Range;
use std::sync::Arc;

use crate::edit::{EditOutcome, RedoOutcome, TextDelta, UndoOutcome};
use crate::selection::position_is_valid;
use crate::text_store::{StringTextStore, TextStore, validate_range};
use crate::undo::{UndoEntry, UndoManager};
use crate::{
    CursorSnapshot, DocumentError, DocumentSnapshot, EditRequest, Generation, Hash32, LineEnding,
};

/// Sole runtime authority for the working Markdown text.
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
    /// Create a clean document from durable content.
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

    pub fn empty(line_ending: LineEnding) -> Self {
        Self::loaded("", line_ending, None)
    }

    pub fn text(&self) -> &str {
        self.store.as_str()
    }

    pub fn len_bytes(&self) -> usize {
        self.store.len_bytes()
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

    pub fn is_dirty(&self) -> bool {
        self.generation != self.saved_generation
    }

    pub fn can_undo(&self) -> bool {
        self.undo.can_undo()
    }

    pub fn can_redo(&self) -> bool {
        self.undo.can_redo()
    }

    /// Explicit O(n) immutable snapshot for workers and render projections.
    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            text: Arc::from(self.store.as_str()),
            generation: self.generation,
            line_ending: self.line_ending,
        }
    }

    pub fn text_for_disk(&self) -> String {
        self.line_ending.apply(self.store.as_str())
    }

    /// Apply one typed edit request.
    ///
    /// All fallible validation, including generation overflow, finishes before the
    /// canonical string or history is mutated.
    pub fn edit(&mut self, request: EditRequest) -> Result<EditOutcome, DocumentError> {
        if request.expected_generation != self.generation {
            return Err(DocumentError::StaleEdit {
                expected: request.expected_generation,
                current: self.generation,
            });
        }

        let text = self.store.as_str();
        validate_range(text, &request.range)?;
        if !request.cursor_before.is_valid_for(text) {
            return Err(DocumentError::InvalidTextPosition);
        }
        if !cursor_is_valid_after_replace(
            text,
            &request.range,
            &request.inserted,
            request.cursor_after,
        ) {
            return Err(DocumentError::InvalidTextPosition);
        }

        let deleted = &text[request.range.clone()];
        if deleted == request.inserted {
            return Ok(EditOutcome {
                generation: self.generation,
                dirty: self.is_dirty(),
                undo_recorded: false,
                grouped: false,
                delta: None,
            });
        }

        let next_generation = self
            .generation
            .checked_next()
            .ok_or(DocumentError::GenerationExhausted)?;
        let delta = TextDelta::new(
            request.range.clone(),
            Arc::from(deleted),
            Arc::from(request.inserted),
            request.cursor_before,
            request.cursor_after,
        );

        self.store.replace(delta.range.clone(), &delta.inserted)?;
        let record = self.undo.record(UndoEntry::new(
            delta.clone(),
            request.meta.kind,
            request.meta.timestamp_ms,
        ));
        self.generation = next_generation;

        Ok(EditOutcome {
            generation: self.generation,
            dirty: self.is_dirty(),
            undo_recorded: record.recorded,
            grouped: record.grouped,
            delta: Some(delta),
        })
    }

    /// Undo one logical history entry transactionally.
    pub fn undo(&mut self) -> Result<UndoOutcome, DocumentError> {
        let entry = self
            .undo
            .peek_undo()
            .cloned()
            .ok_or(DocumentError::UndoUnavailable)?;
        let inverse = entry.delta.inverse();
        self.validate_recorded_delta(&inverse)?;
        if !cursor_is_valid_after_replace(
            self.store.as_str(),
            &inverse.range,
            &inverse.inserted,
            inverse.cursor_after,
        ) {
            return Err(DocumentError::InvalidTextPosition);
        }
        let next_generation = self
            .generation
            .checked_next()
            .ok_or(DocumentError::GenerationExhausted)?;

        self.store
            .replace(inverse.range.clone(), &inverse.inserted)?;
        self.undo.commit_undo();
        self.generation = next_generation;
        Ok(UndoOutcome {
            generation: self.generation,
            cursor: inverse.cursor_after,
            delta: inverse,
        })
    }

    /// Redo one logical history entry transactionally.
    pub fn redo(&mut self) -> Result<RedoOutcome, DocumentError> {
        let entry = self
            .undo
            .peek_redo()
            .cloned()
            .ok_or(DocumentError::RedoUnavailable)?;
        let forward = entry.delta.clone();
        self.validate_recorded_delta(&forward)?;
        if !cursor_is_valid_after_replace(
            self.store.as_str(),
            &forward.range,
            &forward.inserted,
            forward.cursor_after,
        ) {
            return Err(DocumentError::InvalidTextPosition);
        }
        let next_generation = self
            .generation
            .checked_next()
            .ok_or(DocumentError::GenerationExhausted)?;

        self.store
            .replace(forward.range.clone(), &forward.inserted)?;
        self.undo.commit_redo();
        self.generation = next_generation;
        Ok(RedoOutcome {
            generation: self.generation,
            cursor: forward.cursor_after,
            delta: forward,
        })
    }

    /// Accept a receipt from a persistence worker without clearing newer dirtiness.
    pub fn acknowledge_persisted(
        &mut self,
        persisted: Generation,
        hash: Hash32,
    ) -> Result<(), DocumentError> {
        if persisted > self.generation {
            return Err(DocumentError::InvalidPersistedGeneration);
        }
        if persisted > self.saved_generation {
            self.saved_generation = persisted;
            self.base_disk_hash = Some(hash);
        } else if persisted == self.saved_generation {
            // A same-generation rewrite can legitimately change durable bytes
            // (for example BOM removal during recovery). Its receipt must refresh
            // the OCC base even though canonical text generation did not change.
            self.base_disk_hash = Some(hash);
        }
        Ok(())
    }

    /// Reconciliation gate for external reload or accepted recovery content.
    pub fn replace_from_reconciliation(
        &mut self,
        text: &str,
        line_ending: LineEnding,
        hash: Hash32,
    ) -> Result<Generation, DocumentError> {
        let next_generation = self
            .generation
            .checked_next()
            .ok_or(DocumentError::GenerationExhausted)?;
        self.store.replace_all(LineEnding::to_internal(text));
        self.line_ending = line_ending;
        self.undo.clear();
        self.generation = next_generation;
        self.saved_generation = next_generation;
        self.base_disk_hash = Some(hash);
        Ok(next_generation)
    }

    /// Replace canonical runtime text with recovery evidence that has not yet
    /// been published. The previous durable fingerprint remains the guarded
    /// save base and the new generation is deliberately dirty.
    pub fn replace_from_unpersisted_recovery(
        &mut self,
        text: &str,
        line_ending: LineEnding,
    ) -> Result<Generation, DocumentError> {
        let next_generation = self
            .generation
            .checked_next()
            .ok_or(DocumentError::GenerationExhausted)?;
        self.store.replace_all(LineEnding::to_internal(text));
        self.line_ending = line_ending;
        self.undo.clear();
        self.generation = next_generation;
        Ok(next_generation)
    }

    /// Restore the startup meaning of a missing canonical note after the user
    /// discards temporary recovery evidence. The coordinator will subsequently
    /// publish the empty note through the normal guarded create path.
    pub fn reset_to_missing_document(&mut self) -> Result<Generation, DocumentError> {
        let next_generation = self
            .generation
            .checked_next()
            .ok_or(DocumentError::GenerationExhausted)?;
        self.store.replace_all(String::new());
        self.line_ending = LineEnding::Crlf;
        self.undo.clear();
        self.generation = next_generation;
        self.saved_generation = next_generation;
        self.base_disk_hash = None;
        Ok(next_generation)
    }

    fn validate_recorded_delta(&self, delta: &TextDelta) -> Result<(), DocumentError> {
        validate_range(self.store.as_str(), &delta.range)?;
        if &self.store.as_str()[delta.range.clone()] != delta.deleted.as_ref() {
            return Err(DocumentError::DeletedTextMismatch);
        }
        Ok(())
    }

    #[cfg(test)]
    fn set_generation_for_test(&mut self, generation: Generation) {
        self.generation = generation;
    }
}

fn cursor_is_valid_after_replace(
    text: &str,
    range: &Range<usize>,
    inserted: &str,
    cursor: CursorSnapshot,
) -> bool {
    position_is_valid_after_replace(text, range, inserted, cursor.selection.anchor.byte)
        && position_is_valid_after_replace(text, range, inserted, cursor.selection.active.byte)
}

fn position_is_valid_after_replace(
    text: &str,
    range: &Range<usize>,
    inserted: &str,
    byte: usize,
) -> bool {
    let Some(inserted_end) = range.start.checked_add(inserted.len()) else {
        return false;
    };
    let Some(after_len) = text
        .len()
        .checked_sub(range.end.saturating_sub(range.start))
        .and_then(|length| length.checked_add(inserted.len()))
    else {
        return false;
    };
    if byte > after_len {
        return false;
    }
    if byte <= range.start {
        return position_is_valid(text, byte);
    }
    if byte <= inserted_end {
        return inserted.is_char_boundary(byte - range.start);
    }
    range
        .end
        .checked_add(byte - inserted_end)
        .is_some_and(|original| position_is_valid(text, original))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::undo::{MAX_UNDO_BYTES, MAX_UNDO_ENTRIES};
    use crate::{EditKind, EditMeta, Selection};

    fn hash(byte: u8) -> Hash32 {
        Hash32::new([byte; 32])
    }

    fn request(
        doc: &DocumentState,
        range: Range<usize>,
        inserted: impl Into<String>,
        before: CursorSnapshot,
        after: CursorSnapshot,
        kind: EditKind,
        time: u64,
    ) -> EditRequest {
        EditRequest::new(
            doc.generation(),
            range,
            inserted,
            before,
            after,
            EditMeta::new(kind, time),
        )
    }

    #[test]
    fn edit_rejects_stale_generation_without_state_change() {
        let mut doc = DocumentState::empty(LineEnding::Lf);
        let stale = EditRequest::new(
            Generation::for_test(9),
            0..0,
            "x",
            CursorSnapshot::caret(0),
            CursorSnapshot::caret(1),
            EditMeta::new(EditKind::Typing, 0),
        );
        let before = doc.snapshot();
        assert!(matches!(
            doc.edit(stale),
            Err(DocumentError::StaleEdit { .. })
        ));
        assert_eq!(doc.snapshot(), before);
        assert!(!doc.can_undo());
    }

    #[test]
    fn no_op_changes_nothing_and_preserves_redo() {
        let mut doc = DocumentState::loaded("a", LineEnding::Lf, None);
        let insert = request(
            &doc,
            1..1,
            "b",
            CursorSnapshot::caret(1),
            CursorSnapshot::caret(2),
            EditKind::Typing,
            0,
        );
        doc.edit(insert).unwrap();
        doc.undo().unwrap();
        let generation = doc.generation();
        let noop = request(
            &doc,
            0..1,
            "a",
            CursorSnapshot::caret(1),
            CursorSnapshot::caret(1),
            EditKind::Other,
            1,
        );
        let outcome = doc.edit(noop).unwrap();
        assert!(outcome.delta.is_none());
        assert_eq!(doc.generation(), generation);
        assert!(doc.can_redo());
    }

    #[test]
    fn reverse_selection_is_valid_for_replacement() {
        let mut doc = DocumentState::loaded("hello world", LineEnding::Lf, None);
        let before = CursorSnapshot::new(Selection::new(11, 6));
        let after = CursorSnapshot::caret(12);
        let edit = request(
            &doc,
            6..11,
            "世界",
            before,
            after,
            EditKind::SelectionReplace,
            0,
        );
        doc.edit(edit).unwrap();
        assert_eq!(doc.text(), "hello 世界");
        assert_eq!(doc.undo().unwrap().cursor, before);
        assert_eq!(doc.text(), "hello world");
    }

    #[test]
    fn generation_exhaustion_fails_before_mutation() {
        let mut doc = DocumentState::empty(LineEnding::Lf);
        doc.set_generation_for_test(Generation::for_test(u64::MAX));
        let edit = request(
            &doc,
            0..0,
            "x",
            CursorSnapshot::caret(0),
            CursorSnapshot::caret(1),
            EditKind::Typing,
            0,
        );
        assert_eq!(doc.edit(edit), Err(DocumentError::GenerationExhausted));
        assert_eq!(doc.text(), "");
        assert!(!doc.can_undo());
    }

    #[test]
    fn invalid_cursor_after_is_failure_atomic() {
        let mut doc = DocumentState::loaded("中", LineEnding::Lf, None);
        let edit = request(
            &doc,
            0..0,
            "a",
            CursorSnapshot::caret(0),
            CursorSnapshot::caret(2),
            EditKind::Typing,
            0,
        );
        let before = doc.snapshot();
        assert_eq!(doc.edit(edit), Err(DocumentError::InvalidTextPosition));
        assert_eq!(doc.snapshot(), before);
    }

    #[test]
    fn undo_and_redo_advance_generation_and_restore_cursor() {
        let mut doc = DocumentState::loaded("abc", LineEnding::Lf, None);
        let edit = request(
            &doc,
            3..3,
            "def",
            CursorSnapshot::caret(3),
            CursorSnapshot::caret(6),
            EditKind::Paste,
            0,
        );
        let edited = doc.edit(edit).unwrap().generation;
        let undone = doc.undo().unwrap();
        assert_eq!(doc.text(), "abc");
        assert_eq!(undone.cursor, CursorSnapshot::caret(3));
        assert!(undone.generation > edited);
        let redone = doc.redo().unwrap();
        assert_eq!(doc.text(), "abcdef");
        assert_eq!(redone.cursor, CursorSnapshot::caret(6));
        assert!(redone.generation > undone.generation);
    }

    #[test]
    fn undo_failure_keeps_history_and_generation_unchanged() {
        let mut doc = DocumentState::empty(LineEnding::Lf);
        let edit = request(
            &doc,
            0..0,
            "abc",
            CursorSnapshot::caret(0),
            CursorSnapshot::caret(3),
            EditKind::Paste,
            0,
        );
        doc.edit(edit).unwrap();
        doc.store.replace_all("different".to_owned());
        let generation = doc.generation();
        let undo_len = doc.undo.undo_len();
        let redo_len = doc.undo.redo_len();
        assert_eq!(doc.undo(), Err(DocumentError::DeletedTextMismatch));
        assert_eq!(doc.generation(), generation);
        assert_eq!(doc.undo.undo_len(), undo_len);
        assert_eq!(doc.undo.redo_len(), redo_len);
        assert_eq!(doc.text(), "different");
    }

    #[test]
    fn oversized_edit_succeeds_without_recording_history() {
        let mut doc = DocumentState::empty(LineEnding::Lf);
        let huge = "x".repeat(MAX_UNDO_BYTES);
        let edit = request(
            &doc,
            0..0,
            huge.clone(),
            CursorSnapshot::caret(0),
            CursorSnapshot::caret(huge.len()),
            EditKind::Paste,
            0,
        );
        let outcome = doc.edit(edit).unwrap();
        assert_eq!(doc.len_bytes(), huge.len());
        assert!(!outcome.undo_recorded);
        assert!(!doc.can_undo());
        assert_eq!(doc.undo.history_bytes(), 0);
    }

    #[test]
    fn history_entry_limit_evicts_oldest() {
        let mut doc = DocumentState::empty(LineEnding::Lf);
        for index in 0..MAX_UNDO_ENTRIES + 5 {
            let offset = doc.len_bytes();
            let edit = request(
                &doc,
                offset..offset,
                "x",
                CursorSnapshot::caret(offset),
                CursorSnapshot::caret(offset + 1),
                EditKind::Paste,
                index as u64,
            );
            doc.edit(edit).unwrap();
        }
        assert_eq!(doc.undo.undo_len(), MAX_UNDO_ENTRIES);
        assert!(doc.undo.history_bytes() <= MAX_UNDO_BYTES);
    }

    #[test]
    fn empty_history_returns_typed_errors() {
        let mut doc = DocumentState::empty(LineEnding::Lf);
        assert_eq!(doc.undo(), Err(DocumentError::UndoUnavailable));
        assert_eq!(doc.redo(), Err(DocumentError::RedoUnavailable));
    }

    #[test]
    fn persisted_acknowledgement_never_marks_newer_edits_clean() {
        let mut doc = DocumentState::empty(LineEnding::Lf);
        let first = request(
            &doc,
            0..0,
            "a",
            CursorSnapshot::caret(0),
            CursorSnapshot::caret(1),
            EditKind::Typing,
            0,
        );
        let saved = doc.edit(first).unwrap().generation;
        let second = request(
            &doc,
            1..1,
            "b",
            CursorSnapshot::caret(1),
            CursorSnapshot::caret(2),
            EditKind::Typing,
            1_000,
        );
        doc.edit(second).unwrap();
        doc.acknowledge_persisted(saved, hash(1)).unwrap();
        assert!(doc.is_dirty());
        assert_eq!(doc.saved_generation(), saved);
        let future = doc.generation().checked_next().unwrap();
        assert_eq!(
            doc.acknowledge_persisted(future, hash(2)),
            Err(DocumentError::InvalidPersistedGeneration)
        );
    }

    #[test]
    fn first_generation_zero_persist_establishes_base_hash() {
        let mut doc = DocumentState::empty(LineEnding::Lf);
        let durable_hash = hash(7);
        doc.acknowledge_persisted(Generation::initial(), durable_hash)
            .unwrap();
        assert_eq!(doc.base_disk_hash(), Some(durable_hash));
        assert!(!doc.is_dirty());
    }

    #[test]
    fn same_generation_rewrite_refreshes_durable_base_hash() {
        let original = hash(3);
        let rewritten = hash(4);
        let mut doc = DocumentState::loaded("same", LineEnding::Crlf, Some(original));
        doc.acknowledge_persisted(Generation::initial(), rewritten)
            .unwrap();
        assert_eq!(doc.base_disk_hash(), Some(rewritten));
        assert!(!doc.is_dirty());
    }

    #[test]
    fn recovery_evidence_is_dirty_until_a_real_persist_receipt() {
        let durable = hash(8);
        let mut doc = DocumentState::loaded("disk", LineEnding::Lf, Some(durable));
        let generation = doc
            .replace_from_unpersisted_recovery("temporary", LineEnding::Crlf)
            .unwrap();
        assert_eq!(doc.text(), "temporary");
        assert!(doc.is_dirty());
        assert_eq!(doc.base_disk_hash(), Some(durable));

        let published = hash(9);
        doc.acknowledge_persisted(generation, published).unwrap();
        assert!(!doc.is_dirty());
        assert_eq!(doc.base_disk_hash(), Some(published));
    }

    #[test]
    fn discarding_recovery_for_missing_note_restores_empty_clean_state() {
        let mut doc = DocumentState::empty(LineEnding::Crlf);
        doc.replace_from_unpersisted_recovery("temporary", LineEnding::Lf)
            .unwrap();
        doc.reset_to_missing_document().unwrap();
        assert_eq!(doc.text(), "");
        assert!(!doc.is_dirty());
        assert_eq!(doc.base_disk_hash(), None);
        assert_eq!(doc.line_ending(), LineEnding::Crlf);
    }

    #[test]
    fn snapshot_includes_line_ending_and_remains_immutable() {
        let mut doc = DocumentState::loaded("a\r\nb", LineEnding::Crlf, None);
        let snapshot = doc.snapshot();
        let edit = request(
            &doc,
            3..3,
            "!",
            CursorSnapshot::caret(3),
            CursorSnapshot::caret(4),
            EditKind::Typing,
            0,
        );
        doc.edit(edit).unwrap();
        assert_eq!(&*snapshot.text, "a\nb");
        assert_eq!(snapshot.line_ending, LineEnding::Crlf);
        assert_eq!(doc.text(), "a\nb!");
    }

    #[test]
    fn reconciliation_is_the_only_external_replacement_gate() {
        let mut doc = DocumentState::loaded("local", LineEnding::Lf, None);
        let generation = doc
            .replace_from_reconciliation("external\r\ntext", LineEnding::Crlf, hash(7))
            .unwrap();
        assert_eq!(doc.text(), "external\ntext");
        assert_eq!(doc.generation(), generation);
        assert_eq!(doc.saved_generation(), generation);
        assert!(!doc.is_dirty());
        assert!(!doc.can_undo());
    }

    #[test]
    fn unicode_is_not_normalized() {
        let mut doc = DocumentState::empty(LineEnding::Lf);
        let decomposed = "e\u{301}";
        let edit = request(
            &doc,
            0..0,
            decomposed,
            CursorSnapshot::caret(0),
            CursorSnapshot::caret(decomposed.len()),
            EditKind::ImeCommit,
            0,
        );
        doc.edit(edit).unwrap();
        assert_eq!(doc.text().as_bytes(), decomposed.as_bytes());
    }
}
