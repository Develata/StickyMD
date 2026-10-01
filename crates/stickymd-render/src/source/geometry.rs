//! Source-projection geometry, hit testing, scrolling, and caret mapping.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use cosmic_text::{Affinity, Align, Attrs, Buffer, Cursor, Family, Scroll, Shaping, Wrap};
use unicode_segmentation::UnicodeSegmentation;

use crate::scroll::ScrollAnchor;

use super::projection::{
    EditorRect, PADDING_DIP, PreeditVisual, SourceProjection, SourceProjectionError,
    scaled_metrics, selection_valid,
};

#[cfg(test)]
mod preedit_tests;

pub(super) struct PreeditViewport {
    pub left: f32,
    pub right: f32,
    pub offset: f32,
    pub caret: EditorRect,
}

impl SourceProjection {
    pub fn scroll_by(&mut self, vertical_px: f32) -> Scroll {
        let current = self.buffer.scroll();
        self.buffer.set_scroll(Scroll::new(
            current.line,
            current.vertical + vertical_px,
            0.0,
        ));
        self.buffer.shape_until_scroll(&mut self.font_system, false);
        self.buffer.scroll()
    }

    pub fn scroll(&self) -> Scroll {
        self.buffer.scroll()
    }

    /// Maps the viewport top to a generation-local canonical source byte.
    pub fn scroll_anchor(&self) -> ScrollAnchor {
        let byte = self.hit_test(self.padding(), self.padding());
        ScrollAnchor::point(byte)
    }

    /// Aligns the viewport top with a semantic source anchor.
    ///
    /// The source byte is converted to a cosmic-text cursor first. A second
    /// layout-relative adjustment handles wrapped logical lines without using
    /// document-wide scroll percentages.
    pub fn scroll_to_anchor(
        &mut self,
        anchor: ScrollAnchor,
    ) -> Result<Scroll, SourceProjectionError> {
        if anchor.source_end > self.canonical.len() {
            return Err(SourceProjectionError::InvalidPosition);
        }
        let source_byte = if anchor.source_end > anchor.source_byte {
            let span = anchor.source_end - anchor.source_byte;
            let interpolated = anchor.source_byte.saturating_add(
                ((span as f64 * f64::from(anchor.block_fraction)).round() as usize).min(span),
            );
            self.nearest_grapheme_boundary(interpolated)
        } else {
            anchor.source_byte
        };
        let cursor = self
            .cursor_for_global(source_byte)
            .ok_or(SourceProjectionError::InvalidPosition)?;
        self.buffer.set_scroll(Scroll::new(cursor.line, 0.0, 0.0));
        self.ensure_caret_visible(source_byte)?;
        let visual_offset = self
            .caret_rect(source_byte)
            .map_or(0.0, |rect| (rect.y - self.padding()).max(0.0));
        let revealed = self.buffer.scroll();
        self.buffer.set_scroll(Scroll::new(
            revealed.line,
            revealed.vertical + visual_offset,
            0.0,
        ));
        self.buffer.shape_until_scroll(&mut self.font_system, false);
        Ok(self.buffer.scroll())
    }

    pub fn hit_test(&self, x_px: f32, y_px: f32) -> usize {
        let local_x = (x_px - self.padding()).max(0.0);
        let local_y = (y_px - self.padding()).max(0.0);
        let byte = self
            .buffer
            .hit(local_x, local_y)
            .map_or(self.canonical.len(), |cursor| self.global_byte(cursor));
        self.nearest_grapheme_boundary(byte)
    }

    pub fn caret_rect(&self, byte: usize) -> Option<EditorRect> {
        let cursor = self.cursor_for_global(byte)?;
        for run in self.buffer.layout_runs() {
            if run.line_i == cursor.line
                && let Some(x) = run.cursor_position(&cursor)
            {
                return Some(EditorRect {
                    x: x + self.padding(),
                    y: run.line_top + self.padding(),
                    width: self.scale_factor.max(1.0),
                    height: run.line_height,
                });
            }
        }
        None
    }

    /// Position used by the operating system's IME candidate window.
    pub fn ime_caret_rect(&mut self, source_byte: usize) -> Option<EditorRect> {
        let Some(preedit) = self.preedit.clone() else {
            return self.caret_rect(source_byte);
        };
        if !selection_valid(&self.canonical, preedit.replacement) {
            return None;
        }
        let origin = self.caret_rect(preedit.replacement.start())?;
        let overlay = self.preedit_buffer(&preedit, origin);
        self.preedit_viewport(&overlay, &preedit, origin)
            .map(|viewport| viewport.caret)
    }

    pub fn ensure_caret_visible(&mut self, byte: usize) -> Result<(), SourceProjectionError> {
        let cursor = self
            .cursor_for_global(byte)
            .ok_or(SourceProjectionError::InvalidPosition)?;
        self.buffer
            .shape_until_cursor(&mut self.font_system, cursor, false);
        if self.caret_rect(byte).is_none() {
            // Wrapping can omit the space before this byte. cosmic-text then
            // resolves Before to the start of the logical line, leaving the
            // actual caret off screen. Retry the following glyph boundary only
            // when the preferred position could not be revealed.
            let after = Cursor::new_with_affinity(cursor.line, cursor.index, Affinity::After);
            self.buffer
                .shape_until_cursor(&mut self.font_system, after, false);
        }
        Ok(())
    }

    pub fn vertical_neighbor(&self, byte: usize, direction: i32, preferred_x: f32) -> usize {
        let Some(rectangle) = self.caret_rect(byte) else {
            return byte;
        };
        let y = if direction < 0 {
            rectangle.y - rectangle.height * 0.5
        } else {
            rectangle.y + rectangle.height * 1.5
        };
        self.hit_test(preferred_x, y)
    }

    pub(super) fn preedit_buffer(&mut self, preedit: &PreeditVisual, origin: EditorRect) -> Buffer {
        // The actual composition is set and shaped below. Avoid first shaping
        // an empty default-font line that will immediately be discarded.
        let mut overlay = Buffer::new_empty(scaled_metrics(self.scale_factor));
        overlay.set_size(
            Some((self.width_px as f32 - origin.x).max(1.0)),
            Some(origin.height.max(1.0)),
        );
        overlay.set_wrap(Wrap::None);
        let attrs = Attrs::new().family(Family::Name(self.fonts.cjk_family));
        overlay.set_text(&preedit.text, &attrs, Shaping::Advanced, Some(Align::Left));
        overlay.shape_until_scroll(&mut self.font_system, false);
        overlay
    }

    /// Both painting and IME placement consume this horizontal reveal. Cosmic
    /// layout runs expose unscrolled glyph coordinates, even after scrolling a
    /// buffer to its cursor, so that scroll alone cannot position an overlay.
    pub(super) fn preedit_viewport(
        &self,
        overlay: &Buffer,
        preedit: &PreeditVisual,
        origin: EditorRect,
    ) -> Option<PreeditViewport> {
        let byte = preedit
            .cursor
            .as_ref()
            .map_or(preedit.text.len(), |cursor| cursor.end);
        let cursor = Cursor::new(0, byte);
        let right = self.width_px as f32;
        let width = self.scale_factor.max(1.0).min(right);
        if width <= 0.0 {
            return None;
        }
        let left = origin.x.clamp(0.0, right - width);
        overlay.layout_runs().find_map(|run| {
            let logical_x = run.cursor_position(&cursor)?;
            let visible_x = logical_x.clamp(0.0, (right - left - width).max(0.0));
            Some(PreeditViewport {
                left,
                right,
                offset: logical_x - visible_x,
                caret: EditorRect {
                    x: left + visible_x,
                    y: origin.y,
                    width,
                    height: run.line_height,
                },
            })
        })
    }

    pub(super) fn cursor_for_global(&self, byte: usize) -> Option<Cursor> {
        if byte > self.canonical.len() || !self.canonical.is_char_boundary(byte) {
            return None;
        }
        let line = self.line_starts.partition_point(|start| *start <= byte) - 1;
        Some(Cursor::new(line, byte - self.line_starts[line]))
    }

    fn global_byte(&self, cursor: Cursor) -> usize {
        self.line_starts
            .get(cursor.line)
            .copied()
            .unwrap_or(self.canonical.len())
            .saturating_add(cursor.index)
            .min(self.canonical.len())
    }

    fn nearest_grapheme_boundary(&self, byte: usize) -> usize {
        if byte >= self.canonical.len() {
            return self.canonical.len();
        }
        let line = self.line_starts.partition_point(|start| *start <= byte) - 1;
        let line_start = self.line_starts[line];
        let line_end = self
            .line_starts
            .get(line + 1)
            .map_or(self.canonical.len(), |next| next.saturating_sub(1));
        let local = byte.saturating_sub(line_start).min(line_end - line_start);
        if local == line_end - line_start {
            return line_end;
        }
        self.canonical[line_start..line_end]
            .grapheme_indices(true)
            .map(|(index, _)| index)
            .take_while(|index| *index <= local)
            .last()
            .map_or(line_start, |index| line_start + index)
    }

    pub(super) fn padding(&self) -> f32 {
        PADDING_DIP * self.scale_factor
    }

    pub(super) fn content_width(&self) -> f32 {
        (self.width_px as f32 - self.padding() * 2.0).max(1.0)
    }

    pub(super) fn content_height(&self) -> f32 {
        (self.height_px as f32 - self.padding() * 2.0).max(1.0)
    }
}
