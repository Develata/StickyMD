//! Frozen Comrak dialect and transient-Arena to owned-tree conversion.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#markdown-semantics

use std::sync::Arc;
use std::time::{Duration, Instant};

use comrak::nodes::{AstNode, ListType, NodeMath, NodeValue, TableAlignment as ComrakAlignment};
use comrak::{Arena, Options, parse_document};
use stickymd_core::DocumentSnapshot;
use thiserror::Error;

use super::{
    BlockNode, CodeBlockNode, ImageKind, InlineNode, LinkKind, ListItem, ListNode, MathNode,
    OwnedDocumentTree, SourceMap, SourceRange, TableAlignment, TableCell, TableNode, TableRow,
};

pub const MAX_PREVIEW_SOURCE_BYTES: usize = 5 * 1024 * 1024;
pub const MAX_PARSE_DEPTH: usize = 256;
pub const MAX_OWNED_NODES: usize = 200_000;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PreviewParseError {
    #[error("preview source is {actual} bytes; limit is {limit} bytes")]
    SourceTooLarge { actual: usize, limit: usize },
    #[error("Markdown nesting exceeds the preview depth limit of {limit}")]
    DepthLimit { limit: usize },
    #[error("Markdown AST exceeds the preview node limit of {limit}")]
    NodeLimit { limit: usize },
}

/// Exact Phase 5 Markdown dialect. Comrak remains the only semantic parser.
fn markdown_options() -> Options<'static> {
    let mut options = Options::default();
    options.extension.strikethrough = true;
    options.extension.table = true;
    options.extension.autolink = true;
    options.extension.tasklist = true;
    options.extension.math_dollars = true;
    options.extension.math_latex = true;
    options
}

/// Stateless parser facade. Its Arena is created and dropped within `parse`.
#[derive(Debug, Default, Clone, Copy)]
pub struct PreviewParser;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreviewParseMetrics {
    pub comrak_parse: Duration,
    pub owned_conversion: Duration,
}

impl PreviewParser {
    pub fn parse(
        &self,
        snapshot: &DocumentSnapshot,
    ) -> Result<OwnedDocumentTree, PreviewParseError> {
        self.parse_with_metrics(snapshot).map(|(tree, _)| tree)
    }

    /// Parses through a transient Comrak Arena and reports the two explicit
    /// performance stages without changing ownership semantics.
    pub fn parse_with_metrics(
        &self,
        snapshot: &DocumentSnapshot,
    ) -> Result<(OwnedDocumentTree, PreviewParseMetrics), PreviewParseError> {
        if snapshot.text.len() > MAX_PREVIEW_SOURCE_BYTES {
            return Err(PreviewParseError::SourceTooLarge {
                actual: snapshot.text.len(),
                limit: MAX_PREVIEW_SOURCE_BYTES,
            });
        }

        let arena = Arena::new();
        let parse_started = Instant::now();
        let root = parse_document(&arena, &snapshot.text, &markdown_options());
        let comrak_parse = parse_started.elapsed();
        let conversion_started = Instant::now();
        let (blocks, node_count) = {
            let source_map = SourceMap::new(&snapshot.text);
            let mut context = ConvertContext {
                source: &snapshot.text,
                source_map: &source_map,
                node_count: 0,
            };
            let blocks = context.convert_blocks(root, 0)?;
            (blocks, context.node_count)
        };
        let owned_conversion = conversion_started.elapsed();

        Ok((
            OwnedDocumentTree {
                generation: snapshot.generation,
                source: Arc::clone(&snapshot.text),
                blocks,
                node_count,
            },
            PreviewParseMetrics {
                comrak_parse,
                owned_conversion,
            },
        ))
    }
}

struct ConvertContext<'a> {
    source: &'a str,
    source_map: &'a SourceMap,
    node_count: usize,
}

impl ConvertContext<'_> {
    fn touch(&mut self, depth: usize) -> Result<(), PreviewParseError> {
        if depth > MAX_PARSE_DEPTH {
            return Err(PreviewParseError::DepthLimit {
                limit: MAX_PARSE_DEPTH,
            });
        }
        self.node_count = self
            .node_count
            .checked_add(1)
            .ok_or(PreviewParseError::NodeLimit {
                limit: MAX_OWNED_NODES,
            })?;
        if self.node_count > MAX_OWNED_NODES {
            return Err(PreviewParseError::NodeLimit {
                limit: MAX_OWNED_NODES,
            });
        }
        Ok(())
    }

    fn source_range<'arena>(&self, node: &'arena AstNode<'arena>) -> Option<SourceRange> {
        self.source_map
            .range(self.source, node.data.borrow().sourcepos)
    }

    fn convert_blocks<'arena>(
        &mut self,
        parent: &'arena AstNode<'arena>,
        depth: usize,
    ) -> Result<Vec<BlockNode>, PreviewParseError> {
        let mut blocks = Vec::new();
        for child in parent.children() {
            self.append_block(child, depth + 1, &mut blocks)?;
        }
        Ok(blocks)
    }

    fn append_block<'arena>(
        &mut self,
        node: &'arena AstNode<'arena>,
        depth: usize,
        output: &mut Vec<BlockNode>,
    ) -> Result<(), PreviewParseError> {
        self.touch(depth)?;
        let value = node.data.borrow().value.clone();
        let source_range = self.source_range(node);
        match value {
            NodeValue::Paragraph => {
                let children = node.children().collect::<Vec<_>>();
                if let [only] = children.as_slice()
                    && let NodeValue::Math(math) = only.data.borrow().value.clone()
                    && math.display_math
                {
                    self.touch(depth + 1)?;
                    output.push(BlockNode::DisplayMath(self.math_node(only, math)));
                } else {
                    output.push(BlockNode::Paragraph {
                        content: self.convert_inlines(node, depth)?,
                        source_range,
                    });
                }
            }
            NodeValue::Heading(heading) => output.push(BlockNode::Heading {
                level: heading.level.clamp(1, 6),
                content: self.convert_inlines(node, depth)?,
                source_range,
            }),
            NodeValue::BlockQuote => output.push(BlockNode::BlockQuote {
                blocks: self.convert_blocks(node, depth)?,
                source_range,
            }),
            NodeValue::List(list) => {
                let mut items = Vec::new();
                for item in node.children() {
                    self.touch(depth + 1)?;
                    let checked = match &item.data.borrow().value {
                        NodeValue::TaskItem(task) => Some(task.symbol.is_some()),
                        NodeValue::Item(_) => None,
                        _ => None,
                    };
                    items.push(ListItem {
                        checked,
                        blocks: self.convert_blocks(item, depth + 1)?,
                        source_range: self.source_range(item),
                    });
                }
                output.push(BlockNode::List(ListNode {
                    ordered: list.list_type == ListType::Ordered,
                    start: list.start.max(1),
                    tight: list.tight,
                    items,
                    source_range,
                }));
            }
            NodeValue::CodeBlock(code) => output.push(BlockNode::CodeBlock(CodeBlockNode {
                literal: code.literal,
                info: code.info,
                fenced: code.fenced,
                source_range,
            })),
            NodeValue::HtmlBlock(html) => output.push(BlockNode::HtmlLiteral {
                literal: html.literal,
                source_range,
            }),
            NodeValue::ThematicBreak => output.push(BlockNode::ThematicBreak { source_range }),
            NodeValue::Table(table) => {
                let alignments = table
                    .alignments
                    .iter()
                    .copied()
                    .map(table_alignment)
                    .collect();
                let mut rows = Vec::new();
                for row in node.children() {
                    self.touch(depth + 1)?;
                    let header = matches!(row.data.borrow().value, NodeValue::TableRow(true));
                    let mut cells = Vec::new();
                    for cell in row.children() {
                        self.touch(depth + 2)?;
                        cells.push(TableCell {
                            content: self.convert_inlines(cell, depth + 2)?,
                            source_range: self.source_range(cell),
                        });
                    }
                    rows.push(TableRow {
                        header,
                        cells,
                        source_range: self.source_range(row),
                    });
                }
                output.push(BlockNode::Table(TableNode {
                    alignments,
                    rows,
                    source_range,
                }));
            }
            NodeValue::Document => output.extend(self.convert_blocks(node, depth)?),
            // Disabled extensions and unexpected containers are projected via
            // their standard block children. No alternate Markdown semantics
            // are invented here.
            other if other.block() => output.extend(self.convert_blocks(node, depth)?),
            _ => {
                let content = self.convert_inlines_from_node(node, depth)?;
                if !content.is_empty() {
                    output.push(BlockNode::Paragraph {
                        content,
                        source_range,
                    });
                }
            }
        }
        Ok(())
    }

    fn convert_inlines<'arena>(
        &mut self,
        parent: &'arena AstNode<'arena>,
        depth: usize,
    ) -> Result<Vec<InlineNode>, PreviewParseError> {
        let mut output = Vec::new();
        for child in parent.children() {
            output.extend(self.convert_inline(child, depth + 1)?);
        }
        Ok(output)
    }

    fn convert_inlines_from_node<'arena>(
        &mut self,
        node: &'arena AstNode<'arena>,
        depth: usize,
    ) -> Result<Vec<InlineNode>, PreviewParseError> {
        if node.children().next().is_some() {
            self.convert_inlines(node, depth)
        } else {
            self.convert_inline_value(node, node.data.borrow().value.clone(), depth)
        }
    }

    fn convert_inline<'arena>(
        &mut self,
        node: &'arena AstNode<'arena>,
        depth: usize,
    ) -> Result<Vec<InlineNode>, PreviewParseError> {
        self.touch(depth)?;
        let value = node.data.borrow().value.clone();
        self.convert_inline_value(node, value, depth)
    }

    fn convert_inline_value<'arena>(
        &mut self,
        node: &'arena AstNode<'arena>,
        value: NodeValue,
        depth: usize,
    ) -> Result<Vec<InlineNode>, PreviewParseError> {
        let source_range = self.source_range(node);
        let inline = match value {
            NodeValue::Text(text) => InlineNode::Text {
                literal: text.into_owned(),
                source_range,
            },
            NodeValue::Emph => InlineNode::Emphasis {
                children: self.convert_inlines(node, depth)?,
                source_range,
            },
            NodeValue::Strong => InlineNode::Strong {
                children: self.convert_inlines(node, depth)?,
                source_range,
            },
            NodeValue::Strikethrough => InlineNode::Strikethrough {
                children: self.convert_inlines(node, depth)?,
                source_range,
            },
            NodeValue::Code(code) => InlineNode::Code {
                literal: code.literal,
                source_range,
            },
            NodeValue::Link(link) => InlineNode::Link {
                kind: classify_link(&link.url),
                destination: link.url,
                title: link.title,
                children: self.convert_inlines(node, depth)?,
                source_range,
            },
            NodeValue::Image(image) => InlineNode::Image {
                kind: classify_image(&image.url),
                destination: image.url,
                title: image.title,
                alt: self.plain_inline_text(node, depth)?,
                source_range,
            },
            NodeValue::Math(math) => {
                let math = self.math_node(node, math);
                if math.display {
                    // A display-math node mixed with other inline content is
                    // retained as an inline placeholder instead of changing
                    // Comrak's tree structure.
                    InlineNode::InlineMath(math)
                } else {
                    InlineNode::InlineMath(math)
                }
            }
            NodeValue::SoftBreak => InlineNode::SoftBreak { source_range },
            NodeValue::LineBreak => InlineNode::HardBreak { source_range },
            NodeValue::HtmlInline(literal) | NodeValue::Raw(literal) => InlineNode::HtmlLiteral {
                literal,
                source_range,
            },
            other => {
                if node.children().next().is_some() {
                    return self.convert_inlines(node, depth);
                }
                if let Some(text) = other.text() {
                    InlineNode::Text {
                        literal: text.to_owned(),
                        source_range,
                    }
                } else {
                    return Ok(Vec::new());
                }
            }
        };
        Ok(vec![inline])
    }

    fn math_node<'arena>(&self, node: &'arena AstNode<'arena>, math: NodeMath) -> MathNode {
        let source_range = self.source_range(node);
        let source_literal = source_range
            .and_then(|range| range.text(self.source))
            .unwrap_or(&math.literal)
            .to_owned();
        MathNode {
            literal: math.literal,
            source_literal,
            display: math.display_math,
            source_range,
        }
    }

    fn plain_inline_text<'arena>(
        &mut self,
        parent: &'arena AstNode<'arena>,
        depth: usize,
    ) -> Result<String, PreviewParseError> {
        let mut text = String::new();
        let mut stack = parent
            .children()
            .map(|child| (child, depth + 1))
            .collect::<Vec<_>>();
        stack.reverse();
        while let Some((node, node_depth)) = stack.pop() {
            self.touch(node_depth)?;
            let value = node.data.borrow().value.clone();
            match value {
                NodeValue::Text(value) => text.push_str(&value),
                NodeValue::Code(code) => text.push_str(&code.literal),
                NodeValue::SoftBreak | NodeValue::LineBreak => text.push(' '),
                _ => {
                    let children = node.children().collect::<Vec<_>>();
                    stack.extend(
                        children
                            .into_iter()
                            .rev()
                            .map(|child| (child, node_depth + 1)),
                    );
                }
            }
        }
        Ok(text)
    }
}

fn table_alignment(alignment: ComrakAlignment) -> TableAlignment {
    match alignment {
        ComrakAlignment::None => TableAlignment::None,
        ComrakAlignment::Left => TableAlignment::Left,
        ComrakAlignment::Center => TableAlignment::Center,
        ComrakAlignment::Right => TableAlignment::Right,
    }
}

fn classify_link(destination: &str) -> LinkKind {
    let trimmed = destination.trim();
    if trimmed.is_empty() || windows_absolute_path(trimmed) || scheme(trimmed).is_none() {
        return LinkKind::Relative;
    }
    match scheme(trimmed).unwrap_or_default() {
        "http" => LinkKind::Http,
        "https" => LinkKind::Https,
        "mailto" => LinkKind::Mailto,
        "file" => LinkKind::File,
        _ => LinkKind::Blocked,
    }
}

fn classify_image(destination: &str) -> ImageKind {
    let trimmed = destination.trim();
    if windows_absolute_path(trimmed) || trimmed.starts_with('/') || trimmed.starts_with('\\') {
        return ImageKind::LocalAbsolute;
    }
    match scheme(trimmed) {
        Some("http" | "https") => ImageKind::Remote,
        Some("file") => ImageKind::LocalAbsolute,
        Some(_) => ImageKind::Unsupported,
        None => ImageKind::LocalRelative,
    }
}

fn scheme(destination: &str) -> Option<&str> {
    let colon = destination.find(':')?;
    let candidate = &destination[..colon];
    (!candidate.is_empty()
        && candidate.as_bytes()[0].is_ascii_alphabetic()
        && candidate
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.')))
    .then(|| candidate.to_ascii_lowercase())
    .map(|lower| match lower.as_str() {
        "http" => "http",
        "https" => "https",
        "mailto" => "mailto",
        "file" => "file",
        _ => "blocked",
    })
}

fn windows_absolute_path(destination: &str) -> bool {
    let bytes = destination.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stickymd_core::{DocumentSnapshot, Generation, LineEnding};

    use super::*;

    fn snapshot(source: &str) -> DocumentSnapshot {
        DocumentSnapshot {
            text: Arc::from(source),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        }
    }

    #[test]
    fn dialect_enables_only_approved_extensions() {
        let options = markdown_options();
        assert!(options.extension.strikethrough);
        assert!(options.extension.table);
        assert!(options.extension.autolink);
        assert!(options.extension.tasklist);
        assert!(options.extension.math_dollars);
        assert!(options.extension.math_latex);
        assert!(!options.extension.footnotes);
        assert!(!options.extension.math_code);
        assert!(!options.extension.wikilinks_title_after_pipe);
        assert!(!options.extension.description_lists);
    }

    #[test]
    fn arena_is_not_exposed_and_owned_tree_keeps_snapshot_source() {
        let snapshot = snapshot("# 标题\n\nparagraph");
        let tree = PreviewParser.parse(&snapshot).expect("parse");
        assert_eq!(tree.generation, snapshot.generation);
        assert!(Arc::ptr_eq(&tree.source, &snapshot.text));
        assert_eq!(tree.blocks.len(), 2);
    }

    #[test]
    fn parses_gfm_html_links_images_and_lists() {
        let source = concat!(
            "# H\n\n**strong** *em* ~~strike~~ `code`\n\n",
            "- [x] done\n- [ ] todo\n\n",
            "> quote\n\n| a | b |\n| :- | -: |\n| 1 | 2 |\n\n",
            "[safe](https://example.com) [blocked](javascript:alert(1))\n\n",
            "![remote](https://example.com/a.png) ![local](images/a.png)\n\n",
            "<script>alert(1)</script>\n",
        );
        let tree = PreviewParser.parse(&snapshot(source)).expect("parse");
        assert!(
            tree.blocks
                .iter()
                .any(|node| matches!(node, BlockNode::Heading { .. }))
        );
        assert!(
            tree.blocks
                .iter()
                .any(|node| matches!(node, BlockNode::List(_)))
        );
        assert!(
            tree.blocks
                .iter()
                .any(|node| matches!(node, BlockNode::BlockQuote { .. }))
        );
        assert!(
            tree.blocks
                .iter()
                .any(|node| matches!(node, BlockNode::Table(_)))
        );
        assert!(
            tree.blocks
                .iter()
                .any(|node| matches!(node, BlockNode::HtmlLiteral { .. }))
        );
        let debug = format!("{tree:#?}");
        assert!(debug.contains("Https"));
        assert!(debug.contains("Blocked"));
        assert!(debug.contains("Remote"));
        assert!(debug.contains("LocalRelative"));
    }

    #[test]
    fn preserves_ordered_nested_lists_break_kinds_and_thematic_rules() {
        let source = concat!(
            "1. ordered\n",
            "   - nested\n\n",
            "soft\nbreak\n\n",
            "hard  \nbreak\n\n",
            "---\n",
        );
        let tree = PreviewParser.parse(&snapshot(source)).expect("parse");
        let BlockNode::List(list) = &tree.blocks[0] else {
            panic!("expected ordered list");
        };
        assert!(list.ordered);
        assert!(
            list.items[0]
                .blocks
                .iter()
                .any(|block| { matches!(block, BlockNode::List(nested) if !nested.ordered) })
        );
        assert!(matches!(
            &tree.blocks[1],
            BlockNode::Paragraph { content, .. }
                if content.iter().any(|inline| matches!(inline, InlineNode::SoftBreak { .. }))
        ));
        assert!(matches!(
            &tree.blocks[2],
            BlockNode::Paragraph { content, .. }
                if content.iter().any(|inline| matches!(inline, InlineNode::HardBreak { .. }))
        ));
        assert!(matches!(tree.blocks[3], BlockNode::ThematicBreak { .. }));
    }

    #[test]
    fn all_four_math_delimiters_come_from_comrak_and_code_is_literal() {
        let source = "$a$\n\n$$b$$\n\n\\(c\\)\n\n\\[d\\]\n\n`$not_math$`";
        let tree = PreviewParser.parse(&snapshot(source)).expect("parse");
        let debug = format!("{tree:#?}");
        assert_eq!(debug.matches("MathNode").count(), 4);
        assert!(debug.contains("$not_math$"));
    }

    #[test]
    fn source_ranges_roundtrip_unicode_literals() {
        let source = "中文 **Rust🙂** and `é`";
        let tree = PreviewParser.parse(&snapshot(source)).expect("parse");
        for block in &tree.blocks {
            if let Some(range) = block.source_range() {
                assert_eq!(range.text(source), Some(source));
            }
        }
        let BlockNode::Paragraph { content, .. } = &tree.blocks[0] else {
            panic!("expected paragraph");
        };
        assert!(content.iter().any(|inline| matches!(
            inline,
            InlineNode::Strong { children, .. }
                if children.iter().any(|child| matches!(
                    child,
                    InlineNode::Text { literal, .. } if literal == "Rust🙂"
                ))
        )));
        let code = content.iter().find_map(|inline| match inline {
            InlineNode::Code {
                literal,
                source_range,
            } => Some((literal, source_range)),
            _ => None,
        });
        let (literal, source_range) = code.expect("inline code");
        assert_eq!(literal, "é");
        assert_eq!(
            source_range.and_then(|range| range.text(source)),
            Some("`é`")
        );
    }

    #[test]
    fn classifies_allowed_and_blocked_destinations_without_executing_them() {
        assert_eq!(classify_link("HTTP://example.com"), LinkKind::Http);
        assert_eq!(classify_link("mailto:a@example.com"), LinkKind::Mailto);
        assert_eq!(classify_link("file:///C:/a.md"), LinkKind::File);
        assert_eq!(classify_link("images/a.png"), LinkKind::Relative);
        assert_eq!(classify_link("C:/notes/a.md"), LinkKind::Relative);
        assert_eq!(classify_link("javascript:alert(1)"), LinkKind::Blocked);
        assert_eq!(classify_link("data:text/plain,x"), LinkKind::Blocked);
        assert_eq!(classify_image("https://x/a.png"), ImageKind::Remote);
        assert_eq!(classify_image("C:/a.png"), ImageKind::LocalAbsolute);
        assert_eq!(classify_image("custom:a"), ImageKind::Unsupported);
    }

    #[test]
    fn resource_limits_fail_closed() {
        let oversized = "x".repeat(MAX_PREVIEW_SOURCE_BYTES + 1);
        assert!(matches!(
            PreviewParser.parse(&snapshot(&oversized)),
            Err(PreviewParseError::SourceTooLarge { .. })
        ));

        let deep = format!("{}x", "> ".repeat(MAX_PARSE_DEPTH + 10));
        assert!(matches!(
            PreviewParser.parse(&snapshot(&deep)),
            Err(PreviewParseError::DepthLimit { .. })
        ));

        let node_bomb = "- x\n".repeat(MAX_OWNED_NODES / 3 + 16);
        assert!(matches!(
            PreviewParser.parse(&snapshot(&node_bomb)),
            Err(PreviewParseError::NodeLimit { .. })
        ));
    }

    #[test]
    fn ten_thousand_deterministic_malformed_inputs_never_panic() {
        let alphabet = [
            '#', '*', '_', '~', '`', '$', '\\', '[', ']', '(', ')', '<', '>', '|', ':', '\n', '中',
            '🙂', '\0', 'a', ' ', '-', '&', ';',
        ];
        let mut state = 0x5eed_1234_89ab_cdef_u64;
        for case in 0..10_000 {
            let length = case % 97;
            let mut source = String::new();
            for _ in 0..length {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                source.push(alphabet[(state as usize) % alphabet.len()]);
            }
            let snapshot = snapshot(&source);
            let tree = PreviewParser
                .parse(&snapshot)
                .expect("bounded malformed input must remain representable");
            assert!(Arc::ptr_eq(&tree.source, &snapshot.text));
        }
    }

    #[test]
    fn adversarial_large_constructs_remain_bounded_and_semantic() {
        let long_code = format!("```text\n{}\n```", "x".repeat(10_000));
        let code = PreviewParser.parse(&snapshot(&long_code)).unwrap();
        assert!(matches!(code.blocks[0], BlockNode::CodeBlock(_)));

        let mut table = String::new();
        table.push('|');
        for column in 0..20 {
            table.push_str(&format!(" c{column} |"));
        }
        table.push('\n');
        table.push('|');
        for _ in 0..20 {
            table.push_str(" --- |");
        }
        table.push('\n');
        for row in 0..100 {
            table.push('|');
            for column in 0..20 {
                table.push_str(&format!(" {row}:{column} |"));
            }
            table.push('\n');
        }
        let table = PreviewParser.parse(&snapshot(&table)).unwrap();
        let BlockNode::Table(table) = &table.blocks[0] else {
            panic!("expected 100x20 table");
        };
        assert_eq!(table.rows.len(), 101);
        assert!(table.rows.iter().all(|row| row.cells.len() == 20));

        let math = "$x$ ".repeat(2_000);
        let math = PreviewParser.parse(&snapshot(&math)).unwrap();
        assert!(math.node_count >= 2_000);
    }
}
