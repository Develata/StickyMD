//! Pane scroll input, scrollbar gestures and semantic split synchronization.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars
//! plan_ref: docs/plan/06_markdown_math_rendering.md#split-scroll-sync

use winit::event::{ElementState, MouseButton};
use winit::window::CursorIcon;

use super::StickyApp;
use super::scrollbar::{GUTTER_DIP, ScrollPane, Scrollbar, ScrollbarPress};
use crate::config::ViewMode;

impl StickyApp {
    pub(super) fn scrollbar_geometry(&mut self) -> [Option<Scrollbar>; 2] {
        if !self.window_accepts_editor_mutation() {
            return [None, None];
        }
        let Some(geometry) = self.view_geometry() else {
            return [None, None];
        };
        let preview_current = self.preview_layout_is_current();
        let dpi = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);
        let source =
            geometry
                .source
                .zip(self.projection.as_mut())
                .and_then(|(pane, projection)| {
                    let metrics = projection.scroll_metrics();
                    Scrollbar::new(
                        ScrollPane::Source,
                        pane,
                        dpi,
                        metrics.fraction,
                        metrics.viewport_fraction,
                        geometry.divider_x.is_some(),
                    )
                });
        let preview =
            geometry
                .preview
                .zip(self.preview_frame.as_ref())
                .and_then(|(pane, frame)| {
                    if !preview_current {
                        return None;
                    }
                    let height = f64::from(frame.document_height());
                    let viewport = f64::from(pane.height);
                    Scrollbar::new(
                        ScrollPane::Preview,
                        pane,
                        dpi,
                        f64::from(self.preview_scroll_y) / (height - viewport).max(1.0),
                        viewport / height.max(viewport),
                        false,
                    )
                });
        [source, preview]
    }

    pub(super) fn scrollbar_gutter_at_cursor(&self) -> Option<ScrollPane> {
        let geometry = self.view_geometry()?;
        let dpi = self.window.as_ref()?.scale_factor();
        [
            (ScrollPane::Source, geometry.source),
            (ScrollPane::Preview, geometry.preview),
        ]
        .into_iter()
        .find_map(|(side, pane)| {
            let pane = pane?;
            let right = f64::from(pane.x + pane.width);
            (self.cursor_position.x >= right
                && self.cursor_position.x < right + f64::from(GUTTER_DIP) * dpi
                && self.cursor_position.y >= f64::from(pane.y)
                && self.cursor_position.y < f64::from(pane.y + pane.height))
            .then_some(side)
        })
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
        if let Some(bar) = bars
            .into_iter()
            .flatten()
            .find(|bar| bar.contains(self.cursor_position))
        {
            match bar.press(self.cursor_position.y) {
                ScrollbarPress::Drag(drag) => {
                    self.scrollbars.drag = Some(drag);
                    self.refresh_window_guards(None);
                }
                ScrollbarPress::Page(direction) => {
                    let height = self
                        .view_geometry()
                        .and_then(|geometry| match bar.pane {
                            ScrollPane::Source => geometry.source,
                            ScrollPane::Preview => geometry.preview,
                        })
                        .map_or(0, |pane| pane.height);
                    self.scroll_pane_by(bar.pane, direction * height as f32 * 0.9);
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
        let hovered = bars
            .iter()
            .flatten()
            .find(|bar| gutter == Some(bar.pane))
            .map(|bar| bar.pane);
        if self.scrollbars.hovered != hovered {
            self.scrollbars.hovered = hovered;
            self.request_redraw();
        }
        let Some(drag) = self.scrollbars.drag else {
            return false;
        };
        let Some(bar) = bars.into_iter().flatten().find(|bar| bar.pane == drag.pane) else {
            self.cancel_scrollbar_drag();
            return true;
        };
        if let Some(window) = &self.window {
            window.set_cursor(CursorIcon::Default)
        }
        self.scroll_pane_to_fraction(drag.pane, bar.fraction_at(drag, self.cursor_position.y));
        true
    }

    pub(super) fn cancel_scrollbar_drag(&mut self) {
        if self.scrollbars.drag.take().is_some() {
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
