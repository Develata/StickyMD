//! Explicit, source-preserving equals-line cleanup inside formulas.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#semantic-math-delimiter-conversion
//!
//! Written as a stopgap for kivikakk/comrak#880 and kept after it: Comrak 1.0's
//! opt-in block math (`math_dollars_block`, off in StickyMD) only covers `$$`
//! fence lines, while `\[...\]`, `$$a...b$$` and multiline inline formulas
//! still lose a lone `=` line to a Setext heading. The `$` action runs it
//! before the delimiter conversion, so that line can no longer hide the formula
//! from that conversion. It has one caller and no dependency on the converter,
//! so it can be removed on its own.

use std::ops::Range;

use comrak::arena_tree::NodeEdge;
use comrak::nodes::{NodeMath, NodeValue, Sourcepos};
use comrak::{Arena, parse_document};

/// A pure text projection; callers own the edit transaction and selection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EqualsLineConversion {
    text: String,
    removed: Vec<Range<usize>>,
}

impl EqualsLineConversion {
    /// How many equals lines the step joined. Invariant: `collect_joins` removes two ranges per
    /// joined line, the line break before the `=` and the one after it.
    pub fn joined_lines(&self) -> usize {
        debug_assert!(self.removed.len().is_multiple_of(2));
        self.removed.len() / 2
    }

    /// The position in `text()` of the `=` of each joined line, in order. Invariant:
    /// `removed` is sorted and holds each join as two adjacent ranges, the one that ends at its
    /// `=` and the one that starts after it. So an `=` moves left by every byte removed before
    /// it: the earlier pairs and its own first range. One pass, O(joins).
    pub fn joined_equals(&self) -> impl Iterator<Item = usize> + '_ {
        let mut removed_before = 0;
        self.removed.chunks_exact(2).map(move |join| {
            let (before, after) = (&join[0], &join[1]);
            let equal = before.end - removed_before - before.len();
            removed_before += before.len() + after.len();
            equal
        })
    }

    /// Each joined line in the input, in order: the last byte of the line above it, its `=`,
    /// and the first byte of the line below it. Same invariant as `joined_equals`; the lines
    /// above and below hold text, so both bytes exist. O(joins).
    pub fn joins(&self) -> impl Iterator<Item = [usize; 3]> + '_ {
        self.removed
            .chunks_exact(2)
            .map(|join| [join[0].start - 1, join[0].end, join[1].end])
    }

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

/// Joins bare equals lines inside formulas: the `$$...$$`, `\[...\]`,
/// `\(...\)` and `$...$` formulas Comrak finds with Setext off. Comrak never
/// finds one in code, HTML or a link reference definition, so nothing else
/// needs protecting. A scope must contain the entire formula. Invalid
/// scopes/source maps, structured TeX, overlapping joins and no matches all
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
    let mut removed = Vec::new();
    for formula in &scan_formulas(source)? {
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
    // Comrak's formulas do not nest, so joins never overlap; if they did, the
    // positions would disagree and the action fails closed.
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

/// A `$$...$$`, `\[...\]`, `\(...\)` or `$...$` formula: `outer` includes the
/// delimiters, `body` excludes them.
struct Formula {
    outer: Range<usize>,
    body: Range<usize>,
}

/// Comrak owns formula recognition, in the Preview's dialect. The local Setext
/// option only keeps equals lines from splitting multiline math during this
/// explicit text action; PreviewParser and delimiter conversion are not
/// involved. Formulas in tables (one line each, so never joined) and in link
/// or image text are left out, as is a `$...$` touching another `$`; a formula
/// whose place does not check out (`locate`) is skipped.
fn scan_formulas(source: &str) -> Option<Vec<Formula>> {
    let arena = Arena::new();
    let mut options = crate::preview::markdown_options();
    options.parse.ignore_setext = true;
    let root = parse_document(&arena, source, &options);
    let lines = Lines::new(source);
    let mut formulas = Vec::new();
    // Container state is kept on entry and exit, so no node walks its
    // ancestors. Tables do not nest.
    let mut in_table = false;
    let mut in_links = 0_usize;
    for edge in root.traverse() {
        let node = match edge {
            NodeEdge::End(node) => {
                let value = &node.data.borrow().value;
                if in_table {
                    in_table = !matches!(value, NodeValue::Table(_));
                } else if matches!(value, NodeValue::Link(_) | NodeValue::Image(_)) {
                    in_links = in_links.saturating_sub(1);
                }
                continue;
            }
            NodeEdge::Start(_) if in_table => continue,
            NodeEdge::Start(node) => node,
        };
        let data = node.data.borrow();
        match &data.value {
            NodeValue::Table(_) => in_table = true,
            NodeValue::Link(_) | NodeValue::Image(_) => in_links += 1,
            NodeValue::Math(math) if in_links == 0 => {
                if let Some(Located { open, close, outer }) =
                    locate(source, &lines, data.sourcepos, math)
                    && !(open == "$" && touches_dollar(source, &outer))
                {
                    let body = outer.start + open.len()..outer.end - close.len();
                    formulas.push(Formula { outer, body });
                }
            }
            _ => {}
        }
    }
    Some(formulas)
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
}

/// Where a math node sits in the source.
struct Located {
    open: &'static str,
    close: &'static str,
    /// The formula's source, delimiters included.
    outer: Range<usize>,
}

/// Locates a math node and checks the place against the node itself. Comrak
/// has misreported positions: before 0.56 the end column of an inline that
/// spans lines and starts after its block's first line (kivikakk/comrak#855),
/// and before 1.0 every inline position in a paragraph that began with link
/// reference definitions, whose removed lines it still counted. The checks
/// stay as a guard. So the opener must sit on the reported start line and
/// match the node's kind,
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
    let end = formula_end(source, lines, position, close)?;
    let body = source.get(start.checked_add(open.len())?..end.checked_sub(close.len())?)?;
    let visible = |text: &str| {
        text.bytes()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect::<Vec<_>>()
    };
    (visible(body) == visible(&math.literal)).then_some(Located {
        open,
        close,
        outer: start..end,
    })
}

/// The exclusive end of a formula whose closer is `close`. A multiline end is
/// taken from the end line, never from the column: the closer Comrak matched
/// is the first unescaped one on that line (`unescaped_closer_end`), since
/// none can occur earlier in the formula.
fn formula_end(source: &str, lines: &Lines, position: Sourcepos, close: &str) -> Option<usize> {
    let line_start = lines.start(position.end.line)?;
    let line_end = lines.end(position.end.line)?;
    let end = if position.start.line == position.end.line {
        line_start.checked_add(position.end.column)?
    } else {
        line_start + unescaped_closer_end(source.get(line_start..line_end)?, close)?
    };
    (end <= line_end && source.get(..end)?.ends_with(close)).then_some(end)
}

/// The end of the first unescaped `close` (`$$`, `$`, `\)` or `\]`) in `line`.
/// As in Comrak's math spec, a backslash escapes the byte after it unless it is
/// itself escaped: `$` and `$$` close after an even run of backslashes, and
/// `\)` or `\]` where its backslash ends an odd run, so `a\\$`, `a\$$b$$` and
/// `\\)b\)` close at their last delimiter. The scan starts at the line start, which no escape
/// crosses. Skipping the escaped byte may stop inside a multi-byte character;
/// its continuation bytes are never ASCII, so the result is a char boundary.
/// O(line bytes).
fn unescaped_closer_end(line: &str, close: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    if close == "$$" {
        let mut index = 0;
        while index < bytes.len() {
            match bytes[index] {
                b'\\' => index += 2,
                b'$' if bytes.get(index + 1) == Some(&b'$') => return Some(index + 2),
                _ => index += 1,
            }
        }
        return None;
    }
    let latex_close = close.as_bytes().get(1).copied();
    let mut index = 0;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' if latex_close.is_some() && bytes.get(index + 1).copied() == latex_close => {
                return Some(index + 2);
            }
            b'\\' => index += 2,
            b'$' if close == "$" => return Some(index + 1),
            _ => index += 1,
        }
    }
    None
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
