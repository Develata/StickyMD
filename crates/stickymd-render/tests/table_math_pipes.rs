//! Table escaping must preserve canonical ranges through all preview consumers.

use std::sync::Arc;

use stickymd_core::{DocumentSnapshot, Generation, LineEnding};
use stickymd_render::preview::{
    BlockNode, ImageRewrite, InlineNode, MathNode, PreviewParser, PreviewPipeline,
    PreviewSelection, PreviewTheme, TableNode, collect_image_occurrences,
    convert_latex_math_delimiters, rewrite_image_occurrences,
};

fn snapshot(source: &str) -> DocumentSnapshot {
    DocumentSnapshot {
        text: Arc::from(source),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    }
}

fn screenshot_table(formula: &str) -> String {
    format!(
        "| $i$ | $v(i)$ | $\\mathcal N_i$ | new children | $c(i)$ | {formula} |\n\
         | --- | --- | --- | --- | --- | --- |\n\
         | 1 | 1 | $\\{{3,4\\}}$ | $\\{{3,4\\}}$ | 2 | 2 |\n"
    )
}

fn first_table(blocks: &[BlockNode]) -> &TableNode {
    match &blocks[0] {
        BlockNode::Table(table) => table,
        BlockNode::BlockQuote { blocks, .. } => first_table(blocks),
        BlockNode::List(list) => first_table(&list.items[0].blocks),
        other => panic!("expected table, got {other:?}"),
    }
}

fn math_nodes(inlines: &[InlineNode]) -> Vec<&MathNode> {
    inlines
        .iter()
        .flat_map(|inline| match inline {
            InlineNode::InlineMath(math) => vec![math],
            InlineNode::Emphasis { children, .. }
            | InlineNode::Strong { children, .. }
            | InlineNode::Link { children, .. } => math_nodes(children),
            _ => Vec::new(),
        })
        .collect()
}

#[test]
fn unescaped_math_pipes_follow_gfm_table_column_rules() {
    let source = snapshot(&screenshot_table(r"$|\mathcal N_i|$"));
    let tree = PreviewParser.parse(&source).unwrap();
    assert!(matches!(
        tree.blocks.first(),
        Some(BlockNode::Paragraph { .. })
    ));
}

#[test]
fn absolute_value_commands_render_a_six_column_table() {
    for formula in [
        r"$\lvert\mathcal N_i\rvert$",
        r"$\left\lvert\mathcal N_i\right\rvert$",
        r"$\|\mathcal N_i\|$",
    ] {
        let source = snapshot(&screenshot_table(formula));
        let tree = PreviewParser.parse(&source).unwrap();
        let table = first_table(&tree.blocks);
        assert_eq!(table.alignments.len(), 6);
        let math = math_nodes(&table.rows[0].cells[5].content)[0];
        assert_eq!(math.source_literal, formula);
        assert_eq!(math.source_range.unwrap().text(&source.text), Some(formula));

        let mut pipeline = PreviewPipeline::new();
        let frame = pipeline
            .build(
                &source,
                1000,
                300,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        assert_eq!(pipeline.math_counters().rasterizations, 6);
        assert!(
            frame
                .copy_selection(frame.select_all())
                .unwrap()
                .contains(formula)
        );
    }
}

#[test]
fn escaped_pipes_preserve_all_math_delimiters_and_following_unicode_ranges() {
    for formula in [r"$\|x\|$", r"$$\|x\|$$", r"\(\|x\|\)", r"\[\|x\|\]"] {
        for (first_prefix, next_prefix) in [("", ""), ("  ", "  "), ("> ", "> "), ("- ", "  ")] {
            let cell = format!("\t 前 \\| 🙂 **{formula}** 中 $z$ ");
            let source = snapshot(&format!(
                "{first_prefix}|{cell}| $u$ |\r\n{next_prefix}| --- | --- |\r\n{next_prefix}|{cell}| $v$ |\r\n\r\n$out$\r\n"
            ));
            let tree = PreviewParser.parse(&source).unwrap();
            let table = first_table(&tree.blocks);
            for (row, next_formula) in table.rows.iter().zip(["$u$", "$v$"]) {
                let math = math_nodes(&row.cells[0].content);
                assert_eq!(math.len(), 2);
                for (math, expected) in math.into_iter().zip([formula, "$z$"]) {
                    assert_eq!(math.source_literal, expected, "{source:?}");
                    assert_eq!(
                        math.source_range.unwrap().text(&source.text),
                        Some(expected)
                    );
                }
                assert_eq!(
                    math_nodes(&row.cells[1].content)[0].source_literal,
                    next_formula
                );
            }
            let BlockNode::Paragraph { content, .. } = tree.blocks.last().unwrap() else {
                panic!("outside paragraph");
            };
            assert_eq!(math_nodes(content)[0].source_literal, "$out$");
        }
    }
}

#[test]
fn escaped_pipes_preserve_inline_code_emphasis_and_link_ranges() {
    let source = snapshot("| \\| *中* **bold** `a\\|b` [link](local.md) |\n| --- |\n");
    let tree = PreviewParser.parse(&source).unwrap();
    let table = first_table(&tree.blocks);
    let ranges: Vec<_> = table.rows[0].cells[0]
        .content
        .iter()
        .filter(|inline| !matches!(inline, InlineNode::Text { .. }))
        .map(|inline| inline.source_range().unwrap().text(&source.text).unwrap())
        .collect();
    assert_eq!(ranges, ["*中*", "**bold**", "`a\\|b`", "[link](local.md)"]);
}

#[test]
fn backslash_runs_do_not_shift_following_math_ranges() {
    for prefix in [r"\|", r"\\\|", r"\\\\\|", r"\\ \|", r"a\\b \|", r"\|\|"] {
        let source = snapshot(&format!("| {prefix} $x$ |\n| --- |\n"));
        let tree = PreviewParser.parse(&source).unwrap();
        let content = &first_table(&tree.blocks).rows[0].cells[0].content;
        assert_eq!(math_nodes(content)[0].source_literal, "$x$");
    }
}

#[test]
fn image_export_after_escaped_pipes_rewrites_only_the_image() {
    let source = snapshot("| 前 \\| ![中](a.png) 后 |\n| --- |\n| z |\n");
    let tree = PreviewParser.parse(&source).unwrap();
    let images = collect_image_occurrences(&tree).unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(
        images[0].source_range.text(&source.text),
        Some("![中](a.png)")
    );
    let rewritten = rewrite_image_occurrences(
        &source.text,
        vec![ImageRewrite {
            source_range: images[0].source_range,
            destination: "export/a.png".into(),
            title: String::new(),
            alt: images[0].alt.clone(),
        }],
    )
    .unwrap();
    assert_eq!(rewritten, source.text.replace("(a.png)", "(export/a.png)"));
}

#[test]
fn delimiter_conversion_after_escaped_pipes_preserves_surrounding_source() {
    let source = snapshot("| 前 \\| \\(\\|x\\|\\) 中 \\[\\|y\\|\\] 后 |\n| --- |\n| z |\n");
    let conversion = convert_latex_math_delimiters(&source, None)
        .unwrap()
        .unwrap();
    assert_eq!(conversion.replacement_count(), 2);
    assert_eq!(
        conversion.text(),
        "| 前 \\| $\\|x\\|$ 中 $$\\|y\\|$$ 后 |\n| --- |\n| z |\n"
    );
}
