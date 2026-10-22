//! Pane scroll input, scrollbar gestures and semantic split synchronization.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars
//! plan_ref: docs/plan/06_markdown_math_rendering.md#split-scroll-sync

use winit::event::{ElementState, MouseButton};
use winit::window::CursorIcon;

use super::StickyApp;
use super::preview_runtime::ViewGeometry;
use super::scroll_application::{ScrollApplication, ScrollRequest};
use super::scrollbar::{
    DragUpdate, PressOutcome, PreviewFacts, ScrollPane, Scrollbar, gutter_at, page_pixels,
    view_bars,
};

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
        self.scrollbar_geometry_for(geometry, dpi, preview_current)
    }

    /// Apply native geometry observations to the actual cached projections.
    fn scrollbar_geometry_for(
        &mut self,
        geometry: ViewGeometry,
        dpi: f32,
        preview_current: bool,
    ) -> [Option<Scrollbar>; 2] {
        // Measured only for a pane the view shows.
        let source = geometry
            .source
            .and(self.projection.as_mut())
            .map(|projection| projection.scroll_metrics());
        let preview = geometry
            .preview
            .and(self.preview_frame.as_ref())
            .map(|frame| PreviewFacts {
                document_height: frame.document_height(),
                scroll_y: self.preview_scroll_y,
                current: preview_current,
            });
        view_bars(dpi, geometry, source, preview)
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
        let Some(geometry) = self.view_geometry() else {
            return false;
        };
        let dpi = self
            .window
            .as_ref()
            .map_or(1.0, |w| w.scale_factor() as f32);
        self.press_scrollbar_at(geometry, dpi, self.preview_layout_is_current())
    }

    fn press_scrollbar_at(
        &mut self,
        geometry: ViewGeometry,
        dpi: f32,
        preview_current: bool,
    ) -> bool {
        if !self.window_accepts_editor_mutation() {
            return false;
        }
        let bars = self.scrollbar_geometry_for(geometry, dpi, preview_current);
        if let Some(outcome) = self.scrollbars.press(bars, self.cursor_position) {
            match outcome {
                PressOutcome::DragStarted => self.refresh_window_guards(None),
                PressOutcome::Page { pane, direction } => {
                    // The pane height is read only for a confirmed track press.
                    let height = match pane {
                        ScrollPane::Source => geometry.source,
                        ScrollPane::Preview => geometry.preview,
                    }
                    .map_or(0, |content| content.height);
                    self.scroll_pane_by(pane, page_pixels(direction, height));
                }
            }
            self.request_redraw();
            return true;
        }
        // Empty gutter is not an editor click (including while a preview loads).
        gutter_at(
            self.cursor_position,
            [
                (ScrollPane::Source, geometry.source),
                (ScrollPane::Preview, geometry.preview),
            ],
            f64::from(dpi),
        )
        .is_some()
    }

    pub(super) fn handle_scrollbar_cursor(&mut self) -> bool {
        let geometry = self.view_geometry();
        let dpi = self
            .window
            .as_ref()
            .map_or(1.0, |w| w.scale_factor() as f32);
        self.move_scrollbar_at(geometry, dpi, self.preview_layout_is_current())
    }

    fn move_scrollbar_at(
        &mut self,
        geometry: Option<ViewGeometry>,
        dpi: f32,
        preview_current: bool,
    ) -> bool {
        if !self.window_accepts_editor_mutation() {
            self.cancel_scrollbar_drag();
            return false;
        }
        if self.session.dragging_selection || self.preview_dragging {
            return false;
        }
        let bars = geometry.map_or([None, None], |g| {
            self.scrollbar_geometry_for(g, dpi, preview_current)
        });
        let gutter = geometry.and_then(|g| {
            gutter_at(
                self.cursor_position,
                [
                    (ScrollPane::Source, g.source),
                    (ScrollPane::Preview, g.preview),
                ],
                f64::from(dpi),
            )
        });
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
        let horizontal_ended = self.preview_horizontal.drag.take().is_some();
        if self.scrollbars.end_drag() || horizontal_ended {
            self.refresh_window_guards(None);
            self.request_redraw();
        }
    }

    fn scroll_pane_to_fraction(&mut self, pane: ScrollPane, fraction: f64) {
        self.apply_pane_scroll(pane, ScrollRequest::Fraction(fraction));
    }

    pub(super) fn scroll_pane_by(&mut self, pane: ScrollPane, pixels: f32) {
        self.apply_pane_scroll(pane, ScrollRequest::Pixels(pixels));
    }

    fn apply_pane_scroll(&mut self, driver: ScrollPane, request: ScrollRequest) {
        let preview_current = self.preview_layout_is_current();
        let repaint = ScrollApplication {
            source: self.projection.as_mut(),
            frame: self.preview_frame.as_ref(),
            preview_y: &mut self.preview_scroll_y,
            session: &mut self.session,
        }
        .apply(
            driver,
            request,
            self.config.current().view_mode,
            self.config.current().split_scroll_sync,
            preview_current,
        );
        if repaint {
            self.request_preview_paint();
        }
        self.update_ime_area();
        self.request_redraw();
    }
}

#[cfg(test)]
mod tests;
