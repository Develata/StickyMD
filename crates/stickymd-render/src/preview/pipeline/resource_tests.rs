//! Same-fixture local resource diagnostics; no runtime policy or qualification authority.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#preview-resource-assessment

use super::*;
use std::sync::Arc;
use std::time::Instant;
use stickymd_core::LineEnding;

pub(super) fn dense_formulas(count: usize) -> DocumentSnapshot {
    DocumentSnapshot {
        text: Arc::from(
            (0..count)
                .map(|i| {
                    format!(
                        "$$\\frac{{a+b+c+d+e+f+g+h+i+j+k+l+m+n+{i}}}{{1+2+3+4+5+6+7+8+9}}$$\n\n"
                    )
                })
                .collect::<String>(),
        ),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    }
}

#[test]
#[ignore = "local Release formula budget comparison; run serially with --nocapture"]
fn phase6_preview_resource_formula_budget_diagnostic() {
    assert!(
        !std::hint::black_box(cfg!(debug_assertions)),
        "run this diagnostic in Release"
    );
    for count in [100, 2_000] {
        let snapshot = dense_formulas(count);
        for mib in [8, 16, 32, 64] {
            let mut pipeline = PreviewPipeline::new();
            pipeline
                .math_engine
                .set_raster_budget_for_test(mib * 1024 * 1024);
            let started = Instant::now();
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
            let build_ms = started.elapsed().as_secs_f64() * 1000.0;
            let height = frame.document_height();
            let initial_rasters = pipeline.math_counters().rasterizations;
            let initial_bytes = pipeline.math_engine.cache_sizes().2;
            let started = Instant::now();
            let mut bottom_errors = 0;
            for y in [height / 2.0, f32::MAX, 0.0, f32::MAX] {
                let frame = pipeline
                    .paint(
                        snapshot.generation,
                        300,
                        y,
                        PreviewSelection::default(),
                        PreviewTheme::Light,
                    )
                    .unwrap();
                bottom_errors = frame
                    .index()
                    .boxes()
                    .iter()
                    .filter(|item| item.tooltip.is_some())
                    .count();
                std::hint::black_box(frame.rgba());
            }
            println!(
                "formula_budget count={count} budget_mib={mib} build_ms={build_ms:.3} scroll4_ms={:.3} initial_rasters={initial_rasters} initial_bytes={initial_bytes} final_bytes={} bottom_errors={bottom_errors} height={height:.1}",
                started.elapsed().as_secs_f64() * 1000.0,
                pipeline.math_engine.cache_sizes().2
            );
        }
    }
}

/// Plan 06 `math-raster-lifecycle`: a formula the painter cannot draw (measured at layout, failing
/// at paint, e.g. a font the painter cannot load) stays an error on that formula: the frame still
/// paints, with the math-error marker and the reason as the formula's tooltip, and the text
/// around it.
#[test]
fn a_painter_error_marks_its_formula_and_the_frame_still_paints() {
    let snapshot = DocumentSnapshot {
        text: Arc::from("Before $x^2$ after.\n"),
        generation: stickymd_core::Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let mut pipeline = PreviewPipeline::new();
    pipeline.math_engine.fail_draws_for_test("");
    let frame = pipeline
        .build(
            &snapshot,
            480,
            200,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .expect("a painter error must not fail the frame");
    let error = [190_u8, 73, 55];
    let marked = frame
        .rgba()
        .chunks_exact(4)
        .filter(|pixel| pixel[..3] == error)
        .count();
    assert!(marked > 0, "the formula carries the math-error marker");
    let formula = frame
        .index()
        .boxes()
        .iter()
        .find(|item| item.atomic)
        .expect("the formula keeps its atomic box, so its source stays copyable");
    assert!(
        formula
            .tooltip
            .as_deref()
            .is_some_and(|reason| !reason.is_empty()),
        "the formula's tooltip gives why it failed"
    );
    assert!(
        frame.index().boxes().iter().any(|item| !item.atomic),
        "the text around the formula is laid out"
    );
}

/// The reason goes to the failed formula's own box, matched by identity: of two adjacent formulas
/// only the one that failed carries a tooltip (geometry could make neighbours touch).
#[test]
fn a_painter_error_names_only_the_formula_that_failed() {
    let snapshot = DocumentSnapshot {
        text: Arc::from(
            "Before $a$$b$ after.
",
        ),
        generation: stickymd_core::Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let mut pipeline = PreviewPipeline::new();
    pipeline.math_engine.fail_draws_for_test("b");
    let frame = pipeline
        .build(
            &snapshot,
            480,
            200,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    let mut formulas: Vec<_> = frame
        .index()
        .boxes()
        .iter()
        .filter(|item| item.atomic)
        .collect();
    formulas.sort_by_key(|item| item.selection_range.start);
    assert_eq!(formulas.len(), 2, "two formula boxes");
    assert!(
        formulas[0].tooltip.is_none(),
        "the drawn formula a carries no error"
    );
    assert!(
        formulas[1].tooltip.is_some(),
        "the failed formula b carries its reason"
    );
}

#[test]
fn a_viewport_larger_than_the_raster_cache_still_displays_every_formula() {
    let snapshot = dense_formulas(6);
    let mut pipeline = PreviewPipeline::new();
    pipeline.math_engine.set_raster_budget_for_test(1_024);
    let first = pipeline
        .build(
            &snapshot,
            800,
            1_400,
            3.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    assert_eq!(
        first
            .index()
            .boxes()
            .iter()
            .filter(|item| item.atomic)
            .count(),
        6
    );
    assert!(
        first
            .index()
            .boxes()
            .iter()
            .all(|item| item.tooltip.is_none())
    );
    assert_eq!(pipeline.math_counters().rasterizations, 6);
    assert_eq!(pipeline.math_engine.cache_sizes().2, 0);
    let again = pipeline
        .paint(
            snapshot.generation,
            1_400,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    assert_eq!(first.rgba(), again.rgba());
    assert_eq!(pipeline.counters().layouts, 1);
    assert_eq!(pipeline.math_counters().parse_layout_calls, 6);
    assert_eq!(pipeline.math_counters().rasterizations, 12);
    assert_eq!(pipeline.math_engine.cache_sizes().2, 0);
}

#[test]
#[ignore = "local Release fixed-scale cache attribution; run serially with --nocapture"]
fn phase10_preview_resource_glyph_growth_diagnostic() {
    assert!(
        !std::hint::black_box(cfg!(debug_assertions)),
        "run this diagnostic in Release"
    );
    let mut pipeline = PreviewPipeline::new();
    for batch in 0..20 {
        let mut text = (0..100)
            .filter_map(|n| char::from_u32(0x4e00 + batch * 100 + n))
            .collect::<String>();
        text.extend((0..4).filter_map(|n| char::from_u32(0x1f600 + batch * 4 + n)));
        text.push_str(&format!("\n\n$x_{{{batch}}} + \\frac{{1}}{{2}}$"));
        let snapshot = DocumentSnapshot {
            text: Arc::from(text),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        pipeline
            .build(
                &snapshot,
                800,
                600,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        if batch % 5 == 4 {
            let bytes: usize = pipeline
                .swash_cache
                .image_cache
                .values()
                .flatten()
                .map(|image| image.data.len())
                .sum();
            println!(
                "glyph_growth batch={} entries={} pixel_bytes={bytes} math_sizes={:?}",
                batch + 1,
                pipeline.swash_cache.image_cache.len(),
                pipeline.math_engine.cache_sizes()
            );
        }
    }
    pipeline.release_document_projection();
    println!(
        "glyph_growth released_entries={} math_sizes={:?}",
        pipeline.swash_cache.image_cache.len(),
        pipeline.math_engine.cache_sizes()
    );
}

#[test]
#[ignore = "local retained-allocation attribution, not latency or desktop qualification"]
fn phase10_preview_resource_cache_retention_diagnostic() {
    let mut pipeline = PreviewPipeline::new();
    for batch in 0..120 {
        let snapshot = DocumentSnapshot {
            text: Arc::from(
                (0..100)
                    .filter_map(|n| char::from_u32(0x4e00 + batch * 100 + n))
                    .collect::<String>(),
            ),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        pipeline
            .build(
                &snapshot,
                800,
                600,
                3.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        if batch % 30 == 29 {
            let bytes: usize = pipeline
                .swash_cache
                .image_cache
                .values()
                .flatten()
                .map(|image| image.data.len())
                .sum();
            println!(
                "cache_retention batches={} scale=3 glyph_entries={} glyph_pixels={bytes}",
                batch + 1,
                pipeline.swash_cache.image_cache.len()
            );
        }
    }
    let mut engine = crate::math::MathEngine::new();
    for n in 0..512 {
        let source = format!("\\frac{{{}+{n}}}{{1}}", "x+".repeat(400));
        engine
            .measure(source, crate::math::MathKind::Inline, 17.0)
            .unwrap();
        if n % 128 == 127 {
            println!(
                "cache_retention formulas={} retained_layout_bytes={}",
                n + 1,
                engine.layout_bytes()
            );
        }
    }
}

#[test]
fn phase10_glyph_pressure_purge_rebuilds_identical_pixels_without_layout_work() {
    let mut pipeline = PreviewPipeline::new();
    let snapshot = DocumentSnapshot {
        text: Arc::from("中文 👩‍💻 e\u{301} **bold** $x^2$"),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    };
    let first = pipeline
        .build(
            &snapshot,
            500,
            200,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
    let counters = pipeline.counters();
    pipeline.glyph_budget = crate::glyph_cache::GlyphCacheBudget::for_test(1);
    for _ in 0..2 {
        let repainted = pipeline
            .paint(
                snapshot.generation,
                200,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        assert_eq!(repainted.rgba(), first.rgba());
        assert!(pipeline.swash_cache.image_cache.is_empty());
        assert_eq!(pipeline.counters().layouts, counters.layouts);
    }
}

#[test]
fn phase10_large_formula_layouts_obey_byte_budget_and_rebuild_the_same_metrics() {
    let mut engine = crate::math::MathEngine::new();
    let first = format!("\\frac{{{}+0}}{{1}}", "x+".repeat(400));
    let metrics = engine
        .measure(first.as_str(), crate::math::MathKind::Inline, 17.0)
        .unwrap();
    for n in 1..128 {
        engine
            .measure(
                format!("\\frac{{{}+{n}}}{{1}}", "x+".repeat(400)),
                crate::math::MathKind::Inline,
                17.0,
            )
            .unwrap();
        assert!(engine.layout_bytes() <= 8 * 1024 * 1024);
    }
    assert!(engine.cache_sizes().0 < 128);
    assert!(engine.counters().layout_evictions > 0);
    let before = engine.counters().parse_layout_calls;
    assert_eq!(
        engine
            .measure(first, crate::math::MathKind::Inline, 17.0)
            .unwrap(),
        metrics
    );
    assert_eq!(engine.counters().parse_layout_calls, before + 1);
}
