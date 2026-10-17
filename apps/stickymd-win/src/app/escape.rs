//! The Esc decision. Every Esc key-down, whatever the focus, field or modifiers, is resolved
//! here in the plan 09 order, so no input state can add a second, differently ordered path.
//!
//! plan_ref: docs/plan/09_windows_shell.md#dock-auto-hide

use winit::keyboard::{KeyCode, ModifiersState};

use super::StickyApp;

/// What one Esc key-down does, by the first applicable row of the plan 09 order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EscapeAction {
    /// 1. Close the opacity popup; an uncommitted slider or numeric value is dropped.
    CancelOpacityPopup,
    /// 2. Recovery pending, asset reconciliation running, or the window not accepting edits.
    Ignore,
    /// 3. An editor composition: cancel only the preedit.
    CancelEditorPreedit,
    /// 3. A Search-field composition: the IME cancels its own composition on Esc and Search
    ///    follows the empty preedit it reports, so nothing else may react.
    SearchComposition,
    /// 4. Close Search.
    CloseSearch,
    /// 5. Clear the Preview selection.
    ClearPreviewSelection,
    /// 6. Collapse by the dock rule (a Floating window does not collapse).
    Window,
}

/// The state the Esc order reads.
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct EscapeContext {
    pub(super) opacity_popup_open: bool,
    pub(super) recovery_pending: bool,
    pub(super) asset_reconcile_pending: bool,
    pub(super) accepts_editor_mutation: bool,
    pub(super) editor_composing: bool,
    pub(super) search_open: bool,
    pub(super) search_composing: bool,
    pub(super) preview_focused: bool,
    pub(super) preview_selection: bool,
}

pub(super) fn escape_action(context: EscapeContext) -> EscapeAction {
    if context.opacity_popup_open {
        EscapeAction::CancelOpacityPopup
    } else if context.recovery_pending
        || context.asset_reconcile_pending
        || !context.accepts_editor_mutation
    {
        EscapeAction::Ignore
    } else if context.editor_composing {
        EscapeAction::CancelEditorPreedit
    } else if context.search_open && context.search_composing {
        EscapeAction::SearchComposition
    } else if context.search_open {
        EscapeAction::CloseSearch
    } else if context.preview_focused && context.preview_selection {
        EscapeAction::ClearPreviewSelection
    } else {
        EscapeAction::Window
    }
}

/// Whether a key-down is Esc. Modifiers (Shift, Ctrl, Alt) never change it, and each key-down
/// of a held key is decided on its own (plan 09).
pub(super) fn is_escape(code: Option<KeyCode>, _modifiers: ModifiersState) -> bool {
    code == Some(KeyCode::Escape)
}

impl StickyApp {
    pub(super) fn handle_escape(&mut self) {
        let context = EscapeContext {
            opacity_popup_open: self.controls.opacity_popup_open,
            recovery_pending: self.recovery.is_pending(),
            asset_reconcile_pending: self.asset_reconcile_pending,
            accepts_editor_mutation: self.window_accepts_editor_mutation(),
            editor_composing: self.session.is_composing(),
            search_open: self.search.open,
            search_composing: self.search.is_composing(),
            preview_focused: self.preview_focused,
            preview_selection: !self.preview_selection.is_collapsed(),
        };
        match escape_action(context) {
            EscapeAction::CancelOpacityPopup => self.cancel_opacity_popup(),
            EscapeAction::Ignore | EscapeAction::SearchComposition => {}
            EscapeAction::CancelEditorPreedit => {
                self.session.cancel_preedit();
                self.after_presentation_change();
            }
            EscapeAction::CloseSearch => self.close_search(),
            EscapeAction::ClearPreviewSelection => self.clear_preview_selection(),
            EscapeAction::Window => self.dispatch_escape_to_window(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A context in which Esc reaches the window: input accepted, nothing open or composing.
    fn idle() -> EscapeContext {
        EscapeContext {
            accepts_editor_mutation: true,
            ..EscapeContext::default()
        }
    }

    /// Clears the condition of the row that just won.
    type Clear = fn(&mut EscapeContext);

    #[test]
    fn the_first_applicable_row_wins_in_plan_order() {
        // Every row's condition at once; clearing the winning one reveals the next.
        let mut context = EscapeContext {
            opacity_popup_open: true,
            recovery_pending: true,
            asset_reconcile_pending: true,
            accepts_editor_mutation: false,
            editor_composing: true,
            search_open: true,
            search_composing: true,
            preview_focused: true,
            preview_selection: true,
        };
        let steps: [(EscapeAction, Clear); 7] = [
            (EscapeAction::CancelOpacityPopup, |c| {
                c.opacity_popup_open = false
            }),
            (EscapeAction::Ignore, |c| {
                c.recovery_pending = false;
                c.asset_reconcile_pending = false;
                c.accepts_editor_mutation = true;
            }),
            (EscapeAction::CancelEditorPreedit, |c| {
                c.editor_composing = false
            }),
            (EscapeAction::SearchComposition, |c| {
                c.search_composing = false
            }),
            (EscapeAction::CloseSearch, |c| c.search_open = false),
            (EscapeAction::ClearPreviewSelection, |c| {
                c.preview_selection = false
            }),
            (EscapeAction::Window, |_| {}),
        ];
        for (expected, clear) in steps {
            assert_eq!(escape_action(context), expected, "{context:?}");
            clear(&mut context);
        }
    }

    /// Each guard of row 2 alone stops Esc, including the states the exact layer cannot
    /// produce reliably: asset reconciliation in progress and a quit in progress (the window no
    /// longer accepts edits while quitting).
    #[test]
    fn each_guard_alone_ignores_esc() {
        for guarded in [
            EscapeContext {
                recovery_pending: true,
                ..idle()
            },
            EscapeContext {
                asset_reconcile_pending: true,
                ..idle()
            },
            EscapeContext {
                accepts_editor_mutation: false,
                ..idle()
            },
        ] {
            assert_eq!(escape_action(guarded), EscapeAction::Ignore, "{guarded:?}");
            // The popup row still comes first.
            let popup = EscapeContext {
                opacity_popup_open: true,
                ..guarded
            };
            assert_eq!(escape_action(popup), EscapeAction::CancelOpacityPopup);
        }
    }

    #[test]
    fn an_editor_preedit_owns_esc_even_with_preview_focused() {
        let leftover = EscapeContext {
            editor_composing: true,
            preview_focused: true,
            preview_selection: true,
            ..idle()
        };
        assert_eq!(escape_action(leftover), EscapeAction::CancelEditorPreedit);
    }

    #[test]
    fn preview_without_a_selection_reaches_the_window() {
        let focused = EscapeContext {
            preview_focused: true,
            ..idle()
        };
        assert_eq!(escape_action(focused), EscapeAction::Window);
        // A selection outside a focused Preview does not consume Esc.
        let unfocused = EscapeContext {
            preview_selection: true,
            ..idle()
        };
        assert_eq!(escape_action(unfocused), EscapeAction::Window);
    }

    #[test]
    fn modifiers_never_change_esc() {
        let combinations = [
            ModifiersState::empty(),
            ModifiersState::SHIFT,
            ModifiersState::CONTROL,
            ModifiersState::ALT,
            ModifiersState::SHIFT | ModifiersState::CONTROL,
            ModifiersState::all(),
        ];
        for modifiers in combinations {
            assert!(is_escape(Some(KeyCode::Escape), modifiers), "{modifiers:?}");
            assert!(!is_escape(Some(KeyCode::KeyE), modifiers));
        }
        assert!(!is_escape(None, ModifiersState::CONTROL));
    }
}
