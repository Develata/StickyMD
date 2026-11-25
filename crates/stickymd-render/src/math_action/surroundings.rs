//! The check that a join changes nothing the Preview shows outside its formulas: one walk
//! over the Preview's parse of the original and one over the result, compared node by node.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#semantic-math-delimiter-conversion

use std::ops::Range;

use super::composed_forward_map;
use crate::math_text::EqualsLineConversion;
use crate::preview::{
    BlockNode, ImageKind, InlineNode, LinkKind, MathDelimiterConversion, OwnedDocumentTree,
    SourceRange, TableAlignment, trimmed_math_literal,
};

/// Whether the Preview reads the result as it reads the original everywhere but inside the
/// joined formulas, and reads each joined formula as one formula:
///
/// - each joined formula, mapped, is the exact range of one math node of the result;
/// - every other node, mapped, is the same in both parses (`Mark`): the same kind and
///   fields, at the same place among its containers, with the same start, and the same end
///   unless it is a container;
/// - so is the text outside the joined formulas, compared as runs of adjacent text by mapped
///   range and place among the containers, so no text moves into or out of a quote or item.
///
/// Only these may differ:
///
/// - nodes and text wholly inside a joined formula;
/// - the kind and extent of a paragraph or heading that meets a joined formula. Undoing the
///   Setext split that cut the formula into a heading and a paragraph is the join's purpose,
///   so text on the formula's first line may go from heading to paragraph text; what such a
///   block holds is still compared. Any other block that meets a joined formula, a display
///   formula among them, is compared whole;
/// - a container's end, which the join may move: an item whose formula the join completes
///   ends with that formula.
///
/// Fifth to seventh review (2026-11-25): a join can keep a list item open over the next list,
/// which then renumbers, over indented code, which step 2 would convert, or over the text
/// after the formula's closer; it can re-pair the backticks after the closer, so code becomes
/// math; and since Comrak's budget for reference links is the document's length, a shorter
/// document can leave a reference outside every formula without its link.
///
/// Time: linear in the nodes, the joins and the bytes of the literals compared, when node
/// positions come in document order, as Comrak's did in every input tested; the span cursor
/// and the forward position map then pass once. A position that steps back costs a binary
/// search over the formulas and a mapping from the first removed range. Memory: the
/// original's marks and the mapped formula ranges; the result's marks are compared as they
/// come, and the walk stops at the first difference.
pub(super) fn joins_keep_surroundings(
    original: &OwnedDocumentTree,
    result: &OwnedDocumentTree,
    joins: &EqualsLineConversion,
) -> bool {
    reads_alike(original, result, joins.joined_formulas(), || {
        joins.forward_position_map()
    })
}

/// Plan 07's read-back of a pass that rewrote delimiters: the Preview's parse of its final
/// text against the parse of the original, by the same rule, with positions mapped through
/// both steps. A converted formula is compared like any other node, so it must read back as
/// one formula with the same literal and display, at its mapped range. Like a joined formula,
/// a formula made of single-`$` text must be exactly one math node, and what it covered in the
/// original (text) is not compared.
pub(super) fn pass_reads_back(
    original: &OwnedDocumentTree,
    final_tree: &OwnedDocumentTree,
    joins: Option<&EqualsLineConversion>,
    converted: &MathDelimiterConversion,
) -> bool {
    let mut spans = joins.map_or_else(Vec::new, |joins| joins.joined_formulas().to_vec());
    // Step 2's positions are in step 1's result; the read-back works in the original's.
    let mut unmap = joins.map(EqualsLineConversion::forward_unmap);
    spans.extend(converted.new_formulas().map(|range| match &mut unmap {
        Some(unmap) => unmap(range.start)..unmap(range.end),
        None => range,
    }));
    spans.sort_unstable_by_key(|span| span.start);
    reads_alike(original, final_tree, &spans, || {
        composed_forward_map(joins, Some(converted))
    })
}

/// The comparison of `original` with `other`, the parse of a text derived from it: `spans`
/// are the joined formulas in the original's positions, and `map` makes a fresh forward map
/// from those positions to `other`'s.
fn reads_alike<M: FnMut(usize) -> usize>(
    original: &OwnedDocumentTree,
    other: &OwnedDocumentTree,
    spans: &[Range<usize>],
    map: impl Fn() -> M,
) -> bool {
    let mut forward = map();
    let mapped: Vec<Range<usize>> = spans
        .iter()
        .map(|span| forward(span.start)..forward(span.end))
        .collect();
    let mut before = Vec::new();
    let mut reading = Reading::new(spans, map(), |mark| {
        before.push(mark);
        true
    });
    reading.blocks(&original.blocks);
    reading.finish();
    let mut compared = 0;
    let mut reading = Reading::new(
        &mapped,
        |at| at,
        |mark| {
            let same = before.get(compared) == Some(&mark);
            compared += 1;
            same
        },
    );
    let same = reading.blocks(&other.blocks) && reading.finish();
    let formulas = reading.formulas;
    same && compared == before.len() && formulas == mapped.len()
}

/// One step of a walk over a Preview parse, as `joins_keep_surroundings` compares it.
#[derive(Debug, PartialEq, Eq)]
enum Mark<'a> {
    /// A node starts: what it is, and its mapped start.
    Open(Node<'a>, Option<usize>),
    /// A node ends at its mapped end; None for a container, whose end the join may move.
    Close(Option<usize>),
    /// A run of text outside the joined formulas, as its mapped range. Adjacent text nodes
    /// make one run, so where Comrak splits text does not matter.
    Text(Range<usize>),
    /// Text without a source range, which cannot be placed: its literal.
    UnplacedText(&'a str),
}

/// A node's kind and every field the Preview's rendering or step 2 reads, apart from its
/// children and source range; a formula's literal without the whitespace just inside its
/// delimiters, which step 2 drops and TeX ignores. Text is a `Mark` of its own.
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

    /// The index of the first formula that ends after `start`.
    fn first_after(&mut self, start: usize) -> usize {
        if start < self.last {
            return self.spans.partition_point(|span| span.end <= start);
        }
        self.last = start;
        // Invariant: every span before `next` ends at or before `last`.
        while self
            .spans
            .get(self.next)
            .is_some_and(|span| span.end <= start)
        {
            self.next += 1;
        }
        self.next
    }

    /// Where `range` lies, and the formula it lies in, if it lies wholly inside one.
    fn place(&mut self, range: Option<SourceRange>) -> (Place, Option<&'s Range<usize>>) {
        let Some(range) = range else {
            return (Place::Clear, None);
        };
        let spans = self.spans;
        match spans.get(self.first_after(range.start)) {
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
/// Each walk method returns false as soon as `emit` does; `finish` emits the last text run.
struct Reading<'s, M, E> {
    spans: SpanCursor<'s>,
    map: M,
    emit: E,
    formulas: usize,
    /// Within a node wholly inside a joined formula: nothing is emitted, and the walk only
    /// looks for the formula's math node.
    inside: bool,
    /// The text run not emitted yet, unmapped.
    run: Option<Range<usize>>,
}

impl<'s, 'a, M, E> Reading<'s, M, E>
where
    M: FnMut(usize) -> usize,
    E: FnMut(Mark<'a>) -> bool,
{
    fn new(spans: &'s [Range<usize>], map: M, emit: E) -> Self {
        Self {
            spans: SpanCursor::new(spans),
            map,
            emit,
            formulas: 0,
            inside: false,
            run: None,
        }
    }

    fn finish(&mut self) -> bool {
        self.flush()
    }

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
                let literal = trimmed_math_literal(&math.literal);
                self.node(Node::DisplayMath(literal), range, Role::Part, |_| true)
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
                InlineNode::Text {
                    literal,
                    source_range,
                } => return self.text(*source_range, literal),
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
                        literal: trimmed_math_literal(&math.literal),
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

    /// Adds the parts of a text node outside the joined formulas to the text run.
    fn text(&mut self, range: Option<SourceRange>, literal: &'a str) -> bool {
        if self.inside {
            return true;
        }
        let Some(range) = range else {
            return self.flush() && (self.emit)(Mark::UnplacedText(literal));
        };
        let spans = self.spans.spans;
        let mut at = range.start;
        for span in &spans[self.spans.first_after(range.start)..] {
            if span.start >= range.end {
                break;
            }
            if !self.extend_run(at..span.start) {
                return false;
            }
            at = at.max(span.end);
        }
        self.extend_run(at..range.end)
    }

    /// Extends the text run with `part` when they touch; otherwise emits the run and starts a
    /// new one.
    fn extend_run(&mut self, part: Range<usize>) -> bool {
        if part.is_empty() {
            return true;
        }
        if let Some(run) = &mut self.run
            && run.end == part.start
        {
            run.end = part.end;
            return true;
        }
        let emitted = self.flush();
        self.run = Some(part);
        emitted
    }

    /// Emits the text run, if any. Called before any other mark, so positions are still mapped
    /// in document order.
    fn flush(&mut self) -> bool {
        match self.run.take() {
            Some(run) => {
                let start = (self.map)(run.start);
                let end = (self.map)(run.end);
                (self.emit)(Mark::Text(start..end))
            }
            None => true,
        }
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
        let (place, span) = self.spans.place(range);
        if self.inside || place == Place::Inside {
            // A paragraph that is only the formula holds its math node, so the walk goes on
            // inside, emitting nothing, to find it.
            if matches!(node, Node::DisplayMath(_) | Node::InlineMath { .. })
                && range
                    .zip(span)
                    .is_some_and(|(range, span)| range.start == span.start && range.end == span.end)
            {
                self.formulas += 1;
            }
            let outer = std::mem::replace(&mut self.inside, true);
            let kept = inner(self);
            self.inside = outer;
            return kept;
        }
        if place == Place::Meets && role == Role::TextBlock {
            return inner(self);
        }
        if !self.flush() {
            return false;
        }
        let start = range.map(|range| (self.map)(range.start));
        if !(self.emit)(Mark::Open(node, start)) || !inner(self) || !self.flush() {
            return false;
        }
        let end = match role {
            Role::Container => None,
            Role::TextBlock | Role::Part => range.map(|range| (self.map)(range.end)),
        };
        (self.emit)(Mark::Close(end))
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
        // Control: a paragraph there holds only text, `b$$` after the formula as in the
        // result, so the two read alike.
        assert!(super::joins_keep_surroundings(
            &parse("$$a\n=\nc d b$$"),
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
        let mut reading = super::Reading::new(
            joins.joined_formulas(),
            |at| {
                positions.push(at);
                at
            },
            |_| {
                marks += 1;
                true
            },
        );
        assert!(reading.blocks(&tree.blocks) && reading.finish());
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
