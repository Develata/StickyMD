//! One pass of the `$` math action as pure text: equals-line join, then
//! delimiter conversion of the joined text.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#semantic-math-delimiter-conversion
//!
//! The two steps stay independent modules; this only composes them, their
//! position maps and the check that ties step 1 to the Preview's reading of
//! its result (`joins_lie_in_text`). Callers own the generation check, the
//! transaction and the selection.

use std::ops::Range;

use stickymd_core::DocumentSnapshot;

use crate::math_text::{EqualsLineConversion, join_math_equals_lines};
use crate::preview::{
    BlockNode, InlineNode, MAX_PREVIEW_SOURCE_BYTES, MathDelimiterConversion, OwnedDocumentTree,
    PreviewParseError, PreviewParser, SemanticConversionError, SourceRange,
    convert_parsed_latex_math_delimiters, validate_scope,
};

/// The result of one pass that changed the text.
#[derive(Debug)]
pub enum MathActionPass {
    /// Only step 1 changed the text.
    Joined(EqualsLineConversion),
    /// Step 2 changed the text, after step 1 if it did.
    Converted(Option<EqualsLineConversion>, MathDelimiterConversion),
}

impl MathActionPass {
    /// How many places the pass changed, for the notice that reports it: the formulas whose
    /// delimiters it converted, or the equals lines it joined when that is all it did.
    pub fn change_count(&self) -> usize {
        match self {
            Self::Joined(joined) => joined.joined_lines(),
            Self::Converted(_, converted) => converted.replacement_count(),
        }
    }

    pub fn text(&self) -> &str {
        match self {
            Self::Joined(joined) => joined.text(),
            Self::Converted(_, converted) => converted.text(),
        }
    }

    pub fn into_text(self) -> String {
        match self {
            Self::Joined(joined) => joined.into_text(),
            Self::Converted(_, converted) => converted.into_text(),
        }
    }

    /// Maps a byte position of the pass input through both steps.
    pub fn map_position(&self, position: usize) -> usize {
        match self {
            Self::Joined(joined) => joined.map_position(position),
            Self::Converted(joined, converted) => converted.map_position(
                joined
                    .as_ref()
                    .map_or(position, |joined| joined.map_position(position)),
            ),
        }
    }
}

/// Runs step 1 on `snapshot` and step 2 on its result; a scope is mapped
/// between them. Returns None when neither step changes anything.
pub fn run_math_action(
    snapshot: &DocumentSnapshot,
    scope: Option<Range<usize>>,
) -> Result<Option<MathActionPass>, SemanticConversionError> {
    // Step 2 refuses such a source before it parses; step 1 must not parse
    // it first.
    if snapshot.text.len() > MAX_PREVIEW_SOURCE_BYTES {
        return Err(PreviewParseError::SourceTooLarge {
            actual: snapshot.text.len(),
            limit: MAX_PREVIEW_SOURCE_BYTES,
        }
        .into());
    }
    // Known limit (plan 07): the editor and the source maps split lines only at `\n`, while
    // Comrak also breaks at a `\r` that no `\n` follows. Past the first such `\r` their
    // positions disagree and a replacement could land on other bytes, so the pass refuses.
    if has_isolated_cr(&snapshot.text) {
        return Ok(None);
    }
    // Step 1 scans with Setext off, so it can read a definition, indented code,
    // an HTML block or inline code that a Setext heading starts in the Preview's
    // dialect for formula text, and a join can change the blocks after its
    // formula. When it joins anything, the Preview's parse of the original must
    // see each join as one formula a Setext underline split
    // (`joins_keep_blocks`); the Preview's parse of the result, which step 2
    // converts, must hold each joined `=` as text (`joins_lie_in_text`); and
    // every block outside the joined formulas must stay as it was
    // (`joins_keep_surroundings`). Otherwise step 1 is dropped whole and step 2
    // converts the original text with the tree it already has.
    validate_scope(&snapshot.text, scope.as_ref())?;
    let mut joined = join_math_equals_lines(&snapshot.text, scope.clone());
    let tree = match &joined {
        None => PreviewParser.parse(snapshot)?,
        Some(joins) => {
            let original = PreviewParser.parse(snapshot)?;
            let accepted = if joins_keep_blocks(&original, joins) {
                let result = PreviewParser.parse(&DocumentSnapshot {
                    text: joins.text().into(),
                    generation: snapshot.generation,
                    line_ending: snapshot.line_ending,
                })?;
                (joins_lie_in_text(&result, joins)
                    && joins_keep_surroundings(&original, &result, joins))
                .then_some(result)
            } else {
                None
            };
            accepted.unwrap_or_else(|| {
                joined = None;
                original
            })
        }
    };
    let scope = scope.map(|range| match &joined {
        Some(joined) => joined.map_position(range.start)..joined.map_position(range.end),
        None => range,
    });
    Ok(match convert_parsed_latex_math_delimiters(&tree, scope)? {
        Some(converted) => Some(MathActionPass::Converted(joined, converted)),
        None => joined.map(MathActionPass::Joined),
    })
}

/// Whether every joined `=` lies as text in `tree`, the Preview's parse of
/// step 1's result (`Leaves::text_block`).
fn joins_lie_in_text(tree: &OwnedDocumentTree, joins: &EqualsLineConversion) -> bool {
    let leaves = Leaves::new(tree);
    joins
        .joined_equals()
        .all(|equal| leaves.text_block(equal).is_some())
}

/// Whether, in `tree` (the Preview's parse of the original text), every join
/// is one formula that a Setext underline split: the line above and the `=`
/// line are text of one block (the Setext heading, or one paragraph when the
/// `=` is not an underline), and the line below is text of that block or of
/// the next leaf block. So a join never reaches into a definition (no block),
/// code, HTML or a table, nor across an ATX heading or any other block the
/// `$$` pair spans. Leaf blocks follow document order and the three lines are
/// adjacent, so "the next leaf block" is the block the line below starts.
fn joins_keep_blocks(tree: &OwnedDocumentTree, joins: &EqualsLineConversion) -> bool {
    let leaves = Leaves::new(tree);
    joins.joins().all(|[above, equal, below]| {
        let (Some(above), Some(equal), Some(below)) = (
            leaves.text_block(above),
            leaves.text_block(equal),
            leaves.text_block(below),
        ) else {
            return false;
        };
        above == equal && (below == equal || below == equal + 1)
    })
}

/// Whether every leaf block outside the joined formulas keeps its kind, its
/// source range (through the join's position map) and its containers between
/// the Preview's parse of the original and of the result. A join can keep a
/// list item open past its formula, for one: indented code after a blank line
/// then becomes the item's paragraph, which step 2 would convert. Leaves that
/// meet a joined formula may change; that is the join's purpose. O(leaves +
/// joins), since the forward position map passes each removed range once.
fn joins_keep_surroundings(
    original: &OwnedDocumentTree,
    result: &OwnedDocumentTree,
    joins: &EqualsLineConversion,
) -> bool {
    let spans = joins.joined_formulas();
    // Spans, then leaves, are visited in document order, so positions map forward.
    let mut map = joins.forward_position_map();
    let mapped: Vec<Range<usize>> = spans
        .iter()
        .map(|span| map(span.start)..map(span.end))
        .collect();
    let mut map = joins.forward_position_map();
    let mut before = Vec::new();
    outside_leaves(
        &original.blocks,
        spans,
        &mut map,
        &mut Vec::new(),
        &mut before,
    );
    let mut after = Vec::new();
    outside_leaves(
        &result.blocks,
        &mapped,
        &mut |at| at,
        &mut Vec::new(),
        &mut after,
    );
    before == after
}

/// A leaf block as `joins_keep_surroundings` compares it: its kind (and heading
/// level), its mapped source range, and the kind and mapped start of each
/// container around it, outermost first.
#[derive(Debug, PartialEq, Eq)]
struct Surrounding {
    kind: (&'static str, u8),
    range: Option<Range<usize>>,
    containers: Vec<(&'static str, Option<usize>)>,
}

/// Adds the leaves under `blocks` that do not meet a span in `spans` (sorted,
/// disjoint) to `output`, ranges mapped by `map`. A leaf without a source range
/// cannot be placed, so it is always compared.
fn outside_leaves(
    blocks: &[BlockNode],
    spans: &[Range<usize>],
    map: &mut dyn FnMut(usize) -> usize,
    containers: &mut Vec<(&'static str, Option<usize>)>,
    output: &mut Vec<Surrounding>,
) {
    for block in blocks {
        let (kind, range) = match block {
            BlockNode::Paragraph { source_range, .. } => (("paragraph", 0), *source_range),
            BlockNode::Heading {
                level,
                source_range,
                ..
            } => (("heading", *level), *source_range),
            BlockNode::DisplayMath(math) => (("display math", 0), math.source_range),
            BlockNode::CodeBlock(code) => (("code", 0), code.source_range),
            BlockNode::Table(table) => (("table", 0), table.source_range),
            BlockNode::ThematicBreak { source_range } => (("rule", 0), *source_range),
            BlockNode::HtmlLiteral { source_range, .. } => (("html", 0), *source_range),
            BlockNode::BlockQuote {
                blocks,
                source_range,
            } => {
                containers.push(("quote", source_range.map(|range| map(range.start))));
                outside_leaves(blocks, spans, map, containers, output);
                containers.pop();
                continue;
            }
            BlockNode::List(list) => {
                for item in &list.items {
                    containers.push(("item", item.source_range.map(|range| map(range.start))));
                    outside_leaves(&item.blocks, spans, map, containers, output);
                    containers.pop();
                }
                continue;
            }
        };
        let meets_span = range.is_some_and(|range| {
            let at = spans.partition_point(|span| span.end <= range.start);
            spans.get(at).is_some_and(|span| span.start < range.end)
        });
        if !meets_span {
            output.push(Surrounding {
                kind,
                range: range.map(|range| map(range.start)..map(range.end)),
                containers: containers.clone(),
            });
        }
    }
}

/// The leaf blocks of one Preview parse that have a source range, in document
/// order, each with whether it holds text a join may touch, and the ranges of
/// the inline code, Raw HTML, links and images in those text blocks. Leaves do
/// not overlap, nor do protected inlines (nothing inside one is recorded), so
/// a lookup is two binary searches: O(nodes) to build, O(log nodes) a byte.
struct Leaves {
    blocks: Vec<(SourceRange, bool)>,
    protected: Vec<SourceRange>,
}

impl Leaves {
    fn new(tree: &OwnedDocumentTree) -> Self {
        let mut leaves = Self {
            blocks: Vec::new(),
            protected: Vec::new(),
        };
        leaf_blocks(&tree.blocks, &mut leaves.blocks, &mut leaves.protected);
        leaves
    }

    /// The index of the leaf block that holds byte `at` as text: a paragraph,
    /// a heading or a display formula, outside its protected inlines. None in
    /// a code block, an HTML block, a table, or on a line no block holds, such
    /// as a link reference definition. A leaf without a source range holds no
    /// byte, and a text block with a protected inline that has none is not
    /// text, so such cases fail closed.
    fn text_block(&self, at: usize) -> Option<usize> {
        let index = self.blocks.partition_point(|(range, _)| range.end <= at);
        let &(range, text) = self.blocks.get(index)?;
        let protected = self.protected.partition_point(|range| range.end <= at);
        (text
            && range.start <= at
            && self
                .protected
                .get(protected)
                .is_none_or(|range| at < range.start))
        .then_some(index)
    }
}

/// The leaf blocks under `blocks` that have a source range, in document order,
/// each with whether it holds text a join may touch; and, in `protected`, the
/// ranges of the inline code, Raw HTML, links and images in those text blocks.
fn leaf_blocks(
    blocks: &[BlockNode],
    output: &mut Vec<(SourceRange, bool)>,
    protected: &mut Vec<SourceRange>,
) {
    for block in blocks {
        let (range, text) = match block {
            BlockNode::Paragraph {
                source_range,
                content,
            }
            | BlockNode::Heading {
                source_range,
                content,
                ..
            } => (*source_range, protected_inlines(content, protected)),
            BlockNode::DisplayMath(math) => (math.source_range, true),
            BlockNode::BlockQuote { blocks, .. } => {
                leaf_blocks(blocks, output, protected);
                continue;
            }
            BlockNode::List(list) => {
                for item in &list.items {
                    leaf_blocks(&item.blocks, output, protected);
                }
                continue;
            }
            BlockNode::CodeBlock(code) => (code.source_range, false),
            BlockNode::Table(table) => (table.source_range, false),
            BlockNode::ThematicBreak { source_range }
            | BlockNode::HtmlLiteral { source_range, .. } => (*source_range, false),
        };
        output.extend(range.map(|range| (range, text)));
    }
}

/// Adds the ranges of the protected inlines in `inlines` (inline code, Raw
/// HTML, links, images; nothing inside them) to `protected`, in document
/// order. False when one of them has no range, so no join is placed there.
fn protected_inlines(inlines: &[InlineNode], protected: &mut Vec<SourceRange>) -> bool {
    inlines.iter().all(|inline| match inline {
        InlineNode::Emphasis { children, .. }
        | InlineNode::Strong { children, .. }
        | InlineNode::Strikethrough { children, .. } => protected_inlines(children, protected),
        InlineNode::Code { source_range, .. }
        | InlineNode::Link { source_range, .. }
        | InlineNode::Image { source_range, .. }
        | InlineNode::HtmlLiteral { source_range, .. } => match source_range {
            Some(range) => {
                protected.push(*range);
                true
            }
            None => false,
        },
        InlineNode::Text { .. }
        | InlineNode::InlineMath(_)
        | InlineNode::SoftBreak { .. }
        | InlineNode::HardBreak { .. } => true,
    })
}

/// Whether `text` holds a `\r` that is not followed by `\n`: one SIMD search for `\r`, no
/// allocation. A `\r\n` pair is one line break for both Comrak and the source maps.
fn has_isolated_cr(text: &str) -> bool {
    let bytes = text.as_bytes();
    memchr::memchr_iter(b'\r', bytes).any(|index| bytes.get(index + 1) != Some(&b'\n'))
}

#[cfg(test)]
mod tests {
    /// The notice counts one place per joined equals line when that is all a pass did, and one
    /// per converted formula otherwise.
    #[test]
    fn a_pass_counts_joined_lines_or_converted_formulas() {
        let pass = |text: &str| {
            let snapshot = stickymd_core::DocumentSnapshot {
                text: std::sync::Arc::from(text),
                generation: stickymd_core::Generation::initial(),
                line_ending: stickymd_core::LineEnding::Lf,
            };
            super::run_math_action(&snapshot, None)
                .unwrap()
                .expect("the pass changes the text")
        };
        assert_eq!(pass("$$a\n=\nb$$").change_count(), 1);
        assert_eq!(pass("$$a\n=\nb\n=\nc$$").change_count(), 2);
        assert_eq!(pass("$$a\n=\nb$$\n\n$$c\n=\nd$$").change_count(), 2);
        assert_eq!(pass("\\(x\\) and \\(y\\)").change_count(), 2);
    }

    /// Step 1 can shrink a document below the Preview's node limit, but the original is parsed
    /// too when anything joins, so an original over the limit is refused with the parse error,
    /// before any change.
    #[test]
    fn phase11b_math_action_reports_an_original_over_the_node_limit() {
        let text = "$$a\n=\nb$$\n\n".repeat(50_001);
        let snapshot = stickymd_core::DocumentSnapshot {
            text: std::sync::Arc::from(text.as_str()),
            generation: stickymd_core::Generation::initial(),
            line_ending: stickymd_core::LineEnding::Lf,
        };
        assert!(matches!(
            super::run_math_action(&snapshot, None),
            Err(super::SemanticConversionError::Parse(
                super::PreviewParseError::NodeLimit { .. }
            ))
        ));
    }

    /// An invalid scope is refused before either step parses, so a document over the node limit
    /// still reports the scope.
    #[test]
    fn phase11b_math_action_refuses_an_invalid_scope_before_parsing() {
        let text = "a\n\n".repeat(100_001);
        let snapshot = stickymd_core::DocumentSnapshot {
            text: std::sync::Arc::from(text.as_str()),
            generation: stickymd_core::Generation::initial(),
            line_ending: stickymd_core::LineEnding::Lf,
        };
        assert!(matches!(
            super::run_math_action(&snapshot, None),
            Err(super::SemanticConversionError::Parse(
                super::PreviewParseError::NodeLimit { .. }
            ))
        ));
        assert!(matches!(
            super::run_math_action(&snapshot, Some(0..text.len() + 1)),
            Err(super::SemanticConversionError::InvalidScope)
        ));
    }

    /// Step 1 scans with Setext off. After a Setext heading, the Preview's dialect reads the
    /// lines it joins as a link reference definition, indented code or an HTML block, so the
    /// joined `=` is not text in the Preview's parse of the result: step 1 is dropped whole and
    /// step 2 converts the original text (cross-model review, 2026-11-24). A formula after a
    /// definition still joins (`math_text` tests).
    #[test]
    fn phase11b_math_action_drops_joins_the_preview_reads_outside_text() {
        let pass = |text: &str| {
            let snapshot = stickymd_core::DocumentSnapshot {
                text: std::sync::Arc::from(text),
                generation: stickymd_core::Generation::initial(),
                line_ending: stickymd_core::LineEnding::Lf,
            };
            super::run_math_action(&snapshot, None)
                .unwrap()
                .map(|pass| pass.into_text())
        };
        for text in [
            "[r]: /u\nHeading\n===\n[x]: /v \"$$a\n    =\nb$$\"\n\n[x]",
            "Heading\n===\n[x]: /v \"$$a\n    =\nb$$\"\n\n[x]",
            "Heading\n===\n    $$a\n    =\n    b$$",
            "Heading\n===\n<custom-tag>\n$$a\n=\nb$$\n",
            // The Setext heading leaves its backtick unpaired, so the lines below are one code
            // span; with Setext off the backticks pair the other way (second review, 2026-11-24).
            "H `\n===\n`$$a\n    =\nb$$`",
            // The Preview's parse of the original text: the line below the joined `=` starts a
            // definition, indented code or an HTML block, or the `$$` pair spans an ATX heading
            // (third review, 2026-11-24). With a definition in front, `main` left them alone.
            "$$a\n=\n[x]: /v \"b$$\"\n\n[x]",
            "[p]: /p\n$$a\n=\n[x]: /v \"b$$\"\n\n[x]",
            "$$a\n=\n    b$$",
            "$$a\n=\n<custom-tag>\nb$$",
            "$$a\n# H\n=\nb$$",
            "[p]: /p\n$$a\n# H\n=\nb$$",
            // Fourth review: an ATX heading below the `=`, one between the opener and the
            // join, and a quote marker that only the raw source holds. Step 1 takes `$$`
            // formulas from Comrak's nodes alone (USER 2026-11-25), so none of them joins.
            "$$a\n=\n# b$$",
            "[p]: /p\n$$open\n# H\nprose\n=\nnext$$",
            "> $$a\n=\n> b$$",
            // Fifth review: joining keeps the list item open past its formula, so the indented
            // code after the blank line would become the item's paragraph and step 2 would
            // convert it. The blocks around the formula must stay as they were.
            "- $$a\n  =\nb$$\n\n    \\(z\\)",
            "- $$a\n  =\nb$$$\n\n    \\(z\\)",
        ] {
            assert_eq!(pass(text), None, "{text:?}");
            // Step 2 still converts the original text.
            assert_eq!(
                pass(&format!("{text}\n\n\\(x\\)")),
                Some(format!("{text}\n\n$x$")),
                "{text:?}"
            );
        }
        // Control: the same lines without the heading join, a formula after a definition
        // joins, and so does a chain the Setext underlines split into several blocks.
        for (text, joined) in [
            ("[x]: /w\n\n$$a\n    =\nb$$", "[x]: /w\n\n$$a=b$$"),
            ("[p]: /p\n$$a\n=\nb$$", "[p]: /p\n$$a=b$$"),
            ("$$a\n=\nb\n=\nc$$", "$$a=b=c$$"),
            ("- $$a\n  =\n  b$$", "- $$a=b$$"),
            // With Setext off Comrak reads the last line as the item's lazy continuation, so
            // this is one formula; joining completes it inside the item (USER 2026-11-25).
            ("- $$a\n  =\nb$$", "- $$a=b$$"),
            ("> $$a\n=\nb$$", "> $$a=b$$"),
        ] {
            assert_eq!(pass(text).as_deref(), Some(joined), "{text:?}");
        }
    }

    /// Known limit (plan 07): plan 05 keeps an isolated `\r` as an ordinary character and the
    /// source maps count only `\n`, while Comrak reads `\r` as a line break. A document that
    /// holds any `\r` without a following `\n` is left byte-identical. The first two inputs
    /// were rewritten before the refusal: a formula before the first `\r`, and, with mixed
    /// endings, the code span `\(y\)` that the shifted positions pointed at.
    #[test]
    fn phase11b_math_action_leaves_documents_with_an_isolated_cr_unchanged() {
        let pass_in = |text: &str, scope: Option<std::ops::Range<usize>>| {
            let snapshot = stickymd_core::DocumentSnapshot {
                text: std::sync::Arc::from(text),
                generation: stickymd_core::Generation::initial(),
                line_ending: stickymd_core::LineEnding::Lf,
            };
            super::run_math_action(&snapshot, scope)
                .unwrap()
                .map(|pass| pass.into_text())
        };
        let pass = |text: &str| pass_in(text, None);
        for text in [
            "\\(x\\)\rp\r",
            "# p\r# \\(x\\)\na`\\(y\\)`\n",
            "\\[\ra\r=\rb\r\\]\r",
            "$$\ra\r=\rb\r$$\r",
            "p\r\\(x\\) q\r",
            "p\r\\[\rx\r\\]\r",
            "\\(x\\)\n\n$$a\n=\nb$$\nlast\r",
            // The first `\r` is half of a `\r\n`; the isolated one comes later.
            "\\(x\\)\r\np\r",
        ] {
            assert_eq!(pass(text), None, "{text:?} changed");
        }
        // Control: the same documents with `\n` or `\r\n` still convert.
        assert_eq!(pass("\\(x\\)\np\n").as_deref(), Some("$x$\np\n"));
        assert_eq!(pass("\\(x\\)\r\np\r\n").as_deref(), Some("$x$\r\np\r\n"));
        assert_eq!(
            pass("# p\n# \\(x\\)\na`\\(y\\)`\n").as_deref(),
            Some("# p\n# $x$\na`\\(y\\)`\n")
        );
        // A selection of `\(x\)` that leaves the `\r` outside still refuses; with `\r\n` only,
        // the same selection converts.
        assert_eq!(pass_in("\\(x\\)\r\r\n", Some(0..5)), None);
        assert_eq!(
            pass_in("\\(x\\)\r\n", Some(0..5)).as_deref(),
            Some("$x$\r\n")
        );
    }

    /// Known limit (plan 07): the equals-line join does not tell text-mode arguments apart, so
    /// an equals line inside `\text{...}` is joined and its spaces go.
    #[test]
    fn phase11b_math_action_joins_an_equals_line_inside_a_text_argument() {
        let snapshot = stickymd_core::DocumentSnapshot {
            text: std::sync::Arc::from("$$\\text{a\n=\nb}$$"),
            generation: stickymd_core::Generation::initial(),
            line_ending: stickymd_core::LineEnding::Lf,
        };
        let pass = super::run_math_action(&snapshot, None).unwrap();
        assert_eq!(
            pass.map(|pass| pass.into_text()).as_deref(),
            Some("$$\\text{a=b}$$")
        );
    }

    #[test]
    fn an_isolated_cr_is_a_cr_without_a_following_lf() {
        for (text, isolated) in [
            ("", false),
            ("a\nb", false),
            ("a\r\nb", false),
            ("a\r\r\nb", true),
            ("a\rb", true),
            ("a\r", true),
        ] {
            assert_eq!(super::has_isolated_cr(text), isolated, "{text:?}");
        }
    }

    use std::time::{Duration, Instant};

    use comrak::{Arena, parse_document};
    use stickymd_core::{DocumentSnapshot, Generation, LineEnding};

    use super::run_math_action;
    use crate::preview::markdown_options;

    /// 1000 units that each need both steps, leave a bare `=` line outside a
    /// formula (so the second pass parses in full) and hold a paragraph that
    /// starts with a link reference definition (so step 1 protects the lines
    /// that no block covers), padded to 1 MiB with `filler`.
    fn workload(filler: &str) -> String {
        let unit = "\\[\nx^2\n=\ny^2\n\\]\n\nHeading\n=\ntext\n\n[r]: u\nt \\(q\\)\n\n";
        let mut source = unit.repeat(1_000);
        source.push_str(&filler.repeat((1024 * 1024 - source.len()) / filler.len()));
        while source.len() < 1024 * 1024 {
            source.push('x');
        }
        source
    }

    fn snapshot(text: &str) -> DocumentSnapshot {
        DocumentSnapshot {
            text: text.into(),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        }
    }

    /// What `convert_math_delimiters` runs, minus the commit: one pass, then
    /// the second pass that checks the result.
    fn button(input: &DocumentSnapshot) -> Duration {
        let started = Instant::now();
        let pass = run_math_action(input, None).unwrap().unwrap();
        let again = run_math_action(&snapshot(pass.text()), None).unwrap();
        let elapsed = started.elapsed();
        assert!(again.is_none());
        assert_eq!(pass.text().matches("$$\nx^2=y^2\n$$").count(), 1_000);
        assert_eq!(pass.text().matches("Heading\n=\ntext").count(), 1_000);
        // The paragraph after the definition `[r]: u` converts: Comrak 1.0 and
        // later report its positions in place.
        assert_eq!(pass.text().matches("[r]: u\nt $q$").count(), 1_000);
        elapsed
    }

    fn summary(samples: &mut [Duration]) -> (Duration, Duration, Duration) {
        samples.sort_unstable();
        // Nearest-rank p95: the ceil(0.95 n)-th smallest sample.
        let p95_rank = (samples.len() * 95).div_ceil(100);
        (
            samples[samples.len() / 2],
            samples[p95_rank - 1],
            samples[samples.len() - 1],
        )
    }

    #[test]
    fn phase11b_math_action_refuses_an_oversized_source_before_joining() {
        // One byte over the limit, and back under it once the join removes
        // two newlines: only a check before step 1 refuses it.
        let mut text = "x".repeat(super::MAX_PREVIEW_SOURCE_BYTES - 10);
        text.push_str("\n\n$$a\n=\nb$$");
        assert_eq!(text.len(), super::MAX_PREVIEW_SOURCE_BYTES + 1);
        let result = run_math_action(&snapshot(&text), None);
        assert!(
            matches!(
                result,
                Err(super::SemanticConversionError::Parse(
                    super::PreviewParseError::SourceTooLarge { .. }
                ))
            ),
            "{result:?}"
        );
        // The size check comes first: an isolated `\r` does not turn it into a no-op.
        text.push('\r');
        let result = run_math_action(&snapshot(&text), None);
        assert!(
            matches!(
                result,
                Err(super::SemanticConversionError::Parse(
                    super::PreviewParseError::SourceTooLarge { .. }
                ))
            ),
            "{result:?}"
        );
    }

    /// One Comrak parse of `source` with the Preview's options.
    fn parse_once(source: &str) -> Duration {
        let started = Instant::now();
        let arena = Arena::new();
        std::hint::black_box(parse_document(&arena, source, &markdown_options()));
        started.elapsed()
    }

    /// Plan 07's two checks, after three untimed rounds, over 100 timed
    /// clicks that each follow a timed parse of the same document (so a stall
    /// slows both alike):
    ///
    /// - regression: the click median is at most 10 parse medians. The ratio
    ///   removes the host's speed, so a fast machine catches waste too;
    /// - fluency: the nearest-rank p95 (the 95th sample) stays under 100 ms.
    ///   Up to five samples may exceed it, so a stall of a shared CI host that
    ///   slows two or three clicks does not fail it.
    ///
    /// Samples print in run order to show such stalls.
    #[test]
    #[ignore = "Release-only Phase 11-B math button receipt"]
    fn phase11b_performance_math_action_one_mib() {
        let source = workload("plain text filler ");
        let input = snapshot(&source);
        for _ in 0..3 {
            parse_once(&source);
            button(&input);
        }
        let mut parses = Vec::with_capacity(100);
        let mut samples = Vec::with_capacity(100);
        for _ in 0..100 {
            parses.push(parse_once(&source));
            samples.push(button(&input));
        }
        let in_run_order: Vec<_> = samples
            .iter()
            .map(|sample| format!("{:.1}", sample.as_secs_f64() * 1e3))
            .collect();
        eprintln!(
            "phase11b math_action_1mib_1000_math samples_ms=[{}]",
            in_run_order.join(",")
        );
        let (parse, _, _) = summary(&mut parses);
        let (median, p95, max) = summary(&mut samples);
        eprintln!("phase11b math_action_1mib_1000_math median={median:?} p95={p95:?} max={max:?}");
        eprintln!(
            "phase11b math_action_1mib_1000_math parse_median={parse:?} median_over_parse={:.2}",
            median.as_secs_f64() / parse.as_secs_f64()
        );
        assert!(median <= parse * 10, "median={median:?} parse={parse:?}");
        assert!(p95 < Duration::from_millis(100), "p95={p95:?}");
    }

    #[test]
    #[ignore = "Release-only Phase 11-B math button receipt"]
    fn phase11b_performance_math_action_stays_linear_after_reference_definitions() {
        // 23,717 paragraphs that start with a definition, under the Preview's
        // node limit. Parsing so many blocks takes far longer than one large
        // paragraph, so this guards the shape, not the 100 ms bound: the
        // button parses four times, while a per-node search over the lines
        // the definitions leave uncovered would cost hundreds of parses.
        let source = workload("[r]: u\nplain text filler plain text filler\n\n");
        let input = snapshot(&source);
        let mut parses: Vec<_> = (0..9).map(|_| parse_once(&source)).collect();
        let mut samples: Vec<_> = (0..9).map(|_| button(&input)).collect();
        let (parse, _, _) = summary(&mut parses);
        let (median, p95, max) = summary(&mut samples);
        eprintln!(
            "phase11b math_action_1mib_definition_paragraphs parse_median={parse:?} \
             median={median:?} p95={p95:?} max={max:?}"
        );
        assert!(median < parse * 12, "median={median:?} parse={parse:?}");
    }

    /// What a join may never change, read from one Preview parse: link targets and titles,
    /// inline code and Raw HTML outside `formula` (inside it they are the formula's own text
    /// once joined), code and HTML block literals, ATX heading texts, and how many list items
    /// and quotes there are. Setext headings may go: that is the point of a join.
    fn untouchable(text: &str, formula: &std::ops::Range<usize>) -> Vec<String> {
        use crate::preview::{BlockNode, InlineNode, PreviewParser};
        fn inlines(nodes: &[InlineNode], formula: &std::ops::Range<usize>, out: &mut Vec<String>) {
            for node in nodes {
                if node
                    .source_range()
                    .is_some_and(|range| formula.start <= range.start && range.end <= formula.end)
                {
                    continue;
                }
                match node {
                    InlineNode::Link {
                        destination,
                        title,
                        children,
                        ..
                    } => {
                        out.push(format!("link {destination:?} {title:?}"));
                        inlines(children, formula, out);
                    }
                    InlineNode::Image {
                        destination, title, ..
                    } => out.push(format!("image {destination:?} {title:?}")),
                    InlineNode::Code { literal, .. } => out.push(format!("code {literal:?}")),
                    InlineNode::HtmlLiteral { literal, .. } => {
                        out.push(format!("html {literal:?}"))
                    }
                    InlineNode::Emphasis { children, .. }
                    | InlineNode::Strong { children, .. }
                    | InlineNode::Strikethrough { children, .. } => inlines(children, formula, out),
                    _ => {}
                }
            }
        }
        fn blocks(
            text: &str,
            formula: &std::ops::Range<usize>,
            nodes: &[BlockNode],
            out: &mut Vec<String>,
        ) {
            for node in nodes {
                match node {
                    BlockNode::Paragraph { content, .. } => inlines(content, formula, out),
                    BlockNode::Heading {
                        content,
                        source_range,
                        ..
                    } => {
                        let source = source_range.and_then(|range| range.text(text));
                        if source.is_some_and(|source| source.starts_with('#')) {
                            out.push(format!("atx {source:?}"));
                        }
                        inlines(content, formula, out);
                    }
                    BlockNode::BlockQuote { blocks: inner, .. } => {
                        out.push("quote".to_owned());
                        blocks(text, formula, inner, out);
                    }
                    BlockNode::List(list) => {
                        for item in &list.items {
                            out.push("item".to_owned());
                            blocks(text, formula, &item.blocks, out);
                        }
                    }
                    BlockNode::CodeBlock(code) => out.push(format!("codeblock {:?}", code.literal)),
                    BlockNode::HtmlLiteral { literal, .. } => {
                        out.push(format!("htmlblock {literal:?}"))
                    }
                    BlockNode::Table(_) | BlockNode::ThematicBreak { .. } => {}
                    BlockNode::DisplayMath(_) => {}
                }
            }
        }
        let tree = PreviewParser.parse(&snapshot(text)).unwrap();
        let mut out = Vec::new();
        blocks(text, formula, &tree.blocks, &mut out);
        out.sort();
        out
    }

    /// Formula shapes × the block starts a Setext heading lets begin × containers (with every
    /// line prefixed, or the lines after the first lazy) × an optional definition in front ×
    /// what follows the formula (a reference probe, or indented code after a blank line),
    /// through the whole action. Whenever step 1's joins are kept, `untouchable` is as it was
    /// after step 1 and again after step 2 (code stays code, a definition stays a definition),
    /// and the prefix is kept byte for byte. An oracle independent of the checks, with a
    /// kept-join count per container.
    #[derive(Clone, Copy)]
    enum Lazy {
        None,
        All,
        Last,
    }

    #[test]
    fn phase11b_math_action_joins_touch_nothing_but_formula_text() {
        let lines = [
            "b",
            "# h",
            "[x]: /v \"t\"",
            "    code",
            "<custom-tag>",
            "<div>",
            "```",
            "- item",
            "1. item",
            "> q",
            "***",
            "`b`",
            "[x]",
            "b `c",
        ];
        // (prefix, which lines after the first go without it): none, all, or only the last.
        let containers = [
            ("", Lazy::None),
            ("> ", Lazy::None),
            ("> ", Lazy::All),
            ("> ", Lazy::Last),
            ("- ", Lazy::None),
            ("- ", Lazy::All),
            ("- ", Lazy::Last),
        ];
        let mut kept = [0_usize; 7];
        for prefix in ["", "[p]: /p\n", "[x]: /v \"t\"\n\n"] {
            for suffix in ["\n\n[x]\n", "\n\n    \\(z\\)\n\n[x]\n"] {
                for (slot, &(container, lazy)) in containers.iter().enumerate() {
                    for above in lines {
                        for below in lines {
                            for shape in [
                                "$$a\n=\n{below}$$",
                                "$${above}\n=\n{below}$$",
                                "\\[a\n=\n{below}\\]",
                                "$$a\n{above}\n=\n{below}$$",
                                "x \\(a\n=\n{below}\\) y",
                            ] {
                                let formula =
                                    shape.replace("{above}", above).replace("{below}", below);
                                let last = formula.lines().count() - 1;
                                let body = formula
                                    .lines()
                                    .enumerate()
                                    .map(|(index, line)| match (index, lazy) {
                                        (0, _) => format!("{container}{line}"),
                                        (_, Lazy::All) => line.to_owned(),
                                        (index, Lazy::Last) if index == last => line.to_owned(),
                                        _ if container == "> " => format!("> {line}"),
                                        _ => format!("{}{line}", " ".repeat(container.len())),
                                    })
                                    .collect::<Vec<_>>()
                                    .join("\n");
                                let text = format!("{prefix}{body}{suffix}");
                                let Some(pass) = run_math_action(&snapshot(&text), None).unwrap()
                                else {
                                    continue;
                                };
                                let joins = match &pass {
                                    super::MathActionPass::Joined(joins)
                                    | super::MathActionPass::Converted(Some(joins), _) => joins,
                                    super::MathActionPass::Converted(None, _) => continue,
                                };
                                kept[slot] += 1;
                                let span = |text: &str| prefix.len()..text.len() - suffix.len();
                                assert_eq!(
                                    untouchable(joins.text(), &span(joins.text())),
                                    untouchable(&text, &span(&text)),
                                    "{text:?} -> {:?}",
                                    joins.text()
                                );
                                // Step 2 may then convert formulas outside the span (they were
                                // formulas before too); the final reading must still match.
                                let original = span(&text);
                                let shown = pass.map_position(original.start)
                                    ..pass.map_position(original.end);
                                assert_eq!(
                                    untouchable(pass.text(), &shown),
                                    untouchable(&text, &original),
                                    "{text:?} -> {:?}",
                                    pass.text()
                                );
                                assert!(pass.text().starts_with(prefix), "{text:?}");
                            }
                        }
                    }
                }
            }
        }
        // Every container shape keeps joins, so none is checked only vacuously, except quotes
        // whose `=` line keeps its `>`: `> =` is not a bare `=`, so step 1 never joins there.
        assert_eq!((kept[1], kept[3]), (0, 0), "{kept:?}");
        assert!(
            [0, 2, 4, 5, 6].iter().all(|&slot| kept[slot] > 1_000),
            "{kept:?}"
        );
    }
}
