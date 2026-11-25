//! Semantic batch conversion of Comrak-owned LaTeX math delimiters, written in the
//! standard form: `\(…\)` as `$…$` and `\[…\]` as `$$…$$`, with the whitespace just inside
//! the delimiters removed, which also normalizes `$$ … $$` (USER 2026-11-25).
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#semantic-math-delimiter-conversion

use std::ops::Range;

use stickymd_core::DocumentSnapshot;
use thiserror::Error;

use super::{
    BlockNode, InlineNode, MathNode, OwnedDocumentTree, PreviewParseError, PreviewParser,
    SourceRange,
};

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

    /// `map_position` for positions asked in non-decreasing order, as a walk over a document
    /// asks them: the replacements are passed once, so each call is amortized O(1) instead of
    /// O(replacements). A position below the previous one falls back to `map_position`, so the
    /// result is always the same.
    pub(crate) fn forward_position_map(&self) -> impl FnMut(usize) -> usize + '_ {
        let mut next = 0;
        let mut shift = 0_i64;
        let mut last = 0;
        move |position| {
            if position < last {
                return self.map_position(position);
            }
            last = position;
            // Invariant: `shift` is the byte delta of `replacements[..next]`, each of which ends
            // at or before `position`.
            while let Some(replacement) = self.replacements.get(next)
                && replacement.range.end <= position
            {
                shift += replacement.byte_delta();
                next += 1;
            }
            match self.replacements.get(next) {
                Some(replacement) if replacement.range.start <= position => {
                    replacement.map_inner_position(position, shift)
                }
                _ => shifted(position, shift),
            }
        }
    }
}

/// One formula rewritten: its old range, the new text, the new delimiters' lengths, and how
/// many bytes of whitespace the rewrite dropped just inside each old delimiter. Every old
/// delimiter (`\(`, `\)`, `\[`, `\]`, `$$`) is two bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DelimiterReplacement {
    range: SourceRange,
    replacement: String,
    new_open_len: usize,
    new_close_len: usize,
    lead: usize,
    trail: usize,
}

impl DelimiterReplacement {
    fn byte_delta(&self) -> i64 {
        self.replacement.len() as i64 - (self.range.end - self.range.start) as i64
    }

    /// A position inside the old formula: in a delimiter it clamps into the new delimiter, in
    /// dropped whitespace to the edge of the kept body, and in the kept body it keeps its
    /// offset.
    fn map_inner_position(&self, position: usize, shift: i64) -> usize {
        const OLD_DELIMITER_LEN: usize = 2;
        let old_body_start = self.range.start + OLD_DELIMITER_LEN;
        let old_body_end = self.range.end - OLD_DELIMITER_LEN;
        let kept_start = old_body_start + self.lead;
        let kept_end = old_body_end - self.trail;
        let new_start = shifted(self.range.start, shift);
        let new_body_start = new_start + self.new_open_len;
        let new_body_end = new_start + self.replacement.len() - self.new_close_len;

        if position <= old_body_start {
            new_start + (position - self.range.start).min(self.new_open_len)
        } else if position <= kept_start {
            new_body_start
        } else if position <= kept_end {
            new_body_start + position - kept_start
        } else if position <= old_body_end {
            new_body_end
        } else {
            new_body_end + (position - old_body_end).min(self.new_close_len)
        }
    }
}

/// How many bytes of whitespace to drop at the start and at the end of a formula body: the
/// spaces and tabs just inside the delimiters, and line breaks too when `line_breaks`. A
/// whitespace character right after an odd number of backslashes is a TeX control space
/// (`\ `), so it stays, with what follows it. None when nothing else is left.
///
/// Postcondition: `lead + trail < body.len()`, and both cut at UTF-8 boundaries (the dropped
/// bytes are ASCII).
pub(crate) fn math_body_trim(body: &str, line_breaks: bool) -> Option<(usize, usize)> {
    let blank =
        |byte: u8| matches!(byte, b' ' | b'\t') || line_breaks && matches!(byte, b'\n' | b'\r');
    let bytes = body.as_bytes();
    let lead = bytes.iter().take_while(|&&byte| blank(byte)).count();
    if lead == bytes.len() {
        return None;
    }
    let mut end = bytes.len() - bytes.iter().rev().take_while(|&&byte| blank(byte)).count();
    let backslashes = bytes[..end]
        .iter()
        .rev()
        .take_while(|&&byte| byte == b'\\')
        .count();
    if backslashes % 2 == 1 && end < bytes.len() {
        // Keep a `\r\n` pair whole, so no isolated `\r` is left.
        end += if bytes[end..].starts_with(b"\r\n") {
            2
        } else {
            1
        };
    }
    Some((lead, bytes.len() - end))
}

/// A formula literal without the whitespace that `math_body_trim` drops, line breaks
/// included: the read-back compares formulas by it, since that whitespace does not change
/// TeX math.
pub(crate) fn trimmed_math_literal(literal: &str) -> &str {
    match math_body_trim(literal, true) {
        Some((lead, trail)) => &literal[lead..literal.len() - trail],
        None => "",
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
    convert_parsed_latex_math_delimiters(&PreviewParser.parse(snapshot)?, scope)
}

/// `convert_latex_math_delimiters` on a tree the caller already parsed with
/// `PreviewParser`, so the `$` action can read the same parse first.
pub(crate) fn convert_parsed_latex_math_delimiters(
    tree: &OwnedDocumentTree,
    scope: Option<Range<usize>>,
) -> Result<Option<MathDelimiterConversion>, SemanticConversionError> {
    validate_scope(&tree.source, scope.as_ref())?;
    let mut replacements = Vec::new();
    collect_blocks(&tree.blocks, scope.as_ref(), &mut replacements);
    replacements.sort_unstable_by_key(|replacement| replacement.range.start);
    replacements.dedup_by_key(|replacement| replacement.range);
    if replacements.is_empty() {
        return Ok(None);
    }
    let text = apply_replacements(&tree.source, &replacements)?;
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

pub(crate) fn validate_scope(
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
    let source = &math.source_literal;
    // `$` may have no whitespace beside it, so `\(…\)` drops line breaks too; `$$…$$` keeps
    // them, as `$$\n…\n$$` is the standard block form.
    let (open, close, line_breaks) = if source.starts_with("\\(") && source.ends_with("\\)") {
        ("$", "$", true)
    } else if source.starts_with("\\[") && source.ends_with("\\]")
        || source.starts_with("$$") && source.ends_with("$$")
    {
        ("$$", "$$", false)
    } else {
        return;
    };
    if range.end - range.start < 4 || source.len() < 4 {
        return;
    }
    let body = &source[2..source.len() - 2];
    let Some((lead, mut trail)) = math_body_trim(body, line_breaks) else {
        return;
    };
    // A `$$` closer alone on its line keeps what stands before it there: that is the line's
    // indentation, or a quote's `> `, not the formula's. Comrak's literal, which leaves the
    // container prefixes out, tells whether the closer is alone.
    if !line_breaks
        && math
            .literal
            .rsplit_once('\n')
            .is_some_and(|(_, last)| last.bytes().all(|byte| matches!(byte, b' ' | b'\t')))
    {
        trail = 0;
    }
    // `$$…$$` that is already in the standard form stays as it is.
    if source.starts_with("$$") && lead == 0 && trail == 0 {
        return;
    }
    output.push(DelimiterReplacement {
        range,
        replacement: format!("{open}{}{close}", &body[lead..body.len() - trail]),
        new_open_len: open.len(),
        new_close_len: close.len(),
        lead,
        trail,
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

    /// `forward_position_map` gives `map_position`'s answer for every position, mapped in
    /// order, and still after a step back.
    #[test]
    fn phase11b_conversion_forward_position_map_matches_map_position() {
        let source = "a \\( b\t\\) c \\[ d \\] 甲 $$ e $$ \\(f\\)";
        let conversion = converted(source, None).unwrap();
        assert_eq!(conversion.replacement_count(), 4);
        assert_eq!(conversion.text(), "a $b$ c $$d$$ 甲 $$e$$ $f$");
        let mut map = conversion.forward_position_map();
        for position in 0..=source.len() {
            assert_eq!(
                map(position),
                conversion.map_position(position),
                "{position}"
            );
        }
        for position in [3, source.len(), 0, 9, 9, 20] {
            assert_eq!(
                map(position),
                conversion.map_position(position),
                "{position}"
            );
        }
    }

    /// The standard form (plan 07, USER 2026-11-25): the whitespace just inside the
    /// delimiters goes, line breaks too for `$…$`, which may have no whitespace beside its
    /// `$`; `$$…$$` keeps its line breaks. An escaped space stays; a formula with nothing
    /// else is not rewritten; one already in the standard form is left alone.
    #[test]
    fn phase11b_conversion_writes_the_standard_form() {
        for (source, expected) in [
            ("\\( a \\)", Some("$a$")),
            ("\\(a \\)", Some("$a$")),
            ("\\( a\\)", Some("$a$")),
            ("\\(\ta\t\\)", Some("$a$")),
            ("\\(\na\n\\)", Some("$a$")),
            ("\\(\r\na\r\n\\)", Some("$a$")),
            ("\\(\na b\n\\)", Some("$a b$")),
            ("\\( a\\ \\)", Some("$a\\ $")),
            ("\\( a\\\\ \\)", Some("$a\\\\$")),
            ("\\(a\\\r\n\\)", Some("$a\\\r\n$")),
            ("\\[ a \\]", Some("$$a$$")),
            ("\\[\na\n\\]", Some("$$\na\n$$")),
            ("\\[ \na\n \\]", Some("$$\na\n $$")),
            ("- \\[\n  a\n  \\]", Some("- $$\n  a\n  $$")),
            ("> \\[ \n> a\n> \\]", Some("> $$\n> a\n> $$")),
            ("$$ a $$", Some("$$a$$")),
            ("$$\ta$$", Some("$$a$$")),
            ("$$ \na\n $$", Some("$$\na\n $$")),
            ("x $$ a $$ y", Some("x $$a$$ y")),
            ("\\( \\)", None),
            ("\\(  \\)", None),
            ("\\(\\)", None),
            ("\\[ \\]", None),
            ("$$ $$", None),
            ("$$a$$", None),
            ("$$\na\n$$", None),
            ("$a$", None),
        ] {
            assert_eq!(
                converted(source, None).map(|conversion| conversion.text().to_owned()),
                expected.map(str::to_owned),
                "{source:?}"
            );
        }
    }

    /// Positions in the whitespace a rewrite drops clamp to the edge of the kept body.
    #[test]
    fn phase11b_conversion_maps_dropped_whitespace_to_the_kept_body() {
        let conversion = converted("\\(  a  \\) z", None).unwrap();
        assert_eq!(conversion.text(), "$a$ z");
        let mapped: Vec<_> = (0..=11).map(|at| conversion.map_position(at)).collect();
        assert_eq!(mapped, [0, 1, 1, 1, 1, 2, 2, 2, 3, 3, 4, 5]);
    }

    #[test]
    fn phase11b_overlapping_projection_ranges_fail_before_building_text() {
        let replacements = [
            DelimiterReplacement {
                range: SourceRange { start: 0, end: 4 },
                replacement: "$a$".to_owned(),
                new_open_len: 1,
                new_close_len: 1,
                lead: 0,
                trail: 0,
            },
            DelimiterReplacement {
                range: SourceRange { start: 3, end: 7 },
                replacement: "$b$".to_owned(),
                new_open_len: 1,
                new_close_len: 1,
                lead: 0,
                trail: 0,
            },
        ];
        assert_eq!(
            apply_replacements("abcdefgh", &replacements),
            Err(SemanticConversionError::OverlappingSourceRanges)
        );
    }

    #[test]
    fn phase11b_conversion_after_reference_definitions_leaves_titles() {
        // Comrak 1.0 and later report the inlines of a block that starts with
        // definitions in place (0.59 shifted them up by the removed lines), so
        // its formulas convert and a definition title is never rewritten.
        assert!(converted("[x]: u \"\\(abc\\)\"\n12345678$$c=d$$", None).is_none());
        for (source, expected) in [
            (
                "[x]: u \"\\(a\\)\"\n12345678\\(c\\)",
                "[x]: u \"\\(a\\)\"\n12345678$c$",
            ),
            (
                "[x]: u \"\\(a\\)\"\n12345678\\(c\\) tail",
                "[x]: u \"\\(a\\)\"\n12345678$c$ tail",
            ),
            (
                "[x]: u \"\\(a\\)\"\nbar \\(c\\)\n===",
                "[x]: u \"\\(a\\)\"\nbar $c$\n===",
            ),
            ("[x]: u \"\\[t\\]\"\n\\[a\\]", "[x]: u \"\\[t\\]\"\n$$a$$"),
            ("[x]: u\n\n12345678\\(c\\)", "[x]: u\n\n12345678$c$"),
            ("foo\n===\n\\(c\\)", "foo\n===\n$c$"),
            ("# a \\(c\\) ##", "# a $c$ ##"),
            ("bar \\(c\\)  \nnext  ", "bar $c$  \nnext  "),
            ("> quote \\(c\\)\nlazy", "> quote $c$\nlazy"),
            ("\\(a\\)\n  `b\nc`", "$a$\n  `b\nc`"),
            // A paragraph that ends in a link whose destination and title span
            // lines, with a `]:` elsewhere or not.
            ("\\(a\\)\n[x](u\n\"t\")", "$a$\n[x](u\n\"t\")"),
            (
                "\\(a\\)\n[x](u\n\"t\")\n\n```\n]:\n```",
                "$a$\n[x](u\n\"t\")\n\n```\n]:\n```",
            ),
        ] {
            assert_eq!(
                converted(source, None).map(MathDelimiterConversion::into_text),
                Some(expected.to_owned()),
                "{source:?}"
            );
        }
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
