//! Native table layout with fixed, bounded columns.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use cosmic_text::{Align, FontSystem, Metrics};

use crate::source::FontSelection;

use super::layout::{BlockBuild, DecorationRole, LayoutDecoration, make_chunk};
use super::{PreviewRect, RenderBlock, RenderTable, TableAlignment};

const BODY_SIZE_DIP: f32 = 17.0;
const BODY_LINE_DIP: f32 = 26.35;
const INDENT_DIP: f32 = 16.0;
const TABLE_CELL_PAD_DIP: f32 = 8.0;

#[allow(clippy::too_many_arguments)]
pub(super) fn layout_table(
    font_system: &mut FontSystem,
    fonts: &FontSelection,
    block: &RenderBlock,
    table: &RenderTable,
    padding: f32,
    y: f32,
    content_width: f32,
    scale: f32,
    selection_text: &mut String,
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
    let column_width = table_width / columns as f32;
    let cell_padding = TABLE_CELL_PAD_DIP * scale;
    let metrics = Metrics::new(BODY_SIZE_DIP * scale, BODY_LINE_DIP * scale);
    let mut row_y = y;
    let mut chunks = Vec::new();
    let mut decorations = Vec::new();
    let mut boxes = Vec::new();

    for (row_index, row) in table.rows.iter().enumerate() {
        let mut row_chunks = Vec::new();
        let mut row_height = metrics.line_height + cell_padding * 2.0;
        for column in 0..columns {
            let cell_x = table_x + column as f32 * column_width;
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
                spans,
                cell_x + cell_padding,
                row_y + cell_padding,
                (column_width - cell_padding * 2.0).max(1.0),
                metrics,
                align,
                selection_text,
            );
            row_height = row_height.max(built.height + cell_padding * 2.0);
            boxes.extend(built.boxes);
            row_chunks.push(built.chunk);
            if column + 1 < columns {
                selection_text.push('\t');
            }
        }
        for column in 0..columns {
            decorations.push(LayoutDecoration {
                rect: PreviewRect {
                    x: table_x + column as f32 * column_width,
                    y: row_y,
                    width: column_width,
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
        if row_index + 1 < table.rows.len() {
            selection_text.push('\n');
        }
        row_y += row_height;
    }

    BlockBuild {
        height: (row_y - y).max(metrics.line_height + cell_padding * 2.0),
        chunks,
        decorations,
        boxes,
    }
}
