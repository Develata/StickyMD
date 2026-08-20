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
    assert_eq!(outline(&tree), EXPECTED);
    assert!(Arc::ptr_eq(&tree.source, &snapshot.text));
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
