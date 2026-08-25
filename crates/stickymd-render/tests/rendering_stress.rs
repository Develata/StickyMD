use std::cell::RefCell;
use std::collections::BTreeSet;
use std::sync::Arc;

use stickymd_core::{DocumentSnapshot, Generation, LineEnding};
use stickymd_render::image::{
    EncodedImageFormat, ImageMetadata, PreviewImageSource, prepare_rgba_image,
};
use stickymd_render::preview::{
    BlockNode, InlineNode, MathNode, OwnedDocumentTree, PreviewParser, PreviewPipeline,
    PreviewSelection, PreviewTheme,
};

const SOURCE: &str = include_str!("fixtures/rendering-stress.md");

struct StressImages {
    bytes: Vec<u8>,
    loads: RefCell<Vec<String>>,
}

impl StressImages {
    fn new() -> Self {
        let bytes = prepare_rgba_image(16, 8, vec![180; 16 * 8 * 4])
            .expect("stress image")
            .bytes()
            .to_vec();
        Self {
            bytes,
            loads: RefCell::new(Vec::new()),
        }
    }
}

impl PreviewImageSource for StressImages {
    fn inspect(&self, destination: &str) -> Result<Option<ImageMetadata>, String> {
        Ok(destination
            .starts_with("images/stress-")
            .then_some(ImageMetadata {
                format: EncodedImageFormat::Png,
                width: 16,
                height: 8,
            }))
    }

    fn load(&self, destination: &str) -> Result<Option<Vec<u8>>, String> {
        self.loads.borrow_mut().push(destination.to_owned());
        Ok(destination
            .starts_with("images/stress-")
            .then(|| self.bytes.clone()))
    }
}

fn snapshot() -> DocumentSnapshot {
    DocumentSnapshot {
        text: Arc::from(SOURCE),
        generation: Generation::initial(),
        line_ending: LineEnding::Lf,
    }
}

fn collect_math(document: &OwnedDocumentTree) -> Vec<&MathNode> {
    let mut formulas = Vec::new();
    visit_blocks(&document.blocks, &mut formulas);
    formulas
}

fn visit_blocks<'a>(blocks: &'a [BlockNode], formulas: &mut Vec<&'a MathNode>) {
    for block in blocks {
        match block {
            BlockNode::Paragraph { content, .. } | BlockNode::Heading { content, .. } => {
                visit_inlines(content, formulas);
            }
            BlockNode::BlockQuote { blocks, .. } => visit_blocks(blocks, formulas),
            BlockNode::List(list) => {
                for item in &list.items {
                    visit_blocks(&item.blocks, formulas);
                }
            }
            BlockNode::Table(table) => {
                for row in &table.rows {
                    for cell in &row.cells {
                        visit_inlines(&cell.content, formulas);
                    }
                }
            }
            BlockNode::DisplayMath(math) => formulas.push(math),
            BlockNode::CodeBlock(_)
            | BlockNode::ThematicBreak { .. }
            | BlockNode::HtmlLiteral { .. } => {}
        }
    }
}

fn visit_inlines<'a>(inlines: &'a [InlineNode], formulas: &mut Vec<&'a MathNode>) {
    for inline in inlines {
        match inline {
            InlineNode::Emphasis { children, .. }
            | InlineNode::Strong { children, .. }
            | InlineNode::Strikethrough { children, .. }
            | InlineNode::Link { children, .. } => visit_inlines(children, formulas),
            InlineNode::InlineMath(math) => formulas.push(math),
            InlineNode::Text { .. }
            | InlineNode::Code { .. }
            | InlineNode::Image { .. }
            | InlineNode::SoftBreak { .. }
            | InlineNode::HardBreak { .. }
            | InlineNode::HtmlLiteral { .. } => {}
        }
    }
}

#[test]
fn stress_fixture_formulas_match_current_ratex_baseline() {
    let document = PreviewParser
        .parse(&snapshot())
        .expect("stress Markdown parses");
    let formulas = collect_math(&document);
    assert!(
        formulas.len() >= 20,
        "only {} formulas found",
        formulas.len()
    );

    let failures = formulas
        .iter()
        .filter_map(|formula| {
            ratex_parser::parse(&formula.literal)
                .err()
                .map(|error| (formula.source_literal.as_str(), error.to_string()))
        })
        .collect::<Vec<_>>();
    assert!(
        failures.is_empty(),
        "stress fixture contains unsupported RaTeX formulas: {failures:#?}"
    );
}

#[test]
fn stress_fixture_preserves_markdown_priority_and_literal_boundaries() {
    let document = PreviewParser
        .parse(&snapshot())
        .expect("stress Markdown parses");
    let formulas = collect_math(&document);
    let mermaid_blocks = document
        .blocks
        .iter()
        .filter(|block| matches!(block, BlockNode::CodeBlock(code) if code.info == "mermaid"))
        .count();
    assert!(
        document.node_count > 100,
        "stress tree is unexpectedly small"
    );
    assert_eq!(mermaid_blocks, 2);
    assert!(
        document
            .blocks
            .iter()
            .any(|block| matches!(block, BlockNode::HtmlLiteral { literal, .. } if literal.contains("<script>")))
    );
    assert!(
        formulas
            .iter()
            .all(|formula| formula.literal.trim() != "math"),
        "math inside code leaked into RaTeX"
    );
    assert!(SOURCE.contains("[[02_positioning|项目定位篇]]"));
    assert!(SOURCE.contains("STICKYMD_RENDERING_STRESS_END"));
}

#[test]
fn stress_fixture_builds_every_formula_through_native_preview() {
    let document = PreviewParser
        .parse(&snapshot())
        .expect("stress Markdown parses");
    let formulas = collect_math(&document);
    let formula_count = formulas.len() as u64;
    let unique_formula_count = formulas
        .iter()
        .map(|formula| (formula.literal.as_str(), formula.display))
        .collect::<BTreeSet<_>>()
        .len() as u64;
    let mut pipeline = PreviewPipeline::new();
    let frame = pipeline
        .build(
            &snapshot(),
            900,
            700,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .expect("stress preview builds");
    let counters = pipeline.math_counters();
    eprintln!(
        "stress formulas={formula_count} unique={unique_formula_count} parse_layout_calls={} rasterizations={} frame={}x{} boxes={}",
        counters.parse_layout_calls,
        counters.rasterizations,
        frame.width(),
        frame.height(),
        frame.index().boxes().len(),
    );
    assert_eq!(counters.parse_layout_calls, unique_formula_count);
    assert_eq!(
        counters.rasterizations, unique_formula_count,
        "some RaTeX formulas parsed but failed native layout/paint"
    );
}

#[test]
fn stress_fixture_survives_narrow_wide_zoom_theme_and_overscroll_rasters() {
    for (width, scale, theme) in [
        (320, 0.5, PreviewTheme::Light),
        (900, 1.0, PreviewTheme::Light),
        (320, 3.0, PreviewTheme::Dark),
        (900, 3.0, PreviewTheme::Dark),
    ] {
        let mut pipeline = PreviewPipeline::new();
        let frame = pipeline
            .build(
                &snapshot(),
                width,
                360,
                scale,
                f32::MAX,
                PreviewSelection::default(),
                theme,
            )
            .expect("stress preview raster");
        assert_eq!(frame.width(), width);
        assert_eq!(frame.height(), 360);
        assert!(frame.scroll_y().is_finite());
        let first = &frame.rgba()[..4];
        assert!(
            frame.rgba().chunks_exact(4).any(|pixel| pixel != first),
            "stress frame at width={width} scale={scale} is a flat skeleton"
        );
        assert!(
            frame
                .index()
                .text()
                .contains("STICKYMD_RENDERING_STRESS_END"),
            "deep-scroll sentinel left the preview index"
        );
    }
}

#[test]
fn stress_fixture_bottom_image_is_admitted_after_scroll_clamps() {
    let images = StressImages::new();
    let mut pipeline = PreviewPipeline::new();
    pipeline
        .build_with_image_source(
            &snapshot(),
            480,
            240,
            1.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
            Some(&images),
        )
        .expect("stress preview top");
    let initial = images.loads.borrow().clone();
    assert!(initial.iter().any(|path| path == "images/stress-top.png"));
    assert!(
        !initial
            .iter()
            .any(|path| path == "images/stress-bottom.png")
    );

    let bottom = pipeline
        .paint_with_image_source(
            Generation::initial(),
            240,
            f32::MAX,
            PreviewSelection::default(),
            PreviewTheme::Light,
            Some(&images),
        )
        .expect("stress preview bottom");
    let final_loads = images.loads.borrow();
    assert!(bottom.scroll_y().is_finite() && bottom.scroll_y() > 0.0);
    assert!(
        final_loads
            .iter()
            .any(|path| path == "images/stress-bottom.png")
    );
    assert!(
        final_loads.iter().all(|path| !path.starts_with("http")),
        "remote image reached the local image source"
    );
}
