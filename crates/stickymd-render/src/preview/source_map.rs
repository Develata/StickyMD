//! Comrak source-position to canonical UTF-8 byte-range conversion.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#owned-ast-projection

use std::ops::Range;

use comrak::nodes::{AstNode, NodeValue, Sourcepos};

use super::SourceRange;

/// Comrak 0.59 drops link reference definitions from the start of a paragraph
/// (or of the text a Setext underline turns into a heading) without moving its
/// start line, so every inline position in that block is reported shifted up
/// by the removed lines (fixed upstream in 1.0.0-rc.1). Reported lines stay
/// exact, and a block's own end line is right: its last inline ends on it, or
/// one line above a Setext underline. A block whose inlines end earlier is
/// shifted.
pub(crate) fn inlines_shifted<'a>(block: &'a AstNode<'a>) -> bool {
    let data = block.data.borrow();
    let text_end = match &data.value {
        NodeValue::Paragraph => data.sourcepos.end.line,
        NodeValue::Heading(heading) if heading.setext => data.sourcepos.end.line.saturating_sub(1),
        _ => return false,
    };
    block
        .descendants()
        .skip(1)
        .map(|inline| inline.data.borrow().sourcepos.end.line)
        .max()
        .is_some_and(|last| last < text_end)
}

/// Byte spans, by whole lines and in document order, of the blocks under
/// `root` whose inline positions are shifted. Only a source holding `]:` can
/// contain a link reference definition, so any other source is not walked.
pub(super) fn shifted_block_spans<'a>(
    root: &'a AstNode<'a>,
    source: &str,
    map: &SourceMap,
) -> Vec<Range<usize>> {
    if !source.contains("]:") {
        return Vec::new();
    }
    root.descendants()
        .filter(|&node| inlines_shifted(node))
        .filter_map(|node| map.whole_lines(node.data.borrow().sourcepos))
        .collect()
}

/// Immutable line index for the exact canonical snapshot parsed by Comrak.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct SourceMap {
    source_len: usize,
    line_starts: Vec<usize>,
}

impl SourceMap {
    pub(super) fn new(source: &str) -> Self {
        Self {
            source_len: source.len(),
            line_starts: std::iter::once(0)
                .chain(source.match_indices('\n').map(|(index, _)| index + 1))
                .collect(),
        }
    }

    /// Convert Comrak's 1-based byte columns and inclusive end position into
    /// a half-open canonical byte range. Invalid/synthetic positions fail
    /// conservatively instead of guessing.
    pub(super) fn range(&self, source: &str, position: Sourcepos) -> Option<SourceRange> {
        if source.len() != self.source_len
            || position.start.line == 0
            || position.end.line == 0
            || position.start.column == 0
            || position.end.column == 0
        {
            return None;
        }
        let start_line = *self.line_starts.get(position.start.line - 1)?;
        let end_line = *self.line_starts.get(position.end.line - 1)?;
        let start = start_line.checked_add(position.start.column - 1)?;
        // Comrak columns are byte-oriented and `end` is inclusive. Therefore
        // line_start + end.column is the exclusive byte offset.
        let end = end_line.checked_add(position.end.column)?;
        let range = SourceRange::new(start, end)?;
        (end <= source.len() && source.is_char_boundary(start) && source.is_char_boundary(end))
            .then_some(range)
    }

    /// Whole lines from a block's first line to its last, before that line's
    /// `\n`.
    fn whole_lines(&self, position: Sourcepos) -> Option<Range<usize>> {
        let start = *self.line_starts.get(position.start.line.checked_sub(1)?)?;
        let end = self
            .line_starts
            .get(position.end.line)
            .map_or(self.source_len, |next| next - 1);
        (start <= end).then_some(start..end)
    }

    pub(super) fn table_cell(
        &self,
        source: &str,
        position: Sourcepos,
    ) -> Option<TableCellSourceMap> {
        let cell_source = self.range(source, position)?.text(source)?;
        TableCellSourceMap::new(cell_source, position)
    }
}

/// Inverse byte-coordinate projection for Comrak 0.59's table-cell preprocessing
/// (upstream reports canonical columns from 1.0.0-rc.1).
/// Comrak removes the backslash in `\|` before parsing inlines, but reports their
/// columns in that shortened input. Cell boundaries still address canonical text.
/// This runs only on Comrak-confirmed cells: it never recognizes Markdown or math.
pub(super) struct TableCellSourceMap {
    cell: Sourcepos,
    removed_columns: Vec<usize>,
}

impl TableCellSourceMap {
    fn new(source: &str, cell: Sourcepos) -> Option<Self> {
        if cell.start.line != cell.end.line {
            return None;
        }
        let mut removed_columns = Vec::new();
        let mut after_backslash = false;
        for (offset, byte) in source.bytes().enumerate() {
            if after_backslash {
                if byte == b'|' {
                    // Store the removed byte's column in the shortened input.
                    removed_columns
                        .push(cell.start.column.checked_add(offset - 1)? - removed_columns.len());
                }
                after_backslash = false;
            } else {
                after_backslash = byte == b'\\';
            }
        }
        // Ordinary cells need no allocation or per-inline coordinate adjustment.
        (!removed_columns.is_empty()).then_some(Self {
            cell,
            removed_columns,
        })
    }

    pub(super) fn restore(&self, mut position: Sourcepos) -> Option<Sourcepos> {
        if position.start.line != self.cell.start.line || position.end.line != self.cell.end.line {
            return None;
        }
        // Starts include a backslash removed at that boundary; inclusive ends
        // include the corresponding pipe. Both are half-open byte boundaries
        // before SourceMap checks canonical UTF-8 boundaries.
        position.start.column = position.start.column.checked_add(
            self.removed_columns
                .partition_point(|&column| column < position.start.column),
        )?;
        position.end.column = position.end.column.checked_add(
            self.removed_columns
                .partition_point(|&column| column <= position.end.column),
        )?;
        (position.start.column >= self.cell.start.column
            && position.end.column <= self.cell.end.column
            && position.start.column <= position.end.column)
            .then_some(position)
    }
}

#[cfg(test)]
mod tests {
    use comrak::nodes::Sourcepos;

    use super::*;

    #[test]
    fn converts_ascii_cjk_emoji_and_multiline_byte_columns() {
        let source = "a中🙂\nsecond";
        let map = SourceMap::new(source);
        for (position, expected) in [
            ((1, 1, 1, 1), 0..1),
            ((1, 2, 1, 4), 1..4),
            ((1, 5, 1, 8), 4..8),
            ((2, 1, 2, 6), 9..15),
            ((1, 2, 2, 6), 1..15),
        ] {
            let range = map
                .range(source, Sourcepos::from(position))
                .expect("valid source position");
            assert_eq!(range.as_range(), expected);
        }
    }

    #[test]
    fn invalid_or_non_boundary_position_is_unknown() {
        let source = "中";
        let map = SourceMap::new(source);
        assert_eq!(map.range(source, Sourcepos::from((0, 0, 0, 0))), None);
        assert_eq!(map.range(source, Sourcepos::from((1, 2, 1, 2))), None);
        assert_eq!(map.range("different", Sourcepos::from((1, 1, 1, 1))), None);
    }

    #[test]
    fn table_positions_are_restored_before_utf8_validation_and_stay_in_the_cell() {
        let source = "| \\|中 |";
        let source_map = SourceMap::new(source);
        let map = source_map
            .table_cell(source, Sourcepos::from((1, 2, 1, 8)))
            .unwrap();
        for (position, expected) in [
            ((1, 3, 1, 3), "\\|"),
            ((1, 4, 1, 6), "中"),
            ((1, 3, 1, 6), "\\|中"),
        ] {
            let range = source_map
                .range(source, map.restore(position.into()).unwrap())
                .unwrap();
            assert_eq!(range.text(source), Some(expected));
        }
        for invalid in [(0, 0, 0, 0), (2, 3, 2, 3), (1, 1, 1, 3), (1, 3, 1, 8)] {
            assert_eq!(map.restore(invalid.into()), None);
        }
    }
}
