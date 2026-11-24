//! Display math that Comrak leaves among a paragraph's other inline content: style, line, copy.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#display-math-in-paragraph

use super::*;
use crate::math::MathKind;
use crate::preview::layout::{DecorationRole, LayoutContent};
use crate::preview::{PreviewParser, RenderBlock, RenderBlockKind, RenderTreeBuilder};
use std::sync::Arc;

const TIGHT: &str = "都有\n$$\nx^2+y\n$$\n上界";
const BLANK: &str = "都有\n\n$$\nx^2+y\n$$\n\n上界";
const LINE_HEIGHT: f32 = 26.35;

fn snapshot(text: &str) -> DocumentSnapshot {
    DocumentSnapshot {
        text: Arc::from(text),
        generation: Generation::initial(),
        line_ending: stickymd_core::LineEnding::Lf,
    }
}

fn frame(text: &str, width: u32) -> (PreviewPipeline, PreviewFrame) {
    let mut pipeline = PreviewPipeline::new();
    let frame = pipeline
        .build(
            &snapshot(text),
            width,
            2000,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    (pipeline, frame)
}

fn blocks(text: &str) -> Vec<RenderBlock> {
    let owned = PreviewParser.parse(&snapshot(text)).unwrap();
    RenderTreeBuilder.build(&owned).blocks
}

fn math_kinds(block: &RenderBlock) -> Vec<MathKind> {
    block
        .spans
        .iter()
        .filter_map(|span| span.math.as_ref().map(|math| math.kind))
        .collect()
}

/// A formula chunk as `(x, y, width)`.
type Formula = (f32, f32, f32);

/// Every formula chunk and every text chunk (`(x, y)`) of block `index`.
fn chunks(pipeline: &PreviewPipeline, index: usize) -> (Vec<Formula>, Vec<(f32, f32)>) {
    let (mut formulas, mut text) = (Vec::new(), Vec::new());
    for chunk in &pipeline.layout.as_ref().unwrap().blocks[index].chunks {
        match &chunk.content {
            LayoutContent::Math(math) => {
                formulas.push((chunk.x, chunk.y, math.metrics.width as f32));
            }
            LayoutContent::Text(_) => text.push((chunk.x, chunk.y)),
            LayoutContent::Image(_) => {}
        }
    }
    (formulas, text)
}

fn block_height(pipeline: &PreviewPipeline, index: usize) -> f32 {
    let block = &pipeline.layout.as_ref().unwrap().blocks[index];
    block.bottom - block.top
}

fn embedded_backgrounds(pipeline: &PreviewPipeline, index: usize) -> usize {
    pipeline.layout.as_ref().unwrap().blocks[index]
        .decorations
        .iter()
        .filter(|item| item.role == DecorationRole::EmbeddedMathBackground)
        .count()
}

fn formula_box(frame: &PreviewFrame) -> crate::preview::PreviewTextBox {
    frame
        .index()
        .boxes()
        .iter()
        .find(|item| item.atomic)
        .expect("a formula box")
        .clone()
}

/// A drag with the pointer pressed at `from` and released at `to`, both document coordinates,
/// as the Preview input builds a selection: two public hit tests.
fn drag(frame: &PreviewFrame, from: (f32, f32), to: (f32, f32)) -> PreviewSelection {
    PreviewSelection {
        anchor: frame.hit_test(from.0, from.1),
        active: frame.hit_test(to.0, to.1),
    }
}

fn middle(rect: crate::preview::PreviewRect) -> (f32, f32) {
    (rect.x + rect.width * 0.5, rect.y + rect.height * 0.5)
}

#[test]
fn paragraph_embedded_display_math_is_display_style_and_real_inline_math_is_not() {
    for source in [TIGHT, "都有\n\\[\nx^2+y\n\\]\n上界", "a $$x$$ b"] {
        let tree = blocks(source);
        assert_eq!(math_kinds(&tree[0]), [MathKind::Display], "{source:?}");
    }
    assert_eq!(math_kinds(&blocks("a $x$ b")[0]), [MathKind::Inline]);
    assert_eq!(
        math_kinds(&blocks("a $x$ b $$y$$ c")[0]),
        [MathKind::Inline, MathKind::Display]
    );
    // Standalone display math is still its own block.
    assert_eq!(math_kinds(&blocks(BLANK)[1]), [MathKind::Display]);
}

#[test]
fn list_item_and_quote_paragraphs_keep_display_math_but_headings_and_cells_stay_inline() {
    assert_eq!(
        math_kinds(&blocks("- a\n  $$x$$\n  b")[0]),
        [MathKind::Display]
    );
    assert_eq!(
        math_kinds(&blocks("> a\n> $$x$$\n> b")[0]),
        [MathKind::Display]
    );
    assert_eq!(math_kinds(&blocks("# a $$x$$ b")[0]), [MathKind::Inline]);
    let table = blocks("| a |\n|---|\n| t $$x$$ |");
    let RenderBlockKind::Table(table) = &table[0].kind else {
        panic!("a table block");
    };
    let kinds: Vec<MathKind> = table
        .rows
        .iter()
        .flat_map(|row| &row.cells)
        .flatten()
        .filter_map(|span| span.math.as_ref().map(|math| math.kind))
        .collect();
    assert_eq!(kinds, [MathKind::Inline], "exactly one formula, inline");
}

#[test]
fn the_formula_has_its_own_centered_line_between_the_text_lines() {
    let (pipeline, _) = frame(TIGHT, 900);
    let (formulas, text) = chunks(&pipeline, 0);
    let (x, y, width) = formulas[0];
    let (blank_pipeline, _) = frame(BLANK, 900);
    let blank_formula = chunks(&blank_pipeline, 1).0[0];
    // Same size and horizontal place as the standalone block's formula.
    assert_eq!((x, width), (blank_formula.0, blank_formula.2));
    assert_eq!(text.len(), 2, "one text chunk before and one after");
    assert!(text[0].1 + 1.0 < y, "the first text line is above");
    assert!(y + 1.0 < text[1].1, "the second text line is below");
    assert_eq!(text[0].0, text[1].0, "text keeps its left edge");
    let content_width = 900.0 - 2.0 * 24.0;
    assert!((x + width * 0.5 - (24.0 + content_width * 0.5)).abs() < 1.0);
}

#[test]
fn both_delimiters_and_a_same_line_formula_break_the_line() {
    for source in ["都有\n\\[\nx^2+y\n\\]\n上界", "before $$x^2$$ after"] {
        let (pipeline, _) = frame(source, 900);
        let (formulas, text) = chunks(&pipeline, 0);
        assert_eq!(text.len(), 2, "{source:?}");
        assert!(
            text[0].1 < formulas[0].1 && formulas[0].1 < text[1].1,
            "{source:?}"
        );
    }
}

#[test]
fn list_item_and_quote_formulas_are_centered_in_their_indented_content() {
    for source in ["- a\n  $$x$$\n  b", "> a\n> $$x$$\n> b"] {
        let (pipeline, _) = frame(source, 900);
        let (formulas, text) = chunks(&pipeline, 0);
        let (x, y, width) = formulas[0];
        // The text line below the formula starts at the content's left edge.
        let left = text
            .iter()
            .filter(|&&(_, ty)| ty > y)
            .map(|&(tx, _)| tx)
            .fold(f32::MAX, f32::min);
        assert!(text.iter().any(|&(_, ty)| ty < y), "{source:?}");
        assert!(left > 24.0 + 1.0, "{source:?}: the content is indented");
        let center = left + (900.0 - 24.0 - left) * 0.5;
        assert!((x + width * 0.5 - center).abs() < 1.0, "{source:?}");
    }
}

#[test]
fn a_hard_break_beside_the_formula_adds_no_blank_line_but_a_second_one_does() {
    let hard_breaks = |source: &str| {
        blocks(source)[0]
            .spans
            .iter()
            .filter(|span| span.hard_break)
            .count()
    };
    // One break beside a formula is that formula's line break.
    let (soft, _) = frame("a\n$$x$$\nb", 900);
    let (hard, _) = frame("a\\\n$$x$$\\\nb", 900);
    assert_eq!(block_height(&soft, 0), block_height(&hard, 0));
    // Each extra break keeps its blank line: before, after, and between two formulas.
    for (one, two) in [
        ("a\\\n$$x$$", "a\\\n\\\n$$x$$"),
        ("$$x$$\\\nb", "$$x$$\\\n\\\nb"),
        ("$$x$$\\\n$$y$$", "$$x$$\\\n\\\n$$y$$"),
    ] {
        assert_eq!(hard_breaks(two), hard_breaks(one) + 1, "{two:?}");
        let (one_pipeline, _) = frame(one, 900);
        let (two_pipeline, _) = frame(two, 900);
        let extra = block_height(&two_pipeline, 0) - block_height(&one_pipeline, 0);
        assert!((extra - LINE_HEIGHT).abs() < 1.0, "{two:?}: {extra}");
    }
}

#[test]
fn only_a_soft_break_is_left_out_beside_the_formula() {
    // A soft break inside a link carries the link's action but is still a soft break: two
    // formulas around it leave no text line at all.
    let (pipeline, _) = frame("[$$x$$\n$$y$$](/u)", 900);
    let (formulas, text) = chunks(&pipeline, 0);
    assert_eq!(formulas.len(), 2);
    assert!(text.is_empty(), "no blank text line between the formulas");
    // A code span that is just one space is content: it keeps its own layout box.
    let source = "$$x$$` `b";
    let (_, frame) = frame(source, 900);
    let code = source.find("` `").unwrap();
    assert!(frame.index().boxes().iter().any(|item| {
        item.source_range
            .is_some_and(|range| range.start <= code && range.end >= code + 3)
    }));
}

#[test]
fn a_paragraph_holding_only_the_formula_still_has_the_embedded_background() {
    for source in ["[$$x$$](/u)", "*$$x$$*"] {
        let (pipeline, frame) = frame(source, 900);
        let layout = pipeline.layout.as_ref().unwrap();
        assert_eq!(layout.blocks.len(), 1, "{source:?}: one paragraph");
        assert_eq!(embedded_backgrounds(&pipeline, 0), 1, "{source:?}");
        assert!(frame.copy_rails().is_empty(), "{source:?}");
    }
    let (_, frame) = frame("[$$x$$](/u)", 900);
    let (x, y) = middle(formula_box(&frame).rect);
    assert!(frame.action_at(x, y).is_some(), "the link on the formula");
}

#[test]
fn the_embedded_formula_has_a_background_but_never_a_copy_rail() {
    let (pipeline, tight) = frame(TIGHT, 900);
    assert_eq!(embedded_backgrounds(&pipeline, 0), 1);
    assert!(
        pipeline.layout.as_ref().unwrap().blocks[0]
            .decorations
            .iter()
            .all(|item| !item.role.is_block_background())
    );
    assert!(tight.copy_rails().is_empty());
    let (blank_pipeline, blank) = frame(BLANK, 900);
    assert_eq!(
        blank.copy_rails().len(),
        1,
        "the standalone block keeps its rail"
    );
    assert_eq!(
        embedded_backgrounds(&blank_pipeline, 1),
        0,
        "and its own background only"
    );
}

#[test]
fn copy_and_selection_text_are_those_of_the_source() {
    for source in [
        TIGHT,
        "a $$x$$ b",
        "都有\n\\[\nx\n\\]\n上界",
        "a\\\n$$x$$\\\nb",
    ] {
        let (_, frame) = frame(source, 900);
        assert_eq!(
            frame.copy_text(frame.select_all()).unwrap(),
            source,
            "{source:?}"
        );
    }
    // The display text keeps the soft breaks (spaces) the layout no longer draws.
    let (_, tight) = frame(TIGHT, 900);
    assert_eq!(tight.text(), "都有 $$\nx^2+y\n$$ 上界");
    // Dragging across just the formula copies its TeX, without delimiters.
    let rect = formula_box(&tight).rect;
    let selection = drag(
        &tight,
        (rect.x + 1.0, middle(rect).1),
        (rect.right() - 1.0, middle(rect).1),
    );
    assert_eq!(tight.copy_text(selection).unwrap(), "\nx^2+y\n");
}

#[test]
fn a_formula_wider_than_the_pane_ends_the_paragraph_and_still_drags_to_its_full_tex() {
    let tex = format!("{}z", "a+b+c+d+e+f+g+h+".repeat(8));
    let source = format!("lead\n$${tex}$$");
    // The pointer stays inside the pane, as the Preview input clamps it.
    let pane = 300.0;
    let (pipeline, frame) = frame(&source, pane as u32);
    let (formulas, _) = chunks(&pipeline, 0);
    assert!(formulas[0].2 > pane * 2.0, "the formula overflows the pane");
    let rect = formula_box(&frame).rect;
    let y = middle(rect).1;
    let selection = drag(&frame, (rect.x + 1.0, y), (pane - 1.0, y));
    assert_eq!(frame.copy_text(selection).unwrap(), tex);
    // Dragging back from the right edge selects it too.
    let back = drag(&frame, (pane - 1.0, y), (rect.x + 1.0, y));
    assert_eq!(frame.copy_text(back).unwrap(), tex);
}

#[test]
fn a_link_after_the_formula_line_still_hit_tests() {
    let (_, frame) = frame("a\n$$x$$\n[link](https://example.com) b", 900);
    let link = frame
        .index()
        .boxes()
        .iter()
        .find(|item| item.action.is_some())
        .expect("the link has a box")
        .clone();
    let (x, y) = middle(link.rect);
    assert!(frame.action_at(x, y).is_some());
    assert!(
        link.rect.y > formula_box(&frame).rect.bottom(),
        "the link is on the line below"
    );
}

#[test]
fn a_list_item_that_starts_with_the_formula_hits_the_formula_not_the_bullet() {
    let (_, frame) = frame("- [$$x$$](/u) tail", 900);
    let formula = formula_box(&frame);
    let rect = formula.rect;
    let (x, y) = middle(rect);
    assert!(frame.action_at(x, y).is_some(), "the link on the formula");
    let selection = drag(&frame, (rect.x + 1.0, y), (rect.right() - 1.0, y));
    assert_eq!(selection.anchor, formula.selection_range.start);
    assert_eq!(selection.active, formula.selection_range.end);
}
