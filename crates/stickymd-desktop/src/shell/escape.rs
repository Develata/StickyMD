//! Reads the state of the Esc order and applies what `crate::input` decides.
//!
//! plan_ref: docs/plan/09_windows_shell.md#dock-auto-hide

use crate::input::{EscapeAction, EscapeContext, escape_action};

use super::DesktopShell;
use crate::host::Host;

impl<H: Host> DesktopShell<H> {
    pub(super) fn handle_escape(&mut self) {
        let context = EscapeContext {
            opacity_popup_open: self.controls.opacity_popup_open,
            recovery_pending: self.app.recovery().is_pending(),
            asset_reconcile_pending: self.app.assets().reconcile_pending,
            accepts_editor_mutation: self.window_accepts_editor_mutation(),
            editor_composing: self.app.editor.is_composing(),
            search_open: self.app.search.open,
            search_composing: self.app.search.is_composing(),
            preview_focused: self.preview_focused,
            preview_selection: !self.preview_selection.is_collapsed(),
        };
        match escape_action(context) {
            EscapeAction::CancelOpacityPopup => self.cancel_opacity_popup(),
            EscapeAction::Ignore | EscapeAction::SearchComposition => {}
            EscapeAction::CancelEditorPreedit => {
                self.app.editor.cancel_preedit();
                self.after_presentation_change();
            }
            EscapeAction::CloseSearch => self.close_search(),
            EscapeAction::ClearPreviewSelection => self.clear_preview_selection(),
            EscapeAction::Window => self.dispatch_escape_to_window(),
        }
    }
}
