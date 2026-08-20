//! Stateful native preview pipeline owned by the single preview worker.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling

use cosmic_text::{FontSystem, SwashCache};
use stickymd_core::{DocumentSnapshot, Generation};

use crate::math::{MathEngine, MathEngineCounters};
use crate::source::FontSelection;

use super::layout::{LaidOutDocument, layout_document};
use super::paint::{PreviewPaintError, paint_document};
use super::{
    PreviewFrame, PreviewParser, PreviewSelection, PreviewTheme, RenderTree, RenderTreeBuilder,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreviewPipelineCounters {
    pub parses: u64,
    pub render_tree_builds: u64,
    pub layouts: u64,
    pub paints: u64,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PreviewMathCounters {
    pub parse_layout_calls: u64,
    pub rasterizations: u64,
    pub layout_hits: u64,
    pub layout_misses: u64,
    pub layout_evictions: u64,
    pub raster_hits: u64,
    pub raster_misses: u64,
    pub raster_evictions: u64,
}

impl From<MathEngineCounters> for PreviewMathCounters {
    fn from(value: MathEngineCounters) -> Self {
        Self {
            parse_layout_calls: value.parse_layout_calls,
            rasterizations: value.rasterizations,
            layout_hits: value.layout_hits,
            layout_misses: value.layout_misses,
            layout_evictions: value.layout_evictions,
            raster_hits: value.raster_hits,
            raster_misses: value.raster_misses,
            raster_evictions: value.raster_evictions,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PreviewPipelineError {
    #[error(transparent)]
    Parse(#[from] super::PreviewParseError),
    #[error(transparent)]
    Paint(#[from] PreviewPaintError),
    #[error("preview has no current document")]
    NoDocument,
    #[error("preview generation {requested} does not match current {current}")]
    GenerationMismatch {
        requested: Generation,
        current: Generation,
    },
    #[error("preview layout width must be non-zero")]
    InvalidWidth,
}

/// Non-authoritative state kept exclusively on the preview worker thread.
pub struct PreviewPipeline {
    parser: PreviewParser,
    builder: RenderTreeBuilder,
    font_system: FontSystem,
    swash_cache: SwashCache,
    fonts: FontSelection,
    math_engine: MathEngine,
    tree: Option<RenderTree>,
    layout: Option<LaidOutDocument>,
    counters: PreviewPipelineCounters,
}

impl Default for PreviewPipeline {
    fn default() -> Self {
        Self::new()
    }
}

impl PreviewPipeline {
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        Self {
            parser: PreviewParser,
            builder: RenderTreeBuilder,
            font_system,
            swash_cache: SwashCache::new(),
            fonts,
            math_engine: MathEngine::new(),
            tree: None,
            layout: None,
            counters: PreviewPipelineCounters::default(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build(
        &mut self,
        snapshot: &DocumentSnapshot,
        width_px: u32,
        height_px: u32,
        scale: f32,
        scroll_y: f32,
        selection: PreviewSelection,
        theme: PreviewTheme,
    ) -> Result<PreviewFrame, PreviewPipelineError> {
        if width_px == 0 {
            return Err(PreviewPipelineError::InvalidWidth);
        }
        let owned = self.parser.parse(snapshot)?;
        self.counters.parses = self.counters.parses.saturating_add(1);
        let tree = self.builder.build(&owned);
        self.counters.render_tree_builds = self.counters.render_tree_builds.saturating_add(1);
        let layout = layout_document(
            &mut self.font_system,
            &self.fonts,
            &mut self.math_engine,
            &tree,
            width_px,
            scale,
            theme,
        );
        self.counters.layouts = self.counters.layouts.saturating_add(1);
        self.tree = Some(tree);
        self.layout = Some(layout);
        self.paint(snapshot.generation, height_px, scroll_y, selection, theme)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn relayout(
        &mut self,
        generation: Generation,
        width_px: u32,
        height_px: u32,
        scale: f32,
        scroll_y: f32,
        selection: PreviewSelection,
        theme: PreviewTheme,
    ) -> Result<PreviewFrame, PreviewPipelineError> {
        if width_px == 0 {
            return Err(PreviewPipelineError::InvalidWidth);
        }
        let tree = self.tree.as_ref().ok_or(PreviewPipelineError::NoDocument)?;
        if tree.generation != generation {
            return Err(PreviewPipelineError::GenerationMismatch {
                requested: generation,
                current: tree.generation,
            });
        }
        self.layout = Some(layout_document(
            &mut self.font_system,
            &self.fonts,
            &mut self.math_engine,
            tree,
            width_px,
            scale,
            theme,
        ));
        self.counters.layouts = self.counters.layouts.saturating_add(1);
        self.paint(generation, height_px, scroll_y, selection, theme)
    }

    pub fn paint(
        &mut self,
        generation: Generation,
        height_px: u32,
        scroll_y: f32,
        selection: PreviewSelection,
        theme: PreviewTheme,
    ) -> Result<PreviewFrame, PreviewPipelineError> {
        if self
            .layout
            .as_ref()
            .is_some_and(|layout| layout.theme != theme)
        {
            let (width, scale) = self
                .layout
                .as_ref()
                .map(|layout| (layout.width_px, layout.scale))
                .ok_or(PreviewPipelineError::NoDocument)?;
            let tree = self.tree.as_ref().ok_or(PreviewPipelineError::NoDocument)?;
            self.layout = Some(layout_document(
                &mut self.font_system,
                &self.fonts,
                &mut self.math_engine,
                tree,
                width,
                scale,
                theme,
            ));
            self.counters.layouts = self.counters.layouts.saturating_add(1);
        }
        let layout = self
            .layout
            .as_mut()
            .ok_or(PreviewPipelineError::NoDocument)?;
        if layout.generation != generation {
            return Err(PreviewPipelineError::GenerationMismatch {
                requested: generation,
                current: layout.generation,
            });
        }
        let frame = paint_document(
            &mut self.font_system,
            &mut self.swash_cache,
            layout,
            height_px,
            scroll_y,
            selection,
            theme,
        )?;
        self.counters.paints = self.counters.paints.saturating_add(1);
        Ok(frame)
    }

    pub const fn counters(&self) -> PreviewPipelineCounters {
        self.counters
    }

    pub fn math_counters(&self) -> PreviewMathCounters {
        self.math_engine.counters().into()
    }

    pub fn current_generation(&self) -> Option<Generation> {
        self.layout.as_ref().map(|layout| layout.generation)
    }

    pub fn release_math_rasters(&mut self) {
        self.layout = None;
        self.math_engine.release_rasters();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use stickymd_core::{DocumentSnapshot, Generation, LineEnding};

    use super::*;

    fn snapshot(source: &str, generation: Generation) -> DocumentSnapshot {
        DocumentSnapshot {
            text: Arc::from(source),
            generation,
            line_ending: LineEnding::Lf,
        }
    }

    #[test]
    fn resize_relayouts_without_reparsing_and_scroll_only_repaints() {
        let generation = Generation::initial();
        let mut pipeline = PreviewPipeline::new();
        pipeline
            .build(
                &snapshot(&"paragraph words ".repeat(100), generation),
                800,
                300,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();

        for resize in 0..100 {
            pipeline
                .relayout(
                    generation,
                    400 + resize,
                    300,
                    1.0,
                    0.0,
                    PreviewSelection::default(),
                    PreviewTheme::Light,
                )
                .unwrap();
        }

        for scroll in 0..1_000 {
            pipeline
                .paint(
                    generation,
                    300,
                    scroll as f32,
                    PreviewSelection::default(),
                    PreviewTheme::Light,
                )
                .unwrap();
        }
        assert_eq!(
            pipeline.counters(),
            PreviewPipelineCounters {
                parses: 1,
                render_tree_builds: 1,
                layouts: 101,
                paints: 1_101,
            }
        );
    }

    #[test]
    fn formula_resize_scroll_release_scale_and_theme_obey_cache_boundaries() {
        let generation = Generation::initial();
        let mut pipeline = PreviewPipeline::new();
        pipeline
            .build(
                &snapshot("Repeated $x^2$ and $x^2$.", generation),
                800,
                300,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        let built = pipeline.math_counters();
        assert_eq!(built.parse_layout_calls, 1);
        assert_eq!(built.rasterizations, 1);

        for resize in 0..100 {
            pipeline
                .relayout(
                    generation,
                    500 + resize,
                    300,
                    1.0,
                    0.0,
                    PreviewSelection::default(),
                    PreviewTheme::Light,
                )
                .unwrap();
        }
        for scroll in 0..1_000 {
            pipeline
                .paint(
                    generation,
                    300,
                    scroll as f32,
                    PreviewSelection::default(),
                    PreviewTheme::Light,
                )
                .unwrap();
        }
        let stable = pipeline.math_counters();
        assert_eq!(stable.parse_layout_calls, 1);
        assert_eq!(stable.rasterizations, 1);

        pipeline.release_math_rasters();
        assert_eq!(pipeline.current_generation(), None);
        pipeline
            .relayout(
                generation,
                600,
                300,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        let restored = pipeline.math_counters();
        assert_eq!(restored.parse_layout_calls, 1);
        assert_eq!(restored.rasterizations, 2);

        pipeline
            .relayout(
                generation,
                1_200,
                600,
                2.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        let scaled = pipeline.math_counters();
        assert_eq!(scaled.parse_layout_calls, 1);
        assert_eq!(scaled.rasterizations, 3);

        pipeline
            .paint(
                generation,
                600,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Dark,
            )
            .unwrap();
        let themed = pipeline.math_counters();
        assert_eq!(themed.parse_layout_calls, 2);
        assert_eq!(themed.rasterizations, 4);
    }

    #[test]
    fn generation_mismatch_fails_without_replacing_current_projection() {
        let mut pipeline = PreviewPipeline::new();
        pipeline
            .build(
                &snapshot("old", Generation::initial()),
                400,
                200,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        let newer = Generation::initial().checked_next().unwrap();
        assert!(matches!(
            pipeline.paint(
                newer,
                200,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light
            ),
            Err(PreviewPipelineError::GenerationMismatch { .. })
        ));
        assert_eq!(pipeline.current_generation(), Some(Generation::initial()));
    }

    #[test]
    fn selection_copy_covers_math_image_table_code_and_html_literals() {
        let source = concat!(
            "# Heading\n\n",
            "> quote\n\n- item\n\n",
            "```rust\nfn main() {}\n```\n\n",
            "| a | b |\n| - | - |\n| 1 | 2 |\n\n",
            "$x^2$ ![diagram](images/diagram.png) <b>literal</b>\n",
        );
        let mut pipeline = PreviewPipeline::new();
        let frame = pipeline
            .build(
                &snapshot(source, Generation::initial()),
                900,
                500,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        let copied = frame.index().copy(frame.index().select_all()).unwrap();
        for expected in [
            "Heading",
            "quote",
            "item",
            "fn main() {}",
            "a\tb",
            "$x^2$",
            "diagram",
            "<b>",
            "literal",
            "</b>",
        ] {
            assert!(
                copied.contains(expected),
                "missing {expected:?} in {copied:?}"
            );
        }
        assert!(!copied.contains("images/diagram.png"));
    }

    #[test]
    fn painting_culls_blocks_outside_the_viewport() {
        let source = (0..300)
            .map(|index| format!("paragraph {index}\n\n"))
            .collect::<String>();
        let mut pipeline = PreviewPipeline::new();
        let frame = pipeline
            .build(
                &snapshot(&source, Generation::initial()),
                600,
                120,
                1.0,
                3_000.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        assert!(frame.visible_blocks() > 0);
        assert!(frame.visible_blocks() < 20);
    }

    #[test]
    fn logical_geometry_scales_at_100_125_150_and_200_percent() {
        let mut left_edges = Vec::new();
        for (scale, width) in [(1.0, 800), (1.25, 1_000), (1.5, 1_200), (2.0, 1_600)] {
            let mut pipeline = PreviewPipeline::new();
            let frame = pipeline
                .build(
                    &snapshot("中文 English", Generation::initial()),
                    width,
                    300,
                    scale,
                    0.0,
                    PreviewSelection::default(),
                    PreviewTheme::Light,
                )
                .unwrap();
            let first = frame.index().boxes().first().unwrap();
            left_edges.push(first.rect.x);
            assert!((first.rect.x - 24.0 * scale).abs() <= 2.0);
            assert!(first.rect.height >= 20.0 * scale);
        }
        assert!(left_edges.windows(2).all(|pair| pair[1] > pair[0]));
    }

    #[test]
    #[ignore = "Release-only Phase 5 performance receipt"]
    fn phase5_preview_release_baseline() {
        for (bytes, hard_p95) in [
            (20 * 1024, Duration::from_millis(100)),
            (100 * 1024, Duration::from_millis(400)),
            (1024 * 1024, Duration::from_secs(2)),
        ] {
            let cold = measure_cold_fixture(bytes);
            let samples = measure_fixture(bytes, 20);
            let total_p95 = percentile(&samples.total, 95);
            println!(
                "phase5 bytes={bytes} cold={cold:?} warm_repeats=20 comrak={:?}/{:?}/{:?} owned={:?}/{:?}/{:?} render={:?}/{:?}/{:?} layout={:?}/{:?}/{:?} paint={:?}/{:?}/{:?} total={:?}/{:?}/{:?}",
                percentile(&samples.comrak, 50),
                percentile(&samples.comrak, 95),
                max(&samples.comrak),
                percentile(&samples.owned, 50),
                percentile(&samples.owned, 95),
                max(&samples.owned),
                percentile(&samples.render, 50),
                percentile(&samples.render, 95),
                max(&samples.render),
                percentile(&samples.layout, 50),
                percentile(&samples.layout, 95),
                max(&samples.layout),
                percentile(&samples.paint, 50),
                percentile(&samples.paint, 95),
                max(&samples.paint),
                percentile(&samples.total, 50),
                total_p95,
                max(&samples.total),
            );
            assert!(
                total_p95 <= hard_p95,
                "{bytes}-byte preview p95 {total_p95:?} exceeds {hard_p95:?}"
            );
        }
    }

    #[test]
    #[ignore = "Release-only Phase 6 math-document performance receipt"]
    fn phase6_math_document_release_baseline() {
        for (bytes, formulas, hard_p95) in [
            (20 * 1024, 20, Duration::from_millis(100)),
            (100 * 1024, 100, Duration::from_millis(400)),
            (1024 * 1024, 500, Duration::from_secs(2)),
        ] {
            let source = math_fixture(bytes, formulas);
            let cold = measure_cold_source(&source);
            let samples = measure_math_source(&source, 20);
            let p95 = percentile(&samples.total, 95);
            println!(
                "phase6 math_document bytes={bytes} formulas={formulas} cold={cold:?} warm_repeats=20 comrak={:?}/{:?}/{:?} owned={:?}/{:?}/{:?} render={:?}/{:?}/{:?} layout={:?}/{:?}/{:?} paint={:?}/{:?}/{:?} total={:?}/{p95:?}/{:?}",
                percentile(&samples.comrak, 50),
                percentile(&samples.comrak, 95),
                max(&samples.comrak),
                percentile(&samples.owned, 50),
                percentile(&samples.owned, 95),
                max(&samples.owned),
                percentile(&samples.render, 50),
                percentile(&samples.render, 95),
                max(&samples.render),
                percentile(&samples.layout, 50),
                percentile(&samples.layout, 95),
                max(&samples.layout),
                percentile(&samples.paint, 50),
                percentile(&samples.paint, 95),
                max(&samples.paint),
                percentile(&samples.total, 50),
                max(&samples.total),
            );
            assert!(
                p95 <= hard_p95,
                "{bytes}-byte/{formulas}-formula preview p95 {p95:?} exceeds {hard_p95:?}"
            );
        }
    }

    fn math_fixture(target_bytes: usize, formulas: usize) -> String {
        let mut source = String::with_capacity(target_bytes + formulas * 32);
        for index in 0..formulas {
            source.push_str(&format!(
                "Paragraph {index}: $\\frac{{x_{{{index}}}^2+1}}{{{}}}$ text.\n\n",
                index + 1
            ));
        }
        const FILLER: &str =
            "中文 Preview paragraph with **strong** text and a [link](https://example.com).\n\n";
        while source.len() + FILLER.len() <= target_bytes {
            source.push_str(FILLER);
        }
        source
    }

    fn measure_cold_source(source: &str) -> Duration {
        let snapshot = snapshot(source, Generation::initial());
        let started = Instant::now();
        PreviewPipeline::new()
            .build(
                &snapshot,
                900,
                600,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        started.elapsed()
    }

    fn measure_math_source(source: &str, repeats: usize) -> StageSamples {
        let snapshot = snapshot(source, Generation::initial());
        let parser = PreviewParser;
        let builder = RenderTreeBuilder;
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        let mut math_engine = MathEngine::new();
        let mut swash_cache = SwashCache::new();

        // Whole-document refresh normally reuses the worker-owned formula
        // caches. Cold first-load cost is reported separately above.
        let (owned, _) = parser.parse_with_metrics(&snapshot).unwrap();
        let tree = builder.build(&owned);
        let _ = layout_document(
            &mut font_system,
            &fonts,
            &mut math_engine,
            &tree,
            900,
            1.0,
            PreviewTheme::Light,
        );

        let mut samples = StageSamples::default();
        for _ in 0..repeats {
            let total_started = Instant::now();
            let (owned, parse) = parser.parse_with_metrics(&snapshot).unwrap();
            let render_started = Instant::now();
            let tree = builder.build(&owned);
            let render = render_started.elapsed();
            let layout_started = Instant::now();
            let mut layout = layout_document(
                &mut font_system,
                &fonts,
                &mut math_engine,
                &tree,
                900,
                1.0,
                PreviewTheme::Light,
            );
            let layout_duration = layout_started.elapsed();
            let paint_started = Instant::now();
            paint_document(
                &mut font_system,
                &mut swash_cache,
                &mut layout,
                600,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
            samples.comrak.push(parse.comrak_parse);
            samples.owned.push(parse.owned_conversion);
            samples.render.push(render);
            samples.layout.push(layout_duration);
            samples.paint.push(paint_started.elapsed());
            samples.total.push(total_started.elapsed());
        }
        samples
    }

    fn measure_cold_fixture(bytes: usize) -> Duration {
        let source = fixture(bytes);
        let snapshot = snapshot(&source, Generation::initial());
        let started = Instant::now();
        PreviewPipeline::new()
            .build(
                &snapshot,
                900,
                700,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        started.elapsed()
    }

    #[derive(Default)]
    struct StageSamples {
        comrak: Vec<Duration>,
        owned: Vec<Duration>,
        render: Vec<Duration>,
        layout: Vec<Duration>,
        paint: Vec<Duration>,
        total: Vec<Duration>,
    }

    fn measure_fixture(bytes: usize, repeats: usize) -> StageSamples {
        let source = fixture(bytes);
        let snapshot = snapshot(&source, Generation::initial());
        let parser = PreviewParser;
        let builder = RenderTreeBuilder;
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        let mut math_engine = MathEngine::new();
        let mut swash_cache = SwashCache::new();
        let mut samples = StageSamples::default();

        for _ in 0..repeats {
            let total_started = Instant::now();
            let (owned, parse) = parser.parse_with_metrics(&snapshot).unwrap();
            let render_started = Instant::now();
            let tree = builder.build(&owned);
            let render = render_started.elapsed();
            let layout_started = Instant::now();
            let mut layout = layout_document(
                &mut font_system,
                &fonts,
                &mut math_engine,
                &tree,
                900,
                1.0,
                PreviewTheme::Light,
            );
            let layout_duration = layout_started.elapsed();
            let paint_started = Instant::now();
            paint_document(
                &mut font_system,
                &mut swash_cache,
                &mut layout,
                700,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
            samples.comrak.push(parse.comrak_parse);
            samples.owned.push(parse.owned_conversion);
            samples.render.push(render);
            samples.layout.push(layout_duration);
            samples.paint.push(paint_started.elapsed());
            samples.total.push(total_started.elapsed());
        }
        samples
    }

    fn fixture(bytes: usize) -> String {
        let rich = concat!(
            "## Heading\n\n",
            "中文 English **bold** *italic* [link](https://example.com) and $x^2$.\n\n",
            "- [x] task item\n- ordinary item\n\n",
            "| left | right |\n| :--- | ---: |\n| value | 42 |\n\n",
        );
        let paragraph = format!(
            "{}\n\n",
            "This is a representative long Markdown paragraph with 中文 text and numbers 12345. "
                .repeat(12)
        );
        let mut source = String::with_capacity(bytes + paragraph.len());
        source.push_str(rich);
        while source.len() < bytes {
            source.push_str(&paragraph);
        }
        let mut boundary = bytes;
        while !source.is_char_boundary(boundary) {
            boundary -= 1;
        }
        source.truncate(boundary);
        source
    }

    fn percentile(samples: &[Duration], percentile: usize) -> Duration {
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let index = (sorted.len().saturating_sub(1) * percentile) / 100;
        sorted[index]
    }

    fn max(samples: &[Duration]) -> Duration {
        samples.iter().copied().max().unwrap_or_default()
    }
}
