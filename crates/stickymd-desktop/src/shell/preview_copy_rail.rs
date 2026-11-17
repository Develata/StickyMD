//! The copy rails over the shown Preview frame: hover, press, the click/drag threshold and the
//! copy. Render produces each frame's rails and what they copy; this module only routes the
//! pointer to them (plan 06 #preview-copy-rail).
//!
//! A rail belongs to the frame on screen, not to the document: it is hit, hovered and copied on
//! the frame that is blitted, even while that frame lags the document (rule 4 of plan 06
//! #preview-copy-semantics), unlike a selection press, which waits for a current frame.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-rail

use stickymd_app::instruction::AppIntent;
use stickymd_render::preview::{PreviewCopyRail, PreviewFrame};
use winit::dpi::PhysicalPosition;

use super::DesktopShell;
use crate::geometry::PaneRect;
use crate::host::Host;
use crate::preview::{meaningful_preview_drag, pane_local};

#[derive(Debug, Default)]
pub(super) struct CopyRailInteraction {
    /// The rail painted hovered in the last frame; only a change of it asks for a redraw.
    pub hovered: Option<PreviewCopyRail>,
    /// A left press that began on a rail.
    pub press: Option<RailPress>,
}

#[derive(Debug)]
pub(super) struct RailPress {
    /// What the rail copies, read at the press from the frame then shown, so a frame arriving
    /// before the release cannot change it.
    text: Option<String>,
    start: PhysicalPosition<f64>,
    /// Moved past the click/drag threshold: the release copies nothing.
    moved: bool,
}

impl<H: Host> DesktopShell<H> {
    /// The Preview frame on screen and the pane it is blitted into: the frame the window draws,
    /// whatever its generation (`render.rs`).
    fn shown_preview_frame(&self) -> Option<(&PreviewFrame, PaneRect)> {
        let pane = self.view_geometry()?.preview?;
        self.preview_frame
            .as_ref()
            .filter(|frame| frame.width() == pane.width && frame.height() == pane.height)
            .map(|frame| (frame, pane))
    }

    /// The rail under the pointer, if nothing drawn over the Preview covers it and no other drag
    /// owns the pointer. Derived on demand, so a new frame, a scroll or a layout change cannot
    /// leave a stale answer.
    pub(super) fn copy_rail_under_pointer(&self) -> Option<PreviewCopyRail> {
        if !self.pointer_inside_window
            || self.preview_dragging
            || self.app.editor.dragging_selection
            || self.preview_horizontal.drag.is_some()
            || self.scrollbars.drag.is_some()
            || self.shell_cursor_icon().is_some()
            || self.notice_bar_cursor().is_some()
        {
            return None;
        }
        let (frame, pane) = self.shown_preview_frame()?;
        let (x, y) = pane_local(pane, self.cursor_position)?;
        frame.copy_rail_at(x, y + frame.scroll_y())
    }

    /// Takes a left press on a rail before it can start a selection. `true` when it did.
    pub(super) fn press_copy_rail(&mut self) -> bool {
        let Some(rail) = self.copy_rail_under_pointer() else {
            return false;
        };
        let text = self
            .shown_preview_frame()
            .and_then(|(frame, _)| frame.copy_rail_text(rail));
        self.copy_rail.press = Some(RailPress {
            text,
            start: self.cursor_position,
            moved: false,
        });
        true
    }

    /// A move during a rail press: past the threshold the press is a drag, which neither copies
    /// nor selects. `true` while a rail press owns the pointer.
    pub(super) fn move_copy_rail_press(&mut self, position: PhysicalPosition<f64>) -> bool {
        let scale = self.shell_dpi().unwrap_or(1.0);
        let Some(press) = &mut self.copy_rail.press else {
            return false;
        };
        press.moved |= meaningful_preview_drag(press.start, position, scale);
        true
    }

    /// Ends a rail press; a click that stayed within the threshold copies. `true` when a rail
    /// press was ended.
    pub(super) fn release_copy_rail(&mut self) -> bool {
        let Some(press) = self.copy_rail.press.take() else {
            return false;
        };
        if !press.moved
            && let Some(text) = press.text
        {
            self.dispatch(AppIntent::WriteClipboard { text });
        }
        true
    }

    /// Asks for one redraw when the rail under the pointer is not the one last painted hovered.
    pub(super) fn refresh_copy_rail_hover(&mut self) {
        if self.copy_rail_under_pointer() != self.copy_rail.hovered {
            self.request_redraw();
        }
    }
}
