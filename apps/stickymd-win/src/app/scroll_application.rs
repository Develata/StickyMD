//! Apply pane scrolling to projections and session scroll coordinates only.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars
//! plan_ref: docs/plan/06_markdown_math_rendering.md#split-scroll-sync

use stickymd_render::preview::PreviewFrame;
use stickymd_render::source::SourceProjection;

use super::scrollbar::{ScrollPane, preview_offset_at, preview_offset_by};
use crate::config::ViewMode;
use crate::interaction::EditorSession;

#[derive(Clone, Copy)]
pub(super) enum ScrollRequest {
    Fraction(f64),
    Pixels(f32),
}

/// Borrowed projections, never the canonical coordinator or a mutable frame.
/// No worker, window, filesystem or configuration mutation is available here.
pub(super) struct ScrollApplication<'a> {
    pub source: Option<&'a mut SourceProjection>,
    pub frame: Option<&'a PreviewFrame>,
    pub preview_y: &'a mut f32,
    pub session: &'a mut EditorSession,
}

impl ScrollApplication<'_> {
    /// Returns whether the existing Preview needs repainting. The one driver
    /// moves first; semantic synchronization updates its peer at most once.
    pub fn apply(
        mut self,
        driver: ScrollPane,
        request: ScrollRequest,
        mode: ViewMode,
        sync_enabled: bool,
        preview_current: bool,
    ) -> bool {
        match driver {
            ScrollPane::Source => {
                if let Some(source) = &mut self.source {
                    match request {
                        ScrollRequest::Fraction(fraction) => {
                            source.scroll_to_fraction(fraction);
                        }
                        ScrollRequest::Pixels(pixels) => {
                            source.scroll_by(pixels);
                        }
                    }
                }
            }
            ScrollPane::Preview => match request {
                ScrollRequest::Fraction(fraction) => {
                    if let Some(frame) = self.frame {
                        *self.preview_y =
                            preview_offset_at(fraction, frame.document_height(), frame.height());
                    }
                }
                ScrollRequest::Pixels(pixels) => {
                    *self.preview_y = self.frame.map_or(0.0, |frame| {
                        preview_offset_by(
                            *self.preview_y,
                            pixels,
                            frame.document_height(),
                            frame.height(),
                        )
                    });
                }
            },
        }
        let sync = mode == ViewMode::Split && sync_enabled;
        if sync
            && let (Some(frame), Some(source)) =
                (self.frame.filter(|_| preview_current), &mut self.source)
        {
            match driver {
                ScrollPane::Source => {
                    if let Some(y) = frame.y_for_scroll_anchor(source.scroll_anchor()) {
                        *self.preview_y = y.clamp(
                            0.0,
                            (frame.document_height() - frame.height() as f32).max(0.0),
                        );
                    }
                }
                ScrollPane::Preview => {
                    if let Some(anchor) = frame.scroll_anchor_at_y(*self.preview_y) {
                        let _ = source.scroll_to_anchor(anchor);
                    }
                }
            }
        }
        if let Some(source) = self.source {
            let scroll = source.scroll();
            self.session.scroll.line = scroll.line;
            self.session.scroll.vertical_px = scroll.vertical;
            self.session.scroll.horizontal_px = scroll.horizontal;
        }
        driver == ScrollPane::Preview || sync
    }
}

#[cfg(test)]
mod tests;
