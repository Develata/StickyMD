//! Source-projection geometry, hit testing, scrolling, and caret mapping.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use cosmic_text::{Align, Attrs, Buffer, Cursor, Family, Scroll, Shaping, Wrap};
use unicode_segmentation::UnicodeSegmentation;

use super::projection::{
    EditorRect, PADDING_DIP, PreeditVisual, SourceProjection, SourceProjectionError,
    scaled_metrics, selection_valid,
};

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
        let mut overlay = self.preedit_buffer(&preedit, origin);
        let byte = preedit
            .cursor
            .as_ref()
            .map_or(preedit.text.len(), |cursor| cursor.end);
        let cursor = Cursor::new(0, byte);
        overlay.shape_until_cursor(&mut self.font_system, cursor, false);
        overlay.layout_runs().find_map(|run| {
            run.cursor_position(&cursor).map(|x| EditorRect {
                x: origin.x + x,
                y: origin.y,
                width: self.scale_factor.max(1.0),
                height: run.line_height,
            })
        })
    }

    pub fn ensure_caret_visible(&mut self, byte: usize) -> Result<(), SourceProjectionError> {
        let cursor = self
            .cursor_for_global(byte)
            .ok_or(SourceProjectionError::InvalidPosition)?;
        self.buffer
            .shape_until_cursor(&mut self.font_system, cursor, false);
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
        let mut overlay = Buffer::new(&mut self.font_system, scaled_metrics(self.scale_factor));
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
