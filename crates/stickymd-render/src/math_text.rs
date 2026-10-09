//! Explicit, source-preserving text cleanup for display-dollar formulas.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#semantic-math-delimiter-conversion

use std::ops::Range;

use comrak::nodes::{NodeValue, Sourcepos};
use comrak::{Arena, Options, parse_document};

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

/// Joins bare equals lines only inside complete, unescaped display-dollar
/// pairs. A scope must contain the entire pair. Invalid scopes/source maps,
/// protected Markdown, structured TeX and no matches all fail closed as None.
/// This function neither converts delimiters nor changes preview semantics.
pub fn join_display_math_equals(
    source: &str,
    scope: Option<Range<usize>>,
) -> Option<EqualsLineConversion> {
    if scope
        .as_ref()
        .is_some_and(|range| source.get(range.clone()).is_none())
        || !source.contains("$$")
        || !source.lines().any(|line| line.trim() == "=")
    {
        return None;
    }
    let protected = protected_ranges(source)?;
    let bytes = source.as_bytes();
    let mut cursor = 0;
    let mut protection = 0;
    let mut open: Option<(usize, bool)> = None;
    let mut removed = Vec::new();
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
                        && scope.as_ref().is_none_or(|range| {
                            body_start - 2 >= range.start && cursor <= range.end
                        })
                    {
                        collect_joins(&source[body_start..start], body_start, &mut removed);
                    }
                } else {
                    open = Some((cursor, false));
                }
            }
            _ => cursor += 1,
        }
    }
    if removed.is_empty() {
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

/// Comrak owns Markdown exclusions. The local Setext option only prevents
/// equals lines from splitting multiline code/math during this explicit
/// text action; PreviewParser and delimiter conversion are not involved.
fn protected_ranges(source: &str) -> Option<Vec<Range<usize>>> {
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.math_dollars = true;
    options.extension.math_latex = true;
    options.parse.ignore_setext = true;
    let root = parse_document(&arena, source, &options);
    let line_starts: Vec<_> = std::iter::once(0)
        .chain(source.match_indices('\n').map(|(index, _)| index + 1))
        .collect();
    let mut ranges = Vec::new();
    for node in root.descendants() {
        let data = node.data.borrow();
        let protected = match &data.value {
            NodeValue::Code(_)
            | NodeValue::CodeBlock(_)
            | NodeValue::HtmlInline(_)
            | NodeValue::HtmlBlock(_)
            | NodeValue::Link(_)
            | NodeValue::Image(_) => true,
            NodeValue::Math(math) => !math.display_math || !math.dollar_math,
            _ => false,
        };
        if protected {
            ranges.push(source_range(source, &line_starts, data.sourcepos)?);
        }
    }
    ranges.sort_unstable_by_key(|range| range.start);
    Some(ranges)
}

fn source_range(source: &str, lines: &[usize], position: Sourcepos) -> Option<Range<usize>> {
    let start = lines
        .get(position.start.line.checked_sub(1)?)?
        .checked_add(position.start.column.checked_sub(1)?)?;
    let end = lines
        .get(position.end.line.checked_sub(1)?)?
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
