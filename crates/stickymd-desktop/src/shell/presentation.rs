//! Window presentation and source-surface projection helpers.
//!
//! plan_ref: docs/plan/03_system_architecture.md#interaction-shell

use std::time::Instant;

use winit::dpi::{PhysicalPosition, PhysicalSize};

use super::{CARET_BLINK, DesktopShell};
use crate::controls::ControlLayout;
use crate::host::Host;
use crate::search::search_field_display;
use stickymd_app::config::ViewMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PendingRedraw {
    None,
    CaretOnly,
    Full,
}

impl<H: Host> DesktopShell<H> {
    pub(super) fn request_redraw(&mut self) {
        self.pending_redraw = PendingRedraw::Full;
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }

    fn reset_caret_blink(&mut self) {
        self.app.editor.caret_visible = true;
        self.next_blink = Instant::now() + CARET_BLINK;
    }

    /// Whether the source caret is both meaningful and visible enough to
    /// justify a periodic redraw. A collapsed edge sensor may retain native
    /// keyboard focus, but it is not an editable source surface.
    pub(super) fn caret_animation_active(&self) -> bool {
        self.app.editor.focused
            && !self.preview_focused
            && !self.app.search.open
            && !self.app.editor.is_composing()
            && self.app.window_accepts_mutation()
    }

    pub(super) fn sync_preedit(&mut self) {
        if let Some(projection) = &mut self.projection {
            projection.set_preedit(self.app.editor.preedit_visual());
        }
    }

    pub(super) fn update_ime_area(&mut self) {
        let Some(window) = self.window.as_ref().cloned() else {
            return;
        };
        if self.app.search.open
            && let Some(layout) = self.search_layout()
        {
            let scale = window.scale_factor() as f32;
            let focused = self.app.search.focused;
            let (display, cursor) = search_field_display(&self.app.search, focused);
            let spec = layout.field_spec(focused, scale);
            if let Some(caret) = self
                .projection
                .as_mut()
                .and_then(|projection| projection.ui_text_field_caret(&display, cursor, spec))
            {
                window.set_ime_cursor_area(
                    PhysicalPosition::new(caret.x.round() as i32, caret.y.round() as i32),
                    PhysicalSize::new(
                        caret.width.max(1.0).round() as u32,
                        caret.height.max(1.0).round() as u32,
                    ),
                );
            }
            return;
        }
        let origin = self
            .view_geometry()
            .and_then(|geometry| geometry.source)
            .map_or((0, 0), |pane| (pane.x, pane.y));
        let Some(projection) = &mut self.projection else {
            return;
        };
        if let Some(caret) = projection.ime_caret_rect(self.app.editor.selection.active.byte) {
            window.set_ime_cursor_area(
                PhysicalPosition::new(
                    origin.0 as i32 + caret.x.round() as i32,
                    origin.1 as i32 + caret.y.round() as i32,
                ),
                PhysicalSize::new(
                    caret.width.max(1.0).round() as u32,
                    caret.height.max(1.0).round() as u32,
                ),
            );
        }
    }

    pub(super) fn after_presentation_change(&mut self) {
        if self.app.config().current().view_mode != ViewMode::Preview
            && let Some(projection) = &mut self.projection
        {
            let _ = projection.ensure_caret_visible(self.app.editor.selection.active.byte);
            let scroll = projection.scroll();
            self.app.editor.scroll.line = scroll.line;
            self.app.editor.scroll.vertical_px = scroll.vertical;
            self.app.editor.scroll.horizontal_px = scroll.horizontal;
        }
        self.sync_preedit();
        self.reset_caret_blink();
        self.update_ime_area();
        self.request_redraw();
    }

    /// Lays the shell out for the window's size and DPI and the view. Runs when any of them
    /// changes: the window's creation, every resize (a DPI change comes through `resize` too) and a
    /// view change. A surface that failed to resize keeps showing the old frame, so the layout
    /// stays with it and a press lands on what is on screen.
    pub(super) fn sync_control_layout(&mut self) {
        let mode = self.app.config().current().view_mode;
        self.control_layout = self
            .shell_view()
            .map(|(size, dpi)| ControlLayout::new(mode, size, dpi));
    }

    pub(super) fn resize(&mut self, size: PhysicalSize<u32>) {
        if size.width == 0 || size.height == 0 {
            return;
        }
        self.cancel_scrollbar_drag();
        if let Some(surface) = &mut self.surface
            && let Err(error) = surface.resize(size.width, size.height)
        {
            self.diagnostic = Some(error.to_string());
        } else {
            self.sync_control_layout();
        }
        // Resize events are projection facts. Phase 8 commits durable geometry
        // once at the stable user move/resize boundary, never once per frame.
        self.configure_viewports();
        self.request_preview_relayout();
        self.update_ime_area();
        self.request_redraw();
    }
}
