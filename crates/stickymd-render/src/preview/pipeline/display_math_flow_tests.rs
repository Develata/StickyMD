//! Display math that Comrak leaves among a paragraph's other inline content: style, line, copy.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#display-math-in-paragraph

use super::*;
use crate::math::MathKind;
use crate::preview::layout::{DecorationRole, LayoutContent};
use crate::preview::{PreviewParser, RenderBlock, RenderTreeBuilder};
use std::sync::Arc;

const TIGHT: &str = "都有\n$$\nx^2+y\n$$\n上界";
const BLANK: &str = "都有\n\n$$\nx^2+y\n$$\n\n上界";

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

type Chunks = (Option<(f32, f32, f32)>, Vec<(f32, f32)>);

/// The first formula chunk and every text chunk of block `index`, as `(x, y, width)`.
fn chunks(pipeline: &PreviewPipeline, index: usize) -> Chunks {
    let mut formula = None;
    let mut text = Vec::new();
    for chunk in &pipeline.layout.as_ref().unwrap().blocks[index].chunks {
        match &chunk.content {
            LayoutContent::Math(math) => {
                formula.get_or_insert((chunk.x, chunk.y, math.metrics.width as f32));
            }
            LayoutContent::Text(_) => text.push((chunk.x, chunk.y)),
            LayoutContent::Image(_) => {}
        }
    }
    (formula, text)
}

#[test]
fn paragraph_embedded_display_math_is_display_style_and_real_inline_math_is_not() {
    for source in [TIGHT, "都有\n\\[\nx^2+y\n\\]\n上界", "a $$x$$ b"] {
        let tree = blocks(source);
        assert_eq!(math_kinds(&tree[0]), [MathKind::Display], "{source:?}");
    }
    assert_eq!(math_kinds(&blocks("a $x$ b")[0]), [MathKind::Inline]);
    // Comrak's inline `$` form never becomes display, whatever its neighbours.
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
    let crate::preview::RenderBlockKind::Table(table) = &table[0].kind else {
        panic!("a table block");
    };
    for cell in table.rows.iter().flat_map(|row| &row.cells) {
        for span in cell {
            assert!(
                span.math
                    .as_ref()
                    .is_none_or(|math| math.kind == MathKind::Inline)
            );
        }
    }
}

#[test]
fn the_formula_has_its_own_centered_line_between_the_text_lines() {
    let (pipeline, _) = frame(TIGHT, 900);
    let (formula, text) = chunks(&pipeline, 0);
    let (x, y, width) = formula.unwrap();
    let (blank_pipeline, _) = frame(BLANK, 900);
    let (blank_formula, _) = chunks(&blank_pipeline, 1);
    let blank_formula = blank_formula.unwrap();
    // Same size and horizontal place as the standalone block's formula.
    assert_eq!((x, width), (blank_formula.0, blank_formula.2));
    assert_eq!(text.len(), 2, "one text chunk before and one after");
    assert!(
        text[0].1 + 1.0 < y,
        "the first text line is above the formula"
    );
    assert!(
        y + 1.0 < text[1].1,
        "the second text line is below the formula"
    );
    assert_eq!(text[0].0, text[1].0, "text keeps its left edge");
    let content_width = 900.0 - 2.0 * 24.0;
    assert!((x + width * 0.5 - (24.0 + content_width * 0.5)).abs() < 1.0);
}

#[test]
fn both_delimiters_and_a_same_line_formula_break_the_line() {
    for source in ["都有\n\\[\nx^2+y\n\\]\n上界", "before $$x^2$$ after"] {
        let (pipeline, _) = frame(source, 900);
        let (formula, text) = chunks(&pipeline, 0);
        let (_, y, _) = formula.unwrap();
        assert_eq!(text.len(), 2, "{source:?}");
        assert!(text[0].1 < y && y < text[1].1, "{source:?}");
    }
}

#[test]
fn list_item_and_quote_formulas_are_centered_in_their_own_width() {
    for source in ["- a\n  $$x$$\n  b", "> a\n> $$x$$\n> b"] {
        let (pipeline, _) = frame(source, 900);
        let (formula, text) = chunks(&pipeline, 0);
        let (_, y, _) = formula.unwrap();
        assert!(text.iter().any(|&(_, ty)| ty < y), "{source:?}");
        assert!(text.iter().any(|&(_, ty)| ty > y), "{source:?}");
    }
}

#[test]
fn a_hard_break_beside_the_formula_adds_no_blank_line() {
    let (soft, _) = frame("a\n$$x$$\nb", 900);
    let (hard, _) = frame("a\\\n$$x$$\\\nb", 900);
    let height = |pipeline: &PreviewPipeline| pipeline.layout.as_ref().unwrap().blocks[0].bottom;
    assert_eq!(height(&soft), height(&hard));
}

#[test]
fn the_embedded_formula_has_a_background_but_never_a_copy_rail() {
    let (pipeline, tight) = frame(TIGHT, 900);
    let block = &pipeline.layout.as_ref().unwrap().blocks[0];
    assert!(
        block
            .decorations
            .iter()
            .any(|item| item.role == DecorationRole::EmbeddedMathBackground)
    );
    assert!(
        block
            .decorations
            .iter()
            .all(|item| !item.role.is_block_background())
    );
    assert!(tight.copy_rails().is_empty());
    let (_, blank) = frame(BLANK, 900);
    assert_eq!(
        blank.copy_rails().len(),
        1,
        "the standalone block keeps its rail"
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
        let all = frame.select_all();
        assert_eq!(frame.copy_text(all).unwrap(), source, "{source:?}");
    }
    // The display text keeps the soft breaks (spaces) the layout no longer draws.
    let (_, tight) = frame(TIGHT, 900);
    assert_eq!(tight.text(), "都有 $$\nx^2+y\n$$ 上界");
    // A selection of just the formula copies its TeX, without delimiters.
    let boxed = tight
        .index()
        .boxes()
        .iter()
        .find(|item| item.atomic)
        .unwrap()
        .clone();
    let selection = PreviewSelection {
        anchor: boxed.selection_range.start,
        active: boxed.selection_range.end,
    };
    assert_eq!(tight.copy_text(selection).unwrap(), "\nx^2+y\n");
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
    let (x, y) = (
        link.rect.x + link.rect.width * 0.5,
        link.rect.y + link.rect.height * 0.5,
    );
    assert!(frame.action_at(x, y).is_some());
    let formula = frame
        .index()
        .boxes()
        .iter()
        .find(|item| item.atomic)
        .unwrap();
    assert!(
        link.rect.y > formula.rect.bottom(),
        "the link is on the line below"
    );
}
