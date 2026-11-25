//! Comrak source-position to canonical UTF-8 byte-range conversion.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#owned-ast-projection

use comrak::nodes::Sourcepos;

use super::SourceRange;

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
}
