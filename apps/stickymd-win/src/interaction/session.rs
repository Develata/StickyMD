//! Non-authoritative editor session and IME composition state.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#ime-semantics

use std::ops::Range;

use stickymd_core::{EditKind, Generation, Selection};
use stickymd_render::source::PreeditVisual;

use super::navigation::{
    logical_line_end, logical_line_start, next_grapheme_boundary, previous_grapheme_boundary,
};
use crate::instruction::AppIntent;

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScrollState {
    pub line: usize,
    pub vertical_px: f32,
    pub horizontal_px: f32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeCompositionState {
    Inactive,
    Enabled,
    Preediting {
        text: String,
        cursor: Option<Range<usize>>,
        replacement: Selection,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImeSignal {
    Enabled,
    Disabled,
    Preedit {
        text: String,
        cursor: Option<Range<usize>>,
    },
    Commit(String),
}

#[derive(Debug, Clone)]
pub struct EditorSession {
    pub selection: Selection,
    pub scroll: ScrollState,
    pub preferred_x: Option<f32>,
    pub ime: ImeCompositionState,
    pub dragging_selection: bool,
    pub caret_visible: bool,
    pub focused: bool,
}

impl Default for EditorSession {
    fn default() -> Self {
        Self {
            selection: Selection::caret(0),
            scroll: ScrollState::default(),
            preferred_x: None,
            ime: ImeCompositionState::Inactive,
            dragging_selection: false,
            caret_visible: true,
            focused: false,
        }
    }
}

impl EditorSession {
    pub fn is_composing(&self) -> bool {
        matches!(self.ime, ImeCompositionState::Preediting { .. })
    }

    pub fn preedit_visual(&self) -> Option<PreeditVisual> {
        match &self.ime {
            ImeCompositionState::Preediting {
                text,
                cursor,
                replacement,
            } => Some(PreeditVisual {
                text: text.clone(),
                cursor: cursor.clone(),
                replacement: *replacement,
            }),
            ImeCompositionState::Inactive | ImeCompositionState::Enabled => None,
        }
    }

    pub fn handle_ime(
        &mut self,
        signal: ImeSignal,
        generation: Generation,
        timestamp_ms: u64,
    ) -> Option<AppIntent> {
        match signal {
            ImeSignal::Enabled => {
                self.ime = ImeCompositionState::Enabled;
                None
            }
            ImeSignal::Disabled => {
                self.ime = ImeCompositionState::Inactive;
                None
            }
            ImeSignal::Preedit { text, cursor } => {
                if text.is_empty() {
                    self.ime = ImeCompositionState::Enabled;
                    return None;
                }
                let replacement = match &self.ime {
                    ImeCompositionState::Preediting { replacement, .. } => *replacement,
                    ImeCompositionState::Inactive | ImeCompositionState::Enabled => self.selection,
                };
                let cursor = valid_preedit_cursor(&text, cursor);
                self.ime = ImeCompositionState::Preediting {
                    text,
                    cursor,
                    replacement,
                };
                None
            }
            ImeSignal::Commit(text) => {
                let replacement = match &self.ime {
                    ImeCompositionState::Preediting { replacement, .. } => *replacement,
                    ImeCompositionState::Inactive | ImeCompositionState::Enabled => self.selection,
                };
                self.ime = ImeCompositionState::Enabled;
                if text.is_empty() {
                    return None;
                }
                Some(edit_intent(
                    generation,
                    replacement,
                    text,
                    EditKind::ImeCommit,
                    timestamp_ms,
                ))
            }
        }
    }

    pub fn handle_keyboard_text(
        &mut self,
        text: &str,
        generation: Generation,
        timestamp_ms: u64,
    ) -> Option<AppIntent> {
        if self.is_composing() || text.is_empty() {
            return None;
        }
        Some(edit_intent(
            generation,
            self.selection,
            text.to_owned(),
            EditKind::Typing,
            timestamp_ms,
        ))
    }

    pub fn insert(
        &self,
        generation: Generation,
        text: impl Into<String>,
        kind: EditKind,
        timestamp_ms: u64,
    ) -> AppIntent {
        edit_intent(generation, self.selection, text.into(), kind, timestamp_ms)
    }

    pub fn backspace(
        &self,
        text: &str,
        generation: Generation,
        timestamp_ms: u64,
    ) -> Option<AppIntent> {
        if self.is_composing() {
            return None;
        }
        let selection = if self.selection.is_collapsed() {
            let active = self.selection.active.byte;
            if active == 0 {
                return None;
            }
            Selection::new(previous_grapheme_boundary(text, active), active)
        } else {
            self.selection
        };
        Some(edit_intent(
            generation,
            selection,
            String::new(),
            if self.selection.is_collapsed() {
                EditKind::Backspace
            } else {
                EditKind::SelectionReplace
            },
            timestamp_ms,
        ))
    }

    pub fn delete_forward(
        &self,
        text: &str,
        generation: Generation,
        timestamp_ms: u64,
    ) -> Option<AppIntent> {
        if self.is_composing() {
            return None;
        }
        let selection = if self.selection.is_collapsed() {
            let active = self.selection.active.byte;
            let next = next_grapheme_boundary(text, active);
            if next == active {
                return None;
            }
            Selection::new(active, next)
        } else {
            self.selection
        };
        Some(edit_intent(
            generation,
            selection,
            String::new(),
            if self.selection.is_collapsed() {
                EditKind::DeleteForward
            } else {
                EditKind::SelectionReplace
            },
            timestamp_ms,
        ))
    }

    pub fn move_horizontal(&mut self, text: &str, direction: i32, extend: bool) {
        if self.is_composing() {
            return;
        }
        let target = if !extend && !self.selection.is_collapsed() {
            if direction < 0 {
                self.selection.start()
            } else {
                self.selection.end()
            }
        } else if direction < 0 {
            previous_grapheme_boundary(text, self.selection.active.byte)
        } else {
            next_grapheme_boundary(text, self.selection.active.byte)
        };
        self.move_to(target, extend);
    }

    pub fn move_line_boundary(&mut self, text: &str, end: bool, extend: bool) {
        if self.is_composing() {
            return;
        }
        let active = self.selection.active.byte;
        let target = if end {
            logical_line_end(text, active)
        } else {
            logical_line_start(text, active)
        };
        self.move_to(target, extend);
    }

    pub fn move_document_boundary(&mut self, text_len: usize, end: bool, extend: bool) {
        if !self.is_composing() {
            self.move_to(if end { text_len } else { 0 }, extend);
        }
    }

    pub fn move_to(&mut self, byte: usize, extend: bool) {
        self.selection = if extend {
            Selection::new(self.selection.anchor.byte, byte)
        } else {
            Selection::caret(byte)
        };
        self.preferred_x = None;
        self.caret_visible = true;
    }

    pub fn cancel_preedit(&mut self) {
        self.ime = match self.ime {
            ImeCompositionState::Inactive => ImeCompositionState::Inactive,
            ImeCompositionState::Enabled | ImeCompositionState::Preediting { .. } => {
                ImeCompositionState::Enabled
            }
        };
    }

    pub fn accept_document_selection(&mut self, selection: Selection) {
        self.selection = selection;
        self.preferred_x = None;
        self.caret_visible = true;
    }
}

fn edit_intent(
    generation: Generation,
    selection: Selection,
    inserted: String,
    kind: EditKind,
    timestamp_ms: u64,
) -> AppIntent {
    AppIntent::Edit {
        expected_generation: generation,
        selection,
        inserted,
        kind,
        timestamp_ms,
    }
}

fn valid_preedit_cursor(text: &str, cursor: Option<Range<usize>>) -> Option<Range<usize>> {
    cursor.filter(|range| {
        range.start <= range.end
            && range.end <= text.len()
            && text.is_char_boundary(range.start)
            && text.is_char_boundary(range.end)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preedit_updates_only_session_and_commit_emits_one_intent() {
        let generation = Generation::initial();
        let mut session = EditorSession {
            selection: Selection::new(0, 3),
            ..EditorSession::default()
        };
        assert_eq!(session.handle_ime(ImeSignal::Enabled, generation, 0), None);
        assert_eq!(
            session.handle_ime(
                ImeSignal::Preedit {
                    text: "nihao".to_owned(),
                    cursor: Some(5..5),
                },
                generation,
                1,
            ),
            None
        );
        let intent = session
            .handle_ime(ImeSignal::Commit("你好".to_owned()), generation, 2)
            .unwrap();
        assert!(matches!(
            intent,
            AppIntent::Edit {
                selection,
                kind: EditKind::ImeCommit,
                ..
            } if selection == Selection::new(0, 3)
        ));
        assert!(!session.is_composing());
    }

    #[test]
    fn keyboard_text_after_commit_is_not_guessed_to_be_a_duplicate() {
        let generation = Generation::initial();
        let mut session = EditorSession::default();
        session.handle_ime(ImeSignal::Enabled, generation, 0);
        session.handle_ime(
            ImeSignal::Preedit {
                text: "nihao".to_owned(),
                cursor: None,
            },
            generation,
            1,
        );
        assert!(
            session
                .handle_keyboard_text("nihao", generation, 2)
                .is_none()
        );
        assert!(
            session
                .handle_ime(ImeSignal::Commit("你好".to_owned()), generation, 3)
                .is_some()
        );
        assert!(
            session
                .handle_keyboard_text("你好", generation, 4)
                .is_some()
        );
    }

    #[test]
    fn empty_preedit_cancels_without_intent() {
        let generation = Generation::initial();
        let mut session = EditorSession::default();
        session.handle_ime(ImeSignal::Enabled, generation, 0);
        session.handle_ime(
            ImeSignal::Preedit {
                text: "x".to_owned(),
                cursor: None,
            },
            generation,
            1,
        );
        assert_eq!(
            session.handle_ime(
                ImeSignal::Preedit {
                    text: String::new(),
                    cursor: None,
                },
                generation,
                2,
            ),
            None
        );
        assert_eq!(session.ime, ImeCompositionState::Enabled);
    }

    #[test]
    fn backspace_and_delete_remove_complete_graphemes() {
        for grapheme in ["a", "中", "é", "e\u{301}", "🙂", "👨‍👩‍👧‍👦", "🇨🇳"]
        {
            let backspace = EditorSession {
                selection: Selection::caret(grapheme.len()),
                ..EditorSession::default()
            }
            .backspace(grapheme, Generation::initial(), 0)
            .unwrap();
            assert!(matches!(
                backspace,
                AppIntent::Edit { selection, .. }
                    if selection.normalized_range() == (0..grapheme.len())
            ));

            let delete = EditorSession::default()
                .delete_forward(grapheme, Generation::initial(), 0)
                .unwrap();
            assert!(matches!(
                delete,
                AppIntent::Edit { selection, .. }
                    if selection.normalized_range() == (0..grapheme.len())
            ));
        }
    }

    #[test]
    fn movement_preserves_or_extends_selection_direction() {
        let text = "a中🙂";
        let mut session = EditorSession {
            selection: Selection::caret(text.len()),
            ..EditorSession::default()
        };
        session.move_horizontal(text, -1, true);
        assert_eq!(session.selection, Selection::new(text.len(), 4));
        session.move_horizontal(text, -1, true);
        assert_eq!(session.selection, Selection::new(text.len(), 1));
        session.move_horizontal(text, -1, false);
        assert_eq!(session.selection, Selection::caret(1));
    }
}
