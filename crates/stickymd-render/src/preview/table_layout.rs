//! Content-sized table columns and block-local overflow geometry.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use cosmic_text::{Align, FontSystem, Metrics};

use crate::image::DecodedImageCache;
use crate::math::MathEngine;
use crate::source::FontSelection;

use super::image_requests::ImageRequests;
use super::layout::{BlockBuild, DecorationRole, LayoutDecoration, make_chunk};
use super::{PreviewRect, RenderBlock, RenderTable, TableAlignment};

const BODY_SIZE_DIP: f32 = 17.0;
const BODY_LINE_DIP: f32 = 26.35;
const INDENT_DIP: f32 = 16.0;
const TABLE_CELL_PAD_DIP: f32 = 8.0;

#[allow(clippy::too_many_arguments)]
pub(super) fn layout_table(
    requested_scroll_x: f32,
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    math_engine: &mut MathEngine,
    block: &RenderBlock,
    table: &RenderTable,
    padding: f32,
    y: f32,
    content_width: f32,
    scale: f32,
    selection_text: &mut String,
    formula_count: &mut usize,
    theme: super::PreviewTheme,
    mut image_source: Option<&mut ImageRequests<'_>>,
    image_cache: &mut DecodedImageCache,
    image_band: (f32, f32),
    text_layout_cache: &mut super::text_layout::TextLayoutCache,
) -> BlockBuild {
    let columns = table
        .rows
        .iter()
        .map(|row| row.cells.len())
        .max()
        .unwrap_or(1)
        .max(table.alignments.len())
        .max(1);
    let indent = block.indent as f32 * INDENT_DIP * scale;
    let table_x = padding + indent;
    let table_width = (content_width - indent).max(1.0);
    let cell_padding = TABLE_CELL_PAD_DIP * scale;
    let metrics = Metrics::new(BODY_SIZE_DIP * scale, BODY_LINE_DIP * scale);
    // Measure each cell at a readable wrap width. Reuse the existing shaper and
    // math engine; this pass owns no selection projection or image pixels.
    let mut widths = vec![64.0 * scale; columns];
    let mut scratch = String::new();
    let mut measured_formula_count = *formula_count;
    for row in &table.rows {
        for (column, spans) in row.cells.iter().enumerate() {
            scratch.clear();
            let measured = make_chunk(
                font_system,
                fonts,
                math_engine,
                spans,
                0.0,
                0.0,
                320.0 * scale,
                metrics,
                Align::Left,
                &mut scratch,
                &mut measured_formula_count,
                theme,
                image_source.as_deref_mut(),
                image_cache,
                (f32::MAX, f32::MAX),
                text_layout_cache,
            );
            widths[column] = widths[column].max(measured.right_edge() + cell_padding * 2.0);
        }
    }
    let measured_width: f32 = widths.iter().sum();
    if measured_width < table_width {
        for width in &mut widths {
            *width += (table_width - measured_width) / columns as f32;
        }
    }
    let content_width: f32 = widths.iter().sum();
    let scroll_x = requested_scroll_x.clamp(0.0, (content_width - table_width).max(0.0));
    let mut positions = Vec::with_capacity(columns);
    let mut next_x = table_x;
    for width in &widths {
        positions.push(next_x);
        next_x += width;
    }
    let mut row_y = y;
    let mut chunks = Vec::new();
    let mut decorations = Vec::new();
    let mut boxes = Vec::new();

    for (row_index, row) in table.rows.iter().enumerate() {
        let mut row_chunks = Vec::new();
        let mut row_decorations = Vec::new();
        let mut row_height = metrics.line_height + cell_padding * 2.0;
        for column in 0..columns {
            let cell_x = positions[column];
            let column_width = widths[column];
            let spans = row.cells.get(column).map_or(&[][..], Vec::as_slice);
            let align = match table
                .alignments
                .get(column)
                .copied()
                .unwrap_or(TableAlignment::None)
            {
                TableAlignment::Center => Align::Center,
                TableAlignment::Right => Align::Right,
                TableAlignment::None | TableAlignment::Left => Align::Left,
            };
            let built = make_chunk(
                font_system,
                fonts,
                math_engine,
                spans,
                cell_x + cell_padding,
                row_y + cell_padding,
                (column_width - cell_padding * 2.0).max(1.0),
                metrics,
                align,
                selection_text,
                formula_count,
                theme,
                image_source.as_deref_mut(),
                image_cache,
                // Offscreen columns retain metadata and source mapping only.
                // ImageSlots refreshes their rasters when horizontal input reveals them.
                if cell_x + column_width - scroll_x >= table_x
                    && cell_x - scroll_x <= table_x + table_width
                {
                    image_band
                } else {
                    (f32::MAX, f32::MAX)
                },
                text_layout_cache,
            );
            row_height = row_height.max(built.height + cell_padding * 2.0);
            boxes.extend(built.boxes);
            row_chunks.extend(built.chunks);
            row_decorations.extend(built.decorations);
            if column + 1 < columns {
                selection_text.push('\t');
            }
        }
        for column in 0..columns {
            decorations.push(LayoutDecoration {
                rect: PreviewRect {
                    x: positions[column],
                    y: row_y,
                    width: widths[column],
                    height: row_height,
                },
                role: if row.header {
                    DecorationRole::TableHeader
                } else {
                    DecorationRole::TableCell
                },
            });
        }
        chunks.extend(row_chunks);
        decorations.extend(row_decorations);
        if row_index + 1 < table.rows.len() {
            selection_text.push('\n');
        }
        row_y += row_height;
    }

    let height = (row_y - y).max(metrics.line_height + cell_padding * 2.0);
    let horizontal = super::horizontal::HorizontalLayout::new(
        table_x,
        y,
        table_width,
        height,
        content_width,
        scale,
    );
    BlockBuild {
        height: height + horizontal.map_or(0.0, |value| value.bar_height),
        horizontal,
        chunks,
        decorations,
        boxes,
    }
}
