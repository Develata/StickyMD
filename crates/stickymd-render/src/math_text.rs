//! Explicit, source-preserving equals-line cleanup inside formulas.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#semantic-math-delimiter-conversion
//!
//! A stopgap until Comrak parses display math as blocks (kivikakk/comrak#880).
//! The `$` action runs it before the delimiter conversion, so a lone `=` line
//! can no longer turn the line above into a Setext heading and hide the formula
//! from that conversion. It has one caller and no dependency on the converter,
//! so it can be removed on its own.

use std::ops::Range;

use comrak::arena_tree::NodeEdge;
use comrak::nodes::{AstNode, NodeMath, NodeValue, Sourcepos};
use comrak::{Arena, parse_document};

/// A pure text projection; callers own the edit transaction and selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EqualsLineConversion {
    text: String,
    removed: Vec<Range<usize>>,
}

impl EqualsLineConversion {
    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn into_text(self) -> String {
        self.text
    }

    /// Retained bytes keep their position relative to each other; positions
    /// inside deleted whitespace clamp to the corresponding join.
    pub fn map_position(&self, position: usize) -> usize {
        let mut shift = 0;
        for range in &self.removed {
            if position <= range.start {
                break;
            }
            shift += position.min(range.end) - range.start;
            if position < range.end {
                break;
            }
        }
        position - shift
    }
}

/// Joins bare equals lines inside formulas: complete, unescaped `$$` pairs,
/// and the `\[...\]`, `\(...\)` and `$...$` formulas Comrak finds with Setext
/// off. A scope must contain the entire formula. Invalid scopes/source maps,
/// protected Markdown, structured TeX, overlapping joins and no matches all
/// fail closed as None. This function neither converts delimiters nor changes
/// preview semantics.
pub fn join_math_equals_lines(
    source: &str,
    scope: Option<Range<usize>>,
) -> Option<EqualsLineConversion> {
    if scope
        .as_ref()
        .is_some_and(|range| source.get(range.clone()).is_none())
        || !(source.contains('$') || source.contains("\\[") || source.contains("\\("))
        || !source.lines().any(|line| line.trim() == "=")
    {
        return None;
    }
    let scan = scan_formulas(source)?;
    let mut removed = Vec::new();
    collect_dollar_pairs(source, &scan.protected, scope.as_ref(), &mut removed);
    for formula in &scan.formulas {
        if scope.as_ref().is_none_or(|range| {
            formula.outer.start >= range.start && formula.outer.end <= range.end
        }) {
            collect_joins(
                &source[formula.body.clone()],
                formula.body.start,
                &mut removed,
            );
        }
    }
    // A `\[...\]` inside a `$$` pair yields the same joins twice; any other
    // overlap means the two scans disagree, so the action fails closed.
    removed.sort_unstable_by_key(|range| (range.start, range.end));
    removed.dedup();
    if removed.is_empty() || removed.windows(2).any(|pair| pair[0].end > pair[1].start) {
        return None;
    }
    let mut text = String::with_capacity(source.len());
    let mut cursor = 0;
    for range in &removed {
        text.push_str(source.get(cursor..range.start)?);
        cursor = range.end;
    }
    text.push_str(source.get(cursor..)?);
    Some(EqualsLineConversion { text, removed })
}

/// The original display-dollar scan: complete, unescaped `$$` pairs, which may
/// span blank lines. A pair that meets a protected range is left alone.
fn collect_dollar_pairs(
    source: &str,
    protected: &[Range<usize>],
    scope: Option<&Range<usize>>,
    removed: &mut Vec<Range<usize>>,
) {
    let bytes = source.as_bytes();
    let mut cursor = 0;
    let mut protection = 0;
    let mut open: Option<(usize, bool)> = None;
    while cursor < bytes.len() {
        if let Some(range) = protected.get(protection)
            && cursor >= range.start
        {
            if let Some((_, blocked)) = &mut open {
                *blocked = true;
            }
            cursor = cursor.max(range.end);
            protection += 1;
            continue;
        }
        match bytes[cursor] {
            // Escaped dollars cannot become delimiters. Advancing
            // by bytes is safe here: slices are taken only at ASCII markers.
            b'\\' => cursor = (cursor + 2).min(bytes.len()),
            b'$' => {
                let start = cursor;
                while cursor < bytes.len() && bytes[cursor] == b'$' {
                    cursor += 1;
                }
                if cursor - start != 2 || (start > 0 && bytes[start - 1] == b'$') {
                    continue;
                }
                if let Some((body_start, blocked)) = open.take() {
                    if !blocked
                        && scope.is_none_or(|range| {
                            body_start - 2 >= range.start && cursor <= range.end
                        })
                    {
                        collect_joins(&source[body_start..start], body_start, removed);
                    }
                } else {
                    open = Some((cursor, false));
                }
            }
            _ => cursor += 1,
        }
    }
}

struct FormulaScan {
    /// Code, HTML, links, images, tables and inline math, sorted by start.
    protected: Vec<Range<usize>>,
    formulas: Vec<Formula>,
}

/// A `\[...\]`, `\(...\)` or `$...$` formula: `outer` includes the delimiters,
/// `body` excludes them.
struct Formula {
    outer: Range<usize>,
    body: Range<usize>,
}

/// Comrak owns Markdown exclusions and formula recognition, in the Preview's
/// dialect. The local Setext option only keeps equals lines from splitting
/// multiline code/math during this explicit text action; PreviewParser and
/// delimiter conversion are not involved.
fn scan_formulas(source: &str) -> Option<FormulaScan> {
    let arena = Arena::new();
    let mut options = crate::preview::markdown_options();
    options.parse.ignore_setext = true;
    let root = parse_document(&arena, source, &options);
    let lines = Lines::new(source);
    // Only a source holding `]:` can contain a link reference definition.
    let may_shift = source.contains("]:");
    let mut protected = Vec::new();
    let mut formulas = Vec::new();
    // Container state is kept on entry and exit, so no node walks its
    // ancestors. Tables do not nest and paragraphs do not nest.
    let mut in_table = false;
    let mut in_links = 0_usize;
    let mut shifted: Option<&AstNode<'_>> = None;
    for edge in root.traverse() {
        let node = match edge {
            NodeEdge::End(node) => {
                let value = &node.data.borrow().value;
                if let Some(paragraph) = shifted {
                    if std::ptr::eq(paragraph, node) {
                        shifted = None;
                    }
                } else if in_table {
                    in_table = !matches!(value, NodeValue::Table(_));
                } else if matches!(value, NodeValue::Link(_) | NodeValue::Image(_)) {
                    in_links = in_links.saturating_sub(1);
                }
                continue;
            }
            NodeEdge::Start(_) if shifted.is_some() || in_table => continue,
            NodeEdge::Start(node) => node,
        };
        let data = node.data.borrow();
        match &data.value {
            // A table is protected whole, by its lines: formulas in cells sit on
            // one line and never join, and Comrak shifts cell inline columns
            // around `\|`, so nothing inside a table is located by column.
            NodeValue::Table(_) => {
                protected.push(lines.whole(data.sourcepos)?);
                in_table = true;
            }
            // Its lines are right, its inline positions are not (see
            // `inlines_shifted`), so it is protected whole and left alone.
            NodeValue::Paragraph if may_shift && crate::preview::inlines_shifted(node) => {
                protected.push(lines.whole(data.sourcepos)?);
                shifted = Some(node);
            }
            NodeValue::Code(_)
            | NodeValue::CodeBlock(_)
            | NodeValue::HtmlInline(_)
            | NodeValue::HtmlBlock(_) => {
                protected.push(source_range(source, &lines, data.sourcepos)?)
            }
            NodeValue::Link(_) | NodeValue::Image(_) => {
                protected.push(source_range(source, &lines, data.sourcepos)?);
                in_links += 1;
            }
            NodeValue::Math(math) => match locate(source, &lines, data.sourcepos, math) {
                Some(Located::DollarPair) => {}
                Some(Located::Formula { open, close, outer }) => {
                    if !math.display_math {
                        protected.push(outer.clone());
                    }
                    if in_links == 0 && !(open == "$" && touches_dollar(source, &outer)) {
                        let body = outer.start + open.len()..outer.end - close.len();
                        formulas.push(Formula { outer, body });
                    }
                }
                // Inline math stays protected for the dollar-pair scan, as
                // before, so an inline formula that cannot be located fails
                // the whole step closed; a display one is only skipped.
                None if !math.display_math => return None,
                None => {}
            },
            _ => {}
        }
    }
    if may_shift {
        protect_detached_lines(source, root, &lines, &mut protected);
    }
    protected.sort_unstable_by_key(|range| range.start);
    Some(FormulaScan {
        protected,
        formulas,
    })
}

/// Comrak detaches a paragraph that held only link reference definitions, so
/// its lines, a multiline title included, belong to no node. With the lines of
/// every leaf block marked, each line left over that carries text is protected
/// whole, so the dollar-pair scan cannot join inside a definition title: any
/// join needs a bare `=` line, and that line is never blank or bare markup.
/// Lines of an empty block quote or list item (`>`, `-`, `1.`) are left alone.
/// A block type not listed here leaves its lines protected, which fails closed.
fn protect_detached_lines<'a>(
    source: &str,
    root: &'a AstNode<'a>,
    lines: &Lines,
    protected: &mut Vec<Range<usize>>,
) {
    let count = lines.starts.len();
    let mut covered = vec![false; count + 1];
    for node in root.descendants() {
        let data = node.data.borrow();
        if matches!(
            data.value,
            NodeValue::Paragraph
                | NodeValue::Heading(_)
                | NodeValue::CodeBlock(_)
                | NodeValue::HtmlBlock(_)
                | NodeValue::ThematicBreak
                | NodeValue::Table(_)
        ) {
            let first = data.sourcepos.start.line.max(1);
            let last = data.sourcepos.end.line.min(count);
            if let Some(lines) = covered.get_mut(first..=last) {
                lines.fill(true);
            }
        }
    }
    for (line, &covered) in covered.iter().enumerate().skip(1) {
        if let (false, Some(start), Some(end)) = (covered, lines.start(line), lines.end(line))
            && source
                .get(start..end)
                .is_some_and(|text| !markup_only(text))
        {
            protected.push(start..end);
        }
    }
}

/// A line with nothing but container markup: block quote `>`s, then at most
/// one list marker, and spaces. It carries no text a title could hold.
fn markup_only(line: &str) -> bool {
    let mut rest = line.trim_start();
    while let Some(after) = rest.strip_prefix('>') {
        rest = after.trim_start();
    }
    let digits = rest.len() - rest.trim_start_matches(|c: char| c.is_ascii_digit()).len();
    let rest = match rest.strip_prefix(['-', '*', '+']) {
        Some(after) => after,
        None if digits > 0 => rest[digits..].strip_prefix(['.', ')']).unwrap_or(rest),
        None => rest,
    };
    rest.trim().is_empty()
}

/// Line starts of the scanned source; Comrak lines and columns are 1-based
/// bytes.
struct Lines {
    starts: Vec<usize>,
    len: usize,
}

impl Lines {
    fn new(source: &str) -> Self {
        Self {
            starts: std::iter::once(0)
                .chain(source.match_indices('\n').map(|(index, _)| index + 1))
                .collect(),
            len: source.len(),
        }
    }

    fn start(&self, line: usize) -> Option<usize> {
        self.starts.get(line.checked_sub(1)?).copied()
    }

    /// The end of a line, before its `\n`.
    fn end(&self, line: usize) -> Option<usize> {
        self.start(line)?;
        Some(self.starts.get(line).map_or(self.len, |next| next - 1))
    }

    /// Whole lines from a block's first line to its last.
    fn whole(&self, position: Sourcepos) -> Option<Range<usize>> {
        Some(self.start(position.start.line)?..self.end(position.end.line)?)
    }
}

/// Where a math node sits in the source.
enum Located {
    /// A `$$` node: the dollar-pair scan owns it, so its end is never read.
    DollarPair,
    /// A `\[...\]`, `\(...\)` or `$...$` node; `outer` includes the delimiters.
    Formula {
        open: &'static str,
        close: &'static str,
        outer: Range<usize>,
    },
}

/// Locates a math node and checks the place against the node itself. Comrak
/// has misreported positions: before 0.56 the end column of an inline that
/// spans lines and starts after its block's first line (kivikakk/comrak#855),
/// and up to 0.59 every inline position in a paragraph that began with link
/// reference definitions, whose removed lines it still counts (that paragraph
/// is skipped earlier, but the checks stay as a guard). So the opener must
/// sit on the reported start line and match the node's kind,
/// and the body must hold exactly the node's content apart from ASCII
/// whitespace (Comrak normalizes only that). A misplaced range, or one that
/// takes in a container marker such as `>`, fails these checks.
fn locate(source: &str, lines: &Lines, position: Sourcepos, math: &NodeMath) -> Option<Located> {
    let start = lines
        .start(position.start.line)?
        .checked_add(position.start.column.checked_sub(1)?)?;
    let first_line = source.get(start..lines.end(position.start.line)?)?;
    let (open, close) = [("$$", "$$"), ("\\[", "\\]"), ("\\(", "\\)"), ("$", "$")]
        .into_iter()
        .find(|(open, _)| first_line.starts_with(open))?;
    if math.display_math != matches!(open, "$$" | "\\[") {
        return None;
    }
    if open == "$$" {
        return Some(Located::DollarPair);
    }
    let end = formula_end(source, lines, position, close)?;
    let body = source.get(start.checked_add(open.len())?..end.checked_sub(close.len())?)?;
    let visible = |text: &str| {
        text.bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect::<Vec<_>>()
    };
    (visible(body) == visible(&math.literal)).then_some(Located::Formula {
        open,
        close,
        outer: start..end,
    })
}

/// The exclusive end of a formula whose closer is `close`. A multiline end is
/// taken from the end line, never from the column: the closer Comrak matched
/// is the first `\)` or `\]`, or the first `$` not after a backslash, on that
/// line, since none can occur earlier in the formula.
fn formula_end(source: &str, lines: &Lines, position: Sourcepos, close: &str) -> Option<usize> {
    let line_start = lines.start(position.end.line)?;
    let line_end = lines.end(position.end.line)?;
    let end = if position.start.line == position.end.line {
        line_start.checked_add(position.end.column)?
    } else {
        let line = source.get(line_start..line_end)?;
        let closer = if close == "$" {
            line.bytes()
                .enumerate()
                .position(|(index, byte)| byte == b'$' && !line[..index].ends_with('\\'))?
        } else {
            line.find(close)?
        };
        line_start + closer + close.len()
    };
    (end <= line_end && source.get(..end)?.ends_with(close)).then_some(end)
}

/// A `$...$` formula touching another `$` (as in `\$$a$$`) reads as a broken
/// `$$` pair, so it is left alone.
fn touches_dollar(source: &str, outer: &Range<usize>) -> bool {
    source
        .get(..outer.start)
        .is_none_or(|before| before.ends_with('$'))
        || source
            .get(outer.end..)
            .is_none_or(|after| after.starts_with('$'))
}

fn source_range(source: &str, lines: &Lines, position: Sourcepos) -> Option<Range<usize>> {
    let start = lines
        .start(position.start.line)?
        .checked_add(position.start.column.checked_sub(1)?)?;
    let end = lines
        .start(position.end.line)?
        .checked_add(position.end.column)?;
    source.get(start..end)?;
    Some(start..end)
}

fn collect_joins(body: &str, offset: usize, removed: &mut Vec<Range<usize>>) {
    if has_structured_tex(body) {
        return;
    }
    let lines: Vec<_> = body
        .split_inclusive('\n')
        .scan(offset, |start, line| {
            let item = (*start, line);
            *start += line.len();
            Some(item)
        })
        .collect();
    for triple in lines.windows(3) {
        let [
            (before_start, before),
            (equals_start, equals),
            (after_start, after),
        ] = triple
        else {
            continue;
        };
        if equals.trim() != "="
            || matches!(before.trim(), "" | "=")
            || matches!(after.trim(), "" | "=")
        {
            continue;
        }
        let equal = equals_start + equals.len() - equals.trim_start().len();
        removed.push(before_start + before.trim_end().len()..equal);
        removed.push(equal + 1..after_start + after.len() - after.trim_start().len());
    }
}

/// This is a conservative opt-out, not a TeX parser: all environments and
/// explicit line/space commands are kept verbatim.
fn has_structured_tex(body: &str) -> bool {
    let mut chars = body.chars().peekable();
    while let Some(character) = chars.next() {
        if character == '%' {
            return true;
        }
        if character != '\\' {
            continue;
        }
        let Some(next) = chars.next() else {
            return true;
        };
        if next == '\\' || next.is_whitespace() {
            return true;
        }
        if next.is_ascii_alphabetic() {
            let mut command = String::from(next);
            while let Some(next) = chars.next_if(char::is_ascii_alphabetic) {
                command.push(next);
            }
            if matches!(
                command.as_str(),
                "begin" | "end" | "cr" | "crcr" | "newline" | "linebreak" | "displaybreak"
            ) {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests;
