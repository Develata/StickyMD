//! Source text, selection, caret, preedit, and diagnostic painting.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use cosmic_text::{Color, Cursor};
use stickymd_core::Selection;
use tiny_skia::Pixmap;

use super::paint::{blend_glyph_rect, fill, rect};
use super::projection::{PreeditVisual, SourceProjection, SourceProjectionError, selection_valid};

impl SourceProjection {
    pub fn paint(
        &mut self,
        pixmap: &mut Pixmap,
        selection: Selection,
        focused: bool,
        caret_visible: bool,
        diagnostic: Option<&str>,
    ) -> Result<(), SourceProjectionError> {
        if !selection_valid(&self.canonical, selection) {
            return Err(SourceProjectionError::InvalidPosition);
        }

        fill(pixmap, (248, 246, 239, 255));
        self.buffer.shape_until_scroll(&mut self.font_system, false);
        if !selection.is_collapsed() {
            self.paint_selection(pixmap, selection);
        }

        let padding = self.padding() as i32;
        self.buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            Color::rgb(40, 38, 34),
            |x, y, width, height, color| {
                blend_glyph_rect(pixmap, x + padding, y + padding, width, height, color);
            },
        );

        if let Some(preedit) = self.preedit.clone() {
            self.paint_preedit(pixmap, &preedit)?;
        } else if focused
            && caret_visible
            && let Some(caret) = self.caret_rect(selection.active.byte)
        {
            rect(
                pixmap,
                caret.x,
                caret.y,
                caret.width.max(1.0),
                caret.height,
                (32, 79, 143, 255),
            );
        }

        if let Some(message) = diagnostic {
            self.paint_diagnostic(pixmap, message);
        }
        Ok(())
    }

    fn paint_selection(&self, pixmap: &mut Pixmap, selection: Selection) {
        let Some(start) = self.cursor_for_global(selection.start()) else {
            return;
        };
        let Some(end) = self.cursor_for_global(selection.end()) else {
            return;
        };
        for run in self.buffer.layout_runs() {
            for (x, width) in run.highlight(start, end) {
                rect(
                    pixmap,
                    x + self.padding(),
                    run.line_top + self.padding(),
                    width,
                    run.line_height,
                    (176, 207, 243, 210),
                );
            }
        }
    }

    fn paint_preedit(
        &mut self,
        pixmap: &mut Pixmap,
        preedit: &PreeditVisual,
    ) -> Result<(), SourceProjectionError> {
        if !selection_valid(&self.canonical, preedit.replacement) {
            return Err(SourceProjectionError::InvalidPosition);
        }
        let Some(origin) = self.caret_rect(preedit.replacement.start()) else {
            return Ok(());
        };
        let mut overlay = self.preedit_buffer(preedit, origin);
        let width = overlay
            .layout_runs()
            .map(|run| run.line_w)
            .fold(0.0f32, f32::max)
            .max(self.scale_factor * 2.0);
        rect(
            pixmap,
            origin.x,
            origin.y,
            width,
            origin.height,
            (248, 246, 239, 255),
        );

        if let Some(cursor) = &preedit.cursor
            && cursor.start < cursor.end
        {
            let start = Cursor::new(0, cursor.start);
            let end = Cursor::new(0, cursor.end);
            for run in overlay.layout_runs() {
                for (x, width) in run.highlight(start, end) {
                    rect(
                        pixmap,
                        origin.x + x,
                        origin.y,
                        width,
                        origin.height,
                        (176, 207, 243, 210),
                    );
                }
            }
        }
        overlay.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            Color::rgb(27, 52, 82),
            |x, y, width, height, color| {
                blend_glyph_rect(
                    pixmap,
                    x + origin.x as i32,
                    y + origin.y as i32,
                    width,
                    height,
                    color,
                );
            },
        );
        rect(
            pixmap,
            origin.x,
            origin.y + origin.height - self.scale_factor.max(1.0),
            width,
            self.scale_factor.max(1.0),
            (47, 104, 166, 255),
        );
        if let Some(cursor) = &preedit.cursor {
            let cursor = Cursor::new(0, cursor.end);
            if let Some(x) = overlay
                .layout_runs()
                .find_map(|run| run.cursor_position(&cursor))
            {
                rect(
                    pixmap,
                    origin.x + x,
                    origin.y,
                    self.scale_factor.max(1.0),
                    origin.height,
                    (32, 79, 143, 255),
                );
            }
        }
        Ok(())
    }

    fn paint_diagnostic(&mut self, pixmap: &mut Pixmap, message: &str) {
        if message.is_empty() {
            return;
        }
        let height = 34.0 * self.scale_factor;
        rect(
            pixmap,
            0.0,
            self.height_px as f32 - height,
            self.width_px as f32,
            height,
            (246, 224, 184, 245),
        );
        if message.contains("[F7") {
            let action_start = self.width_px as f32 * 0.58;
            let action_width = (self.width_px as f32 - action_start) * 0.5;
            rect(
                pixmap,
                action_start,
                self.height_px as f32 - height + self.scale_factor,
                action_width - self.scale_factor,
                height - 2.0 * self.scale_factor,
                (232, 201, 145, 245),
            );
            rect(
                pixmap,
                action_start + action_width,
                self.height_px as f32 - height + self.scale_factor,
                action_width - self.scale_factor,
                height - 2.0 * self.scale_factor,
                (239, 211, 163, 245),
            );
        }
        if self.diagnostic_text != message {
            self.diagnostic_text.clear();
            self.diagnostic_text.push_str(message);
            let attrs =
                cosmic_text::Attrs::new().family(cosmic_text::Family::Name(self.fonts.cjk_family));
            self.diagnostic_buffer.set_text(
                &self.diagnostic_text,
                &attrs,
                cosmic_text::Shaping::Advanced,
                Some(cosmic_text::Align::Left),
            );
        }
        self.diagnostic_buffer
            .shape_until_scroll(&mut self.font_system, false);
        let x_offset = (12.0 * self.scale_factor) as i32;
        let y_offset = (self.height_px as f32 - height + 7.0 * self.scale_factor) as i32;
        self.diagnostic_buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            Color::rgb(72, 52, 20),
            |x, y, width, height, color| {
                blend_glyph_rect(pixmap, x + x_offset, y + y_offset, width, height, color);
            },
        );
    }
}
