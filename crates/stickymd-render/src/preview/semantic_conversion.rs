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

    /// The indices of the rewrites whose formula `converted`, the Preview's parse of this
    /// conversion's text, does not read back as itself (plan 07): one math node at the
    /// rewritten range, display math exactly when the new delimiters are `$$`, and, for a
    /// rewritten formula, its literal (without the whitespace just inside) the one Comrak read
    /// before the rewrite. Single-`$` text was no formula, so its literal is not compared.
    /// O(M log M + R log M) for M math nodes and R rewrites.
    pub(crate) fn failing_replacements(&self, converted: &OwnedDocumentTree) -> Vec<usize> {
        let mut maths = Vec::new();
        math_nodes(&converted.blocks, &mut maths);
        maths.sort_unstable_by_key(|(range, ..)| range.start);
        let mut shift = 0_i64;
        let mut failing = Vec::new();
        for (index, replacement) in self.replacements.iter().enumerate() {
            let start = shifted(replacement.range.start, shift);
            shift += replacement.byte_delta();
            let end = start + replacement.replacement.len();
            let reads = maths
                .binary_search_by_key(&start, |(range, ..)| range.start)
                .is_ok_and(|at| {
                    let (range, literal, display) = maths[at];
                    range.end == end
                        && display == (replacement.new_open_len == 2)
                        && replacement
                            .expected
                            .as_deref()
                            .is_none_or(|expected| trimmed_math_literal(literal) == expected)
                });
            if !reads {
                failing.push(index);
            }
        }
        failing
    }

    /// The conversion of `source`, the text this one converted, without the rewrites at the
    /// sorted indices `drop`; None when none is left.
    pub(crate) fn without(
        &self,
        source: &str,
        drop: &[usize],
    ) -> Result<Option<Self>, SemanticConversionError> {
        let replacements: Vec<_> = self
            .replacements
            .iter()
            .enumerate()
            .filter(|(index, _)| drop.binary_search(index).is_err())
            .map(|(_, replacement)| replacement.clone())
            .collect();
        if replacements.is_empty() {
            return Ok(None);
        }
        let text = apply_replacements(source, &replacements)?;
        Ok(Some(Self { text, replacements }))
    }

    /// The ranges, in the converted text's input, of the single-`$` text the conversion made
    /// formulas of (plan 07): the read-back requires each to read back as one formula.
    pub(crate) fn new_formulas(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.replacements
            .iter()
            .filter(|replacement| replacement.expected.is_none())
            .map(|replacement| replacement.range.as_range())
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

/// One formula rewritten: its old range, the new text, the old and new delimiters' lengths,
/// how many bytes of whitespace the rewrite dropped just inside each old delimiter, and the
/// literal Comrak read before the rewrite, without that whitespace; None when the old text was
/// single-`$` text rather than a Comrak formula.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DelimiterReplacement {
    range: SourceRange,
    replacement: String,
    old_delimiter_len: usize,
    new_open_len: usize,
    new_close_len: usize,
    lead: usize,
    trail: usize,
    expected: Option<String>,
}

impl DelimiterReplacement {
    fn byte_delta(&self) -> i64 {
        self.replacement.len() as i64 - (self.range.end - self.range.start) as i64
    }

    /// A position inside the old formula: in a delimiter it clamps into the new delimiter, in
    /// dropped whitespace to the edge of the kept body, and in the kept body it keeps its
    /// offset.
    fn map_inner_position(&self, position: usize, shift: i64) -> usize {
        let old_body_start = self.range.start + self.old_delimiter_len;
        let old_body_end = self.range.end - self.old_delimiter_len;
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
/// spaces, tabs and full-width spaces (U+3000) just inside the delimiters, and line breaks too
/// when `line_breaks`. A whitespace character right after an odd number of backslashes is a
/// TeX control space (`\ `), so it stays, with what follows it. None when nothing else is
/// left.
///
/// Postcondition: `lead + trail < body.len()`, and both cut at char boundaries.
pub(crate) fn math_body_trim(body: &str, line_breaks: bool) -> Option<(usize, usize)> {
    let blank =
        |c: char| matches!(c, ' ' | '\t' | '\u{3000}') || line_breaks && matches!(c, '\n' | '\r');
    let lead = body.len() - body.trim_start_matches(blank).len();
    if lead == body.len() {
        return None;
    }
    let mut end = body.trim_end_matches(blank).len();
    let backslashes = body.as_bytes()[..end]
        .iter()
        .rev()
        .take_while(|&&byte| byte == b'\\')
        .count();
    if backslashes % 2 == 1
        && let Some(escaped) = body[end..].chars().next()
    {
        // Keep a `\r\n` pair whole, so no isolated `\r` is left.
        end += if body[end..].starts_with("\r\n") {
            2
        } else {
            escaped.len_utf8()
        };
    }
    Some((lead, body.len() - end))
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
    collect_blocks(
        &tree.source,
        &tree.blocks,
        scope.as_ref(),
        &mut replacements,
    );
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
    source: &str,
    blocks: &[BlockNode],
    scope: Option<&Range<usize>>,
    output: &mut Vec<DelimiterReplacement>,
) {
    for block in blocks {
        match block {
            BlockNode::Paragraph { content, .. } | BlockNode::Heading { content, .. } => {
                collect_inlines(content, scope, output);
                collect_dollar_text(source, content, scope, output);
            }
            BlockNode::BlockQuote { blocks, .. } => collect_blocks(source, blocks, scope, output),
            BlockNode::List(list) => {
                for item in &list.items {
                    collect_blocks(source, &item.blocks, scope, output);
                }
            }
            BlockNode::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        collect_inlines(&cell.content, scope, output);
                        collect_dollar_text(source, &cell.content, scope, output);
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
    // them, as `$$\n…\n$$` is the standard block form. A `$…$` Comrak read can only have
    // full-width spaces just inside.
    let (old, open, close, line_breaks) = if source.starts_with("\\(") && source.ends_with("\\)") {
        (2, "$", "$", true)
    } else if source.starts_with("\\[") && source.ends_with("\\]")
        || source.starts_with("$$") && source.ends_with("$$")
    {
        (2, "$$", "$$", false)
    } else if source.starts_with('$') && source.ends_with('$') {
        (1, "$", "$", true)
    } else {
        return;
    };
    if source.len() < 2 * old || range.end - range.start != source.len() {
        return;
    }
    // A formula with nothing but whitespace in what Comrak read (quote prefixes and line
    // breaks left out) is not rewritten, whatever its delimiters keep.
    let expected = trimmed_math_literal(&math.literal);
    if expected.is_empty() {
        return;
    }
    let body = &source[old..source.len() - old];
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
    // Dollar math already in the standard form stays as it is.
    if source.starts_with('$') && lead == 0 && trail == 0 {
        return;
    }
    output.push(DelimiterReplacement {
        range,
        replacement: format!("{open}{}{close}", &body[lead..body.len() - trail]),
        old_delimiter_len: old,
        new_open_len: open.len(),
        new_close_len: close.len(),
        lead,
        trail,
        expected: Some(expected.to_owned()),
    });
}

/// The math nodes under `blocks`: range, literal and whether display.
fn math_nodes<'a>(blocks: &'a [BlockNode], output: &mut Vec<(SourceRange, &'a str, bool)>) {
    fn inlines<'a>(nodes: &'a [InlineNode], output: &mut Vec<(SourceRange, &'a str, bool)>) {
        for inline in nodes {
            match inline {
                InlineNode::Emphasis { children, .. }
                | InlineNode::Strong { children, .. }
                | InlineNode::Strikethrough { children, .. }
                | InlineNode::Link { children, .. } => inlines(children, output),
                InlineNode::InlineMath(math) => {
                    output.extend(
                        math.source_range
                            .map(|range| (range, &*math.literal, math.display)),
                    );
                }
                InlineNode::Text { .. }
                | InlineNode::Code { .. }
                | InlineNode::Image { .. }
                | InlineNode::SoftBreak { .. }
                | InlineNode::HardBreak { .. }
                | InlineNode::HtmlLiteral { .. } => {}
            }
        }
    }
    for block in blocks {
        match block {
            BlockNode::Paragraph { content, .. } | BlockNode::Heading { content, .. } => {
                inlines(content, output);
            }
            BlockNode::BlockQuote { blocks, .. } => math_nodes(blocks, output),
            BlockNode::List(list) => {
                for item in &list.items {
                    math_nodes(&item.blocks, output);
                }
            }
            BlockNode::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        inlines(&cell.content, output);
                    }
                }
            }
            BlockNode::DisplayMath(math) => {
                output.extend(
                    math.source_range
                        .map(|range| (range, &*math.literal, math.display)),
                );
            }
            BlockNode::CodeBlock(_)
            | BlockNode::ThematicBreak { .. }
            | BlockNode::HtmlLiteral { .. } => {}
        }
    }
}

/// Single-`$` text that Comrak left as text because of the whitespace just inside it, made a
/// formula by plan 07's rule: the candidates are unescaped lone `$` in this block's text (not
/// in links, images, code, HTML or formulas); a line of the block with exactly two pairs them
/// when nothing but text lies between; `dollar_pair` then decides.
fn collect_dollar_text(
    source: &str,
    inlines: &[InlineNode],
    scope: Option<&Range<usize>>,
    output: &mut Vec<DelimiterReplacement>,
) {
    let mut runs = Vec::new();
    text_runs(inlines, &mut runs, &mut true);
    let bytes = source.as_bytes();
    // (position, index of its text run), in document order.
    let mut candidates = Vec::new();
    for (index, run) in runs.iter().enumerate() {
        for at in memchr::memchr_iter(b'$', &bytes[run.clone()]) {
            let at = run.start + at;
            let backslashes = bytes[..at]
                .iter()
                .rev()
                .take_while(|&&byte| byte == b'\\')
                .count();
            let lone = (at == 0 || bytes[at - 1] != b'$') && bytes.get(at + 1) != Some(&b'$');
            if backslashes % 2 == 0 && lone {
                candidates.push((at, index));
            }
        }
    }
    let mut first = 0;
    while first < candidates.len() {
        // The candidates of one line: no line break between neighbours.
        let mut end = first + 1;
        while end < candidates.len()
            && memchr::memchr(b'\n', &bytes[candidates[end - 1].0..candidates[end].0]).is_none()
        {
            end += 1;
        }
        if let [(open, open_run), (close, close_run)] = candidates[first..end]
            && open_run == close_run
            && let Some(replacement) = dollar_pair(source, open, close, scope)
        {
            output.push(replacement);
        }
        first = end;
    }
}

/// The text of `inlines` as runs of touching text nodes, outside links and images; emphasis
/// is entered, so its markers end a run. `touching` says whether the next text node may
/// extend the last run.
fn text_runs(inlines: &[InlineNode], runs: &mut Vec<Range<usize>>, touching: &mut bool) {
    for inline in inlines {
        match inline {
            InlineNode::Text {
                source_range: Some(range),
                ..
            } => match runs.last_mut() {
                Some(run) if *touching && run.end == range.start => run.end = range.end,
                _ => {
                    runs.push(range.as_range());
                    *touching = true;
                }
            },
            InlineNode::Emphasis { children, .. }
            | InlineNode::Strong { children, .. }
            | InlineNode::Strikethrough { children, .. } => {
                *touching = false;
                text_runs(children, runs, touching);
                *touching = false;
            }
            InlineNode::Text { .. }
            | InlineNode::Link { .. }
            | InlineNode::Image { .. }
            | InlineNode::Code { .. }
            | InlineNode::InlineMath(_)
            | InlineNode::SoftBreak { .. }
            | InlineNode::HardBreak { .. }
            | InlineNode::HtmlLiteral { .. } => *touching = false,
        }
    }
}

/// The rewrite of the single-`$` text from `open` to `close` (both `$`) into `$content$`, when
/// plan 07's rule takes it for a formula: whitespace just inside at least one `$`, content left
/// once it is gone, not starting with a digit nor followed by one (prices), and looking like
/// a formula: it holds one of `\ ^ _ { = +`, or is one word of ASCII letters and digits.
fn dollar_pair(
    source: &str,
    open: usize,
    close: usize,
    scope: Option<&Range<usize>>,
) -> Option<DelimiterReplacement> {
    if scope.is_some_and(|scope| open < scope.start || close + 1 > scope.end) {
        return None;
    }
    let inner = &source[open + 1..close];
    let (lead, trail) = math_body_trim(inner, false)?;
    let content = &inner[lead..inner.len() - trail];
    let formula_like = content
        .bytes()
        .any(|byte| matches!(byte, b'\\' | b'^' | b'_' | b'{' | b'=' | b'+'))
        || content.bytes().all(|byte| byte.is_ascii_alphanumeric());
    // An escaped space kept at the end would stand before the closing `$`, which then does
    // not close.
    let ends_blank = content.ends_with([' ', '\t', '\u{3000}']);
    let accepted = (lead > 0 || trail > 0)
        && !ends_blank
        && !content.starts_with(char::is_numeric)
        && !source[close + 1..].starts_with(char::is_numeric)
        && formula_like;
    accepted.then(|| DelimiterReplacement {
        range: SourceRange {
            start: open,
            end: close + 1,
        },
        replacement: format!("${content}$"),
        old_delimiter_len: 1,
        new_open_len: 1,
        new_close_len: 1,
        lead,
        trail,
        expected: None,
    })
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
            // Nothing but line breaks, or a quote's prefixes, in what Comrak read.
            ("\\[\n\\]", None),
            ("> \\[\n> \\]", None),
            ("$$a$$", None),
            ("$$\na\n$$", None),
            ("$a$", None),
            // Full-width spaces just inside go like spaces; one right after an odd number of
            // backslashes is escaped and stays; nothing but full-width spaces is no formula.
            ("\\(\u{3000}a\u{3000}\\)", Some("$a$")),
            ("$\u{3000}a\u{3000}$", Some("$a$")),
            ("\\[\u{3000}a \\]", Some("$$a$$")),
            ("\\(a\\\u{3000}\\)", Some("$a\\\u{3000}$")),
            ("\\(\u{3000}\\)", None),
            ("$ \u{3000}a\u{3000} $", Some("$a$")),
        ] {
            assert_eq!(
                converted(source, None).map(|conversion| conversion.text().to_owned()),
                expected.map(str::to_owned),
                "{source:?}"
            );
        }
    }

    /// Plan 07's single-`$` rule (USER 2026-11-25, rule b): each row of the report's risk table,
    /// and the edges of the rule. Only text that the rule takes for a formula is rewritten.
    #[test]
    fn phase11b_conversion_makes_formulas_of_single_dollar_text_only_by_the_rule() {
        for (source, expected) in [
            // Not formulas: the corpus line, prose and prices.
            (
                "* 转义测试：\\$ Not a math $ (反斜杠转义应生效，显示普通 $ 符号)。",
                None,
            ),
            ("Prices: $ USD 5 and $ CAD 7 today.", None),
            ("Run $ npm test and check $ output.", None),
            ("The $ symbol and the $ sign.", None),
            ("价格 $ 5 和 $ 10", None),
            // Formulas.
            ("Formula $a $ here.", Some("Formula $a$ here.")),
            ("Formula $ a$ here.", Some("Formula $a$ here.")),
            ("Both $ a $ here.", Some("Both $a$ here.")),
            ("Math $a$ and $ b $ after.", Some("Math $a$ and $b$ after.")),
            ("Emph *$ a $* text.", Some("Emph *$a$* text.")),
            ("$ x^2 + y $", Some("$x^2 + y$")),
            ("$\t\\alpha $", Some("$\\alpha$")),
            ("# $ a $", Some("# $a$")),
            ("| $ a $ | b |\n|---|---|", Some("| $a$ | b |\n|---|---|")),
            ("$ a $b", Some("$a$b")),
            // Left alone: code and links hold no candidates; a formula starting with a digit, or
            // a digit after the closer, is taken for a price; lines without exactly two, or with
            // other nodes between; a formula across lines; several plain words; an escaped space
            // before the closer; no whitespace inside at all.
            ("Code `$ a $` stays.", None),
            ("Link [$ a $](u) text.", None),
            ("Formula $ 2x + 1 $ here.", None),
            ("$ a $2", None),
            ("One $ a $ and $ three.", None),
            ("$ a $ *x* $ b $", None),
            ("$ *a* $", None),
            ("Cross $ a\nb $ line.", None),
            ("$ a b $", None),
            // Full-width digits count as digits for the price rule.
            ("$ \u{ff12}+\u{ff13} $", None),
            ("$ x $\u{ff12}", None),
            // A known false positive of rule b: one ASCII word.
            ("Currency $ USD $ today.", Some("Currency $USD$ today.")),
            ("$ a\\ $", None),
            ("$  $", None),
            ("a $b$c d", None),
            // Markup between the two; an escaped `$` is no candidate; a `$` next to another
            // (here a formula's closer) is none either.
            ("$ x^2 *y* $", None),
            ("\\$ x $ y^2 $", Some("\\$ x $y^2$")),
            ("$a$$ b^2 $", None),
        ] {
            assert_eq!(
                converted(source, None).map(|conversion| conversion.text().to_owned()),
                expected.map(str::to_owned),
                "{source:?}"
            );
        }
        // A selection must hold the whole pair.
        let source = "x $ a $ y";
        assert!(converted(source, Some(0..6)).is_none());
        assert_eq!(converted(source, Some(2..7)).unwrap().text(), "x $a$ y");
    }

    /// A rewrite reads back only as one math node at its range, with its body as the literal
    /// (whitespace just inside aside) and display math exactly for `$$`. Each condition alone,
    /// on hand-built parses of the converted text `$$ab$$`.
    #[test]
    fn phase11b_conversion_names_the_rewrites_that_do_not_read_back() {
        let conversion = converted("\\[ab\\]", None).unwrap();
        assert_eq!(conversion.text(), "$$ab$$");
        let parse = |start: usize, end: usize, literal: &str, display: bool| OwnedDocumentTree {
            generation: Generation::initial(),
            source: Arc::from(conversion.text()),
            blocks: vec![BlockNode::Paragraph {
                content: vec![InlineNode::InlineMath(MathNode {
                    literal: literal.to_owned(),
                    source_literal: String::new(),
                    display,
                    source_range: SourceRange::new(start, end),
                })],
                source_range: SourceRange::new(0, 6),
            }],
            node_count: 2,
        };
        assert!(
            conversion
                .failing_replacements(&parse(0, 6, "ab", true))
                .is_empty()
        );
        assert!(
            conversion
                .failing_replacements(&parse(0, 6, " ab ", true))
                .is_empty()
        );
        for (start, end, literal, display) in [
            (1, 6, "ab", true),
            (0, 5, "ab", true),
            (0, 6, "ab", false),
            (0, 6, "ac", true),
        ] {
            assert_eq!(
                conversion.failing_replacements(&parse(start, end, literal, display)),
                [0],
                "{start}..{end} {literal:?} {display}"
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
                old_delimiter_len: 2,
                new_open_len: 1,
                new_close_len: 1,
                lead: 0,
                trail: 0,
                expected: None,
            },
            DelimiterReplacement {
                range: SourceRange { start: 3, end: 7 },
                replacement: "$b$".to_owned(),
                old_delimiter_len: 2,
                new_open_len: 1,
                new_close_len: 1,
                lead: 0,
                trail: 0,
                expected: None,
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
