//! Regression evidence for preview scrolling with the production image adapter.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use super::*;
use std::sync::Arc;
use std::time::Instant;
use stickymd_core::LineEnding;

struct NoLocalImages;

fn require_release() {
    if cfg!(debug_assertions) {
        panic!("run this benchmark with --release");
    }
}

impl PreviewImageSource for NoLocalImages {
    fn inspect(&self, _: &str) -> Result<Option<crate::image::ImageMetadata>, String> {
        panic!("this source contains no local image nodes")
    }

    fn load(&self, _: &str) -> Result<Option<Vec<u8>>, String> {
        panic!("this source contains no local image nodes")
    }
}

fn text_pipeline(bytes: usize, suffix: &str) -> PreviewPipeline {
    let line = "中文 Rust preview scrolling without local images.\n\n";
    let text = line.repeat(bytes.div_ceil(line.len())) + suffix;
    let snapshot = DocumentSnapshot {
        text: Arc::from(text),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let mut pipeline = PreviewPipeline::new();
    pipeline
        .build_with_image_source(
            &snapshot,
            800,
            300,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
            Some(&NoLocalImages),
        )
        .unwrap();
    pipeline
}

#[test]
fn scrolling_without_local_images_never_relayouts_with_an_image_adapter() {
    for suffix in [
        "",
        "![remote](https://example.com/image.png)",
        "`![literal](local.png)`",
    ] {
        let mut pipeline = text_pipeline(20 * 1024, suffix);
        let before = pipeline.counters();
        for scroll in [2_000.0, 4_000.0, 0.0] {
            let frame = pipeline
                .paint_with_image_source(
                    Generation::initial(),
                    300,
                    scroll,
                    PreviewSelection::default(),
                    PreviewTheme::Light,
                    Some(&NoLocalImages),
                )
                .unwrap();
            assert_eq!(frame.scroll_y(), scroll);
        }
        assert_eq!(
            pipeline.counters().layouts,
            before.layouts,
            "suffix={suffix}"
        );
        assert_eq!(pipeline.counters().parses, before.parses);
    }
}

#[test]
#[ignore = "Release-only production-adapter scrolling benchmark"]
fn phase5_preview_release_baseline_image_adapter_scroll() {
    require_release();
    for bytes in [100 * 1024, 1024 * 1024] {
        let mut pipeline = text_pipeline(bytes, "");
        let mut samples = Vec::with_capacity(30);
        let before = pipeline.counters();
        for index in 0..33 {
            let started = Instant::now();
            std::hint::black_box(
                pipeline
                    .paint_with_image_source(
                        Generation::initial(),
                        300,
                        (index % 2) as f32 * 4_000.0,
                        PreviewSelection::default(),
                        PreviewTheme::Light,
                        Some(&NoLocalImages),
                    )
                    .unwrap(),
            );
            if index >= 3 {
                samples.push(started.elapsed());
            }
        }
        samples.sort_unstable();
        eprintln!(
            "image_adapter_text_scroll bytes={bytes} samples=30 median={:?} p95={:?} max={:?} added_layouts={}",
            samples[15],
            samples[28],
            samples[29],
            pipeline.counters().layouts - before.layouts,
        );
    }
}

fn code_pipeline(rows: usize) -> PreviewPipeline {
    let source = format!(
        "```text\n{}```\n",
        "中文 Rust long code block line 0123456789\n".repeat(rows)
    );
    let mut pipeline = PreviewPipeline::new();
    pipeline
        .build(
            &DocumentSnapshot {
                text: Arc::from(source),
                generation: Generation::initial(),
                line_ending: LineEnding::Lf,
            },
            800,
            300,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    pipeline
}

#[test]
fn offscreen_code_glyphs_are_not_rasterized() {
    let source = format!(
        "```text\n{}{}\n```",
        "visible\n".repeat(200),
        (0..400)
            .map(|index| char::from_u32(0x4e00 + index).unwrap())
            .collect::<String>()
    );
    let mut pipeline = PreviewPipeline::new();
    pipeline
        .build(
            &DocumentSnapshot {
                text: Arc::from(source),
                generation: Generation::initial(),
                line_ending: LineEnding::Lf,
            },
            800,
            300,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    let cached = pipeline.swash_cache.image_cache.len();
    eprintln!("long code block first viewport cached_glyphs={cached}");
    assert!(cached < 32, "offscreen glyphs filled the cache: {cached}");
}

#[test]
fn preview_zoom_does_not_accumulate_obsolete_glyph_rasters() {
    let mut pipeline = text_pipeline(1024, "");
    let initial = pipeline.swash_cache.image_cache.len();
    for scale in [1.25, 1.5, 2.0, 2.5, 3.0, 1.0] {
        pipeline
            .relayout(
                Generation::initial(),
                800,
                300,
                scale,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
    }
    let final_count = pipeline.swash_cache.image_cache.len();
    eprintln!("preview glyph rasters initial={initial} after_zoom_cycle={final_count}");
    assert_eq!(final_count, initial);
}

#[test]
fn releasing_preview_rasters_also_releases_text_glyphs() {
    for document_release in [false, true] {
        let mut pipeline = text_pipeline(1024, "");
        assert!(!pipeline.swash_cache.image_cache.is_empty());
        if document_release {
            pipeline.release_document_projection();
        } else {
            pipeline.release_raster_caches();
        }
        assert!(pipeline.swash_cache.image_cache.is_empty());
        assert!(pipeline.swash_cache.outline_command_cache.is_empty());
    }
}

#[test]
#[ignore = "Release-only long-block viewport painting benchmark"]
fn phase5_preview_release_baseline_long_code_scroll() {
    require_release();
    for rows in [1_000, 5_000] {
        let mut pipeline = code_pipeline(rows);
        let mut samples = Vec::new();
        for index in 0..33 {
            let started = Instant::now();
            std::hint::black_box(
                pipeline
                    .paint(
                        Generation::initial(),
                        300,
                        5_000.0 + (index % 2) as f32 * 1_000.0,
                        PreviewSelection::default(),
                        PreviewTheme::Light,
                    )
                    .unwrap(),
            );
            if index >= 3 {
                samples.push(started.elapsed());
            }
        }
        samples.sort_unstable();
        eprintln!(
            "long_code_scroll rows={rows} samples=30 median={:?} p95={:?} max={:?}",
            samples[15], samples[28], samples[29]
        );
    }
}

#[test]
fn local_image_detection_uses_semantics_including_table_cells() {
    for (source, expected) in [
        ("![local](image.png)", true),
        ("| image |\n| --- |\n| ![local](image.png) |", true),
        ("> - ![local](image.png)", true),
        ("`![literal](image.png)`", false),
        ("![remote](https://example.com/image.png)", false),
        ("```\n![literal](image.png)\n```", false),
    ] {
        let snapshot = DocumentSnapshot {
            text: Arc::from(source),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let owned = PreviewParser.parse(&snapshot).unwrap();
        let tree = RenderTreeBuilder.build(&owned);
        assert_eq!(tree.has_local_images(), expected, "{source}");
    }
}

#[test]
fn formula_pressure_keeps_live_layout_bounded_and_preserves_source() {
    let formulas: Vec<_> = (0..100)
        .map(|index| {
            format!("$$\\frac{{a+b+c+d+e+f+g+h+i+j+k+l+m+n+{index}}}{{1+2+3+4+5+6+7+8+9}}$$")
        })
        .collect();
    let snapshot = DocumentSnapshot {
        text: Arc::from(formulas.join("\n\n")),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let mut pipeline = PreviewPipeline::new();
    let frame = pipeline
        .build(
            &snapshot,
            800,
            300,
            3.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    let copied = frame.copy_selection(frame.select_all()).unwrap();
    assert!(
        formulas
            .iter()
            .all(|formula| copied.contains(formula.as_str()))
    );
    let rasterizations = pipeline.math_counters().rasterizations;
    assert!(rasterizations > 0 && rasterizations < formulas.len() as u64);
    let live_bytes: usize = pipeline
        .layout
        .as_ref()
        .unwrap()
        .blocks
        .iter()
        .flat_map(|block| &block.chunks)
        .filter_map(|chunk| match &chunk.content {
            super::super::layout::LayoutContent::Math(raster) => Some(raster.pixels.len()),
            _ => None,
        })
        .sum();
    assert!(live_bytes <= 8 * 1024 * 1024);
    assert!(pipeline.math_engine.cache_sizes().2 >= live_bytes);
    let bottom = pipeline
        .paint(
            Generation::initial(),
            300,
            f32::MAX,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    assert!(bottom.index().boxes().iter().any(|item| {
        item.tooltip
            .as_deref()
            .is_some_and(|tooltip| tooltip.contains("safety budget"))
    }));
    pipeline.release_document_projection();
    assert_eq!(pipeline.math_engine.cache_sizes().2, 0);
    pipeline
        .build(
            &DocumentSnapshot {
                text: Arc::from("$y$"),
                ..snapshot
            },
            800,
            300,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    assert_eq!(pipeline.math_counters().rasterizations, rasterizations + 1);
}
