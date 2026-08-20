//! The sole interaction-to-document mutation coordinator.
//!
//! plan_ref: docs/plan/03_system_architecture.md#flow-coordination

use stickymd_core::{
    CursorSnapshot, DocumentError, DocumentSnapshot, DocumentState, EditKind, EditMeta,
    EditRequest, ExternalFileFact, Generation, Hash32, Selection, TextDelta,
};
use thiserror::Error;

use super::{ClipboardError, ClipboardPort};
use crate::instruction::AppIntent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEffect {
    DocumentChanged {
        generation: Generation,
        selection: Selection,
        delta: TextDelta,
    },
    ClipboardWritten,
    NoOp,
}

/// Borrowed, immutable observation of the canonical document.
///
/// It cannot outlive the coordinator borrow and cannot mutate or become a
/// second text authority. Owned snapshots remain explicit O(n) boundaries.
#[derive(Debug, Clone, Copy)]
pub struct EditorDocumentView<'a> {
    pub text: &'a str,
    pub generation: Generation,
    pub dirty: bool,
    pub base_disk_hash: Option<Hash32>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EditorFlowError {
    #[error(transparent)]
    Document(#[from] DocumentError),
    #[error(transparent)]
    Clipboard(#[from] ClipboardError),
}

pub struct EditorCoordinator<C> {
    document: DocumentState,
    clipboard: C,
}

impl<C: ClipboardPort> EditorCoordinator<C> {
    pub fn new(document: DocumentState, clipboard: C) -> Self {
        Self {
            document,
            clipboard,
        }
    }

    #[cfg(test)]
    pub fn empty(clipboard: C) -> Self {
        Self::new(
            DocumentState::empty(stickymd_core::LineEnding::Crlf),
            clipboard,
        )
    }

    pub fn snapshot(&self) -> DocumentSnapshot {
        self.document.snapshot()
    }

    pub fn view(&self) -> EditorDocumentView<'_> {
        EditorDocumentView {
            text: self.document.text(),
            generation: self.document.generation(),
            dirty: self.document.is_dirty(),
            base_disk_hash: self.document.base_disk_hash(),
        }
    }

    pub fn acknowledge_persisted(
        &mut self,
        generation: Generation,
        fingerprint: Hash32,
    ) -> Result<(), EditorFlowError> {
        self.document
            .acknowledge_persisted(generation, fingerprint)
            .map_err(Into::into)
    }

    pub fn reconcile_external(
        &mut self,
        fact: &ExternalFileFact,
    ) -> Result<Generation, EditorFlowError> {
        self.document
            .replace_from_reconciliation(&fact.text, fact.line_ending, fact.fingerprint)
            .map_err(Into::into)
    }

    pub fn load_unpersisted_recovery(
        &mut self,
        text: &str,
        line_ending: stickymd_core::LineEnding,
    ) -> Result<Generation, EditorFlowError> {
        self.document
            .replace_from_unpersisted_recovery(text, line_ending)
            .map_err(Into::into)
    }

    pub fn reset_to_missing_document(&mut self) -> Result<Generation, EditorFlowError> {
        self.document
            .reset_to_missing_document()
            .map_err(Into::into)
    }

    pub fn dispatch(&mut self, intent: AppIntent) -> Result<AppEffect, EditorFlowError> {
        match intent {
            AppIntent::Edit {
                expected_generation,
                selection,
                inserted,
                kind,
                timestamp_ms,
            } => self.replace(expected_generation, selection, inserted, kind, timestamp_ms),
            AppIntent::Undo => {
                let outcome = self.document.undo()?;
                Ok(AppEffect::DocumentChanged {
                    generation: outcome.generation,
                    selection: outcome.cursor.selection,
                    delta: outcome.delta,
                })
            }
            AppIntent::Redo => {
                let outcome = self.document.redo()?;
                Ok(AppEffect::DocumentChanged {
                    generation: outcome.generation,
                    selection: outcome.cursor.selection,
                    delta: outcome.delta,
                })
            }
            AppIntent::CopySelection {
                expected_generation,
                selection,
            } => self.copy(expected_generation, selection),
            AppIntent::CutSelection {
                expected_generation,
                selection,
                timestamp_ms,
            } => self.cut(expected_generation, selection, timestamp_ms),
            AppIntent::PasteText {
                expected_generation,
                selection,
                timestamp_ms,
            } => self.paste(expected_generation, selection, timestamp_ms),
        }
    }

    fn replace(
        &mut self,
        expected_generation: Generation,
        selection: Selection,
        inserted: String,
        kind: EditKind,
        timestamp_ms: u64,
    ) -> Result<AppEffect, EditorFlowError> {
        let inserted = normalize_runtime_newlines(inserted);
        let range = selection.normalized_range();
        let effective_kind = if !selection.is_collapsed()
            && matches!(
                kind,
                EditKind::Typing | EditKind::Backspace | EditKind::DeleteForward
            ) {
            EditKind::SelectionReplace
        } else {
            kind
        };
        let cursor_after = CursorSnapshot::caret(range.start + inserted.len());
        let request = EditRequest::new(
            expected_generation,
            range,
            inserted,
            CursorSnapshot::new(selection),
            cursor_after,
            EditMeta::new(effective_kind, timestamp_ms),
        );
        let outcome = self.document.edit(request)?;
        let Some(delta) = outcome.delta else {
            return Ok(AppEffect::NoOp);
        };
        Ok(AppEffect::DocumentChanged {
            generation: outcome.generation,
            selection: cursor_after.selection,
            delta,
        })
    }

    fn copy(
        &mut self,
        expected_generation: Generation,
        selection: Selection,
    ) -> Result<AppEffect, EditorFlowError> {
        let selected = self
            .selected_text(expected_generation, selection)?
            .to_owned();
        if selected.is_empty() {
            return Ok(AppEffect::NoOp);
        }
        self.clipboard.write_text(&selected)?;
        Ok(AppEffect::ClipboardWritten)
    }

    fn cut(
        &mut self,
        expected_generation: Generation,
        selection: Selection,
        timestamp_ms: u64,
    ) -> Result<AppEffect, EditorFlowError> {
        let selected = self
            .selected_text(expected_generation, selection)?
            .to_owned();
        if selected.is_empty() {
            return Ok(AppEffect::NoOp);
        }

        // Clipboard write deliberately precedes deletion. Failure must leave the
        // canonical document, generation, and history unchanged.
        self.clipboard.write_text(&selected)?;
        self.replace(
            expected_generation,
            selection,
            String::new(),
            EditKind::SelectionReplace,
            timestamp_ms,
        )
    }

    fn paste(
        &mut self,
        expected_generation: Generation,
        selection: Selection,
        timestamp_ms: u64,
    ) -> Result<AppEffect, EditorFlowError> {
        self.require_generation(expected_generation)?;
        let Some(text) = self.clipboard.read_text()? else {
            return Ok(AppEffect::NoOp);
        };
        if text.is_empty() {
            return Ok(AppEffect::NoOp);
        }
        self.replace(
            expected_generation,
            selection,
            text,
            EditKind::Paste,
            timestamp_ms,
        )
    }

    fn selected_text(
        &self,
        expected_generation: Generation,
        selection: Selection,
    ) -> Result<&str, EditorFlowError> {
        self.require_generation(expected_generation)?;
        let range = selection.normalized_range();
        self.document
            .text()
            .get(range)
            .ok_or(DocumentError::InvalidTextPosition.into())
    }

    fn require_generation(&self, expected: Generation) -> Result<(), EditorFlowError> {
        if expected != self.document.generation() {
            return Err(DocumentError::StaleEdit {
                expected,
                current: self.document.generation(),
            }
            .into());
        }
        Ok(())
    }
}

fn normalize_runtime_newlines(text: String) -> String {
    if text.contains('\r') {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct MockClipboard {
        text: Option<String>,
        fail_read: bool,
        fail_write: bool,
    }

    impl ClipboardPort for MockClipboard {
        fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
            if self.fail_read {
                Err(ClipboardError::Unavailable("read failed".to_owned()))
            } else {
                Ok(self.text.clone())
            }
        }

        fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
            if self.fail_write {
                Err(ClipboardError::Unavailable("write failed".to_owned()))
            } else {
                self.text = Some(text.to_owned());
                Ok(())
            }
        }
    }

    fn edit(coordinator: &mut EditorCoordinator<MockClipboard>, text: &str) -> AppEffect {
        coordinator
            .dispatch(AppIntent::Edit {
                expected_generation: coordinator.view().generation,
                selection: Selection::caret(0),
                inserted: text.to_owned(),
                kind: EditKind::Paste,
                timestamp_ms: 0,
            })
            .unwrap()
    }

    #[test]
    fn typed_edit_flows_through_coordinator_and_normalizes_newlines() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        let effect = edit(&mut coordinator, "a\r\nb\rc");
        assert!(matches!(effect, AppEffect::DocumentChanged { .. }));
        assert_eq!(&*coordinator.snapshot().text, "a\nb\nc");
    }

    #[test]
    fn copy_reads_canonical_selection() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        edit(&mut coordinator, "这是 Rust 测试");
        let generation = coordinator.view().generation;
        let effect = coordinator
            .dispatch(AppIntent::CopySelection {
                expected_generation: generation,
                selection: Selection::new(3, 11),
            })
            .unwrap();
        assert_eq!(effect, AppEffect::ClipboardWritten);
        assert_eq!(coordinator.clipboard.text.as_deref(), Some("是 Rust"));
    }

    #[test]
    fn copy_failure_leaves_canonical_state_unchanged() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        edit(&mut coordinator, "secret");
        coordinator.clipboard.fail_write = true;
        let before = coordinator.snapshot();
        let result = coordinator.dispatch(AppIntent::CopySelection {
            expected_generation: before.generation,
            selection: Selection::new(0, 6),
        });
        assert!(matches!(result, Err(EditorFlowError::Clipboard(_))));
        assert_eq!(coordinator.snapshot(), before);
    }

    #[test]
    fn cut_clipboard_failure_is_state_atomic() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        edit(&mut coordinator, "secret");
        coordinator.clipboard.fail_write = true;
        let before = coordinator.snapshot();
        let result = coordinator.dispatch(AppIntent::CutSelection {
            expected_generation: before.generation,
            selection: Selection::new(0, 6),
            timestamp_ms: 1,
        });
        assert!(matches!(result, Err(EditorFlowError::Clipboard(_))));
        assert_eq!(coordinator.snapshot(), before);
    }

    #[test]
    fn paste_empty_is_noop_and_paste_text_replaces_selection() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        edit(&mut coordinator, "hello world");
        let generation = coordinator.view().generation;
        assert_eq!(
            coordinator
                .dispatch(AppIntent::PasteText {
                    expected_generation: generation,
                    selection: Selection::caret(0),
                    timestamp_ms: 1,
                })
                .unwrap(),
            AppEffect::NoOp
        );
        coordinator.clipboard.text = Some("中国".to_owned());
        coordinator
            .dispatch(AppIntent::PasteText {
                expected_generation: generation,
                selection: Selection::new(6, 11),
                timestamp_ms: 2,
            })
            .unwrap();
        assert_eq!(&*coordinator.snapshot().text, "hello 中国");
    }

    #[test]
    fn undo_and_redo_return_incremental_deltas() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        edit(&mut coordinator, "你好");
        let undo = coordinator.dispatch(AppIntent::Undo).unwrap();
        assert!(matches!(
            undo,
            AppEffect::DocumentChanged { ref delta, .. }
                if delta.deleted() == "你好" && delta.inserted().is_empty()
        ));
        let redo = coordinator.dispatch(AppIntent::Redo).unwrap();
        assert!(matches!(
            redo,
            AppEffect::DocumentChanged { ref delta, .. }
                if delta.deleted().is_empty() && delta.inserted() == "你好"
        ));
    }

    #[test]
    fn cut_success_copies_then_deletes_and_can_be_undone() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        edit(&mut coordinator, "alpha 中文 omega");
        let generation = coordinator.view().generation;
        coordinator
            .dispatch(AppIntent::CutSelection {
                expected_generation: generation,
                selection: Selection::new(6, 12),
                timestamp_ms: 1,
            })
            .unwrap();
        assert_eq!(coordinator.clipboard.text.as_deref(), Some("中文"));
        assert_eq!(&*coordinator.snapshot().text, "alpha  omega");
        coordinator.dispatch(AppIntent::Undo).unwrap();
        assert_eq!(&*coordinator.snapshot().text, "alpha 中文 omega");
    }

    #[test]
    fn paste_read_failure_is_state_atomic() {
        let clipboard = MockClipboard {
            fail_read: true,
            ..MockClipboard::default()
        };
        let mut coordinator = EditorCoordinator::empty(clipboard);
        edit(&mut coordinator, "keep");
        let before = coordinator.snapshot();
        let result = coordinator.dispatch(AppIntent::PasteText {
            expected_generation: before.generation,
            selection: Selection::new(0, 4),
            timestamp_ms: 1,
        });
        assert!(matches!(result, Err(EditorFlowError::Clipboard(_))));
        assert_eq!(coordinator.snapshot(), before);
    }

    #[test]
    fn stale_copy_is_rejected_before_clipboard_access() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        let stale = coordinator.view().generation;
        edit(&mut coordinator, "new");
        let result = coordinator.dispatch(AppIntent::CopySelection {
            expected_generation: stale,
            selection: Selection::new(0, 3),
        });
        assert!(matches!(
            result,
            Err(EditorFlowError::Document(DocumentError::StaleEdit { .. }))
        ));
        assert!(coordinator.clipboard.text.is_none());
    }
}
