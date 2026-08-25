//! The sole interaction-to-document mutation coordinator.
//!
//! plan_ref: docs/plan/03_system_architecture.md#flow-coordination

use stickymd_core::{
    AssetEffect, CursorSnapshot, DocumentError, DocumentSnapshot, DocumentState, EditKind,
    EditMeta, EditRequest, ExternalFileFact, Generation, Hash32, Selection, TextDelta,
};
use stickymd_render::preview::{SemanticConversionError, convert_latex_math_delimiters};
use thiserror::Error;

use super::{ClipboardError, ClipboardPaste, ClipboardPort, PendingAssetPaste};
use crate::instruction::AppIntent;
use crate::source_search::{literal_range_matches, replace_all_literal};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppEffect {
    DocumentChanged {
        generation: Generation,
        selection: Selection,
        delta: TextDelta,
        asset_effects: Vec<AssetEffect>,
    },
    AssetPasteRequested(PendingAssetPaste),
    /// A durable external fact replaced canonical text. The shell must fully
    /// resync disposable projections and request runtime asset convergence;
    /// neither follow-up is optional at this boundary.
    ExternalDocumentReconciled {
        generation: Generation,
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
    #[error(transparent)]
    SemanticConversion(#[from] SemanticConversionError),
    #[error("literal search range no longer matches the current canonical document")]
    SearchRangeMismatch,
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

    pub fn managed_ref_counts(
        &self,
    ) -> std::collections::HashMap<stickymd_core::ManagedAssetName, usize> {
        self.document.managed_ref_counts().clone()
    }

    pub fn view(&self) -> EditorDocumentView<'_> {
        EditorDocumentView {
            text: self.document.text(),
            generation: self.document.generation(),
            dirty: self.document.is_dirty(),
            base_disk_hash: self.document.base_disk_hash(),
        }
    }

    /// Reads text for transient shell inputs without exposing clipboard
    /// ownership or bypassing the execution-domain port.
    pub fn read_clipboard_text(&mut self) -> Result<Option<String>, EditorFlowError> {
        self.clipboard.read_text().map_err(Into::into)
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

    pub fn reconcile_external_for_runtime(
        &mut self,
        fact: &ExternalFileFact,
    ) -> Result<AppEffect, EditorFlowError> {
        let generation = self.reconcile_external(fact)?;
        Ok(AppEffect::ExternalDocumentReconciled { generation })
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
                    asset_effects: outcome.asset_effects,
                })
            }
            AppIntent::Redo => {
                let outcome = self.document.redo()?;
                Ok(AppEffect::DocumentChanged {
                    generation: outcome.generation,
                    selection: outcome.cursor.selection,
                    delta: outcome.delta,
                    asset_effects: outcome.asset_effects,
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
            AppIntent::PasteClipboard {
                expected_generation,
                selection,
                timestamp_ms,
            } => self.paste_clipboard(expected_generation, selection, timestamp_ms),
            AppIntent::ConvertLatexMathDelimiters {
                expected_generation,
                selection,
                scope_to_selection,
                timestamp_ms,
            } => self.convert_math_delimiters(
                expected_generation,
                selection,
                scope_to_selection,
                timestamp_ms,
            ),
            AppIntent::ReplaceLiteralMatch {
                expected_generation,
                range,
                query,
                replacement,
                options,
                timestamp_ms,
            } => {
                self.require_generation(expected_generation)?;
                if !literal_range_matches(self.document.text(), range.clone(), &query, options) {
                    return Err(EditorFlowError::SearchRangeMismatch);
                }
                self.replace(
                    expected_generation,
                    Selection::new(range.start, range.end),
                    replacement,
                    EditKind::SelectionReplace,
                    timestamp_ms,
                )
            }
            AppIntent::ReplaceAllLiteral {
                expected_generation,
                query,
                replacement,
                options,
                timestamp_ms,
            } => {
                self.require_generation(expected_generation)?;
                let Some((output, _count)) =
                    replace_all_literal(self.document.text(), &query, &replacement, options)
                else {
                    return Ok(AppEffect::NoOp);
                };
                self.replace(
                    expected_generation,
                    Selection::new(0, self.document.text().len()),
                    output,
                    EditKind::Other,
                    timestamp_ms,
                )
            }
            AppIntent::WriteClipboard { text } => {
                if text.is_empty() {
                    Ok(AppEffect::NoOp)
                } else {
                    self.clipboard.write_text(&text)?;
                    Ok(AppEffect::ClipboardWritten)
                }
            }
        }
    }

    pub fn commit_prepared_paste(
        &mut self,
        expected_generation: Generation,
        selection: Selection,
        markdown: String,
        timestamp_ms: u64,
    ) -> Result<AppEffect, EditorFlowError> {
        self.replace(
            expected_generation,
            selection,
            markdown,
            EditKind::Paste,
            timestamp_ms,
        )
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
            asset_effects: outcome.asset_effects,
        })
    }

    fn convert_math_delimiters(
        &mut self,
        expected_generation: Generation,
        selection: Selection,
        scope_to_selection: bool,
        timestamp_ms: u64,
    ) -> Result<AppEffect, EditorFlowError> {
        self.require_generation(expected_generation)?;
        let snapshot = self.document.snapshot();
        let scope = scope_to_selection.then(|| selection.normalized_range());
        let Some(conversion) = convert_latex_math_delimiters(&snapshot, scope)? else {
            return Ok(AppEffect::NoOp);
        };
        let cursor_after = CursorSnapshot::new(Selection::new(
            conversion.map_position(selection.anchor.byte),
            conversion.map_position(selection.active.byte),
        ));
        let request = EditRequest::new(
            expected_generation,
            0..snapshot.text.len(),
            conversion.into_text(),
            CursorSnapshot::new(selection),
            cursor_after,
            EditMeta::new(EditKind::Other, timestamp_ms),
        );
        let outcome = self.document.edit(request)?;
        let Some(delta) = outcome.delta else {
            return Ok(AppEffect::NoOp);
        };
        Ok(AppEffect::DocumentChanged {
            generation: outcome.generation,
            selection: cursor_after.selection,
            delta,
            asset_effects: outcome.asset_effects,
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

    fn paste_clipboard(
        &mut self,
        expected_generation: Generation,
        selection: Selection,
        timestamp_ms: u64,
    ) -> Result<AppEffect, EditorFlowError> {
        self.require_generation(expected_generation)?;
        let Some(payload) = self.clipboard.read_paste()? else {
            return Ok(AppEffect::NoOp);
        };
        match payload {
            ClipboardPaste::Text(text) => {
                if text.is_empty() {
                    Ok(AppEffect::NoOp)
                } else {
                    self.replace(
                        expected_generation,
                        selection,
                        text,
                        EditKind::Paste,
                        timestamp_ms,
                    )
                }
            }
            payload => Ok(AppEffect::AssetPasteRequested(PendingAssetPaste {
                expected_generation,
                selection,
                timestamp_ms,
                payload,
            })),
        }
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
        paste: Option<ClipboardPaste>,
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

        fn read_paste(&mut self) -> Result<Option<ClipboardPaste>, ClipboardError> {
            if self.fail_read {
                Err(ClipboardError::Unavailable("read failed".to_owned()))
            } else if self.paste.is_some() {
                Ok(self.paste.clone())
            } else {
                Ok(self.text.clone().map(ClipboardPaste::Text))
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
                .dispatch(AppIntent::PasteClipboard {
                    expected_generation: generation,
                    selection: Selection::caret(0),
                    timestamp_ms: 1,
                })
                .unwrap(),
            AppEffect::NoOp
        );
        coordinator.clipboard.text = Some("中国".to_owned());
        coordinator
            .dispatch(AppIntent::PasteClipboard {
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
        let result = coordinator.dispatch(AppIntent::PasteClipboard {
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

    #[test]
    fn phase7_image_paste_is_deferred_and_stale_commit_is_rejected() {
        let clipboard = MockClipboard {
            paste: Some(ClipboardPaste::EncodedImage(vec![1, 2, 3])),
            ..MockClipboard::default()
        };
        let mut coordinator = EditorCoordinator::empty(clipboard);
        let captured = coordinator.view().generation;
        let effect = coordinator
            .dispatch(AppIntent::PasteClipboard {
                expected_generation: captured,
                selection: Selection::caret(0),
                timestamp_ms: 1,
            })
            .unwrap();
        assert!(matches!(effect, AppEffect::AssetPasteRequested(_)));
        edit(&mut coordinator, "newer");
        let before = coordinator.snapshot();
        let result = coordinator.commit_prepared_paste(
            captured,
            Selection::caret(0),
            "![](images/stickymd-0123456789abcdef0123.png)".into(),
            2,
        );
        assert!(matches!(
            result,
            Err(EditorFlowError::Document(DocumentError::StaleEdit { .. }))
        ));
        assert_eq!(coordinator.snapshot(), before);
    }

    #[test]
    fn external_runtime_reload_emits_projection_and_asset_reconcile_effect() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        edit(&mut coordinator, "local dirty text");
        let external = "![](images/stickymd-0123456789abcdef0123.png)";
        let fact = ExternalFileFact {
            fingerprint: stickymd_core::hash_bytes(external.as_bytes()),
            text: external.into(),
            line_ending: stickymd_core::LineEnding::Lf,
            durable_len: external.len(),
        };

        let effect = coordinator.reconcile_external_for_runtime(&fact).unwrap();
        assert_eq!(
            effect,
            AppEffect::ExternalDocumentReconciled {
                generation: coordinator.view().generation,
            }
        );
        assert_eq!(
            coordinator
                .managed_ref_counts()
                .values()
                .copied()
                .sum::<usize>(),
            1
        );
        assert!(!coordinator.view().dirty);
    }

    #[test]
    fn phase11b_math_delimiter_batch_is_one_generation_and_one_undo_step() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        let source = "前 \\(x\\) 中 \\[y\\] 后";
        edit(&mut coordinator, source);
        let before_generation = coordinator.view().generation;

        let effect = coordinator
            .dispatch(AppIntent::ConvertLatexMathDelimiters {
                expected_generation: before_generation,
                selection: Selection::caret(source.len()),
                scope_to_selection: false,
                timestamp_ms: 10,
            })
            .unwrap();
        assert!(matches!(effect, AppEffect::DocumentChanged { .. }));
        assert_eq!(
            coordinator.view().generation.value(),
            before_generation.value() + 1
        );
        assert_eq!(&*coordinator.snapshot().text, "前 $x$ 中 $$y$$ 后");

        coordinator.dispatch(AppIntent::Undo).unwrap();
        assert_eq!(&*coordinator.snapshot().text, source);
        coordinator.dispatch(AppIntent::Redo).unwrap();
        assert_eq!(&*coordinator.snapshot().text, "前 $x$ 中 $$y$$ 后");
    }

    #[test]
    fn phase11b_math_delimiter_selection_scope_and_no_match_are_transactional() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        let source = "\\(a\\) xx \\(b\\) yy";
        edit(&mut coordinator, source);
        let start = source.find("\\(b").unwrap();
        let end = start + "\\(b\\)".len();
        coordinator
            .dispatch(AppIntent::ConvertLatexMathDelimiters {
                expected_generation: coordinator.view().generation,
                selection: Selection::new(end, start),
                scope_to_selection: true,
                timestamp_ms: 20,
            })
            .unwrap();
        assert_eq!(&*coordinator.snapshot().text, "\\(a\\) xx $b$ yy");

        let before = coordinator.snapshot();
        let outcome = coordinator
            .dispatch(AppIntent::ConvertLatexMathDelimiters {
                expected_generation: before.generation,
                selection: Selection::caret(0),
                scope_to_selection: true,
                timestamp_ms: 21,
            })
            .unwrap();
        assert_eq!(outcome, AppEffect::NoOp);
        assert_eq!(coordinator.snapshot(), before);
    }

    #[test]
    fn phase14_literal_replace_is_stale_safe_and_undoable() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        edit(&mut coordinator, "Rust rust");
        let generation = coordinator.view().generation;
        coordinator
            .dispatch(AppIntent::ReplaceLiteralMatch {
                expected_generation: generation,
                range: 5..9,
                query: "rust".into(),
                replacement: "中文".into(),
                options: crate::instruction::LiteralSearchOptions {
                    case_sensitive: true,
                },
                timestamp_ms: 30,
            })
            .unwrap();
        assert_eq!(&*coordinator.snapshot().text, "Rust 中文");
        coordinator.dispatch(AppIntent::Undo).unwrap();
        assert_eq!(&*coordinator.snapshot().text, "Rust rust");

        let before = coordinator.snapshot();
        let result = coordinator.dispatch(AppIntent::ReplaceLiteralMatch {
            expected_generation: before.generation,
            range: 0..4,
            query: "different".into(),
            replacement: "x".into(),
            options: crate::instruction::LiteralSearchOptions::default(),
            timestamp_ms: 31,
        });
        assert_eq!(result, Err(EditorFlowError::SearchRangeMismatch));
        assert_eq!(coordinator.snapshot(), before);
    }

    #[test]
    fn phase14_literal_replace_all_is_one_generation_and_one_undo_step() {
        let mut coordinator = EditorCoordinator::empty(MockClipboard::default());
        edit(&mut coordinator, "Rust rust RUST");
        let before = coordinator.view().generation;
        coordinator
            .dispatch(AppIntent::ReplaceAllLiteral {
                expected_generation: before,
                query: "rust".into(),
                replacement: "R".into(),
                options: crate::instruction::LiteralSearchOptions {
                    case_sensitive: false,
                },
                timestamp_ms: 40,
            })
            .unwrap();
        assert_eq!(coordinator.view().generation.value(), before.value() + 1);
        assert_eq!(&*coordinator.snapshot().text, "R R R");
        coordinator.dispatch(AppIntent::Undo).unwrap();
        assert_eq!(&*coordinator.snapshot().text, "Rust rust RUST");
    }
}
