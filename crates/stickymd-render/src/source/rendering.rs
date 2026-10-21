//! Source text, selection, caret, preedit, and diagnostic painting.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use cosmic_text::{Color, Cursor};
use stickymd_core::Selection;
use tiny_skia::Pixmap;

use super::paint::{GlyphClip, blend_glyph_rect, blend_glyph_rect_clipped, fill, rect};
use super::projection::{PreeditVisual, SourceProjection, SourceProjectionError, selection_valid};

/// Fixed source-editor palette selected by the Windows shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceTheme {
    Light,
    Dark,
}

/// Geometry and scale for one transient shell text projection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UiTextSpec {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub scale: f32,
}

#[derive(Clone, Copy)]
struct SourcePalette {
    background: (u8, u8, u8, u8),
    text: Color,
    selection: (u8, u8, u8, u8),
    caret: (u8, u8, u8, u8),
    preedit_text: Color,
    diagnostic_background: (u8, u8, u8, u8),
    diagnostic_action_primary: (u8, u8, u8, u8),
    diagnostic_action_secondary: (u8, u8, u8, u8),
    diagnostic_text: Color,
}

impl SourcePalette {
    fn for_theme(theme: SourceTheme) -> Self {
        match theme {
            SourceTheme::Light => Self {
                background: (248, 246, 239, 255),
                text: Color::rgb(40, 38, 34),
                selection: (176, 207, 243, 210),
                caret: (32, 79, 143, 255),
                preedit_text: Color::rgb(27, 52, 82),
                diagnostic_background: (246, 224, 184, 245),
                diagnostic_action_primary: (232, 201, 145, 245),
                diagnostic_action_secondary: (239, 211, 163, 245),
                diagnostic_text: Color::rgb(72, 52, 20),
            },
            SourceTheme::Dark => Self {
                background: (31, 31, 29, 255),
                text: Color::rgb(226, 223, 214),
                selection: (62, 91, 125, 230),
                caret: (129, 179, 236, 255),
                preedit_text: Color::rgb(205, 224, 247),
                diagnostic_background: (78, 62, 37, 248),
                diagnostic_action_primary: (102, 77, 38, 248),
                diagnostic_action_secondary: (91, 69, 36, 248),
                diagnostic_text: Color::rgb(245, 224, 184),
            },
        }
    }
}

impl SourceProjection {
    /// Draws one transient shell text line using the existing font database and
    /// glyph cache. The text is a disposable UI projection, never document data.
    pub fn paint_ui_text(
        &mut self,
        pixmap: &mut Pixmap,
        text: &str,
        spec: UiTextSpec,
        theme: SourceTheme,
    ) {
        self.shape_ui_text(text, spec);
        let scale = spec.scale.max(0.5);
        let color = SourcePalette::for_theme(theme).text;
        let clip = GlyphClip {
            left: spec.x.round() as i32,
            top: spec.y.round() as i32,
            right: (spec.x + spec.width).round() as i32,
            bottom: (spec.y + 22.0 * scale).round() as i32,
        };
        self.ui_buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            color,
            |glyph_x, glyph_y, width, height, color| {
                blend_glyph_rect_clipped(
                    pixmap,
                    glyph_x + spec.x.round() as i32,
                    glyph_y + spec.y.round() as i32,
                    width,
                    height,
                    color,
                    clip,
                );
            },
        );
    }

    /// Paints a single-line editable shell field and its caret from one shaped
    /// geometry projection. Long text scrolls horizontally just enough to keep
    /// the caret visible; no document/editor state is mutated.
    pub fn paint_ui_text_field(
        &mut self,
        pixmap: &mut Pixmap,
        text: &str,
        cursor: usize,
        spec: UiTextSpec,
        theme: SourceTheme,
    ) -> Option<super::EditorRect> {
        let (offset, caret) = self.shape_ui_text_field(text, cursor, spec)?;
        let palette = SourcePalette::for_theme(theme);
        let clip = ui_text_clip(spec);
        let origin_x = (spec.x - offset).round() as i32;
        let origin_y = spec.y.round() as i32;
        self.ui_buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            palette.text,
            |glyph_x, glyph_y, width, height, color| {
                blend_glyph_rect_clipped(
                    pixmap,
                    glyph_x + origin_x,
                    glyph_y + origin_y,
                    width,
                    height,
                    color,
                    clip,
                );
            },
        );
        rect(
            pixmap,
            caret.x,
            caret.y,
            caret.width,
            caret.height,
            palette.caret,
        );
        Some(caret)
    }

    /// Returns the caret rectangle using the same shaping and horizontal
    /// offset as `paint_ui_text_field`.
    pub fn ui_text_field_caret(
        &mut self,
        text: &str,
        cursor: usize,
        spec: UiTextSpec,
    ) -> Option<super::EditorRect> {
        self.shape_ui_text_field(text, cursor, spec)
            .map(|(_, caret)| caret)
    }

    /// Maps a shell-space x coordinate to a valid byte boundary in text as `paint_ui_text` paints
    /// it: from the field's left edge, with no horizontal offset (an unfocused field).
    pub fn ui_text_hit(&mut self, text: &str, spec: UiTextSpec, shell_x: f32) -> Option<usize> {
        self.shape_ui_text(text, spec);
        let local_x = (shell_x - spec.x).max(0.0);
        let hit = self
            .ui_buffer
            .hit(local_x, 10.0 * spec.scale.max(0.5))
            .map_or(text.len(), |cursor| cursor.index)
            .min(text.len());
        text.is_char_boundary(hit).then_some(hit)
    }

    /// Shapes `text` into the one-line UI buffer as `paint_ui_text` and `ui_text_hit` read it.
    fn shape_ui_text(&mut self, text: &str, spec: UiTextSpec) {
        let scale = spec.scale.max(0.5);
        self.ui_buffer.set_metrics_and_size(
            cosmic_text::Metrics::new(13.0 * scale, 20.0 * scale),
            Some(spec.width.max(1.0)),
            Some(22.0 * scale),
        );
        let attrs =
            cosmic_text::Attrs::new().family(cosmic_text::Family::Name(self.fonts.cjk_family));
        self.ui_buffer.set_text(
            text,
            &attrs,
            cosmic_text::Shaping::Advanced,
            Some(cosmic_text::Align::Left),
        );
        self.ui_buffer
            .shape_until_scroll(&mut self.font_system, false);
    }

    /// Maps a shell-space x coordinate to a valid byte boundary in the field.
    pub fn ui_text_field_hit(
        &mut self,
        text: &str,
        cursor: usize,
        spec: UiTextSpec,
        shell_x: f32,
    ) -> Option<usize> {
        let (offset, _) = self.shape_ui_text_field(text, cursor, spec)?;
        // The offset can be negative (right-to-left text), so the layout x is not clamped.
        let local_x = shell_x - spec.x + offset;
        let hit = self
            .ui_buffer
            .hit(local_x, 10.0 * spec.scale.max(0.5))
            .map_or(text.len(), |cursor| cursor.index)
            .min(text.len());
        text.is_char_boundary(hit).then_some(hit)
    }

    fn shape_ui_text_field(
        &mut self,
        text: &str,
        cursor: usize,
        spec: UiTextSpec,
    ) -> Option<(f32, super::EditorRect)> {
        if cursor > text.len() || !text.is_char_boundary(cursor) {
            return None;
        }
        let scale = spec.scale.max(0.5);
        self.ui_buffer.set_metrics_and_size(
            cosmic_text::Metrics::new(13.0 * scale, 20.0 * scale),
            Some(spec.width.max(1.0)),
            Some(22.0 * scale),
        );
        let attrs =
            cosmic_text::Attrs::new().family(cosmic_text::Family::Name(self.fonts.cjk_family));
        self.ui_buffer.set_text(
            text,
            &attrs,
            cosmic_text::Shaping::Advanced,
            Some(cosmic_text::Align::Left),
        );
        let cursor = Cursor::new(0, cursor);
        self.ui_buffer
            .shape_until_cursor(&mut self.font_system, cursor, false);
        let run = self.ui_buffer.layout_runs().next()?;
        let logical_x = run.cursor_position(&cursor)?;
        let caret_width = scale.max(1.0);
        let right_inset = (spec.width - caret_width - 2.0 * scale).max(0.0);
        // The visible window follows the caret both ways: an overflowing right-to-left line lays
        // out to the left of the field, so its caret can sit at a negative x.
        let offset = if logical_x > right_inset {
            logical_x - right_inset
        } else if logical_x < 0.0 {
            logical_x
        } else {
            0.0
        };
        Some((
            offset,
            super::EditorRect {
                x: spec.x + logical_x - offset,
                y: spec.y + run.line_top,
                width: caret_width,
                height: run.line_height,
            },
        ))
    }

    pub fn paint(
        &mut self,
        pixmap: &mut Pixmap,
        selection: Selection,
        focused: bool,
        caret_visible: bool,
        diagnostic: Option<&str>,
        theme: SourceTheme,
    ) -> Result<(), SourceProjectionError> {
        if !selection_valid(&self.canonical, selection) {
            return Err(SourceProjectionError::InvalidPosition);
        }

        let palette = SourcePalette::for_theme(theme);
        fill(pixmap, palette.background);
        self.buffer.shape_until_scroll(&mut self.font_system, false);
        if !selection.is_collapsed() {
            self.paint_selection(pixmap, selection, palette);
        }

        let padding = self.padding() as i32;
        self.buffer.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            palette.text,
            |x, y, width, height, color| {
                blend_glyph_rect(pixmap, x + padding, y + padding, width, height, color);
            },
        );

        if let Some(preedit) = self.preedit.clone() {
            self.paint_preedit(pixmap, &preedit, palette)?;
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
                palette.caret,
            );
        }

        if let Some(message) = diagnostic {
            self.paint_diagnostic(pixmap, message, palette);
        }
        Ok(())
    }

    /// Paints only the caret over an already cached source frame.
    ///
    /// Caret blinking must not reshape and rasterize the entire document. The
    /// Windows shell blits a caret-free cached frame, then calls this bounded
    /// overlay operation when the caret is visible.
    pub fn paint_caret_overlay(
        &self,
        pixmap: &mut Pixmap,
        source_byte: usize,
        origin_x: f32,
        origin_y: f32,
        theme: SourceTheme,
    ) -> Result<(), SourceProjectionError> {
        if source_byte > self.canonical.len() || !self.canonical.is_char_boundary(source_byte) {
            return Err(SourceProjectionError::InvalidPosition);
        }
        if let Some(caret) = self.caret_rect(source_byte) {
            rect(
                pixmap,
                origin_x + caret.x,
                origin_y + caret.y,
                caret.width.max(1.0),
                caret.height,
                SourcePalette::for_theme(theme).caret,
            );
        }
        Ok(())
    }

    fn paint_selection(&self, pixmap: &mut Pixmap, selection: Selection, palette: SourcePalette) {
        let Some(start) = self.cursor_for_global(selection.start()) else {
            return;
        };
        let Some(end) = self.cursor_for_global(selection.end()) else {
            return;
        };
        for run in self.buffer.layout_runs() {
            if run.line_i < start.line || run.line_i > end.line {
                continue;
            }
            for (x, width) in run.highlight(start, end) {
                rect(
                    pixmap,
                    x + self.padding(),
                    run.line_top + self.padding(),
                    width,
                    run.line_height,
                    palette.selection,
                );
            }
        }
    }

    fn paint_preedit(
        &mut self,
        pixmap: &mut Pixmap,
        preedit: &PreeditVisual,
        palette: SourcePalette,
    ) -> Result<(), SourceProjectionError> {
        if !selection_valid(&self.canonical, preedit.replacement) {
            return Err(SourceProjectionError::InvalidPosition);
        }
        let Some(origin) = self.caret_rect(preedit.replacement.start()) else {
            return Ok(());
        };
        let mut overlay = self.preedit_buffer(preedit, origin);
        let Some(viewport) = self.preedit_viewport(&overlay, preedit, origin) else {
            return Ok(());
        };
        let text_x = viewport.left - viewport.offset;
        let width = overlay
            .layout_runs()
            .map(|run| run.line_w - viewport.offset)
            .fold(0.0f32, f32::max)
            .max(self.scale_factor * 2.0)
            .min(viewport.right - viewport.left);
        rect(
            pixmap,
            viewport.left,
            origin.y,
            width,
            origin.height,
            palette.background,
        );

        if let Some(cursor) = &preedit.cursor
            && cursor.start < cursor.end
        {
            let start = Cursor::new(0, cursor.start);
            let end = Cursor::new(0, cursor.end);
            for run in overlay.layout_runs() {
                for (x, width) in run.highlight(start, end) {
                    let left = (text_x + x).max(viewport.left);
                    let right = (text_x + x + width).min(viewport.right);
                    rect(
                        pixmap,
                        left,
                        origin.y,
                        right - left,
                        origin.height,
                        palette.selection,
                    );
                }
            }
        }
        let clip = GlyphClip {
            left: viewport.left.round() as i32,
            top: origin.y.floor() as i32,
            right: viewport.right as i32,
            bottom: (origin.y + origin.height).ceil() as i32,
        };
        overlay.draw(
            &mut self.font_system,
            &mut self.swash_cache,
            palette.preedit_text,
            |x, y, width, height, color| {
                blend_glyph_rect_clipped(
                    pixmap,
                    x + text_x as i32,
                    y + origin.y as i32,
                    width,
                    height,
                    color,
                    clip,
                );
            },
        );
        rect(
            pixmap,
            viewport.left,
            origin.y + origin.height - self.scale_factor.max(1.0),
            width,
            self.scale_factor.max(1.0),
            palette.caret,
        );
        if preedit.cursor.is_some() {
            let caret = viewport.caret;
            rect(
                pixmap,
                caret.x,
                caret.y,
                caret.width,
                caret.height,
                palette.caret,
            );
        }
        Ok(())
    }

    fn paint_diagnostic(&mut self, pixmap: &mut Pixmap, message: &str, palette: SourcePalette) {
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
            palette.diagnostic_background,
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
                palette.diagnostic_action_primary,
            );
            rect(
                pixmap,
                action_start + action_width,
                self.height_px as f32 - height + self.scale_factor,
                action_width - self.scale_factor,
                height - 2.0 * self.scale_factor,
                palette.diagnostic_action_secondary,
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
            palette.diagnostic_text,
            |x, y, width, height, color| {
                blend_glyph_rect(pixmap, x + x_offset, y + y_offset, width, height, color);
            },
        );
    }
}

fn ui_text_clip(spec: UiTextSpec) -> GlyphClip {
    let scale = spec.scale.max(0.5);
    GlyphClip {
        left: spec.x.round() as i32,
        top: spec.y.round() as i32,
        right: (spec.x + spec.width).round() as i32,
        bottom: (spec.y + 22.0 * scale).round() as i32,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stickymd_core::{DocumentSnapshot, Generation, LineEnding, Selection};
    use tiny_skia::Pixmap;

    use super::{SourceProjection, SourceTheme, UiTextSpec};

    fn changed_pixels_in_band(before: &Pixmap, after: &Pixmap, top: f32, height: f32) -> usize {
        let width = before.width() as usize;
        let start = top.ceil().max(0.0) as usize;
        let end = (top + height).floor().min(before.height() as f32) as usize;
        before
            .data()
            .chunks_exact(4)
            .zip(after.data().chunks_exact(4))
            .enumerate()
            .filter(|(index, (left, right))| {
                let row = index / width;
                (start..end).contains(&row) && left != right
            })
            .count()
    }

    #[test]
    fn same_line_selection_does_not_paint_unselected_lines() {
        let text = "StickyMD basic 选择替换 123 🙂\nnihao 这是Rust的trait示例\n择顶项";
        let snapshot = DocumentSnapshot {
            text: Arc::from(text),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let mut projection = SourceProjection::new(&snapshot, 600, 300, 1.0);
        let selection_start = text.find("示例").expect("fixture selection");
        let selection_end = selection_start + "示例".len();
        let first_line = projection.caret_rect(0).expect("first line");
        let second_line = projection
            .caret_rect(text.find('\n').expect("second line") + 1)
            .expect("second line");
        let third_line_start = text.rfind('\n').expect("third line") + 1;
        let third_line = projection.caret_rect(third_line_start).expect("third line");

        let mut baseline = Pixmap::new(600, 300).expect("baseline pixmap");
        projection
            .paint(
                &mut baseline,
                Selection::caret(selection_start),
                true,
                false,
                None,
                SourceTheme::Light,
            )
            .expect("baseline paint");
        for selection in [
            Selection::new(selection_start, selection_end),
            Selection::new(selection_end, selection_start),
        ] {
            let mut selected = Pixmap::new(600, 300).expect("selected pixmap");
            projection
                .paint(
                    &mut selected,
                    selection,
                    true,
                    false,
                    None,
                    SourceTheme::Light,
                )
                .expect("selection paint");

            assert_eq!(
                changed_pixels_in_band(&baseline, &selected, first_line.y, first_line.height),
                0,
                "a same-line selection painted the preceding logical line"
            );
            assert!(
                changed_pixels_in_band(&baseline, &selected, second_line.y, second_line.height) > 0,
                "the selected logical line received no highlight"
            );
            assert_eq!(
                changed_pixels_in_band(&baseline, &selected, third_line.y, third_line.height),
                0,
                "a same-line selection painted the following logical line"
            );
        }
    }

    /// An unfocused field is painted from its left edge (`paint_ui_text`), so a press on a
    /// painted glyph maps to one of that glyph's boundaries, for overflowing left-to-right and
    /// right-to-left text alike; a logical cursor at 0 is not the zero offset for right-to-left
    /// text, where it sits at the right edge.
    #[test]
    fn an_unfocused_field_hit_reads_the_painted_layout() {
        let snapshot = DocumentSnapshot {
            text: Arc::from(""),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let mut projection = SourceProjection::new(&snapshot, 400, 80, 1.0);
        let spec = UiTextSpec {
            x: 12.0,
            y: 8.0,
            width: 162.0,
            scale: 1.0,
        };
        for text in ["abcdefghij".repeat(8), "\u{5d0}".repeat(80)] {
            let mut pixmap = Pixmap::new(400, 80).unwrap();
            projection.paint_ui_text(&mut pixmap, &text, spec, SourceTheme::Light);
            let rtl = projection
                .ui_buffer
                .layout_runs()
                .next()
                .expect("one run")
                .rtl;
            let glyphs: Vec<(f32, f32, usize, usize)> = projection
                .ui_buffer
                .layout_runs()
                .flat_map(|run| run.glyphs.iter())
                .filter(|glyph| glyph.x >= 0.0 && glyph.x + glyph.w <= spec.width)
                .map(|glyph| (glyph.x, glyph.w, glyph.start, glyph.end))
                .collect();
            assert!(glyphs.len() > 4, "{} glyphs visible", glyphs.len());
            for (x, w, start, end) in [glyphs[0], glyphs[glyphs.len() / 2]] {
                // A press in a glyph's left quarter lands on its left boundary, in its right
                // quarter on its right one: in reading order, start then end left to right,
                // end then start right to left. An offset of a few pixels crosses the middle.
                let (left, right) = if rtl { (end, start) } else { (start, end) };
                for (inside, expected) in [(x + w * 0.25, left), (x + w * 0.75, right)] {
                    let hit = projection
                        .ui_text_hit(&text, spec, spec.x + inside)
                        .expect("hit");
                    assert_eq!(hit, expected, "rtl {rtl}: glyph {start}..{end} at {inside}");
                }
            }
        }
        assert!(
            projection
                .ui_buffer
                .layout_runs()
                .next()
                .is_some_and(|run| run.rtl),
            "the last text checked is right to left"
        );
    }

    /// A focused field keeps its caret visible in both directions: an overflowing right-to-left
    /// line with the cursor at its end has a caret left of the field, so the window scrolls left
    /// to it, and a press on the caret maps back to the cursor.
    #[test]
    fn a_focused_right_to_left_field_reveals_a_caret_left_of_it() {
        let snapshot = DocumentSnapshot {
            text: Arc::from(""),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let mut projection = SourceProjection::new(&snapshot, 400, 80, 1.0);
        let spec = UiTextSpec {
            x: 12.0,
            y: 8.0,
            width: 162.0,
            scale: 1.0,
        };
        let text = "\u{5d0}".repeat(80);
        for cursor in [text.len(), 0, text.len() / 2] {
            let mut pixmap = Pixmap::new(400, 80).unwrap();
            let caret = projection
                .paint_ui_text_field(&mut pixmap, &text, cursor, spec, SourceTheme::Light)
                .expect("field caret");
            assert!(
                caret.x >= spec.x && caret.x + caret.width <= spec.x + spec.width,
                "cursor {cursor}: caret {caret:?} outside the field"
            );
            let hit = projection
                .ui_text_field_hit(&text, cursor, spec, caret.x)
                .expect("field hit");
            assert_eq!(hit, cursor, "cursor {cursor}");
        }
    }

    #[test]
    fn shell_text_field_uses_one_layout_for_caret_hit_and_horizontal_reveal() {
        let snapshot = DocumentSnapshot {
            text: Arc::from(""),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let mut projection = SourceProjection::new(&snapshot, 200, 80, 1.0);
        let text = "iiii 中文 WWWW 🙂 tail";
        let cursor = text.find("tail").unwrap() + "tail".len();
        let spec = UiTextSpec {
            x: 12.0,
            y: 8.0,
            width: 72.0,
            scale: 1.0,
        };
        let mut pixmap = Pixmap::new(200, 80).unwrap();

        let painted = projection
            .paint_ui_text_field(&mut pixmap, text, cursor, spec, SourceTheme::Light)
            .expect("field caret");
        let measured = projection
            .ui_text_field_caret(text, cursor, spec)
            .expect("measured caret");
        let hit = projection
            .ui_text_field_hit(text, cursor, spec, painted.x)
            .expect("field hit");

        assert_eq!(painted, measured);
        assert!(painted.x >= spec.x && painted.x <= spec.x + spec.width);
        assert_eq!(hit, cursor);
        assert!(pixmap.data().chunks_exact(4).any(|pixel| pixel[3] != 0));
    }
}
