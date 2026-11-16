//! The shell's part of the notices: it reports what happened to the session, draws the view the
//! session gives back, and hands clicks on its buttons and on the toolbar marker back. It decides
//! no class, duration or priority (plan 04 #notice-state).
//!
//! plan_ref: docs/plan/09_windows_shell.md#notices

use stickymd_app::flow::notice::Notice;
use stickymd_app::instruction::PersistenceIntent;

use super::DesktopShell;
use crate::controls::{NoticeBarLayout, NoticeRect};
use crate::host::Host;

impl<H: Host> DesktopShell<H> {
    /// Reports `notice` to the session and redraws, so the bar shows it.
    pub(super) fn notify(&mut self, notice: Notice) {
        self.app.notify(notice);
        self.request_redraw();
    }

    /// Where the notice bar is in the window now; `None` while nothing shows or without a window.
    pub(super) fn notice_bar_layout(&self) -> Option<NoticeBarLayout> {
        let view = self.app.notice_view()?;
        let (size, dpi) = self.shell_view()?;
        Some(NoticeBarLayout::new(size, dpi as f32, view.choices))
    }

    /// A left press on a button of the bar gives that choice, as F6 or F7 would. Returns whether
    /// the press was taken; a press elsewhere on the bar goes on to the pane under it.
    pub(super) fn press_notice_button(&mut self) -> bool {
        let Some(choice) = self
            .notice_bar_layout()
            .and_then(|layout| layout.choice_at(self.cursor_position.x, self.cursor_position.y))
        else {
            return false;
        };
        self.dispatch_persistence_intent(if choice.is_primary() {
            PersistenceIntent::ResolvePrimary
        } else {
            PersistenceIntent::ResolveSecondary
        })
    }

    /// Whether the bar shows over any part of `rect`: a partial redraw there would paint over it.
    pub(super) fn notice_bar_covers(&self, rect: NoticeRect) -> bool {
        self.notice_bar_layout()
            .is_some_and(|layout| layout.bar.intersects(rect))
    }

    /// The toolbar marker was clicked: the save failure shows again for 30 s.
    pub(super) fn show_save_fault(&mut self) {
        if self.app.show_save_fault() {
            self.request_redraw();
        }
    }
}
