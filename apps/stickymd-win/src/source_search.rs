//! Pure literal search and single-pass replacement over canonical Source text.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-find-replace

use std::collections::VecDeque;
use std::ops::Range;

use crate::instruction::LiteralSearchOptions;

/// Engineering bound for retained navigation matches (2 MiB at 8 bytes each).
pub const MAX_RETAINED_MATCHES: usize = 262_144;

/// Compact UTF-8 byte range. Runtime note loading is bounded far below u32.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LiteralMatch {
    start: u32,
    end: u32,
}

impl LiteralMatch {
    fn new(range: Range<usize>) -> Option<Self> {
        Some(Self {
            start: u32::try_from(range.start).ok()?,
            end: u32::try_from(range.end).ok()?,
        })
    }

    pub fn range(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiteralMatches {
    pub ranges: Vec<LiteralMatch>,
    pub truncated: bool,
}

pub fn find_literal_matches(
    text: &str,
    query: &str,
    options: LiteralSearchOptions,
) -> LiteralMatches {
    if query.is_empty() {
        return LiteralMatches {
            ranges: Vec::new(),
            truncated: false,
        };
    }
    let mut ranges = Vec::new();
    let mut truncated = false;
    visit_matches(text, query, options, |range| {
        if ranges.len() == MAX_RETAINED_MATCHES {
            truncated = true;
            return false;
        }
        let Some(range) = LiteralMatch::new(range) else {
            truncated = true;
            return false;
        };
        ranges.push(range);
        true
    });
    LiteralMatches { ranges, truncated }
}

/// Builds Replace All output in one forward pass and reports the replacement count.
pub fn replace_all_literal(
    text: &str,
    query: &str,
    replacement: &str,
    options: LiteralSearchOptions,
) -> Option<(String, usize)> {
    if query.is_empty() {
        return None;
    }
    // Keep only the output and the current source cursor. Retaining every range
    // would make Replace All consume O(match count) auxiliary memory on inputs
    // such as a long run of one-byte matches.
    let mut output = String::with_capacity(text.len());
    let mut copied = 0;
    let mut count = 0;
    visit_matches(text, query, options, |range| {
        output.push_str(&text[copied..range.start]);
        output.push_str(replacement);
        copied = range.end;
        count += 1;
        true
    });
    if count == 0 {
        return None;
    }
    output.push_str(&text[copied..]);
    Some((output, count))
}

pub fn literal_range_matches(
    text: &str,
    range: Range<usize>,
    query: &str,
    options: LiteralSearchOptions,
) -> bool {
    let Some(candidate) = text.get(range) else {
        return false;
    };
    if options.case_sensitive {
        candidate == query
    } else {
        candidate
            .chars()
            .flat_map(char::to_lowercase)
            .eq(query.chars().flat_map(char::to_lowercase))
    }
}

fn visit_matches(
    text: &str,
    query: &str,
    options: LiteralSearchOptions,
    mut visit: impl FnMut(Range<usize>) -> bool,
) {
    if options.case_sensitive {
        for (start, matched) in text.match_indices(query) {
            if !visit(start..start + matched.len()) {
                break;
            }
        }
        return;
    }

    if text.is_ascii() && query.is_ascii() {
        visit_ascii_case_insensitive(text, query, visit);
        return;
    }
    visit_unicode_case_insensitive(text, query, visit);
}

fn visit_ascii_case_insensitive(
    text: &str,
    query: &str,
    mut visit: impl FnMut(Range<usize>) -> bool,
) {
    let pattern = query
        .bytes()
        .map(|byte| byte.to_ascii_lowercase())
        .collect::<Vec<_>>();
    let prefix = kmp_prefix(&pattern);
    let mut matched = 0;
    for (index, byte) in text.bytes().enumerate() {
        let folded = byte.to_ascii_lowercase();
        while matched > 0 && pattern[matched] != folded {
            matched = prefix[matched - 1];
        }
        if pattern[matched] == folded {
            matched += 1;
        }
        if matched == pattern.len() {
            let end = index + 1;
            if !visit(end - pattern.len()..end) {
                return;
            }
            // Match `str::match_indices`: replacements never overlap.
            matched = 0;
        }
    }
}

fn visit_unicode_case_insensitive(
    text: &str,
    query: &str,
    mut visit: impl FnMut(Range<usize>) -> bool,
) {
    let pattern = query
        .chars()
        .flat_map(char::to_lowercase)
        .collect::<Vec<_>>();
    if pattern.is_empty() {
        return;
    }
    let prefix = kmp_prefix(&pattern);
    let mut matched = 0;
    // Only the last pattern-length tokens are needed to recover the source
    // start and reject matches beginning inside a lowercase expansion.
    let mut boundaries = VecDeque::with_capacity(pattern.len());
    for (source_start, character) in text.char_indices() {
        let source_end = source_start + character.len_utf8();
        let mut folded = character.to_lowercase().peekable();
        let mut first = true;
        while let Some(token) = folded.next() {
            let last = folded.peek().is_none();
            while matched > 0 && pattern[matched] != token {
                matched = prefix[matched - 1];
            }
            if pattern[matched] == token {
                matched += 1;
            }
            boundaries.push_back((source_start, first));
            if boundaries.len() > pattern.len() {
                boundaries.pop_front();
            }
            if matched == pattern.len() {
                let (match_start, starts_at_source_boundary) =
                    boundaries.front().copied().unwrap_or((source_start, false));
                if starts_at_source_boundary && last && !visit(match_start..source_end) {
                    return;
                }
                // An invalid boundary match belongs to the current expanded
                // source scalar; resetting cannot skip a valid match starting
                // at the next scalar boundary.
                matched = 0;
                boundaries.clear();
            }
            first = false;
        }
    }
}

fn kmp_prefix<T: Eq>(pattern: &[T]) -> Vec<usize> {
    let mut prefix = vec![0; pattern.len()];
    let mut matched = 0;
    for index in 1..pattern.len() {
        while matched > 0 && pattern[index] != pattern[matched] {
            matched = prefix[matched - 1];
        }
        if pattern[index] == pattern[matched] {
            matched += 1;
        }
        prefix[index] = matched;
    }
    prefix
}

#[cfg(test)]
mod tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn literal_search_handles_unicode_case_and_char_boundaries() {
        let matches = find_literal_matches(
            "Rust RUST rust 中文中",
            "rust",
            LiteralSearchOptions {
                case_sensitive: false,
            },
        );
        assert_eq!(matches.ranges.len(), 3);
        assert!(
            matches
                .ranges
                .iter()
                .all(|range| text_boundary("Rust RUST rust 中文中", range.range()))
        );
        assert_eq!(
            find_literal_matches(
                "Ä ä A",
                "ä",
                LiteralSearchOptions {
                    case_sensitive: false,
                }
            )
            .ranges
            .len(),
            2
        );
    }

    #[test]
    fn replace_all_is_one_linear_projection_and_preserves_unmatched_bytes() {
        let (output, count) =
            replace_all_literal("a中文a中文a", "中文", "🙂", LiteralSearchOptions::default())
                .unwrap();
        assert_eq!(output, "a🙂a🙂a");
        assert_eq!(count, 2);
    }

    #[test]
    fn lowercase_expansion_never_creates_a_mid_codepoint_source_range() {
        let matches = find_literal_matches(
            "İ i",
            "i",
            LiteralSearchOptions {
                case_sensitive: false,
            },
        );
        assert_eq!(matches.ranges.len(), 1);
        assert_eq!(matches.ranges[0].range(), "İ ".len().."İ i".len());
    }

    #[test]
    fn replace_all_does_not_require_retaining_match_ranges() {
        let source = "a".repeat(100_000);
        let (output, count) =
            replace_all_literal(&source, "a", "bb", LiteralSearchOptions::default()).unwrap();
        assert_eq!(count, 100_000);
        assert_eq!(output.len(), 200_000);
        assert!(output.bytes().all(|byte| byte == b'b'));
    }

    #[test]
    #[ignore = "Release-only one MiB literal-search performance receipt"]
    fn phase14_one_mib_unicode_case_insensitive_search_p95_is_bounded() {
        let line = "中文 Rust 🙂 combining e\u{301} line\n";
        let mut source = String::with_capacity(1024 * 1024 + line.len());
        while source.len() < 1024 * 1024 {
            source.push_str(line);
        }
        let options = LiteralSearchOptions {
            case_sensitive: false,
        };
        for _ in 0..3 {
            std::hint::black_box(find_literal_matches(&source, "RUST", options));
        }
        let mut samples = Vec::with_capacity(30);
        for _ in 0..30 {
            let started = Instant::now();
            let matches = find_literal_matches(&source, "RUST", options);
            assert!(!matches.ranges.is_empty());
            samples.push(started.elapsed());
        }
        samples.sort_unstable();
        let median = samples[samples.len() / 2];
        let p95 = samples[samples.len() * 95 / 100];
        let max = samples[samples.len() - 1];
        eprintln!("phase14 literal-search 1MiB median={median:?} p95={p95:?} max={max:?}");
        assert!(p95.as_millis() < 50, "1 MiB search p95 {p95:?} >= 50 ms");
    }

    fn text_boundary(text: &str, range: Range<usize>) -> bool {
        text.is_char_boundary(range.start) && text.is_char_boundary(range.end)
    }
}
