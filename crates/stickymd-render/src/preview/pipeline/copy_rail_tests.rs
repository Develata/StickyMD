//! The copy rails a frame produces: where they stand, when they are left out, what a click on one
//! copies, and how they are painted over the frame (P17-R01, R02, R03, R05).
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-rail

use super::*;
use crate::preview::layout::DecorationRole;
use crate::preview::{COPY_RAIL_WIDTH_DIP, PreviewCopyRail, PreviewRect};
use std::sync::Arc;

fn build(
    pipeline: &mut PreviewPipeline,
    text: &str,
    generation: Generation,
    (width, height): (u32, u32),
    scale: f32,
    scroll_y: f32,
) -> PreviewFrame {
    pipeline
        .build(
            &DocumentSnapshot {
                text: Arc::from(text),
                generation,
                line_ending: stickymd_core::LineEnding::Lf,
            },
            width,
            height,
            scale,
            scroll_y,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap()
}

fn frame(text: &str, width: u32, scale: f32) -> (PreviewPipeline, PreviewFrame) {
    let mut pipeline = PreviewPipeline::new();
    let frame = build(
        &mut pipeline,
        text,
        Generation::initial(),
        (width, 4000),
        scale,
        0.0,
    );
    (pipeline, frame)
}

/// The code or math background of each laid-out block, by render block index.
fn backgrounds(pipeline: &PreviewPipeline) -> Vec<Option<PreviewRect>> {
    pipeline
        .layout
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .map(|block| {
            block
                .decorations
                .iter()
                .find(|decoration| {
                    matches!(
                        decoration.role,
                        DecorationRole::CodeBackground | DecorationRole::MathBackground
                    )
                })
                .map(|decoration| decoration.rect)
        })
        .collect()
}

fn overlaps(a: PreviewRect, b: PreviewRect) -> bool {
    a.x < b.right() && b.x < a.right() && a.y < b.bottom() && b.y < a.bottom()
}

const WIDE_LINE: &str = "let overflow = [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25];";

fn corpus() -> String {
    format!(
        "# Title\n\nA paragraph with $x$ inline.\n\n    indented code\n\n```rust\nfn main() {{}}\n```\n\n$$x^2$$\n\n\
         <div>\nraw html\n</div>\n\n> ```\n> quoted\n> ```\n\n- item\n\n  ```\n  listed\n  ```\n\n\
         | a | b |\n|---|---|\n| 1 | 2 |\n\n```\n{WIDE_LINE}\n```\n\n\
         $$\\frac{{{}z}}{{2}}$$\n\n```\n```\n\nThe end.\n",
        "a+b+c+d+e+f+g+h+".repeat(4)
    )
}

/// P17-R01: one rail per code, Raw HTML and display-math block, standing on the background's
/// right edge at its height; none for any other block. Beside an overflowing block it ends with
/// the background, above the bottom of the horizontal track.
#[test]
fn a_rail_stands_on_the_right_edge_of_each_code_and_math_background() {
    let (pipeline, frame) = frame(&corpus(), 520, 1.0);
    let backgrounds = backgrounds(&pipeline);
    let rails = frame.copy_rails();
    assert_eq!(
        rails.len(),
        backgrounds.iter().flatten().count(),
        "every code, HTML and display-math block has a rail, and nothing else does"
    );
    // Six fenced or indented code blocks, one HTML block, two display formulas.
    assert_eq!(rails.len(), 9);
    for rail in rails {
        let background = backgrounds[rail.block()].expect("a rail stands beside a background");
        assert_eq!(
            rail.rect(),
            PreviewRect {
                x: background.right(),
                y: background.y,
                width: COPY_RAIL_WIDTH_DIP,
                height: background.height,
            }
        );
    }
    let layout = pipeline.layout.as_ref().unwrap();
    let overflowing: Vec<_> = rails
        .iter()
        .filter_map(|rail| {
            layout.blocks[rail.block()]
                .horizontal
                .map(|horizontal| (rail, horizontal))
        })
        .collect();
    assert_eq!(
        overflowing.len(),
        2,
        "the wide code line and the wide formula"
    );
    for (rail, horizontal) in overflowing {
        let track_bottom = horizontal.viewport.bottom() + horizontal.bar_height;
        assert!(rail.rect().bottom() < track_bottom);
    }
}

/// P17-R02: at every width and scale, each rail lies wholly in the frame and overlaps no text,
/// formula, image, decoration, horizontal track or other rail. A rail that would not fit is left
/// out, and the sweep reaches both cases.
#[test]
fn a_rail_overlaps_nothing_and_is_left_out_when_it_does_not_fit() {
    let deep = "- a\n  - b\n    - c\n      - d\n\n        ```\n        deep\n        ```\n";
    let (mut shown, mut hidden) = (0, 0);
    for text in [corpus(), deep.to_owned()] {
        for (width, scale) in [(520, 1.0), (320, 1.5), (180, 1.0), (180, 3.0), (120, 2.0)] {
            let (pipeline, frame) = frame(&text, width, scale);
            let layout = pipeline.layout.as_ref().unwrap();
            let rails = frame.copy_rails();
            for (block, background) in backgrounds(&pipeline).into_iter().enumerate() {
                let Some(background) = background else {
                    continue;
                };
                let fits = background.right() >= 0.0
                    && background.right() + COPY_RAIL_WIDTH_DIP * scale <= width as f32;
                let rail = rails.iter().find(|rail| rail.block() == block);
                assert_eq!(
                    rail.is_some(),
                    fits,
                    "block {block} at {width} px, {scale}x"
                );
                if fits {
                    shown += 1;
                } else {
                    hidden += 1;
                }
            }
            let mut occupied: Vec<PreviewRect> =
                frame.index().boxes().iter().map(|item| item.rect).collect();
            for block in &layout.blocks {
                occupied.extend(block.decorations.iter().map(|decoration| decoration.rect));
                occupied.extend(block.horizontal.map(|horizontal| PreviewRect {
                    y: horizontal.viewport.bottom(),
                    height: horizontal.bar_height,
                    ..horizontal.viewport
                }));
            }
            for (index, rail) in rails.iter().enumerate() {
                let rect = rail.rect();
                assert!(rect.x >= 0.0 && rect.right() <= width as f32);
                for other in &occupied {
                    assert!(!overlaps(rect, *other), "{rect:?} overlaps {other:?}");
                }
                for other in &rails[index + 1..] {
                    assert!(!overlaps(rect, other.rect()));
                }
            }
        }
    }
    assert!(shown > 0 && hidden > 0, "shown {shown}, hidden {hidden}");
}

/// P17-R03 (Render): a click copies the object's whole content by rule 1, without fences, quote
/// or list prefixes or formula delimiters, from the frame's own snapshot.
#[test]
fn a_rail_copies_its_whole_object() {
    for (source, expected) in [
        ("```rust\nfn a() {}\nlet b;\n```", "fn a() {}\nlet b;\n"),
        ("> ```\n> one\n> two\n> ```", "one\ntwo\n"),
        ("- item\n\n  ```\n  one\n  two\n  ```", "one\ntwo\n"),
        ("    indented\n    code", "indented\ncode\n"),
        ("<div>\n<b>raw</b>\n</div>", "<div>\n<b>raw</b>\n</div>"),
        ("$$x^2$$", "x^2"),
        // Rule 1 copies Comrak's literal as it is, newlines inside the delimiters included.
        ("$$\n\\frac{1}{2}\n$$", "\n\\frac{1}{2}\n"),
        ("```\n```", ""),
    ] {
        let (_, frame) = frame(source, 520, 1.0);
        let [rail] = frame.copy_rails() else {
            panic!("{source:?} shows one rail, not {:?}", frame.copy_rails());
        };
        assert_eq!(
            frame.copy_rail_text(*rail).as_deref(),
            Some(expected),
            "{source:?}"
        );
    }
}

/// P17-R05 (Render): a rail answers only to the frame that shows it. A frame of another
/// generation or width copies nothing for it, while its own frame still copies its own
/// snapshot.
#[test]
fn a_rail_copies_only_from_its_own_frame() {
    let mut pipeline = PreviewPipeline::new();
    let old = build(
        &mut pipeline,
        "```\nold\n```",
        Generation::initial(),
        (520, 400),
        1.0,
        0.0,
    );
    let rail = old.copy_rails()[0];
    let newer = build(
        &mut pipeline,
        "```\nnew\n```",
        Generation::initial().checked_next().unwrap(),
        (520, 400),
        1.0,
        0.0,
    );
    let narrower = build(
        &mut pipeline,
        "```\nold\n```",
        Generation::initial(),
        (400, 400),
        1.0,
        0.0,
    );
    assert_eq!(old.copy_rail_text(rail).as_deref(), Some("old\n"));
    assert_eq!(newer.copy_rail_text(rail), None);
    assert_eq!(narrower.copy_rail_text(rail), None);
    assert_eq!(
        newer.copy_rail_text(newer.copy_rails()[0]).as_deref(),
        Some("new\n")
    );
}

/// The visible rectangle is what a click hits, half-open; the background beside it is not.
#[test]
fn a_rail_is_hit_on_its_visible_rectangle() {
    let (_, frame) = frame("```\ncode\n```", 520, 1.0);
    let rail = frame.copy_rails()[0];
    let rect = rail.rect();
    let middle = rect.y + rect.height / 2.0;
    assert_eq!(frame.copy_rail_at(rect.x, middle), Some(rail));
    assert_eq!(frame.copy_rail_at(rect.right() - 0.5, rect.y), Some(rail));
    assert_eq!(frame.copy_rail_at(rect.right(), middle), None);
    assert_eq!(frame.copy_rail_at(rect.x - 0.5, middle), None);
    assert_eq!(frame.copy_rail_at(rect.x, rect.bottom()), None);
}

/// Painting the rails over a blitted frame changes only pixels inside the rails' visible parts,
/// clipped to the frame; the hovered rail is painted differently from one at rest.
#[test]
fn rails_paint_only_their_visible_part_and_brighten_when_hovered() {
    let mut pipeline = PreviewPipeline::new();
    // Scrolled so that the first block's rail is cut by the frame's top edge.
    let text = "```\none\ntwo\nthree\nfour\n```\n\nafter";
    let frame = build(
        &mut pipeline,
        text,
        Generation::initial(),
        (300, 60),
        1.0,
        40.0,
    );
    let rail: PreviewCopyRail = frame.copy_rails()[0];
    assert!(rail.rect().y < frame.scroll_y() && rail.rect().bottom() > frame.scroll_y());
    let (origin_x, origin_y) = (10, 20);
    let mut base = tiny_skia::Pixmap::new(400, 300).unwrap();
    frame.blit_to(&mut base, origin_x, origin_y);
    let paint = |hovered| {
        let mut target = base.clone();
        frame.paint_copy_rails(&mut target, origin_x, origin_y, hovered);
        target
    };
    let (rest, hovered) = (paint(None), paint(Some(rail)));
    let rect = rail.rect();
    let inside = |x: u32, y: u32| {
        let (x, y) = (x as f32 + 0.5, y as f32 + 0.5);
        let local_y = y - origin_y as f32 + frame.scroll_y();
        let local_x = x - origin_x as f32;
        // Anti-aliased edges may touch the pixels that straddle the rail's edges.
        local_x > rect.x - 1.0
            && local_x < rect.right() + 1.0
            && local_y > rect.y.max(frame.scroll_y()) - 1.0
            && local_y < rect.bottom() + 1.0
            && y >= origin_y as f32
    };
    let mut changed = 0;
    for y in 0..base.height() {
        for x in 0..base.width() {
            let before = base.pixel(x, y).unwrap();
            for painted in [&rest, &hovered] {
                if painted.pixel(x, y).unwrap() != before {
                    assert!(inside(x, y), "pixel ({x}, {y}) changed outside the rail");
                    changed += 1;
                }
            }
        }
    }
    assert!(changed > 0);
    let (x, y) = (origin_x + (rect.x + rect.width / 2.0) as u32, origin_y + 5);
    assert_ne!(rest.pixel(x, y), hovered.pixel(x, y));
}

/// A code block whose background alone reaches into the viewport, at the bottom edge (the
/// background starts 4 DIP above the block) or at the top edge (it ends 4 DIP below), is still
/// painted with its rail, cut to what shows.
#[test]
fn a_background_that_alone_reaches_the_viewport_keeps_its_rail() {
    let text = format!(
        "{}```\ncode\n```\n\n{}",
        "para\n\n".repeat(20),
        "para\n\n".repeat(40)
    );
    let mut pipeline = PreviewPipeline::new();
    build(
        &mut pipeline,
        &text,
        Generation::initial(),
        (520, 200),
        1.0,
        0.0,
    );
    let (index, block) = pipeline
        .layout
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .enumerate()
        .find(|(_, block)| {
            block
                .decorations
                .iter()
                .any(|decoration| decoration.role == DecorationRole::CodeBackground)
        })
        .map(|(index, block)| (index, (block.top, block.bottom)))
        .unwrap();
    let (top, bottom) = block;
    for (scroll_y, at_y) in [
        // The viewport ends 2 DIP above the block, inside the background's top.
        (top - 2.0 - 200.0, top - 3.0),
        // The viewport starts 2 DIP below the block, inside the background's bottom.
        (bottom + 2.0, bottom + 3.0),
    ] {
        let frame = build(
            &mut pipeline,
            &text,
            Generation::initial(),
            (520, 200),
            1.0,
            scroll_y,
        );
        assert_eq!(frame.scroll_y(), scroll_y);
        let rail = frame
            .copy_rails()
            .iter()
            .find(|rail| rail.block() == index)
            .copied()
            .unwrap_or_else(|| panic!("no rail at scroll {scroll_y}"));
        let x = rail.rect().x + 1.0;
        assert_eq!(frame.copy_rail_at(x, at_y), Some(rail));
    }
}
