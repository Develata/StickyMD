//! Pane scroll input, scrollbar gestures and semantic split synchronization.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars
//! plan_ref: docs/plan/06_markdown_math_rendering.md#split-scroll-sync

use winit::event::{ElementState, MouseButton};
use winit::window::CursorIcon;

use super::StickyApp;
use super::scrollbar::{
    DragUpdate, PaneScroll, PressOutcome, ScrollPane, Scrollbar, gutter_at, page_pixels, pane_bars,
    preview_scroll,
};
use crate::config::ViewMode;

impl StickyApp {
    pub(super) fn scrollbar_geometry(&mut self) -> [Option<Scrollbar>; 2] {
        let accepts_input = self.window_accepts_editor_mutation();
        let Some(geometry) = accepts_input.then(|| self.view_geometry()).flatten() else {
            return [None, None];
        };
        let preview_current = self.preview_layout_is_current();
        let dpi = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);
        let source = geometry
            .source
            .zip(self.projection.as_mut())
            .map(|(content, projection)| {
                let metrics = projection.scroll_metrics();
                PaneScroll {
                    content,
                    fraction: metrics.fraction,
                    visible: metrics.viewport_fraction,
                }
            });
        let preview =
            geometry
                .preview
                .zip(self.preview_frame.as_ref())
                .and_then(|(content, frame)| {
                    preview_scroll(
                        content,
                        preview_current,
                        frame.document_height(),
                        self.preview_scroll_y,
                    )
                });
        pane_bars(
            accepts_input,
            dpi,
            geometry.divider_x.is_some(),
            source,
            preview,
        )
    }

    pub(super) fn scrollbar_gutter_at_cursor(&self) -> Option<ScrollPane> {
        let geometry = self.view_geometry()?;
        let dpi = self.window.as_ref()?.scale_factor();
        gutter_at(
            self.cursor_position,
            [
                (ScrollPane::Source, geometry.source),
                (ScrollPane::Preview, geometry.preview),
            ],
            dpi,
        )
    }

    pub(super) fn handle_scrollbar_button(
        &mut self,
        state: ElementState,
        button: MouseButton,
    ) -> bool {
        if button != MouseButton::Left {
            return false;
        }
        if state == ElementState::Released {
            if self.scrollbars.drag.is_some() {
                self.cancel_scrollbar_drag();
                return true;
            }
            return false;
        }
        let bars = self.scrollbar_geometry();
        if let Some(outcome) = self.scrollbars.press(bars, self.cursor_position) {
            match outcome {
                PressOutcome::DragStarted => self.refresh_window_guards(None),
                PressOutcome::Page { pane, direction } => {
                    // The pane height is read only for a confirmed track press.
                    let height = self
                        .view_geometry()
                        .and_then(|geometry| match pane {
                            ScrollPane::Source => geometry.source,
                            ScrollPane::Preview => geometry.preview,
                        })
                        .map_or(0, |content| content.height);
                    self.scroll_pane_by(pane, page_pixels(direction, height));
                }
            }
            self.request_redraw();
            return true;
        }
        // Empty gutter is not an editor click (including while a preview loads).
        self.scrollbar_gutter_at_cursor().is_some()
    }

    pub(super) fn handle_scrollbar_cursor(&mut self) -> bool {
        if !self.window_accepts_editor_mutation() {
            self.cancel_scrollbar_drag();
            return false;
        }
        if self.session.dragging_selection || self.preview_dragging {
            return false;
        }
        let bars = self.scrollbar_geometry();
        let gutter = self.scrollbar_gutter_at_cursor();
        let outcome = self
            .scrollbars
            .pointer_moved(bars, gutter, self.cursor_position.y);
        if outcome.hover_changed {
            self.request_redraw();
        }
        match outcome.drag {
            DragUpdate::None => false,
            DragUpdate::Lost => {
                self.refresh_window_guards(None);
                self.request_redraw();
                true
            }
            DragUpdate::ScrollTo { pane, fraction } => {
                if let Some(window) = &self.window {
                    window.set_cursor(CursorIcon::Default)
                }
                self.scroll_pane_to_fraction(pane, fraction);
                true
            }
        }
    }

    pub(super) fn cancel_scrollbar_drag(&mut self) {
        if self.scrollbars.end_drag() {
            self.refresh_window_guards(None);
            self.request_redraw();
        }
    }

    fn scroll_pane_to_fraction(&mut self, pane: ScrollPane, fraction: f64) {
        match pane {
            ScrollPane::Source => {
                if let Some(projection) = &mut self.projection {
                    projection.scroll_to_fraction(fraction);
                }
            }
            ScrollPane::Preview => {
                if let Some(frame) = &self.preview_frame {
                    self.preview_scroll_y = fraction as f32
                        * (frame.document_height() - frame.height() as f32).max(0.0);
                }
            }
        }
        self.after_pane_scroll(pane);
    }

    pub(super) fn scroll_pane_by(&mut self, pane: ScrollPane, pixels: f32) {
        match pane {
            ScrollPane::Source => {
                if let Some(projection) = &mut self.projection {
                    projection.scroll_by(pixels);
                }
            }
            ScrollPane::Preview => {
                let max = self.preview_frame.as_ref().map_or(0.0, |frame| {
                    (frame.document_height() - frame.height() as f32).max(0.0)
                });
                self.preview_scroll_y = (self.preview_scroll_y + pixels).clamp(0.0, max);
            }
        }
        self.after_pane_scroll(pane);
    }

    fn after_pane_scroll(&mut self, driver: ScrollPane) {
        let sync = self.config.current().view_mode == ViewMode::Split
            && self.config.current().split_scroll_sync;
        let preview_current = self.preview_layout_is_current();
        if sync
            && let (Some(frame), Some(projection)) = (
                self.preview_frame.as_ref().filter(|_| preview_current),
                &mut self.projection,
            )
        {
            match driver {
                ScrollPane::Source => {
                    if let Some(y) = frame.y_for_scroll_anchor(projection.scroll_anchor()) {
                        self.preview_scroll_y = y.clamp(
                            0.0,
                            (frame.document_height() - frame.height() as f32).max(0.0),
                        );
                    }
                }
                ScrollPane::Preview => {
                    if let Some(anchor) = frame.scroll_anchor_at_y(self.preview_scroll_y) {
                        let _ = projection.scroll_to_anchor(anchor);
                    }
                }
            }
        }
        if let Some(projection) = &self.projection {
            let scroll = projection.scroll();
            self.session.scroll.line = scroll.line;
            self.session.scroll.vertical_px = scroll.vertical;
            self.session.scroll.horizontal_px = scroll.horizontal;
        }
        if driver == ScrollPane::Preview || sync {
            self.request_preview_paint();
        }
        self.update_ime_area();
        self.request_redraw();
    }
}
