use std::ops::Range;

use super::join_math_equals_lines;

fn assert_conversion(source: &str, expected: &str) {
    let conversion = join_math_equals_lines(source, None).expect(source);
    assert_eq!(conversion.text(), expected, "{source:?}");
    assert!(join_math_equals_lines(conversion.text(), None).is_none());
    assert_eq!(conversion.map_position(source.len()), expected.len());
    for position in (0..=source.len()).filter(|&position| source.is_char_boundary(position)) {
        assert!(expected.is_char_boundary(conversion.map_position(position)));
    }
}

#[test]
fn phase11b_equals_lines_join_chains_and_preserve_other_bytes() {
    for (source, expected) in [
        ("$$\na\n=\nb\n$$", "$$\na=b\n$$"),
        ("$$a\n=\nb\n=\nc$$", "$$a=b=c$$"),
        ("$$\n  a \t\n \t= \n \tb  \n$$", "$$\n  a=b  \n$$"),
        ("$$\r\n a \r\n =\t\r\n b\r\n$$", "$$\r\n a=b\r\n$$"),
        (
            "$$\n甲🙂\n\u{3000}=\u{3000}\n\u{3000}乙\n$$",
            "$$\n甲🙂=乙\n$$",
        ),
        ("$$a\n=\nb\nc\n=\nd$$", "$$a=b\nc=d$$"),
        ("$$a\n=\nb\n\nc\n=\nd$$", "$$a=b\n\nc=d$$"),
        ("$$a\n=\nb$$\n\n$$c\n=\nd$$", "$$a=b$$\n\n$$c=d$$"),
        ("$$a\n=\n\nb\n=\nc$$", "$$a\n=\n\nb=c$$"),
        ("$$=\na\n=\nb\n=$$", "$$=\na=b\n=$$"),
        ("$$a\n=\nb$$\r\nTAIL\r\n", "$$a=b$$\r\nTAIL\r\n"),
        ("- $$a\n  =\n  b$$", "- $$a=b$$"),
        ("标题\n====\n\n$$a\n=\nb$$", "标题\n====\n\n$$a=b$$"),
        ("$$\\frac{a}{b}\n=\nx+\\%$$", "$$\\frac{a}{b}=x+\\%$$"),
    ] {
        assert_conversion(source, expected);
    }
}

#[test]
fn phase11b_equals_lines_preserve_plain_inline_escaped_and_incomplete_inputs() {
    for source in [
        "a\n=\nb",
        "$$a\n\\=\nb$$",
        "$$\n\\=\n$$",
        "$$\\={a}$$",
        "$$a\n+\nb$$",
        "$$a\n<=\nb$$",
        "$$a\n==\nb$$",
        "$$a\n-\nb$$",
        "$$a\n=\nb",
        "a\n=\nb$$",
        "\\$\\$a\n=\nb\\$\\$",
        "\\$$a\n=\nb$$",
        "$$$a\n=\nb$$$",
        "$$$$a\n=\nb$$$$",
        "$$=\nb$$",
        "$$a\n=$$",
        "$$\n=\n$$",
        "$$a\n=\n \t\nb$$",
        "$$a\n \t\n=\nb$$",
        "$$a\n=\n=\nb$$",
        "$$a$$\n=\n$$b$$",
        "$$a\n=$$\n\n$$b\n=\nc",
        "$$a % $$ dollars inside a comment\n=\nb$$",
    ] {
        assert!(join_math_equals_lines(source, None).is_none(), "{source:?}");
    }
}

#[test]
fn phase11b_equals_lines_skip_markdown_code_and_literal_contexts() {
    let formula = "$$a\n=\nb$$";
    for protected in [
        format!("`{formula}`"),
        format!("``{formula}`extra``"),
        format!("```math\n{formula}\n```"),
        format!("~~~text\n{formula}\n~~~"),
        "    $$a\n    =\n    b$$".to_owned(),
        "> ```\n> $$a\n> =\n> b$$\n> ```".to_owned(),
        "- ```\n  $$a\n  =\n  b$$\n  ```".to_owned(),
        format!("<pre>\n{formula}\n</pre>"),
        format!("[{formula}](https://example.com)"),
    ] {
        assert!(
            join_math_equals_lines(&protected, None).is_none(),
            "{protected:?}"
        );
        assert_conversion(
            &format!("{protected}\n\n{formula}"),
            &format!("{protected}\n\n$$a=b$$"),
        );
    }
    for source in [
        "```\n$$a\n=\nb$$",
        "$$a\n\n```\n=\n```\n\nb$$",
        "$$a\n\n`code`\n\nc\n=\nd$$",
    ] {
        assert!(join_math_equals_lines(source, None).is_none(), "{source:?}");
    }
}

#[test]
fn phase11b_equals_lines_skip_tex_comments_breaks_and_environments() {
    for content in [
        "a % comment\n=\nb",
        "a\n=\nb % comment after the join",
        "a\\\\\n=\nb",
        "a\\\\[2pt]\n=\nb",
        "a\\cr\n=\nb",
        "a\\crcr\n=\nb",
        "a\\newline\n=\nb",
        "a\\linebreak[2]\n=\nb",
        "a\\displaybreak\n=\nb",
        "a\\\n=\nb",
        "a\\ \n=\nb",
    ] {
        let source = format!("$${content}$$");
        assert!(
            join_math_equals_lines(&source, None).is_none(),
            "{source:?}"
        );
        assert_conversion(
            &format!("{source}\n\n$$x\n=\ny$$"),
            &format!("{source}\n\n$$x=y$$"),
        );
    }
    for environment in [
        "aligned",
        "align",
        "align*",
        "alignedat",
        "cases",
        "matrix",
        "pmatrix",
        "bmatrix",
        "vmatrix",
        "Vmatrix",
        "smallmatrix",
        "array",
        "gather",
        "split",
    ] {
        let source = format!("$$\\begin{{{environment}}}\na\n=\nb\n\\end{{{environment}}}$$");
        assert!(
            join_math_equals_lines(&source, None).is_none(),
            "{source:?}"
        );
    }
}

#[test]
fn phase11b_equals_lines_scope_requires_a_complete_block() {
    let source = "前 $$a\n=\nb$$ 中 $$甲\n=\n乙$$ 后";
    let start = source.find("$$甲").unwrap();
    let end = source.find(" 后").unwrap();
    let conversion = join_math_equals_lines(source, Some(start..end)).unwrap();
    assert_eq!(conversion.text(), "前 $$a\n=\nb$$ 中 $$甲=乙$$ 后");
    for scope in [
        start + 2..end,
        start..end - 2,
        start..start,
        1..source.len(),
        0..source.len() + 1,
    ] {
        assert!(join_math_equals_lines(source, Some(scope)).is_none());
    }
    let reversed = Range {
        start: end,
        end: start,
    };
    assert!(join_math_equals_lines(source, Some(reversed)).is_none());
}

#[test]
fn phase11b_equals_lines_map_retained_bytes_and_removed_whitespace() {
    let source = "$$\n甲 \r\n \t= \r\n  乙\n$$ tail";
    let conversion = join_math_equals_lines(source, None).unwrap();
    let expected = "$$\n甲=乙\n$$ tail";
    assert_eq!(conversion.text(), expected);
    for retained in ["甲", "=", "乙", "tail"] {
        assert_eq!(
            conversion.map_position(source.find(retained).unwrap()),
            expected.find(retained).unwrap(),
        );
    }
    let before_equal = source.find('甲').unwrap() + '甲'.len_utf8();
    for position in before_equal..source.find('=').unwrap() {
        assert_eq!(
            conversion.map_position(position),
            expected.find('=').unwrap()
        );
    }
    for position in source.find('=').unwrap() + 1..source.find('乙').unwrap() {
        assert_eq!(
            conversion.map_position(position),
            expected.find('乙').unwrap()
        );
    }
}

#[test]
fn phase11b_equals_lines_join_inside_latex_and_single_dollar_formulas() {
    for (source, expected) in [
        (
            "\\[\n\\rho'(g^{op})(y)\n=\n\\rho(g^{op})(y).\n\\]",
            "\\[\n\\rho'(g^{op})(y)=\\rho(g^{op})(y).\n\\]",
        ),
        ("\\[a\n=\nb\\]", "\\[a=b\\]"),
        ("\\(a\n=\nb\\)", "\\(a=b\\)"),
        ("$a\n=\nb$", "$a=b$"),
        ("文中 \\(a\n=\nb\n=\nc\\) 后", "文中 \\(a=b=c\\) 后"),
        ("\\[\r\n a \r\n =\t\r\n b\r\n\\]", "\\[\r\n a=b\r\n\\]"),
        ("- \\[a\n  =\n  b\\]", "- \\[a=b\\]"),
        ("$$x\n=\ny$$ 与 \\[a\n=\nb\\]", "$$x=y$$ 与 \\[a=b\\]"),
        // The same physical join seen from the `$$` pair and the inner
        // formula is applied once.
        ("$$x \\[a\n=\nb\\] y$$", "$$x \\[a=b\\] y$$"),
    ] {
        assert_conversion(source, expected);
    }
}

#[test]
fn phase11b_equals_lines_leave_text_outside_formulas_and_unsafe_formulas() {
    for source in [
        "标题\n=\n正文",
        "\\[\n=\nb\n\\]",
        "\\[\na\n=\n\\]",
        "\\[\na\n\n=\nb\n\\]",
        "\\[\na\n=\n=\nb\n\\]",
        "\\\\[a\n=\nb\\\\]",
        "> \\[\n> a\n> =\n> b\n> \\]",
        "[\\(a\n=\nb\\)](https://example.com)",
        "`\\[a\n=\nb\\]`",
        "\\[a % comment\n=\nb\\]",
        "\\[a\\\\\n=\nb\\]",
        "\\[\\begin{aligned}a\n=\nb\\end{aligned}\\]",
        "\\(a\n=\nb",
    ] {
        assert!(join_math_equals_lines(source, None).is_none(), "{source:?}");
    }
}

#[test]
fn phase11b_equals_lines_scope_requires_a_complete_latex_formula() {
    let source = "前 \\[a\n=\nb\\] 中 \\(甲\n=\n乙\\) 后";
    let start = source.find("\\(甲").unwrap();
    let end = source.find(" 后").unwrap();
    let conversion = join_math_equals_lines(source, Some(start..end)).unwrap();
    assert_eq!(conversion.text(), "前 \\[a\n=\nb\\] 中 \\(甲=乙\\) 后");
    for scope in [start + 1..end, start..end - 1, start..start] {
        assert!(join_math_equals_lines(source, Some(scope)).is_none());
    }
}

#[test]
fn phase11b_equals_lines_locate_misreported_multiline_formula_ends() {
    // Before 0.56, Comrak misreported the end column of a multiline inline
    // that starts after its block's first line (kivikakk/comrak#855). The end
    // is still taken from the end line, so nothing past the real closer joins
    // and display-dollar ends are never read, whatever the column says.
    for (source, expected) in [
        ("Lead\n  \\(a\n=\nb\\)", "Lead\n  \\(a=b\\)"),
        (
            "Lead\n    \\(a\n  =\n b\\) tail",
            "Lead\n    \\(a=b\\) tail",
        ),
        ("Lead\n$a\n       =\nb$\nx\n=\ny$", "Lead\n$a=b$\nx\n=\ny$"),
        ("Lead\n$$a\n   =\nb$$", "Lead\n$$a=b$$"),
        (
            "\\(a\n=\n  b\\) \\(c\n  =\nd\\) tail",
            "\\(a=b\\) \\(c=d\\) tail",
        ),
        (
            "Lead\n\\[a\n   =\nb \\$ c\\] x\\]",
            "Lead\n\\[a=b \\$ c\\] x\\]",
        ),
        ("Lead\n$a\n   =\nb\\$c$ d$", "Lead\n$a=b\\$c$ d$"),
    ] {
        assert_conversion(source, expected);
    }
}

/// Comrak 1.2's escape parity (its math spec): `\\` no longer escapes the closer after it, so
/// `$a\n\\$` closes at its `\\$`, `\(a\n\\)b\)` at its last `\)`, and `\(a\n\\\)` at its `\)`. The
/// multiline closer is found the same way, so these inline formulas are located and the step
/// still joins the display formula after them instead of failing closed.
#[test]
fn phase11b_equals_lines_locate_closers_after_escaped_backslashes() {
    for formula in [
        "$a\n\\\\$ b$",
        "\\(a\n\\\\)b\\)",
        "\\(a\n\\\\\\)",
        "$a\n\\$\\\\$",
    ] {
        assert_conversion(
            &format!("{formula}\n\n\\[x\n=\ny\\]"),
            &format!("{formula}\n\n\\[x=y\\]"),
        );
    }
}

/// A paragraph that starts with link reference definitions: Comrak 1.0 and later report its
/// inline positions in place (0.59 shifted them up by the removed lines), so its real formulas
/// join, while a title that repeats one stays as written.
#[test]
fn phase11b_equals_lines_join_formulas_after_reference_definitions() {
    for (source, expected) in [
        (
            "[x]: u \"\\(a\n=\nb\\)\"\n12345678$$c\n=\nd$$",
            "[x]: u \"\\(a\n=\nb\\)\"\n12345678$$c=d$$",
        ),
        (
            "[x]: u \"a\n=\nb$\"\n1234567890123$c\n=\nd$",
            "[x]: u \"a\n=\nb$\"\n1234567890123$c=d$",
        ),
        (
            "[x]: u \"$a\n=\nb$\"\n12345678$a\n=\nb$",
            "[x]: u \"$a\n=\nb$\"\n12345678$a=b$",
        ),
        ("[x]: u\n\\[a\n=\nb\\]", "[x]: u\n\\[a=b\\]"),
    ] {
        assert_conversion(source, expected);
    }
}

#[test]
fn phase11b_equals_lines_reject_positions_that_disagree_with_the_formula() {
    for source in [
        // Lazy continuation lines put the quote marker inside the range.
        "> $a\n=\n> b$",
        "> \\(a\n=\n> b\\)",
        "> \\[a\n=\n> b\\]",
    ] {
        assert!(join_math_equals_lines(source, None).is_none(), "{source:?}");
    }
    // A paragraph of definitions only is detached by Comrak: its lines,
    // titles included, are protected, inside containers too.
    for source in [
        "[x]: u \"$$a\n=\nb$$\"\n\n[x]",
        "[x]: u \"$$a\n=\nb$$\"",
        "- [x]: u \"$$a\n  =\n  b$$\"\n\n  [x]",
    ] {
        assert!(join_math_equals_lines(source, None).is_none(), "{source:?}");
    }
    // A definition in its own paragraph shifts nothing after it.
    assert_conversion("[x]: u\n\n\\[a\n=\nb\\]", "[x]: u\n\n\\[a=b\\]");
    assert_conversion("[x]: u\n\n$$a\n=\nb$$", "[x]: u\n\n$$a=b$$");
    // A list marker needs a space or the line end, and at most nine digits;
    // anything else is text and stays protected inside a detached title.
    for line in ["-x", "1234567890.", "-=", "="] {
        let source = format!("[x]: u \"$$a\n{line}\n=\nb$$\"");
        assert!(
            join_math_equals_lines(&source, None).is_none(),
            "{source:?}"
        );
    }
    // An empty block quote or list item inside a `$$` pair is markup, not a
    // detached definition, so the pair still joins.
    for marker in [">", "-", "1.", "> -", "- >", "- -", "1. >", "> - >", ">>"] {
        for newline in ["\n", "\r\n"] {
            let source = format!(
                "[x]: u{newline}{newline}$$a{newline}={newline}b{newline}{newline}{marker}{newline}{newline}c$$"
            );
            let expected = format!(
                "[x]: u{newline}{newline}$$a=b{newline}{newline}{marker}{newline}{newline}c$$"
            );
            assert_conversion(&source, &expected);
        }
    }
    // A paragraph that ends in a link whose destination and title span lines
    // joins like any other.
    assert_conversion("\\(a\n=\nb\\)\n[x](u\n\"t\")", "\\(a=b\\)\n[x](u\n\"t\")");
    // Deeply nested quotes are walked once, with no per-node ancestor scan.
    let deep = format!("{}\\[a\n=\nb\\]", "> ".repeat(20_000));
    assert_conversion(&deep, &format!("{}\\[a=b\\]", "> ".repeat(20_000)));
}

#[test]
fn phase11b_equals_lines_leave_tables_whole() {
    for source in [
        "| \\[a | b |\n| --- | --- |\n=\nc\\]",
        "| $$a | b |\n| --- | --- |\n=\nc$$",
    ] {
        assert!(join_math_equals_lines(source, None).is_none(), "{source:?}");
    }
    // Cells with `\|` shift Comrak's inline columns; the table is skipped
    // whole and formulas outside it still join.
    assert_conversion(
        "| \\(x\\|y\\) | `b\\|c` |\n| --- | --- |\n\n\\[x\n=\ny\\]",
        "| \\(x\\|y\\) | `b\\|c` |\n| --- | --- |\n\n\\[x=y\\]",
    );
}

/// `joined_equals` names the `=` of every joined line in the joined text.
#[test]
fn phase11b_equals_lines_name_each_joined_equals_in_the_result() {
    let source = "$$\na \n =\n b\n=\nc$$ x \\[d\n=\ne\\]";
    let conversion = join_math_equals_lines(source, None).unwrap();
    let text = conversion.text();
    let equals: Vec<_> = conversion.joined_equals().collect();
    assert_eq!(text, "$$\na=b=c$$ x \\[d=e\\]");
    assert_eq!(equals.len(), conversion.joined_lines());
    assert!(equals.iter().all(|&at| text.as_bytes()[at] == b'='));
    assert!(equals.windows(2).all(|pair| pair[0] < pair[1]));
}

/// The byte-by-byte scan `collect_dollar_pairs` used before it jumped to the next `$` or `\`
/// with `memchr2`, kept as the reference the jumping scan must match.
fn reference_dollar_pairs(
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
                        super::collect_joins(&source[body_start..start], body_start, removed);
                    }
                } else {
                    open = Some((cursor, false));
                }
            }
            _ => cursor += 1,
        }
    }
}

/// Fixed-seed sources built from the scan's tokens, with sorted disjoint protected ranges and
/// optional scopes on char boundaries: the jumping scan removes exactly what the reference
/// scan removes.
#[test]
fn phase11b_dollar_pair_scan_matches_the_byte_by_byte_reference() {
    let alphabet = [
        "$$", "$$", "$$", "a", "b", "a b", "\n=\n", "\n=\n", "\n", "$", "\\", "\\$", " ", "中",
        "$$$", "=",
    ];
    let mut state = 0x2545_f491_4f6c_dd1d_u64;
    let mut next = |bound: usize| {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((state >> 33) as usize) % bound.max(1)
    };
    let mut joined = 0;
    for case in 0..50_000 {
        let length = case % 16 + 1;
        let source: String = (0..length)
            .map(|_| alphabet[next(alphabet.len())])
            .collect();
        let boundaries: Vec<usize> = (0..=source.len())
            .filter(|&at| source.is_char_boundary(at))
            .collect();
        let mut cuts: Vec<usize> = (0..next(3) * 2)
            .map(|_| boundaries[next(boundaries.len())])
            .collect();
        cuts.sort_unstable();
        cuts.dedup();
        let protected: Vec<Range<usize>> =
            cuts.chunks_exact(2).map(|pair| pair[0]..pair[1]).collect();
        let scope = (next(3) == 0).then(|| {
            let (a, b) = (
                boundaries[next(boundaries.len())],
                boundaries[next(boundaries.len())],
            );
            a.min(b)..a.max(b)
        });
        let (mut expected, mut actual) = (Vec::new(), Vec::new());
        reference_dollar_pairs(&source, &protected, scope.as_ref(), &mut expected);
        super::collect_dollar_pairs(&source, &protected, scope.as_ref(), &mut actual);
        assert_eq!(actual, expected, "{source:?} {protected:?} {scope:?}");
        joined += usize::from(!expected.is_empty());
    }
    // The tokens make pairs with joins common, not only empty results.
    assert!(joined > 100, "{joined}");
}
