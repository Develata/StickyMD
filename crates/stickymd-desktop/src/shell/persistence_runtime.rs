//! Persistence intents and worker receipts of the native shell.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use super::DesktopShell;
use crate::host::Host;
use stickymd_app::execution::IoCompletion;
use stickymd_app::instruction::PersistenceIntent;

impl<H: Host> DesktopShell<H> {
    /// The end of a complete main-thread event pass: after this pass's autosave, external check,
    /// preview and window ticks have scheduled their work. The quit-cancelled notice lasts only
    /// while the quit barrier still needs the note (plan 09); the title then shows the settled
    /// state.
    pub(super) fn finish_event_pass(&mut self) {
        let note_barrier_required = self.window_guards().requires_note_barrier();
        self.quit_notice.settle(note_barrier_required);
        self.update_window_title();
    }

    /// The only writer of the window title (plan 09 title priority).
    pub(super) fn update_window_title(&mut self) {
        let Some(window) = &self.window else { return };
        let title = crate::title::window_title(crate::title::TitleFacts {
            recovery_pending: self.app.recovery().is_pending(),
            quit_cancelled: self.quit_notice.active(),
            conflict: self.app.persistence().conflict().is_some(),
            dirty: self.app.coordinator().view().dirty,
            formula_error: self.formula_error_under_pointer(),
        });
        if title != self.shown_title {
            window.set_title(&title);
            self.shown_title = title.into_owned();
        }
    }

    pub(super) fn dispatch_persistence_intent(&mut self, intent: PersistenceIntent) -> bool {
        match intent {
            PersistenceIntent::SaveNow(reason) => {
                self.with_session(|app, _, out| app.save_now(reason, out));
                true
            }
            PersistenceIntent::Export => {
                self.request_export();
                true
            }
            PersistenceIntent::ResolvePrimary => self.handle_resolution_key(true),
            PersistenceIntent::ResolveSecondary => self.handle_resolution_key(false),
        }
    }

    /// Whether a conflict or a recovery choice consumed the key.
    pub(super) fn handle_resolution_key(&mut self, primary: bool) -> bool {
        let mut consumed = false;
        self.with_session(|app, _, out| consumed = app.resolve(primary, out));
        consumed
    }

    pub(super) fn handle_io_completion(&mut self, completion: IoCompletion) {
        self.with_session(|app, view, out| app.handle_io(completion, view, out));
    }
}
