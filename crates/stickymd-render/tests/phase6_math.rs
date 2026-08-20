use std::sync::Arc;

use stickymd_core::{DocumentSnapshot, Generation, LineEnding};
use stickymd_render::preview::{PreviewPipeline, PreviewSelection, PreviewTheme};

fn snapshot(source: &str) -> DocumentSnapshot {
    DocumentSnapshot {
        text: Arc::from(source),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    }
}

fn build(source: &str, scale: f32) -> (PreviewPipeline, stickymd_render::preview::PreviewFrame) {
    let mut pipeline = PreviewPipeline::new();
    let frame = pipeline
        .build(
            &snapshot(source),
            900,
            700,
            scale,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .expect("formula preview");
    (pipeline, frame)
}

#[test]
fn representative_fixture_renders_at_least_fifty_native_formulas() {
    let formulas = include_str!("fixtures/phase6_formulas.txt")
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    assert!(formulas.len() >= 50);
    let source = formulas
        .iter()
        .map(|formula| format!("$${formula}$$\n\n"))
        .collect::<String>();
    let (pipeline, frame) = build(&source, 1.0);
    let counters = pipeline.math_counters();
    assert_eq!(counters.parse_layout_calls, formulas.len() as u64);
    assert!(
        counters.rasterizations >= 50,
        "only {} representative formulas rasterized",
        counters.rasterizations
    );
    assert!(
        frame
            .rgba()
            .chunks_exact(4)
            .any(|pixel| pixel[0..3] != [248, 246, 239])
    );
    let copied = frame.index().copy(frame.index().select_all()).unwrap();
    assert!(copied.contains("$$\\frac{a}{b}$$"));
    assert!(copied.contains("$$\\text{中文}+x$$"));
}

#[test]
fn malformed_formula_is_atomic_and_does_not_fail_the_document() {
    let source = "before $\\frac{$ after $x^2$";
    let (pipeline, frame) = build(source, 1.0);
    assert_eq!(pipeline.math_counters().parse_layout_calls, 2);
    assert_eq!(pipeline.math_counters().rasterizations, 1);
    assert_eq!(frame.index().copy(frame.index().select_all()), Some(source));
    let atomic = frame
        .index()
        .boxes()
        .iter()
        .filter(|item| item.atomic)
        .collect::<Vec<_>>();
    assert_eq!(atomic.len(), 2);
    assert!(atomic.iter().all(|item| item.rect.width > 0.0));
}

#[test]
fn all_four_delimiters_copy_exact_source_while_code_stays_literal() {
    let source = "$a$\n\n$$b$$\n\n\\(c\\)\n\n\\[d\\]\n\n`$not_math$`";
    let (pipeline, frame) = build(source, 1.0);
    assert_eq!(pipeline.math_counters().parse_layout_calls, 4);
    let copied = frame.index().copy(frame.index().select_all()).unwrap();
    for expected in ["$a$", "$$b$$", "\\(c\\)", "\\[d\\]", "$not_math$"] {
        assert!(
            copied.contains(expected),
            "missing {expected:?} from {copied:?}"
        );
    }
}

#[test]
fn dpi_scales_formula_once_and_display_math_is_centered() {
    let (_, at_100) = build("$$\\frac{a}{b}$$", 1.0);
    let (_, at_200) = build("$$\\frac{a}{b}$$", 2.0);
    let box_100 = at_100
        .index()
        .boxes()
        .iter()
        .find(|item| item.atomic)
        .unwrap();
    let box_200 = at_200
        .index()
        .boxes()
        .iter()
        .find(|item| item.atomic)
        .unwrap();
    let width_ratio = box_200.rect.width / box_100.rect.width;
    let height_ratio = box_200.rect.height / box_100.rect.height;
    assert!(
        (1.8..=2.2).contains(&width_ratio),
        "width ratio {width_ratio}"
    );
    assert!(
        (1.8..=2.2).contains(&height_ratio),
        "height ratio {height_ratio}"
    );
    let center = box_100.rect.x + box_100.rect.width * 0.5;
    assert!((center - 450.0).abs() < 3.0, "display center {center}");
}

#[test]
fn formula_limit_and_source_limit_render_errors_without_aborting_preview() {
    let source = "$$x$$\n\n".repeat(2_001);
    let (pipeline, frame) = build(&source, 1.0);
    assert_eq!(pipeline.math_counters().parse_layout_calls, 1);
    assert_eq!(pipeline.math_counters().rasterizations, 1);
    assert_eq!(
        frame
            .index()
            .boxes()
            .iter()
            .filter(|item| item.atomic)
            .count(),
        2_001
    );

    let huge = format!("${}$", "x".repeat(64 * 1024 + 1));
    let (pipeline, frame) = build(&huge, 1.0);
    assert_eq!(pipeline.math_counters().rasterizations, 0);
    assert_eq!(
        frame.index().copy(frame.index().select_all()),
        Some(huge.as_str())
    );
}

#[test]
fn math_in_markdown_containers_and_overwide_formula_remain_bounded() {
    let source = concat!(
        "# heading $x^2$\n\n",
        "> quote $\\sqrt{x}$\n\n",
        "- list $\\sum_i x_i$\n\n",
        "| formula | value |\n| - | - |\n| $\\frac{a}{b}$ | 1 |\n\n",
        "$$\\sum_{i=1}^{1000} ",
        "xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx",
        "$$",
    );
    let (pipeline, frame) = build(source, 1.0);
    assert_eq!(pipeline.math_counters().rasterizations, 5);
    assert_eq!(frame.width(), 900);
    assert!(
        frame
            .index()
            .boxes()
            .iter()
            .filter(|item| item.atomic)
            .count()
            >= 5
    );
}
