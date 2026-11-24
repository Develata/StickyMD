//! The check that a join changes nothing the Preview shows outside its formulas: one walk
//! over the Preview's parse of the original and one over the result, compared node by node.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#semantic-math-delimiter-conversion

use std::ops::Range;

use crate::math_text::EqualsLineConversion;
use crate::preview::{
    BlockNode, ImageKind, InlineNode, LinkKind, OwnedDocumentTree, SourceRange, TableAlignment,
};

/// Whether the Preview reads the result as it reads the original everywhere but inside the
/// joined formulas, and reads each joined formula as one formula:
///
/// - each joined formula, mapped, is the exact range of one math node of the result;
/// - every other node, mapped, is the same in both parses (`Mark`): the same kind and
///   fields, at the same place among its containers, with the same start, and the same end
///   unless it is a container.
///
/// Only three things may differ: nodes wholly inside a joined formula; text, which
/// may straddle a formula's edge and holds nothing but its bytes; and the kind and extent of
/// a paragraph or heading that meets a joined formula, since undoing the Setext split that cut
/// the formula into a heading and a paragraph is the join's purpose (what such a block holds
/// is still compared). Any other block that meets a joined formula, a display formula among
/// them, is compared whole, so the join cannot turn it into formula text. A container's end is not compared because the join may move it: an item
/// whose formula the join completes ends with that formula.
///
/// Fifth and sixth review (2026-11-25): a join can keep a list item open over the next list,
/// which then renumbers, or over indented code, which step 2 would convert; it can re-pair
/// the backticks after the formula's closer, so code becomes math; and since Comrak's budget
/// for reference links is the document's length, a shorter document can leave a reference
/// outside every formula without its link.
///
/// O(nodes + joins) time: both walks visit nodes in document order, so the span cursor and
/// the forward position map each pass once. O(nodes) memory for the original's marks; the
/// result's are compared as they come and the walk stops at the first difference.
pub(super) fn joins_keep_surroundings(
    original: &OwnedDocumentTree,
    result: &OwnedDocumentTree,
    joins: &EqualsLineConversion,
) -> bool {
    let spans = joins.joined_formulas();
    let mut map = joins.forward_position_map();
    let mapped: Vec<Range<usize>> = spans
        .iter()
        .map(|span| map(span.start)..map(span.end))
        .collect();
    let mut before = Vec::new();
    let mut reading = Reading {
        spans: SpanCursor::new(spans),
        map: joins.forward_position_map(),
        emit: |mark| {
            before.push(mark);
            true
        },
        formulas: 0,
    };
    reading.blocks(&original.blocks);
    let mut compared = 0;
    let mut reading = Reading {
        spans: SpanCursor::new(&mapped),
        map: |at| at,
        emit: |mark| {
            let same = before.get(compared) == Some(&mark);
            compared += 1;
            same
        },
        formulas: 0,
    };
    let same = reading.blocks(&result.blocks);
    let formulas = reading.formulas;
    same && compared == before.len() && formulas == mapped.len()
}

/// One step of a walk over a Preview parse, as `joins_keep_surroundings` compares it: a node
/// starts, with what it is and its mapped start, or it ends.
#[derive(Debug, PartialEq, Eq)]
enum Mark<'a> {
    Open(Node<'a>, Option<usize>),
    /// The mapped end; None for a container, whose end the join may move.
    Close(Option<usize>),
}

/// A node's kind and every field the Preview's rendering or step 2 reads, apart from its
/// children and source range. Text has no entry: it holds only its bytes, which a join keeps
/// outside the joined formulas.
#[derive(Debug, PartialEq, Eq)]
enum Node<'a> {
    Quote,
    List {
        ordered: bool,
        start: usize,
        tight: bool,
    },
    Item {
        checked: Option<bool>,
    },
    Paragraph,
    Heading(u8),
    DisplayMath(&'a str),
    CodeBlock {
        literal: &'a str,
        info: &'a str,
        fenced: bool,
        closed: bool,
    },
    Table(&'a [TableAlignment]),
    Row {
        header: bool,
    },
    Cell,
    Rule,
    HtmlBlock(&'a str),
    Emphasis,
    Strong,
    Strikethrough,
    Code(&'a str),
    Link {
        destination: &'a str,
        title: &'a str,
        kind: LinkKind,
    },
    Image {
        destination: &'a str,
        title: &'a str,
        alt: &'a str,
        kind: ImageKind,
    },
    InlineMath {
        literal: &'a str,
        display: bool,
    },
    SoftBreak,
    HardBreak,
    InlineHtml(&'a str),
}

/// How a node takes part in the comparison when it meets a joined formula without lying
/// inside it.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    /// A quote, list or item: compared, but not its end.
    Container,
    /// A paragraph or a heading: its kind and extent are not compared, what it holds is.
    TextBlock,
    /// Any other leaf block, an inline, a table row or a cell: compared whole.
    Part,
}

/// Where a node's source range lies against the joined formulas.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Place {
    /// Wholly inside one formula.
    Inside,
    /// Partly inside one or more formulas.
    Meets,
    /// Outside every formula, or without a source range, so it cannot be placed.
    Clear,
}

/// The joined formulas of one parse (sorted, disjoint) and a cursor over them. A walk in
/// document order asks for non-decreasing starts, so the cursor passes each formula once; a
/// start below an earlier one is searched instead, so the answer never depends on the order.
struct SpanCursor<'s> {
    spans: &'s [Range<usize>],
    next: usize,
    last: usize,
}

impl<'s> SpanCursor<'s> {
    fn new(spans: &'s [Range<usize>]) -> Self {
        Self {
            spans,
            next: 0,
            last: 0,
        }
    }

    /// Where `range` lies, and the formula it lies in, if it lies wholly inside one.
    fn place(&mut self, range: Option<SourceRange>) -> (Place, Option<&'s Range<usize>>) {
        let Some(range) = range else {
            return (Place::Clear, None);
        };
        let at = if range.start < self.last {
            self.spans.partition_point(|span| span.end <= range.start)
        } else {
            self.last = range.start;
            // Invariant: every span before `next` ends at or before `last`.
            while self
                .spans
                .get(self.next)
                .is_some_and(|span| span.end <= range.start)
            {
                self.next += 1;
            }
            self.next
        };
        match self.spans.get(at) {
            Some(span) if span.start <= range.start && range.end <= span.end => {
                (Place::Inside, Some(span))
            }
            Some(span) if span.start < range.end => (Place::Meets, None),
            _ => (Place::Clear, None),
        }
    }
}

/// A walk over one Preview parse that hands each `Mark` to `emit` in document order, mapping
/// positions with `map`, and counts the math nodes whose range is exactly a joined formula.
/// Each walk method returns false as soon as `emit` does.
struct Reading<'s, M, E> {
    spans: SpanCursor<'s>,
    map: M,
    emit: E,
    formulas: usize,
}

impl<'a, M, E> Reading<'_, M, E>
where
    M: FnMut(usize) -> usize,
    E: FnMut(Mark<'a>) -> bool,
{
    fn blocks(&mut self, blocks: &'a [BlockNode]) -> bool {
        blocks.iter().all(|block| self.block(block))
    }

    fn block(&mut self, block: &'a BlockNode) -> bool {
        let range = block.source_range();
        match block {
            BlockNode::BlockQuote { blocks, .. } => {
                self.node(Node::Quote, range, Role::Container, |walk| {
                    walk.blocks(blocks)
                })
            }
            BlockNode::List(list) => {
                let node = Node::List {
                    ordered: list.ordered,
                    start: list.start,
                    tight: list.tight,
                };
                self.node(node, range, Role::Container, |walk| {
                    list.items.iter().all(|item| {
                        let node = Node::Item {
                            checked: item.checked,
                        };
                        walk.node(node, item.source_range, Role::Container, |walk| {
                            walk.blocks(&item.blocks)
                        })
                    })
                })
            }
            BlockNode::Paragraph { content, .. } => {
                self.node(Node::Paragraph, range, Role::TextBlock, |walk| {
                    walk.inlines(content)
                })
            }
            BlockNode::Heading { level, content, .. } => {
                self.node(Node::Heading(*level), range, Role::TextBlock, |walk| {
                    walk.inlines(content)
                })
            }
            BlockNode::DisplayMath(math) => {
                self.node(Node::DisplayMath(&math.literal), range, Role::Part, |_| {
                    true
                })
            }
            BlockNode::CodeBlock(code) => {
                let node = Node::CodeBlock {
                    literal: &code.literal,
                    info: &code.info,
                    fenced: code.fenced,
                    closed: code.closed,
                };
                self.node(node, range, Role::Part, |_| true)
            }
            BlockNode::Table(table) => {
                self.node(Node::Table(&table.alignments), range, Role::Part, |walk| {
                    table.rows.iter().all(|row| {
                        let node = Node::Row { header: row.header };
                        walk.node(node, row.source_range, Role::Part, |walk| {
                            row.cells.iter().all(|cell| {
                                walk.node(Node::Cell, cell.source_range, Role::Part, |walk| {
                                    walk.inlines(&cell.content)
                                })
                            })
                        })
                    })
                })
            }
            BlockNode::ThematicBreak { .. } => self.node(Node::Rule, range, Role::Part, |_| true),
            BlockNode::HtmlLiteral { literal, .. } => {
                self.node(Node::HtmlBlock(literal), range, Role::Part, |_| true)
            }
        }
    }

    fn inlines(&mut self, inlines: &'a [InlineNode]) -> bool {
        inlines.iter().all(|inline| {
            let (node, children): (Node<'a>, &'a [InlineNode]) = match inline {
                InlineNode::Text { .. } => return true,
                InlineNode::Emphasis { children, .. } => (Node::Emphasis, children),
                InlineNode::Strong { children, .. } => (Node::Strong, children),
                InlineNode::Strikethrough { children, .. } => (Node::Strikethrough, children),
                InlineNode::Link {
                    destination,
                    title,
                    kind,
                    children,
                    ..
                } => (
                    Node::Link {
                        destination,
                        title,
                        kind: *kind,
                    },
                    children,
                ),
                InlineNode::Image {
                    destination,
                    title,
                    alt,
                    kind,
                    ..
                } => (
                    Node::Image {
                        destination,
                        title,
                        alt,
                        kind: *kind,
                    },
                    &[],
                ),
                InlineNode::Code { literal, .. } => (Node::Code(literal), &[]),
                InlineNode::InlineMath(math) => (
                    Node::InlineMath {
                        literal: &math.literal,
                        display: math.display,
                    },
                    &[],
                ),
                InlineNode::SoftBreak { .. } => (Node::SoftBreak, &[]),
                InlineNode::HardBreak { .. } => (Node::HardBreak, &[]),
                InlineNode::HtmlLiteral { literal, .. } => (Node::InlineHtml(literal), &[]),
            };
            self.node(node, inline.source_range(), Role::Part, |walk| {
                walk.inlines(children)
            })
        })
    }

    /// Emits `node` and what `inner` emits, unless the node lies inside a joined formula;
    /// a paragraph or heading that meets one emits only what `inner` does.
    fn node(
        &mut self,
        node: Node<'a>,
        range: Option<SourceRange>,
        role: Role,
        inner: impl FnOnce(&mut Self) -> bool,
    ) -> bool {
        match self.spans.place(range) {
            (Place::Inside, span) => {
                if matches!(node, Node::DisplayMath(_) | Node::InlineMath { .. })
                    && range.zip(span).is_some_and(|(range, span)| {
                        range.start == span.start && range.end == span.end
                    })
                {
                    self.formulas += 1;
                }
                true
            }
            (Place::Meets, _) if role == Role::TextBlock => inner(self),
            _ => {
                let start = range.map(|range| (self.map)(range.start));
                if !(self.emit)(Mark::Open(node, start)) || !inner(self) {
                    return false;
                }
                let end = match role {
                    Role::Container => None,
                    Role::TextBlock | Role::Part => range.map(|range| (self.map)(range.end)),
                };
                (self.emit)(Mark::Close(end))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    fn parse(text: &str) -> crate::preview::OwnedDocumentTree {
        crate::preview::PreviewParser
            .parse(&stickymd_core::DocumentSnapshot {
                text: std::sync::Arc::from(text),
                generation: stickymd_core::Generation::initial(),
                line_ending: stickymd_core::LineEnding::Lf,
            })
            .unwrap()
    }

    /// The result must read each joined formula as one formula. Here the result's parse is
    /// of a text of the same length whose `$$` pair is broken, so the span is a paragraph.
    #[test]
    fn surroundings_require_each_joined_formula_to_be_one_math_node() {
        let joins = crate::math_text::join_math_equals_lines("$$a\n=\nb$$", None).unwrap();
        assert_eq!(joins.text(), "$$a=b$$");
        let original = parse("$$a\n=\nb$$");
        assert!(super::joins_keep_surroundings(
            &original,
            &parse("$$a=b$$"),
            &joins
        ));
        assert!(!super::joins_keep_surroundings(
            &original,
            &parse("\\$a=b$$"),
            &joins
        ));
    }

    /// A display formula block that meets a joined formula is compared whole, unlike the
    /// paragraph or heading that holds the join. Step 1 never produces this layout (it joins
    /// only when the line below the `=` holds formula text before the closer), so the parses
    /// here are of texts laid out like it: in the original, `$$ b$$` starts inside the joined
    /// formula and ends after it; in the result it is gone.
    #[test]
    fn surroundings_compare_a_display_formula_that_meets_a_joined_one() {
        let joins = crate::math_text::join_math_equals_lines("$$a\n=\nc $$b$$", None).unwrap();
        assert_eq!(joins.joined_formulas().to_vec(), vec![0..10]);
        let result = parse("$$a=c $$b$$");
        // Control: a paragraph there holds only text, so the two read alike.
        assert!(super::joins_keep_surroundings(
            &parse("$$a\n=\nc d e f g"),
            &result,
            &joins
        ));
        assert!(!super::joins_keep_surroundings(
            &parse("$$a\n=\n$$ b$$"),
            &result,
            &joins
        ));
    }

    /// A walk in document order maps non-decreasing positions, so the forward position map
    /// never falls back to its search: shown on quotes, lists, a table, nested inlines and
    /// every leaf kind, around a joined formula.
    #[test]
    fn surroundings_map_positions_in_document_order() {
        let text = "> - *a [b `c` d](u) **e***\n>   1. [x] f\n\n| a | *b* |\n|---|---|\n\
                    | `c` | [d](e) |\n\n$$a\n=\nb$$ *x [y](z)* `w`\n\n    code\n\n<div>\nh\n</div>\n\n\
                    ***\n\n![i](j) \\(m\\) h  \nbreak\n\n## t\n";
        let joins = crate::math_text::join_math_equals_lines(text, None).unwrap();
        let tree = parse(text);
        let mut positions = Vec::new();
        let mut marks = 0;
        let mut reading = super::Reading {
            spans: super::SpanCursor::new(joins.joined_formulas()),
            map: |at| {
                positions.push(at);
                at
            },
            emit: |_| {
                marks += 1;
                true
            },
            formulas: 0,
        };
        assert!(reading.blocks(&tree.blocks));
        assert!(positions.is_sorted(), "{positions:?}");
        assert!(marks > 60 && positions.len() > 40, "{marks} {positions:?}");
    }

    /// The span cursor places a range the same way whether starts come in order or not.
    #[test]
    fn span_cursor_places_ranges_out_of_order_as_in_order() {
        let spans = [3..6, 10..12];
        let ranges: Vec<_> = (0..14)
            .flat_map(|start| {
                (start..14).map(move |end| crate::preview::SourceRange::new(start, end))
            })
            .collect();
        let fresh = |range| super::SpanCursor::new(&spans).place(range).0;
        let mut cursor = super::SpanCursor::new(&spans);
        for &range in ranges.iter().chain(ranges.iter().rev()) {
            assert!(cursor.place(range).0 == fresh(range), "{range:?}");
        }
    }
}
