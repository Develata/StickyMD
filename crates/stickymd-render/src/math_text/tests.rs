use std::ops::Range;

use super::join_display_math_equals;

fn assert_conversion(source: &str, expected: &str) {
    let conversion = join_display_math_equals(source, None).expect(source);
    assert_eq!(conversion.text(), expected, "{source:?}");
    assert!(join_display_math_equals(conversion.text(), None).is_none());
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
        "$a\n=\nb$",
        "\\(a\n=\nb\\)",
        "\\[a\n=\nb\\]",
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
        assert!(
            join_display_math_equals(source, None).is_none(),
            "{source:?}"
        );
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
            join_display_math_equals(&protected, None).is_none(),
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
        assert!(
            join_display_math_equals(source, None).is_none(),
            "{source:?}"
        );
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
            join_display_math_equals(&source, None).is_none(),
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
            join_display_math_equals(&source, None).is_none(),
            "{source:?}"
        );
    }
}

#[test]
fn phase11b_equals_lines_scope_requires_a_complete_block() {
    let source = "前 $$a\n=\nb$$ 中 $$甲\n=\n乙$$ 后";
    let start = source.find("$$甲").unwrap();
    let end = source.find(" 后").unwrap();
    let conversion = join_display_math_equals(source, Some(start..end)).unwrap();
    assert_eq!(conversion.text(), "前 $$a\n=\nb$$ 中 $$甲=乙$$ 后");
    for scope in [
        start + 2..end,
        start..end - 2,
        start..start,
        1..source.len(),
        0..source.len() + 1,
    ] {
        assert!(join_display_math_equals(source, Some(scope)).is_none());
    }
    let reversed = Range {
        start: end,
        end: start,
    };
    assert!(join_display_math_equals(source, Some(reversed)).is_none());
}

#[test]
fn phase11b_equals_lines_map_retained_bytes_and_removed_whitespace() {
    let source = "$$\n甲 \r\n \t= \r\n  乙\n$$ tail";
    let conversion = join_display_math_equals(source, None).unwrap();
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
