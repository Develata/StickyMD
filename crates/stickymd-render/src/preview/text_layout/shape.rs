//! Immutable shaping and row locators shared only within one document layout.
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use cosmic_text::Buffer;

pub(super) struct TextShape {
    // Only this module and its parent can reach the Buffer; production callers
    // retain it behind Arc and never mutate it after constructing the locators.
    pub(super) buffer: Buffer,
    pub(super) rows: Vec<TextLayoutRow>,
    pub(super) max_glyph_y_offset: f32,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct TextLayoutRow {
    pub(super) logical_line: usize,
    pub(super) layout_row: usize,
    pub(super) logical_byte_start: usize,
    pub(super) top: f32,
    pub(super) height: f32,
    pub(super) baseline: f32,
}

impl TextLayoutRow {
    pub(super) fn bottom(self) -> f32 {
        self.top + self.height
    }
}

impl TextShape {
    pub(super) fn new(buffer: Buffer) -> Self {
        // Cosmic Text owns all glyph geometry. Derive the paint margin once in
        // O(glyphs), then build fixed-width row locators in O(visual rows).
        let max_glyph_y_offset = buffer
            .layout_runs()
            .flat_map(|run| run.glyphs)
            .fold(0.0f32, |offset, glyph| {
                offset.max((glyph.y - glyph.font_size * glyph.y_offset).abs())
            });
        let mut logical_byte_starts = Vec::with_capacity(buffer.lines.len());
        let mut logical_byte_start = 0usize;
        for line in &buffer.lines {
            logical_byte_starts.push(logical_byte_start);
            logical_byte_start = logical_byte_start
                .saturating_add(line.text().len())
                .saturating_add(line.ending().as_str().len());
        }
        let mut next_layout_row = vec![0usize; buffer.lines.len()];
        let rows = buffer
            .layout_runs()
            .filter_map(|run| {
                let layout_row = next_layout_row.get_mut(run.line_i)?;
                let row = TextLayoutRow {
                    logical_line: run.line_i,
                    layout_row: *layout_row,
                    logical_byte_start: *logical_byte_starts.get(run.line_i)?,
                    top: run.line_top,
                    height: run.line_height,
                    baseline: run.line_y,
                };
                *layout_row = layout_row.saturating_add(1);
                Some(row)
            })
            .collect();
        Self {
            buffer,
            rows,
            max_glyph_y_offset,
        }
    }
}
