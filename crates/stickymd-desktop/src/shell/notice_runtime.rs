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

    /// A press on the bar is the bar's: it floats over what it covers (Search, a pane, a link),
    /// so nothing under it takes the press. A left press on a button gives that choice, as F6 or
    /// F7 would. Returns whether the press landed on the bar.
    pub(super) fn press_notice_bar(&mut self, left: bool) -> bool {
        let Some(layout) = self.notice_bar_layout() else {
            return false;
        };
        let (x, y) = (self.cursor_position.x, self.cursor_position.y);
        if !layout.bar.contains(x, y) {
            return false;
        }
        if left && let Some(choice) = layout.choice_at(x, y) {
            self.dispatch_persistence_intent(if choice.is_primary() {
                PersistenceIntent::ResolvePrimary
            } else {
                PersistenceIntent::ResolveSecondary
            });
        }
        true
    }

    /// The cursor over the bar: a pointer on its buttons, the arrow elsewhere on it.
    pub(super) fn notice_bar_cursor(&self) -> Option<winit::window::CursorIcon> {
        let layout = self.notice_bar_layout()?;
        let (x, y) = (self.cursor_position.x, self.cursor_position.y);
        layout.bar.contains(x, y).then(|| {
            if layout.choice_at(x, y).is_some() {
                winit::window::CursorIcon::Pointer
            } else {
                winit::window::CursorIcon::Default
            }
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
