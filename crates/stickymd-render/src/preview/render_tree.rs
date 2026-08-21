//! Semantic owned-tree to renderer-owned block projection.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use std::sync::Arc;

use stickymd_core::Generation;

use crate::math::MathKind;

use super::{
    BlockNode, ImageKind, InlineNode, LinkKind, ListNode, OwnedDocumentTree, SourceRange,
    TableAlignment, TableNode,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderStyle {
    pub strong: bool,
    pub emphasis: bool,
    pub strikethrough: bool,
    pub code: bool,
    pub link: bool,
    pub html_literal: bool,
    pub math_placeholder: bool,
    pub image_placeholder: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpanAction {
    OpenLink { destination: String, kind: LinkKind },
    RemoteImageLink { destination: String, kind: LinkKind },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderSpan {
    pub text: String,
    /// Text contributed to preview selection and clipboard projection.
    ///
    /// It intentionally differs from `text` for image placeholders: the
    /// preview paints a discoverable placeholder but copying it yields alt
    /// text, as required by the product contract.
    pub copy_text: String,
    pub source_range: Option<SourceRange>,
    pub style: RenderStyle,
    pub action: Option<SpanAction>,
    pub(crate) math: Option<RenderMath>,
    pub(crate) image: Option<RenderImage>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderMath {
    pub source: String,
    pub kind: MathKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RenderImage {
    pub destination: String,
    pub kind: ImageKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderBlockKind {
    Paragraph,
    Heading(u8),
    Quote,
    ListItem,
    CodeBlock { info: String },
    Table(RenderTable),
    ThematicBreak,
    HtmlLiteral,
    DisplayMath,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderBlock {
    pub kind: RenderBlockKind,
    pub spans: Vec<RenderSpan>,
    pub indent: u16,
    pub source_range: Option<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTable {
    pub alignments: Vec<TableAlignment>,
    pub rows: Vec<RenderTableRow>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTableRow {
    pub header: bool,
    pub cells: Vec<Vec<RenderSpan>>,
    pub source_range: Option<SourceRange>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderTree {
    pub generation: Generation,
    pub source: Arc<str>,
    pub blocks: Vec<RenderBlock>,
}

#[derive(Debug, Default, Clone, Copy)]
pub struct RenderTreeBuilder;

impl RenderTreeBuilder {
    pub fn build(&self, document: &OwnedDocumentTree) -> RenderTree {
        let mut blocks = Vec::new();
        self.append_blocks(&document.blocks, 0, false, &mut blocks);
        RenderTree {
            generation: document.generation,
            source: Arc::clone(&document.source),
            blocks,
        }
    }

    fn append_blocks(
        &self,
        input: &[BlockNode],
        indent: u16,
        quoted: bool,
        output: &mut Vec<RenderBlock>,
    ) {
        for block in input {
            match block {
                BlockNode::Paragraph {
                    content,
                    source_range,
                } => output.push(RenderBlock {
                    kind: if quoted {
                        RenderBlockKind::Quote
                    } else {
                        RenderBlockKind::Paragraph
                    },
                    spans: self.inline_spans(content),
                    indent,
                    source_range: *source_range,
                }),
                BlockNode::Heading {
                    level,
                    content,
                    source_range,
                } => output.push(RenderBlock {
                    kind: RenderBlockKind::Heading(*level),
                    spans: self.inline_spans(content),
                    indent,
                    source_range: *source_range,
                }),
                BlockNode::BlockQuote {
                    blocks,
                    source_range,
                } => {
                    let before = output.len();
                    self.append_blocks(blocks, indent.saturating_add(1), true, output);
                    if output.len() == before {
                        output.push(RenderBlock {
                            kind: RenderBlockKind::Quote,
                            spans: Vec::new(),
                            indent,
                            source_range: *source_range,
                        });
                    }
                }
                BlockNode::List(list) => self.append_list(list, indent, quoted, output),
                BlockNode::CodeBlock(code) => output.push(RenderBlock {
                    kind: RenderBlockKind::CodeBlock {
                        info: code.info.clone(),
                    },
                    spans: vec![RenderSpan {
                        text: code.literal.clone(),
                        copy_text: code.literal.clone(),
                        source_range: code.source_range,
                        style: RenderStyle {
                            code: true,
                            ..RenderStyle::default()
                        },
                        action: None,
                        math: None,
                        image: None,
                    }],
                    indent,
                    source_range: code.source_range,
                }),
                BlockNode::Table(table) => output.push(RenderBlock {
                    kind: RenderBlockKind::Table(self.table(table)),
                    spans: Vec::new(),
                    indent,
                    source_range: table.source_range,
                }),
                BlockNode::ThematicBreak { source_range } => output.push(RenderBlock {
                    kind: RenderBlockKind::ThematicBreak,
                    spans: Vec::new(),
                    indent,
                    source_range: *source_range,
                }),
                BlockNode::HtmlLiteral {
                    literal,
                    source_range,
                } => output.push(RenderBlock {
                    kind: RenderBlockKind::HtmlLiteral,
                    spans: vec![RenderSpan {
                        text: literal.clone(),
                        copy_text: literal.clone(),
                        source_range: *source_range,
                        style: RenderStyle {
                            code: true,
                            html_literal: true,
                            ..RenderStyle::default()
                        },
                        action: None,
                        math: None,
                        image: None,
                    }],
                    indent,
                    source_range: *source_range,
                }),
                BlockNode::DisplayMath(math) => output.push(RenderBlock {
                    kind: RenderBlockKind::DisplayMath,
                    spans: vec![RenderSpan {
                        text: math.source_literal.clone(),
                        copy_text: math.source_literal.clone(),
                        source_range: math.source_range,
                        style: RenderStyle {
                            math_placeholder: true,
                            ..RenderStyle::default()
                        },
                        action: None,
                        math: Some(RenderMath {
                            source: math.literal.clone(),
                            kind: MathKind::Display,
                        }),
                        image: None,
                    }],
                    indent,
                    source_range: math.source_range,
                }),
            }
        }
    }

    fn append_list(
        &self,
        list: &ListNode,
        indent: u16,
        quoted: bool,
        output: &mut Vec<RenderBlock>,
    ) {
        for (index, item) in list.items.iter().enumerate() {
            let marker = match item.checked {
                Some(true) => "☑ ".to_owned(),
                Some(false) => "☐ ".to_owned(),
                None if list.ordered => format!("{}. ", list.start + index),
                None => "• ".to_owned(),
            };
            let before = output.len();
            self.append_blocks(&item.blocks, indent.saturating_add(1), quoted, output);
            if let Some(first) = output.get_mut(before) {
                first.kind = RenderBlockKind::ListItem;
                first.spans.insert(
                    0,
                    RenderSpan {
                        copy_text: marker.clone(),
                        text: marker,
                        source_range: None,
                        style: RenderStyle::default(),
                        action: None,
                        math: None,
                        image: None,
                    },
                );
            } else {
                output.push(RenderBlock {
                    kind: RenderBlockKind::ListItem,
                    spans: vec![RenderSpan {
                        copy_text: marker.clone(),
                        text: marker,
                        source_range: None,
                        style: RenderStyle::default(),
                        action: None,
                        math: None,
                        image: None,
                    }],
                    indent: indent.saturating_add(1),
                    source_range: item.source_range,
                });
            }
        }
    }

    fn table(&self, table: &TableNode) -> RenderTable {
        RenderTable {
            alignments: table.alignments.clone(),
            rows: table
                .rows
                .iter()
                .map(|row| RenderTableRow {
                    header: row.header,
                    cells: row
                        .cells
                        .iter()
                        .map(|cell| self.inline_spans(&cell.content))
                        .collect(),
                    source_range: row.source_range,
                })
                .collect(),
        }
    }

    fn inline_spans(&self, input: &[InlineNode]) -> Vec<RenderSpan> {
        let mut output = Vec::new();
        append_inline_spans(input, RenderStyle::default(), &mut output);
        output
    }
}

fn append_inline_spans(input: &[InlineNode], inherited: RenderStyle, output: &mut Vec<RenderSpan>) {
    for inline in input {
        match inline {
            InlineNode::Text {
                literal,
                source_range,
            } => output.push(span(literal, literal, *source_range, inherited, None)),
            InlineNode::Emphasis { children, .. } => append_inline_spans(
                children,
                RenderStyle {
                    emphasis: true,
                    ..inherited
                },
                output,
            ),
            InlineNode::Strong { children, .. } => append_inline_spans(
                children,
                RenderStyle {
                    strong: true,
                    ..inherited
                },
                output,
            ),
            InlineNode::Strikethrough { children, .. } => append_inline_spans(
                children,
                RenderStyle {
                    strikethrough: true,
                    ..inherited
                },
                output,
            ),
            InlineNode::Code {
                literal,
                source_range,
            } => output.push(span(
                literal,
                literal,
                *source_range,
                RenderStyle {
                    code: true,
                    ..inherited
                },
                None,
            )),
            InlineNode::Link {
                destination,
                kind,
                children,
                ..
            } => {
                let start = output.len();
                append_inline_spans(
                    children,
                    RenderStyle {
                        link: true,
                        ..inherited
                    },
                    output,
                );
                for span in &mut output[start..] {
                    span.action = Some(SpanAction::OpenLink {
                        destination: destination.clone(),
                        kind: *kind,
                    });
                }
            }
            InlineNode::Image {
                destination,
                alt,
                kind,
                source_range,
                ..
            } => {
                let label = if alt.is_empty() { "image" } else { alt };
                let mut image_span = span(
                    &format!("[image: {label}] {destination}"),
                    alt,
                    *source_range,
                    RenderStyle {
                        link: matches!(kind, ImageKind::Remote),
                        image_placeholder: true,
                        ..inherited
                    },
                    matches!(kind, ImageKind::Remote).then(|| SpanAction::RemoteImageLink {
                        destination: destination.clone(),
                        kind: if destination
                            .get(..5)
                            .is_some_and(|prefix| prefix.eq_ignore_ascii_case("http:"))
                        {
                            LinkKind::Http
                        } else {
                            LinkKind::Https
                        },
                    }),
                );
                image_span.image = Some(RenderImage {
                    destination: destination.clone(),
                    kind: *kind,
                });
                output.push(image_span);
            }
            InlineNode::InlineMath(math) => output.push(RenderSpan {
                text: math.source_literal.clone(),
                copy_text: math.source_literal.clone(),
                source_range: math.source_range,
                style: RenderStyle {
                    math_placeholder: true,
                    ..inherited
                },
                action: None,
                math: Some(RenderMath {
                    source: math.literal.clone(),
                    kind: MathKind::Inline,
                }),
                image: None,
            }),
            InlineNode::SoftBreak { source_range } => {
                output.push(span(" ", " ", *source_range, inherited, None));
            }
            InlineNode::HardBreak { source_range } => {
                output.push(span("\n", "\n", *source_range, inherited, None));
            }
            InlineNode::HtmlLiteral {
                literal,
                source_range,
            } => output.push(span(
                literal,
                literal,
                *source_range,
                RenderStyle {
                    code: true,
                    html_literal: true,
                    ..inherited
                },
                None,
            )),
        }
    }
}

fn span(
    text: &str,
    copy_text: &str,
    source_range: Option<SourceRange>,
    style: RenderStyle,
    action: Option<SpanAction>,
) -> RenderSpan {
    RenderSpan {
        text: text.to_owned(),
        copy_text: copy_text.to_owned(),
        source_range,
        style,
        action,
        math: None,
        image: None,
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use stickymd_core::{DocumentSnapshot, Generation, LineEnding};

    use super::*;
    use crate::preview::PreviewParser;

    fn build(source: &str) -> RenderTree {
        let snapshot = DocumentSnapshot {
            text: Arc::from(source),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        RenderTreeBuilder.build(&PreviewParser.parse(&snapshot).unwrap())
    }

    #[test]
    fn separates_semantics_from_flat_render_blocks() {
        let tree = build("# H\n\n> **quote**\n\n- [x] task\n\n`code` $x$\n\n---");
        assert!(matches!(tree.blocks[0].kind, RenderBlockKind::Heading(1)));
        assert!(
            tree.blocks
                .iter()
                .any(|block| matches!(block.kind, RenderBlockKind::Quote))
        );
        assert!(
            tree.blocks
                .iter()
                .any(|block| matches!(block.kind, RenderBlockKind::ListItem))
        );
        assert!(
            tree.blocks
                .iter()
                .any(|block| matches!(block.kind, RenderBlockKind::ThematicBreak))
        );
        let debug = format!("{tree:#?}");
        assert!(debug.contains("strong: true"));
        assert!(debug.contains("code: true"));
        assert!(debug.contains("math_placeholder: true"));
    }

    #[test]
    fn blocked_links_remain_visible_but_are_not_openable() {
        let tree = build("[bad](javascript:alert(1)) [good](https://example.com)");
        let actions = tree.blocks[0]
            .spans
            .iter()
            .filter_map(|span| span.action.as_ref())
            .collect::<Vec<_>>();
        assert!(matches!(
            actions[0],
            SpanAction::OpenLink {
                kind: LinkKind::Blocked,
                ..
            }
        ));
        assert!(matches!(
            actions[1],
            SpanAction::OpenLink {
                kind: LinkKind::Https,
                ..
            }
        ));
    }

    #[test]
    fn tables_keep_rows_cells_and_alignment() {
        let tree = build("| a | b |\n| :- | -: |\n| 1 | 2 |");
        let RenderBlockKind::Table(table) = &tree.blocks[0].kind else {
            panic!("expected table");
        };
        assert_eq!(
            table.alignments,
            [TableAlignment::Left, TableAlignment::Right]
        );
        assert_eq!(table.rows.len(), 2);
        assert!(table.rows[0].header);
        assert_eq!(table.rows[1].cells.len(), 2);
    }

    #[test]
    fn image_placeholder_shows_path_but_copies_only_alt_text() {
        let tree = build("![diagram](images/diagram.png)");
        let span = &tree.blocks[0].spans[0];
        assert_eq!(span.text, "[image: diagram] images/diagram.png");
        assert_eq!(span.copy_text, "diagram");
        assert!(span.style.image_placeholder);
    }
}
