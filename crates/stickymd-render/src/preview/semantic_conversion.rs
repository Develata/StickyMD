//! Semantic batch conversion of Comrak-owned LaTeX math delimiters.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#semantic-math-delimiter-conversion

use std::ops::Range;

use stickymd_core::DocumentSnapshot;
use thiserror::Error;

use super::{BlockNode, InlineNode, MathNode, PreviewParseError, PreviewParser, SourceRange};

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SemanticConversionError {
    #[error(transparent)]
    Parse(#[from] PreviewParseError),
    #[error("conversion scope is not a valid UTF-8 range")]
    InvalidScope,
    #[error("semantic conversion source ranges overlap")]
    OverlappingSourceRanges,
    #[error("semantic conversion source range is outside UTF-8 source boundaries")]
    InvalidSourceRange,
}

/// One immutable conversion projection. Applying it remains the document
/// coordinator's responsibility so this type can never become text authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathDelimiterConversion {
    text: String,
    replacements: Vec<DelimiterReplacement>,
}

impl MathDelimiterConversion {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn into_text(self) -> String {
        self.text
    }

    pub fn replacement_count(&self) -> usize {
        self.replacements.len()
    }

    /// Maps one old canonical byte position through all delimiter-length
    /// changes. Positions in formula bodies preserve their relative byte
    /// offset; positions inside a delimiter clamp into the new delimiter.
    pub fn map_position(&self, position: usize) -> usize {
        let mut shift = 0_i64;
        for replacement in &self.replacements {
            if position < replacement.range.start {
                break;
            }
            if position >= replacement.range.end {
                shift += replacement.byte_delta();
                continue;
            }
            return replacement.map_inner_position(position, shift);
        }
        shifted(position, shift)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DelimiterReplacement {
    range: SourceRange,
    replacement: String,
    new_open_len: usize,
    new_close_len: usize,
}

impl DelimiterReplacement {
    fn byte_delta(&self) -> i64 {
        self.replacement.len() as i64 - (self.range.end - self.range.start) as i64
    }

    fn map_inner_position(&self, position: usize, shift: i64) -> usize {
        const OLD_DELIMITER_LEN: usize = 2;
        let old_body_start = self.range.start + OLD_DELIMITER_LEN;
        let old_body_end = self.range.end - OLD_DELIMITER_LEN;
        let new_start = shifted(self.range.start, shift);
        let new_body_start = new_start + self.new_open_len;
        let new_body_end = new_start + self.replacement.len() - self.new_close_len;

        if position <= old_body_start {
            new_start + (position - self.range.start).min(self.new_open_len)
        } else if position <= old_body_end {
            new_body_start + position - old_body_start
        } else {
            new_body_end + (position - old_body_end).min(self.new_close_len)
        }
    }
}

fn shifted(position: usize, shift: i64) -> usize {
    if shift >= 0 {
        position.saturating_add(shift as usize)
    } else {
        position.saturating_sub((-shift) as usize)
    }
}

/// Converts only semantic Comrak math nodes written with `\\(...\\)` or
/// `\\[...\\]`. With a scope, a node is eligible only when its complete
/// source range is contained by that half-open range.
pub fn convert_latex_math_delimiters(
    snapshot: &DocumentSnapshot,
    scope: Option<Range<usize>>,
) -> Result<Option<MathDelimiterConversion>, SemanticConversionError> {
    validate_scope(&snapshot.text, scope.as_ref())?;
    let tree = PreviewParser.parse(snapshot)?;
    let mut replacements = Vec::new();
    collect_blocks(&tree.blocks, scope.as_ref(), &mut replacements);
    replacements.sort_unstable_by_key(|replacement| replacement.range.start);
    replacements.dedup_by_key(|replacement| replacement.range);
    if replacements.is_empty() {
        return Ok(None);
    }
    let text = apply_replacements(&snapshot.text, &replacements)?;
    Ok(Some(MathDelimiterConversion { text, replacements }))
}

/// Builds the converted source in one forward pass. Repeated `replace_range`
/// would shift the remaining document once per formula and degrade to
/// O(document bytes * formula count) on a large note.
fn apply_replacements(
    source: &str,
    replacements: &[DelimiterReplacement],
) -> Result<String, SemanticConversionError> {
    let mut text = String::with_capacity(source.len());
    let mut source_cursor = 0;
    for replacement in replacements {
        if replacement.range.start < source_cursor {
            return Err(SemanticConversionError::OverlappingSourceRanges);
        }
        let unchanged = source
            .get(source_cursor..replacement.range.start)
            .ok_or(SemanticConversionError::InvalidSourceRange)?;
        if source
            .get(replacement.range.start..replacement.range.end)
            .is_none()
        {
            return Err(SemanticConversionError::InvalidSourceRange);
        }
        text.push_str(unchanged);
        text.push_str(&replacement.replacement);
        source_cursor = replacement.range.end;
    }
    text.push_str(
        source
            .get(source_cursor..)
            .ok_or(SemanticConversionError::InvalidSourceRange)?,
    );
    Ok(text)
}

fn validate_scope(
    source: &str,
    scope: Option<&Range<usize>>,
) -> Result<(), SemanticConversionError> {
    let Some(scope) = scope else {
        return Ok(());
    };
    if scope.start > scope.end
        || scope.end > source.len()
        || !source.is_char_boundary(scope.start)
        || !source.is_char_boundary(scope.end)
    {
        return Err(SemanticConversionError::InvalidScope);
    }
    Ok(())
}

fn collect_blocks(
    blocks: &[BlockNode],
    scope: Option<&Range<usize>>,
    output: &mut Vec<DelimiterReplacement>,
) {
    for block in blocks {
        match block {
            BlockNode::Paragraph { content, .. } | BlockNode::Heading { content, .. } => {
                collect_inlines(content, scope, output);
            }
            BlockNode::BlockQuote { blocks, .. } => collect_blocks(blocks, scope, output),
            BlockNode::List(list) => {
                for item in &list.items {
                    collect_blocks(&item.blocks, scope, output);
                }
            }
            BlockNode::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        collect_inlines(&cell.content, scope, output);
                    }
                }
            }
            BlockNode::DisplayMath(math) => collect_math(math, scope, output),
            BlockNode::CodeBlock(_)
            | BlockNode::ThematicBreak { .. }
            | BlockNode::HtmlLiteral { .. } => {}
        }
    }
}

fn collect_inlines(
    inlines: &[InlineNode],
    scope: Option<&Range<usize>>,
    output: &mut Vec<DelimiterReplacement>,
) {
    for inline in inlines {
        match inline {
            InlineNode::Emphasis { children, .. }
            | InlineNode::Strong { children, .. }
            | InlineNode::Strikethrough { children, .. }
            | InlineNode::Link { children, .. } => collect_inlines(children, scope, output),
            InlineNode::InlineMath(math) => collect_math(math, scope, output),
            InlineNode::Text { .. }
            | InlineNode::Code { .. }
            | InlineNode::Image { .. }
            | InlineNode::SoftBreak { .. }
            | InlineNode::HardBreak { .. }
            | InlineNode::HtmlLiteral { .. } => {}
        }
    }
}

fn collect_math(
    math: &MathNode,
    scope: Option<&Range<usize>>,
    output: &mut Vec<DelimiterReplacement>,
) {
    let Some(range) = math.source_range else {
        return;
    };
    if scope.is_some_and(|scope| range.start < scope.start || range.end > scope.end) {
        return;
    }
    let (open, close) =
        if math.source_literal.starts_with("\\(") && math.source_literal.ends_with("\\)") {
            ("$", "$")
        } else if math.source_literal.starts_with("\\[") && math.source_literal.ends_with("\\]") {
            ("$$", "$$")
        } else {
            return;
        };
    if range.end - range.start < 4 {
        return;
    }
    let body = &math.source_literal[2..math.source_literal.len() - 2];
    output.push(DelimiterReplacement {
        range,
        replacement: format!("{open}{body}{close}"),
        new_open_len: open.len(),
        new_close_len: close.len(),
    });
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use stickymd_core::{DocumentSnapshot, Generation, LineEnding};

    use super::*;

    fn snapshot(source: &str) -> DocumentSnapshot {
        DocumentSnapshot {
            text: Arc::from(source),
            generation: Generation::initial(),
            line_ending: LineEnding::Crlf,
        }
    }

    fn converted(source: &str, scope: Option<Range<usize>>) -> Option<MathDelimiterConversion> {
        convert_latex_math_delimiters(&snapshot(source), scope).unwrap()
    }

    #[test]
    fn phase11b_converts_semantic_inline_and_display_latex_delimiters() {
        let source = "前 \\(x+\u{4e2d}\\) 后\n\n\\[a\\\\b\\]";
        let conversion = converted(source, None).unwrap();
        assert_eq!(conversion.text(), "前 $x+中$ 后\n\n$$a\\\\b$$");
        assert_eq!(conversion.replacement_count(), 2);
    }

    #[test]
    fn phase11b_multiline_display_formula_preserves_body_bytes_exactly() {
        let source = "before\n\n\\[\n\\frac{a_中}{b}\\\\\nc+d\n\\]\n\nafter";
        let conversion = converted(source, None).unwrap();
        assert_eq!(
            conversion.text(),
            "before\n\n$$\n\\frac{a_中}{b}\\\\\nc+d\n$$\n\nafter"
        );
    }

    #[test]
    fn phase11b_leaves_dollars_code_plain_text_and_malformed_delimiters_unchanged() {
        let source = "$x$ and $$y$$ and `\\(z\\)`\n\n```text\n\\[code\\]\n```\n\n\\(broken";
        assert!(converted(source, None).is_none());
    }

    #[test]
    fn phase11b_selection_converts_only_fully_contained_semantic_nodes() {
        let source = "\\(a\\) xx \\(b\\) yy \\(c\\)";
        let second_start = source.find("\\(b").unwrap();
        let second_end = second_start + "\\(b\\)".len();
        let conversion = converted(source, Some(second_start..second_end)).unwrap();
        assert_eq!(conversion.text(), "\\(a\\) xx $b$ yy \\(c\\)");

        let partial = converted(source, Some(second_start + 1..second_end));
        assert!(partial.is_none());
    }

    #[test]
    fn phase11b_position_mapping_preserves_body_and_later_offsets() {
        let source = "a \\(xy\\) b \\[z\\] c";
        let conversion = converted(source, None).unwrap();
        let x = source.find('x').unwrap();
        let z = source.find('z').unwrap();
        assert_eq!(
            conversion.map_position(x),
            conversion.text().find('x').unwrap()
        );
        assert_eq!(
            conversion.map_position(z),
            conversion.text().find('z').unwrap()
        );
        assert_eq!(
            conversion.map_position(source.len()),
            conversion.text().len()
        );
    }

    #[test]
    fn phase11b_invalid_scope_is_rejected_without_parsing() {
        let source = "中 \\(x\\)";
        assert_eq!(
            convert_latex_math_delimiters(&snapshot(source), Some(1..source.len())),
            Err(SemanticConversionError::InvalidScope)
        );
    }

    #[test]
    fn phase11b_overlapping_projection_ranges_fail_before_building_text() {
        let replacements = [
            DelimiterReplacement {
                range: SourceRange { start: 0, end: 4 },
                replacement: "$a$".to_owned(),
                new_open_len: 1,
                new_close_len: 1,
            },
            DelimiterReplacement {
                range: SourceRange { start: 3, end: 7 },
                replacement: "$b$".to_owned(),
                new_open_len: 1,
                new_close_len: 1,
            },
        ];
        assert_eq!(
            apply_replacements("abcdefgh", &replacements),
            Err(SemanticConversionError::OverlappingSourceRanges)
        );
    }

    #[test]
    #[ignore = "Release-only Phase 11-B semantic conversion receipt"]
    fn phase11b_performance_one_mib_with_one_thousand_math_nodes() {
        let formula = "\\(x^2+y^2\\)\n";
        let mut source = formula.repeat(1_000);
        source.push_str(&"plain text filler ".repeat((1024 * 1024 - source.len()) / 18));
        while source.len() < 1024 * 1024 {
            source.push('x');
        }
        let snapshot = snapshot(&source);
        let mut samples = Vec::with_capacity(25);
        for _ in 0..25 {
            let started = Instant::now();
            let conversion = convert_latex_math_delimiters(&snapshot, None)
                .unwrap()
                .unwrap();
            assert_eq!(conversion.replacement_count(), 1_000);
            samples.push(started.elapsed());
        }
        samples.sort_unstable();
        let p95 = samples[samples.len() * 95 / 100];
        eprintln!(
            "phase11b semantic_conversion_1mib_1000_math median={:?} p95={p95:?} max={:?}",
            samples[samples.len() / 2],
            samples[samples.len() - 1]
        );
        assert!(p95 < Duration::from_millis(50), "p95={p95:?}");
    }
}
