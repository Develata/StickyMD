//! libtest `--list` output, read strictly.
//!
//! The default format is one `<name>: test` (or `: bench`) line per test, then, when any test
//! was listed, one empty line, then the summary `<n> test(s), <m> benchmark(s)` as the last
//! line. Every line must match; the summary counts must equal the entries, so a truncated or
//! interleaved listing is an error, never a shorter list.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

/// The test names `--list` printed, in order (benchmarks excluded).
pub(crate) fn parse_list(stdout: &str) -> Result<Vec<&str>, String> {
    let text = stdout
        .strip_suffix('\n')
        .ok_or("test listing does not end with a newline")?;
    let lines = text.split('\n').collect::<Vec<_>>();
    let (summary, entries) = lines
        .split_last()
        .ok_or("test listing has no summary line")?;
    let (tests, benches) = summary_counts(summary)?;
    let entries = match entries.split_last() {
        None => entries,
        Some((separator, entries)) if separator.is_empty() && !entries.is_empty() => entries,
        _ => return Err("test listing lacks the blank line before its summary".to_owned()),
    };
    let mut names = Vec::with_capacity(entries.len());
    let mut bench_count = 0;
    for line in entries {
        if let Some(name) = line.strip_suffix(": test").filter(|name| is_name(name)) {
            names.push(name);
        } else if line.strip_suffix(": bench").is_some_and(is_name) {
            bench_count += 1;
        } else {
            return Err(format!("unexpected test listing line {line:?}"));
        }
    }
    if names.len() != tests || bench_count != benches {
        return Err(format!(
            "test listing has {} test(s) and {bench_count} benchmark(s), its summary says {tests} and {benches}",
            names.len()
        ));
    }
    Ok(names)
}

/// `<n> test(s), <m> benchmark(s)`, with libtest's singular for 1.
fn summary_counts(line: &str) -> Result<(usize, usize), String> {
    let invalid = || format!("unexpected test listing summary {line:?}");
    let (tests, benches) = line.split_once(", ").ok_or_else(invalid)?;
    let count = |part: &str, noun: &str| -> Option<usize> {
        let (digits, word) = part.split_once(' ')?;
        let number = digits.parse::<usize>().ok()?;
        let plural = word.strip_prefix(noun)?;
        // Canonical digits only, so `007 tests` is not read as 7.
        (number.to_string() == digits && plural == if number == 1 { "" } else { "s" })
            .then_some(number)
    };
    Ok((
        count(tests, "test").ok_or_else(invalid)?,
        count(benches, "benchmark").ok_or_else(invalid)?,
    ))
}

/// A libtest name: a non-empty path without whitespace or a `: ` separator of its own.
fn is_name(name: &str) -> bool {
    !name.is_empty() && !name.contains(char::is_whitespace) && !name.contains(": ")
}

#[cfg(test)]
mod tests {
    use super::parse_list;

    /// Output of `stickymd_win-*.exe --list` on Rust 1.97.1, shortened.
    const LISTING: &str = "app::escape::tests::each_guard_alone_ignores_esc: test\napp::escape::tests::modifiers_never_change_esc: test\n\n2 tests, 0 benchmarks\n";

    #[test]
    fn the_listing_grammar_matches_this_toolchain() {
        assert_eq!(
            parse_list(LISTING).unwrap(),
            [
                "app::escape::tests::each_guard_alone_ignores_esc",
                "app::escape::tests::modifiers_never_change_esc"
            ]
        );
        assert_eq!(
            parse_list("a::b: test\n\n1 test, 0 benchmarks\n").unwrap(),
            ["a::b"]
        );
        assert!(parse_list("0 tests, 0 benchmarks\n").unwrap().is_empty());
        assert_eq!(
            parse_list("a: test\nb: bench\n\n1 test, 1 benchmark\n").unwrap(),
            ["a"]
        );
    }

    #[test]
    fn a_truncated_or_interleaved_listing_is_an_error() {
        for broken in [
            "",
            "a: test\n",
            "a: test\n\n2 tests, 0 benchmarks\n",
            "a: test\n\n1 tests, 0 benchmarks\n",
            "a: test\n\n01 test, 0 benchmarks\n",
            "a: test\n1 test, 0 benchmarks\n",
            "a: test\nprinted by a test\n\n1 test, 0 benchmarks\n",
            "a: test\n\n1 test, 0 benchmarks",
            "\n0 tests, 0 benchmarks\n",
            "a b: test\n\n1 test, 0 benchmarks\n",
            "a: test\n\n1 test, 0 benchmarks\nextra\n",
        ] {
            assert!(parse_list(broken).is_err(), "{broken:?}");
        }
    }
}
