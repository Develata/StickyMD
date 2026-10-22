//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-horizontal-scroll

use super::*;
use crate::preview::PreviewHorizontalOffsets;
use std::sync::Arc;

fn build(pipeline: &mut PreviewPipeline, text: &str, width: u32, scale: f32) -> PreviewFrame {
    pipeline
        .build(
            &DocumentSnapshot {
                text: Arc::from(text),
                generation: Generation::initial(),
                line_ending: stickymd_core::LineEnding::Lf,
            },
            width,
            600,
            scale,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap()
}

#[test]
fn long_code_scrolls_without_wrapping_or_changing_copy_and_hit_ranges() {
    let mut pipeline = PreviewPipeline::new();
    let line = format!("{}END", "中ab 👩‍💻 e\u{301} ".repeat(60));
    let text = format!("```\n{line}\nshort\n```\n\nFollowing paragraph");
    for scale in [0.75, 1.0, 3.0] {
        let first = build(&mut pipeline, &text, 360, scale);
        let horizontal = pipeline.layout.as_ref().unwrap().blocks[0]
            .horizontal
            .unwrap();
        let target = first
            .horizontal_target_at(horizontal.viewport.x + 1.0, horizontal.viewport.y + 1.0)
            .unwrap();
        assert!(target.max_offset() > 1_000.0);
        assert!(
            horizontal.viewport.height < 50.0 * scale,
            "only two original lines"
        );
        assert!(
            first.index().boxes().len() < 150,
            "offscreen clusters must not be retained in the frame"
        );
        let text_before = first.copy_selection(first.select_all()).unwrap().to_owned();
        let counters = pipeline.counters();
        pipeline.set_horizontal_offsets(
            PreviewHorizontalOffsets::default().with_offset(target, target.max_offset()),
        );
        let end = pipeline
            .paint(
                first.generation(),
                600,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        assert_eq!(pipeline.counters().layouts, counters.layouts);
        assert_eq!(first.document_height(), end.document_height());
        assert_eq!(text_before, end.copy_selection(end.select_all()).unwrap());
        assert!(text_before.starts_with(&line));
        assert_ne!(first.rgba(), end.rgba());
        let last = end
            .index()
            .boxes()
            .iter()
            .filter(|item| item.rect.y < horizontal.viewport.bottom())
            .max_by_key(|item| item.selection_range.end)
            .unwrap();
        assert_eq!(&end.text()[last.selection_range.clone()], "D");
        assert!(
            last.selection_range
                .contains(&end.hit_test(last.rect.x + 0.1, last.rect.y + 1.0))
        );
        assert!(
            end.index()
                .boxes()
                .iter()
                .filter(|item| item.rect.y < horizontal.viewport.bottom())
                .all(|item| item.rect.x >= horizontal.viewport.x
                    && item.rect.right() <= horizontal.viewport.right() + 0.01)
        );
        assert!(
            end.action_at(horizontal.viewport.x - 1.0, horizontal.viewport.y + 1.0)
                .is_none()
        );
        // A resize invalidates block offsets even when the same index is reused.
        let resized = pipeline
            .relayout(
                first.generation(),
                400,
                600,
                scale,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        assert_eq!(
            resized
                .horizontal_target_at(horizontal.viewport.x + 1.0, horizontal.viewport.y + 1.0)
                .unwrap()
                .offset(),
            0.0
        );
        pipeline.set_horizontal_offsets(Default::default());
    }
}

#[test]
fn wide_table_clips_hidden_actions_and_scrolls_math_and_links_together() {
    let mut pipeline = PreviewPipeline::new();
    let source = "| First column | Second column | Third column | Last column |\n| :- | :-: | -: | :- |\n| alpha | beta | $x^2$ | [visible link](https://example.com) $y$ |\n\nparagraph";
    let first = build(&mut pipeline, source, 270, 1.0);
    let table = &pipeline.layout.as_ref().unwrap().blocks[0];
    let horizontal = table.horizontal.unwrap();
    let target = first.horizontal_target_at(25.0, 25.0).unwrap();
    assert!(
        first
            .index()
            .boxes()
            .iter()
            .all(|item| item.action.is_none())
    );
    pipeline.set_horizontal_offsets(
        PreviewHorizontalOffsets::default().with_offset(target, target.max_offset()),
    );
    let end = pipeline
        .paint(
            first.generation(),
            600,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    let link = end
        .index()
        .boxes()
        .iter()
        .find(|item| item.action.is_some())
        .unwrap();
    assert!(
        end.action_at(link.rect.x + 1.0, link.rect.y + 1.0)
            .is_some()
    );
    assert!(
        end.action_at(horizontal.viewport.x - 1.0, link.rect.y + 1.0)
            .is_none()
    );
    assert!(end.index().boxes().iter().any(|item| item.atomic));
    assert_eq!(first.text(), end.text());
    assert!(end.text().contains("alpha\tbeta\t$x^2$\tvisible link $y$"));
    assert_eq!(pipeline.counters().layouts, 1);
    let target = end.horizontal_target_at(25.0, 25.0).unwrap();
    assert_eq!(target.thumb().right(), target.track().right());
    assert_eq!(target.offset_after_drag(-target.track().width), 0.0);
    assert!(target.offset_after_page(target.track().x) < target.offset());
}

#[test]
#[ignore = "Release long-code horizontal paint diagnostic"]
fn phase5_preview_release_baseline_horizontal_long_line() {
    use std::time::Instant;
    assert!(
        !std::hint::black_box(cfg!(debug_assertions)),
        "run this diagnostic in Release"
    );
    let mut pipeline = PreviewPipeline::new();
    let text = format!("```\n{}END\n```", "long_code_123 中文 ".repeat(25_000));
    let started = Instant::now();
    let frame = build(&mut pipeline, &text, 800, 1.0);
    let build_ms = started.elapsed().as_secs_f64() * 1_000.0;
    let target = frame.horizontal_target_at(25.0, 25.0).unwrap();
    let mut samples = Vec::new();
    for n in 0..25 {
        pipeline.set_horizontal_offsets(
            PreviewHorizontalOffsets::default()
                .with_offset(target, target.max_offset() * (n % 3) as f32 / 2.0),
        );
        let started = Instant::now();
        let frame = pipeline
            .paint(
                frame.generation(),
                600,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        assert!(frame.index().boxes().len() < 160);
        samples.push(started.elapsed().as_secs_f64() * 1_000.0);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "horizontal_long_line bytes={} build_ms={build_ms:.3} paint_p50_ms={:.3} paint_p95_ms={:.3} glyph_entries={}",
        text.len(),
        samples[12],
        samples[23],
        pipeline.swash_cache.image_cache.len()
    );
    assert_eq!(pipeline.counters().layouts, 1);
    // Attribute pathological single-line cost to shaping versus viewport paint.
    // Compare both wrap policies with the same already-loaded fonts and source.
    for wrap in [cosmic_text::Wrap::WordOrGlyph, cosmic_text::Wrap::None] {
        let started = Instant::now();
        let chunk = super::super::text_layout::make_text_chunk(
            &mut pipeline.font_system,
            &pipeline.fonts,
            &pipeline.tree.as_ref().unwrap().blocks[0].spans,
            12.0,
            12.0,
            776.0,
            cosmic_text::Metrics::new(15.0, 23.25),
            cosmic_text::Align::Left,
            wrap,
            &mut String::new(),
            &mut super::super::text_layout::TextLayoutCache::default(),
        );
        println!(
            "horizontal_long_line shaping_wrap={wrap:?} shape_ms={:.3} height={}",
            started.elapsed().as_secs_f64() * 1_000.0,
            chunk.height
        );
        std::hint::black_box(chunk);
    }
}

#[test]
fn list_marker_stays_fixed_while_its_code_block_scrolls() {
    let mut pipeline = PreviewPipeline::new();
    let source = format!("- ```\n  {}\n  ```", "long_code ".repeat(60));
    let first = build(&mut pipeline, &source, 360, 1.0);
    let horizontal = pipeline.layout.as_ref().unwrap().blocks[0]
        .horizontal
        .unwrap();
    let target = first
        .horizontal_target_at(horizontal.viewport.x + 1.0, horizontal.viewport.y + 1.0)
        .unwrap();
    let markers = |frame: &PreviewFrame| {
        frame
            .index()
            .boxes()
            .iter()
            .filter(|item| item.rect.right() <= horizontal.viewport.x)
            .map(|item| (item.rect, item.selection_range.clone()))
            .collect::<Vec<_>>()
    };
    let expected = markers(&first);
    assert!(!expected.is_empty());
    pipeline.set_horizontal_offsets(PreviewHorizontalOffsets::default().with_offset(target, 150.0));
    let end = pipeline
        .paint(
            first.generation(),
            600,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    assert_eq!(markers(&end), expected);
    assert_eq!(first.text(), end.text());
    for row in 0..600_usize {
        for x in 0..(horizontal.viewport.x - 8.0) as usize {
            let offset = (row * 360 + x) * 4;
            assert_eq!(
                first.rgba()[offset..offset + 4],
                end.rgba()[offset..offset + 4]
            );
        }
    }
}
