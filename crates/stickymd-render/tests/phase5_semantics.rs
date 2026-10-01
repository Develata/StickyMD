use std::borrow::Cow;
use std::sync::Arc;

use stickymd_core::{DocumentSnapshot, Generation, LineEnding};
use stickymd_render::preview::{
    BlockNode, ImageKind, InlineNode, LinkKind, OwnedDocumentTree, PreviewParser,
};

const SOURCE: &str = include_str!("fixtures/phase5-semantic-all.md");
const EXPECTED: &str = include_str!("fixtures/phase5-owned-outline.txt");

#[test]
fn owned_ast_golden_is_stable_and_arena_free() {
    let snapshot = DocumentSnapshot {
        text: Arc::from(SOURCE),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let tree = PreviewParser.parse(&snapshot).unwrap();
    assert_eq!(outline(&tree), normalized_golden_outline(EXPECTED));
    assert!(Arc::ptr_eq(&tree.source, &snapshot.text));
}

fn normalized_golden_outline(source: &str) -> Cow<'_, str> {
    if source.contains("\r\n") {
        Cow::Owned(source.replace("\r\n", "\n"))
    } else {
        Cow::Borrowed(source)
    }
}

fn outline(document: &OwnedDocumentTree) -> String {
    let mut output = String::new();
    for block in &document.blocks {
        block_outline(block, &mut output);
        output.push('\n');
    }
    output
}

fn block_outline(block: &BlockNode, output: &mut String) {
    match block {
        BlockNode::Paragraph { content, .. } => inline_container("paragraph", content, output),
        BlockNode::Heading { level, content, .. } => {
            inline_container(&format!("heading:{level}"), content, output);
        }
        BlockNode::BlockQuote { blocks, .. } => block_container("quote", blocks, output),
        BlockNode::List(list) => {
            output.push_str(if list.ordered {
                "list:ordered["
            } else {
                "list:unordered["
            });
            for (index, item) in list.items.iter().enumerate() {
                if index != 0 {
                    output.push(',');
                }
                output.push_str(match item.checked {
                    Some(true) => "checked[",
                    Some(false) => "unchecked[",
                    None => "item[",
                });
                for (block_index, block) in item.blocks.iter().enumerate() {
                    if block_index != 0 {
                        output.push(',');
                    }
                    block_outline(block, output);
                }
                output.push(']');
            }
            output.push(']');
        }
        BlockNode::CodeBlock(code) => {
            output.push_str("code:");
            output.push_str(code.info.trim());
        }
        BlockNode::Table(table) => {
            output.push_str(&format!(
                "table:{}x{}",
                table.rows.len(),
                table.rows.first().map_or(0, |row| row.cells.len())
            ));
        }
        BlockNode::ThematicBreak { .. } => output.push_str("rule"),
        BlockNode::HtmlLiteral { literal, .. } => {
            assert_eq!(literal, "<script>alert(1)</script>\n");
            output.push_str("html:block");
        }
        BlockNode::DisplayMath(math) => {
            assert!(math.source_literal.starts_with("\\["));
            output.push_str("math:display");
        }
    }
}

fn block_container(label: &str, blocks: &[BlockNode], output: &mut String) {
    output.push_str(label);
    output.push('[');
    for (index, block) in blocks.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        block_outline(block, output);
    }
    output.push(']');
}

fn inline_container(label: &str, inlines: &[InlineNode], output: &mut String) {
    output.push_str(label);
    output.push('[');
    inline_outline(inlines, output);
    output.push(']');
}

fn inline_outline(inlines: &[InlineNode], output: &mut String) {
    for (index, inline) in inlines.iter().enumerate() {
        if index != 0 {
            output.push(',');
        }
        match inline {
            InlineNode::Text { .. } => output.push_str("text"),
            InlineNode::Emphasis { children, .. } => inline_container("emphasis", children, output),
            InlineNode::Strong { children, .. } => inline_container("strong", children, output),
            InlineNode::Strikethrough { children, .. } => {
                inline_container("strike", children, output)
            }
            InlineNode::Code { .. } => output.push_str("code"),
            InlineNode::Link { kind, children, .. } => {
                output.push_str(match kind {
                    LinkKind::Https => "link:https[",
                    LinkKind::Http => "link:http[",
                    LinkKind::Mailto => "link:mailto[",
                    LinkKind::File => "link:file[",
                    LinkKind::Relative => "link:relative[",
                    LinkKind::Blocked => "link:blocked[",
                });
                inline_outline(children, output);
                output.push(']');
            }
            InlineNode::Image { kind, .. } => output.push_str(match kind {
                ImageKind::LocalRelative => "image:local-relative",
                ImageKind::LocalAbsolute => "image:local-absolute",
                ImageKind::Remote => "image:remote",
                ImageKind::Unsupported => "image:unsupported",
            }),
            InlineNode::InlineMath(_) => output.push_str("math:inline"),
            InlineNode::SoftBreak { .. } => output.push_str("softbreak"),
            InlineNode::HardBreak { .. } => output.push_str("hardbreak"),
            InlineNode::HtmlLiteral { .. } => output.push_str("html:inline"),
        }
    }
}

#[test]
fn golden_outline_is_independent_of_checkout_newlines() {
    assert_eq!(
        normalized_golden_outline("heading[text]\r\nparagraph[text]\r\n"),
        "heading[text]\nparagraph[text]\n"
    );
    assert_eq!(
        normalized_golden_outline("literal\rtext\n"),
        "literal\rtext\n"
    );
}

#[test]
fn phase5_list_table_keeps_cells_and_selection_text() {
    use stickymd_render::preview::{
        PreviewPipeline, PreviewSelection, PreviewTheme, RenderBlockKind, RenderTreeBuilder,
    };
    let source = "- | a | b |\n  | --- | --- |\n  | x | y |\n";
    let snapshot = DocumentSnapshot {
        text: Arc::from(source),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let owned = PreviewParser.parse(&snapshot).unwrap();
    let tree = RenderTreeBuilder.build(&owned);
    assert!(matches!(&tree.blocks[0].kind, RenderBlockKind::Table(table) if table.rows.len() == 2));
    let mut pipeline = PreviewPipeline::new();
    for (width, scale, theme) in [
        (140, 0.5, PreviewTheme::Light),
        (500, 1.0, PreviewTheme::Light),
        (300, 3.0, PreviewTheme::Dark),
    ] {
        let frame = pipeline
            .build(
                &snapshot,
                width,
                300,
                scale,
                0.0,
                PreviewSelection::default(),
                theme,
            )
            .unwrap();
        assert_eq!(
            frame.copy_selection(frame.select_all()),
            Some("• a\tb\nx\ty")
        );
    }
    assert_eq!(snapshot.text.as_ref(), source);
}

#[test]
fn phase5_list_first_blocks_keep_semantics_and_nested_markers() {
    use stickymd_render::preview::{
        PreviewPipeline, PreviewSelection, PreviewTheme, RenderBlockKind, RenderTreeBuilder,
    };
    let cases = [
        ("- # 标题\n", "heading", "• 标题"),
        ("- ```rust\n  let x = 1;\n  ```\n", "code", "• let x = 1;\n"),
        ("- > quote\n", "quote", "• quote"),
        ("- $$x$$\n", "math", "• $$x$$"),
        ("- - inner\n", "list", "• • inner"),
        ("- [x] **完成**\n", "list", "☑ 完成"),
        ("10. # 标题\n", "heading", "10. 标题"),
        ("-\n", "list", "• "),
    ];
    let mut pipeline = PreviewPipeline::new();
    for (source, kind, expected) in cases {
        let snapshot = DocumentSnapshot {
            text: Arc::from(source),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let tree = RenderTreeBuilder.build(&PreviewParser.parse(&snapshot).unwrap());
        assert!(
            matches!(
                (kind, &tree.blocks[0].kind),
                ("heading", RenderBlockKind::Heading(_))
                    | ("code", RenderBlockKind::CodeBlock { .. })
                    | ("quote", RenderBlockKind::Quote)
                    | ("math", RenderBlockKind::DisplayMath)
                    | ("list", RenderBlockKind::ListItem)
            ),
            "{source}: {:?}",
            tree.blocks[0].kind
        );
        let frame = pipeline
            .build(
                &snapshot,
                500,
                300,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        assert_eq!(
            frame.copy_selection(frame.select_all()),
            Some(expected),
            "{source}"
        );
        assert_eq!(snapshot.text.as_ref(), source);
    }
}

#[test]
fn phase5_list_markers_and_content_keep_distinct_hit_targets() {
    use stickymd_render::preview::{PreviewPipeline, PreviewSelection, PreviewTheme};
    let mut pipeline = PreviewPipeline::new();
    for source in [
        "- # 标题 Heading\n",
        "- ```rust\n  let x = 1;\n  ```\n",
        "- | a | b |\n  | --- | --- |\n  | x | y |\n",
        "- > quote\n",
        "- $$x$$\n",
        "- - inner\n",
        "- [x] **完成**\n",
        "- ![ALT](missing.png)\n",
    ] {
        let snapshot = DocumentSnapshot {
            text: Arc::from(source),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let frame = pipeline
            .build(
                &snapshot,
                500,
                300,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        for item in frame.index().boxes() {
            let y = item.rect.y + item.rect.height * 0.5;
            assert_eq!(
                frame.hit_test(item.start_x, y),
                item.selection_range.start,
                "{source}: {item:?}"
            );
            assert_eq!(
                frame.hit_test(item.end_x, y),
                item.selection_range.end,
                "{source}: {item:?}"
            );
        }
    }
}
