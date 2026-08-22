//! Stateful native preview pipeline owned by the single preview worker.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling

use cosmic_text::{FontSystem, SwashCache};
use stickymd_core::{DocumentSnapshot, Generation};

use crate::image::{DecodedImageCache, PreviewImageSource};
use crate::math::{MathEngine, MathEngineCounters};
use crate::source::FontSelection;

use super::layout::{LaidOutDocument, LayoutResources, layout_document};
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
    image_cache: DecodedImageCache,
    image_band: (f32, f32),
    layout_has_image_source: bool,
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
            image_cache: DecodedImageCache::default(),
            image_band: (0.0, 0.0),
            layout_has_image_source: false,
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
        self.build_with_image_source(
            snapshot, width_px, height_px, scale, scroll_y, selection, theme, None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn build_with_image_source(
        &mut self,
        snapshot: &DocumentSnapshot,
        width_px: u32,
        height_px: u32,
        scale: f32,
        scroll_y: f32,
        selection: PreviewSelection,
        theme: PreviewTheme,
        image_source: Option<&dyn PreviewImageSource>,
    ) -> Result<PreviewFrame, PreviewPipelineError> {
        if width_px == 0 {
            return Err(PreviewPipelineError::InvalidWidth);
        }
        let owned = self.parser.parse(snapshot)?;
        self.counters.parses = self.counters.parses.saturating_add(1);
        let tree = self.builder.build(&owned);
        self.counters.render_tree_builds = self.counters.render_tree_builds.saturating_add(1);
        let image_band = image_band(scroll_y, height_px, scale);
        // Layout chunks lease cached image rasters. Drop the prior projection
        // before admitting a replacement so the strict cache budget can evict
        // rasters that are no longer visible instead of mistaking them for
        // still-live memory.
        self.layout = None;
        let layout = layout_document(
            LayoutResources {
                font_system: &mut self.font_system,
                fonts: &self.fonts,
                math_engine: &mut self.math_engine,
                image_source,
                image_cache: &mut self.image_cache,
                image_band,
            },
            &tree,
            width_px,
            scale,
            theme,
        );
        self.counters.layouts = self.counters.layouts.saturating_add(1);
        self.tree = Some(tree);
        self.layout = Some(layout);
        self.image_band = image_band;
        self.layout_has_image_source = image_source.is_some();
        self.paint_with_image_source(
            snapshot.generation,
            height_px,
            scroll_y,
            selection,
            theme,
            image_source,
        )
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
        self.relayout_with_image_source(
            generation, width_px, height_px, scale, scroll_y, selection, theme, None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn relayout_with_image_source(
        &mut self,
        generation: Generation,
        width_px: u32,
        height_px: u32,
        scale: f32,
        scroll_y: f32,
        selection: PreviewSelection,
        theme: PreviewTheme,
        image_source: Option<&dyn PreviewImageSource>,
    ) -> Result<PreviewFrame, PreviewPipelineError> {
        if width_px == 0 {
            return Err(PreviewPipelineError::InvalidWidth);
        }
        let normalized_scale = scale.max(0.5);
        let image_source_available = image_source.is_some();
        if self.layout.as_ref().is_some_and(|layout| {
            layout.generation == generation
                && layout.width_px == width_px
                && layout.scale.to_bits() == normalized_scale.to_bits()
                && layout.theme == theme
                && self.layout_has_image_source == image_source_available
        }) {
            return self.paint_with_image_source(
                generation,
                height_px,
                scroll_y,
                selection,
                theme,
                image_source,
            );
        }
        self.layout = None;
        let tree = self.tree.as_ref().ok_or(PreviewPipelineError::NoDocument)?;
        if tree.generation != generation {
            return Err(PreviewPipelineError::GenerationMismatch {
                requested: generation,
                current: tree.generation,
            });
        }
        let image_band = image_band(scroll_y, height_px, scale);
        self.layout = Some(layout_document(
            LayoutResources {
                font_system: &mut self.font_system,
                fonts: &self.fonts,
                math_engine: &mut self.math_engine,
                image_source,
                image_cache: &mut self.image_cache,
                image_band,
            },
            tree,
            width_px,
            scale,
            theme,
        ));
        self.image_band = image_band;
        self.layout_has_image_source = image_source_available;
        self.counters.layouts = self.counters.layouts.saturating_add(1);
        self.paint_with_image_source(
            generation,
            height_px,
            scroll_y,
            selection,
            theme,
            image_source,
        )
    }

    pub fn paint(
        &mut self,
        generation: Generation,
        height_px: u32,
        scroll_y: f32,
        selection: PreviewSelection,
        theme: PreviewTheme,
    ) -> Result<PreviewFrame, PreviewPipelineError> {
        self.paint_with_image_source(generation, height_px, scroll_y, selection, theme, None)
    }

    pub fn paint_with_image_source(
        &mut self,
        generation: Generation,
        height_px: u32,
        scroll_y: f32,
        selection: PreviewSelection,
        theme: PreviewTheme,
        image_source: Option<&dyn PreviewImageSource>,
    ) -> Result<PreviewFrame, PreviewPipelineError> {
        // The UI may enqueue several wheel events before the previous frame
        // returns. Use the current document extent for lazy-image admission,
        // otherwise an overscroll value can build a band beyond the document
        // while `paint_document` later clamps the visible frame to its bottom.
        let effective_scroll_y = self.layout.as_ref().map_or(scroll_y, |layout| {
            scroll_y.clamp(0.0, (layout.height_px - height_px as f32).max(0.0))
        });
        let image_source_available = image_source.is_some();
        let needs_image_source_refresh = self.layout_has_image_source != image_source_available;
        let needs_image_band = needs_image_source_refresh
            || (image_source_available
                && (effective_scroll_y < self.image_band.0
                    || effective_scroll_y + height_px as f32 > self.image_band.1));
        if needs_image_band {
            let (width, scale) = self
                .layout
                .as_ref()
                .map(|layout| (layout.width_px, layout.scale))
                .ok_or(PreviewPipelineError::NoDocument)?;
            self.layout = None;
            let tree = self.tree.as_ref().ok_or(PreviewPipelineError::NoDocument)?;
            let band = image_band(effective_scroll_y, height_px, scale);
            self.layout = Some(layout_document(
                LayoutResources {
                    font_system: &mut self.font_system,
                    fonts: &self.fonts,
                    math_engine: &mut self.math_engine,
                    image_source,
                    image_cache: &mut self.image_cache,
                    image_band: band,
                },
                tree,
                width,
                scale,
                theme,
            ));
            self.image_band = band;
            self.layout_has_image_source = image_source_available;
            self.counters.layouts = self.counters.layouts.saturating_add(1);
        }
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
            self.layout = None;
            let tree = self.tree.as_ref().ok_or(PreviewPipelineError::NoDocument)?;
            self.layout = Some(layout_document(
                LayoutResources {
                    font_system: &mut self.font_system,
                    fonts: &self.fonts,
                    math_engine: &mut self.math_engine,
                    image_source,
                    image_cache: &mut self.image_cache,
                    image_band: self.image_band,
                },
                tree,
                width,
                scale,
                theme,
            ));
            self.layout_has_image_source = image_source_available;
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
            effective_scroll_y,
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

    pub fn release_raster_caches(&mut self) {
        self.layout = None;
        self.math_engine.release_rasters();
        self.image_cache.clear();
    }

    pub fn image_cache_bytes(&self) -> usize {
        self.image_cache.bytes()
    }

    pub fn image_cache_entries(&self) -> usize {
        self.image_cache.entry_count()
    }

    pub fn image_cache_counters(&self) -> crate::image::ImageCacheCounters {
        self.image_cache.counters()
    }
}

fn image_band(scroll_y: f32, height_px: u32, scale: f32) -> (f32, f32) {
    let margin = 300.0 * scale.max(0.5);
    (
        (scroll_y - margin).max(0.0),
        scroll_y + height_px as f32 + margin,
    )
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use stickymd_core::{DocumentSnapshot, Generation, LineEnding};

    use super::*;

    struct MemoryImages {
        bytes: Vec<u8>,
        loads: Cell<usize>,
    }

    impl crate::image::PreviewImageSource for MemoryImages {
        fn inspect(
            &self,
            destination: &str,
        ) -> Result<Option<crate::image::ImageMetadata>, String> {
            (destination == "image.png")
                .then(|| {
                    crate::image::inspect_encoded_image(&self.bytes).map_err(|e| e.to_string())
                })
                .transpose()
        }

        fn load(&self, destination: &str) -> Result<Option<Vec<u8>>, String> {
            self.loads.set(self.loads.get() + 1);
            Ok((destination == "image.png").then(|| self.bytes.clone()))
        }
    }

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
    fn phase11_identical_relayout_only_repaints_but_image_source_change_rebuilds() {
        let generation = Generation::initial();
        let prepared = crate::image::prepare_rgba_image(8, 4, vec![200; 8 * 4 * 4]).unwrap();
        let images = MemoryImages {
            bytes: prepared.bytes().to_vec(),
            loads: Cell::new(0),
        };
        let mut pipeline = PreviewPipeline::new();
        pipeline
            .build(
                &snapshot("![local](image.png)", generation),
                600,
                300,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        let initial = pipeline.counters();

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
        assert_eq!(pipeline.counters().layouts, initial.layouts);
        assert_eq!(pipeline.counters().paints, initial.paints + 1);

        pipeline
            .relayout_with_image_source(
                generation,
                600,
                300,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&images),
            )
            .unwrap();
        assert_eq!(pipeline.counters().layouts, initial.layouts + 1);
        assert!(images.loads.get() > 0);
    }

    #[test]
    fn phase10_one_hundred_zoom_relayouts_reuse_markdown_and_math_semantics() {
        let generation = Generation::initial();
        let mut pipeline = PreviewPipeline::new();
        pipeline
            .build(
                &snapshot("# Zoom\n\nRepeated $x^2$ and $x^2$.", generation),
                600,
                300,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
            )
            .unwrap();
        for index in 0..100 {
            let scale = 0.5 + (index % 51) as f32 * 0.05;
            pipeline
                .relayout(
                    generation,
                    600,
                    300,
                    scale,
                    0.0,
                    PreviewSelection::default(),
                    PreviewTheme::Light,
                )
                .unwrap();
        }
        assert_eq!(pipeline.counters().parses, 1);
        assert_eq!(pipeline.counters().render_tree_builds, 1);
        assert_eq!(pipeline.counters().layouts, 101);
        assert_eq!(pipeline.math_counters().parse_layout_calls, 1);
        assert!(pipeline.math_counters().rasterizations > 1);
    }

    #[test]
    fn phase10_zoom_reuses_sufficient_image_rasters_and_never_exceeds_pane_width() {
        let prepared = crate::image::prepare_rgba_image(8, 4, vec![200; 8 * 4 * 4]).unwrap();
        let images = MemoryImages {
            bytes: prepared.bytes().to_vec(),
            loads: Cell::new(0),
        };
        let generation = Generation::initial();
        let mut pipeline = PreviewPipeline::new();
        pipeline
            .build_with_image_source(
                &snapshot("![alt](image.png)", generation),
                220,
                120,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&images),
            )
            .unwrap();
        for index in 0..100 {
            pipeline
                .relayout_with_image_source(
                    generation,
                    220,
                    120,
                    0.5 + (index % 51) as f32 * 0.05,
                    0.0,
                    PreviewSelection::default(),
                    PreviewTheme::Light,
                    Some(&images),
                )
                .unwrap();
        }
        assert_eq!(pipeline.image_cache_counters().misses, 1);
        assert!(pipeline.image_cache_counters().hits >= 100);
        let layout = pipeline.layout.as_ref().unwrap();
        assert!(
            layout
                .blocks
                .iter()
                .flat_map(|block| &block.chunks)
                .all(|chunk| {
                    match &chunk.content {
                        crate::preview::layout::LayoutContent::Image(raster) => {
                            chunk.x + raster.width as f32 <= layout.width_px as f32 + 0.5
                        }
                        _ => true,
                    }
                })
        );
    }

    #[test]
    #[ignore = "Release-only Phase 10 zoom relayout performance receipt"]
    fn phase10_zoom_release_baseline() {
        const REPEATS_PER_SCALE: usize = 100;
        const HARD_P95: Duration = Duration::from_millis(50);
        let prepared = crate::image::prepare_rgba_image(64, 32, vec![160; 64 * 32 * 4]).unwrap();
        let images = MemoryImages {
            bytes: prepared.bytes().to_vec(),
            loads: Cell::new(0),
        };
        let generation = Generation::initial();
        let mut source = fixture(20 * 1024);
        source.push_str("\n\nRepeated math $x^2+y^2=1$.\n\n![local](image.png)\n");
        let mut pipeline = PreviewPipeline::new();
        pipeline
            .build_with_image_source(
                &snapshot(&source, generation),
                900,
                680,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&images),
            )
            .unwrap();
        let semantic_counters = pipeline.counters();
        let math_parse_layout_calls = pipeline.math_counters().parse_layout_calls;
        for scale in [0.5_f32, 1.0, 3.0] {
            for _ in 0..10 {
                pipeline
                    .relayout_with_image_source(
                        generation,
                        900,
                        680,
                        scale,
                        0.0,
                        PreviewSelection::default(),
                        PreviewTheme::Light,
                        Some(&images),
                    )
                    .unwrap();
            }
            let mut samples = Vec::with_capacity(REPEATS_PER_SCALE);
            for _ in 0..REPEATS_PER_SCALE {
                let started = Instant::now();
                pipeline
                    .relayout_with_image_source(
                        generation,
                        900,
                        680,
                        scale,
                        0.0,
                        PreviewSelection::default(),
                        PreviewTheme::Light,
                        Some(&images),
                    )
                    .unwrap();
                samples.push(started.elapsed());
            }
            let p95 = percentile(&samples, 95);
            println!(
                "phase10 zoom_percent={} repeats={REPEATS_PER_SCALE} relayout_ms={:.3}/{:.3}/{:.3}",
                (scale * 100.0).round() as u16,
                percentile(&samples, 50).as_secs_f64() * 1_000.0,
                p95.as_secs_f64() * 1_000.0,
                max(&samples).as_secs_f64() * 1_000.0,
            );
            assert!(p95 <= HARD_P95, "zoom {scale} relayout p95={p95:?}");
        }
        assert_eq!(pipeline.counters().parses, semantic_counters.parses);
        assert_eq!(
            pipeline.counters().render_tree_builds,
            semantic_counters.render_tree_builds
        );
        assert_eq!(
            pipeline.math_counters().parse_layout_calls,
            math_parse_layout_calls
        );
        assert!(pipeline.image_cache_bytes() <= crate::image::IMAGE_CACHE_BUDGET_BYTES);
    }

    #[test]
    fn local_image_is_decoded_on_worker_projection_and_remote_never_loads() {
        let prepared = crate::image::prepare_rgba_image(8, 4, vec![200; 8 * 4 * 4]).unwrap();
        let images = MemoryImages {
            bytes: prepared.bytes().to_vec(),
            loads: Cell::new(0),
        };
        let generation = Generation::initial();
        let mut pipeline = PreviewPipeline::new();
        let frame = pipeline
            .build_with_image_source(
                &snapshot("![alt](image.png)", generation),
                400,
                200,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&images),
            )
            .unwrap();
        assert_eq!(images.loads.get(), 1);
        assert!(pipeline.image_cache_bytes() > 0);
        assert_eq!(frame.index().text(), "alt");
        assert_eq!(pipeline.image_cache_counters().misses, 1);

        pipeline
            .build_with_image_source(
                &snapshot("![alt](image.png)", generation),
                400,
                200,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&images),
            )
            .unwrap();
        assert_eq!(pipeline.image_cache_counters().hits, 1);

        pipeline.release_raster_caches();
        assert_eq!(pipeline.image_cache_bytes(), 0);

        images.loads.set(0);
        pipeline
            .build_with_image_source(
                &snapshot("![remote](https://example.com/a.png)", generation),
                400,
                200,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&images),
            )
            .unwrap();
        assert_eq!(images.loads.get(), 0);
    }

    #[test]
    fn phase7_local_images_remain_native_when_mixed_with_text_and_table_cells() {
        let prepared = crate::image::prepare_rgba_image(8, 4, vec![180; 8 * 4 * 4]).unwrap();
        let images = MemoryImages {
            bytes: prepared.bytes().to_vec(),
            loads: Cell::new(0),
        };
        let source =
            "before ![alt](image.png) after\n\n| cell |\n| --- |\n| x ![table](image.png) y |";
        let mut pipeline = PreviewPipeline::new();
        let frame = pipeline
            .build_with_image_source(
                &snapshot(source, Generation::initial()),
                500,
                300,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&images),
            )
            .unwrap();

        let image_chunks = pipeline
            .layout
            .as_ref()
            .unwrap()
            .blocks
            .iter()
            .flat_map(|block| &block.chunks)
            .filter(|chunk| {
                matches!(
                    chunk.content,
                    crate::preview::layout::LayoutContent::Image(_)
                )
            })
            .count();
        assert_eq!(image_chunks, 2);
        assert_eq!(images.loads.get(), 2);
        assert!(frame.index().text().contains("before alt after"));
        assert!(frame.index().text().contains("x table y"));
    }

    #[test]
    fn phase7_corrupt_or_missing_image_isolated_and_lazy_band_keeps_decode_cache_small() {
        struct VaryingImages {
            loads: Cell<usize>,
        }
        impl crate::image::PreviewImageSource for VaryingImages {
            fn inspect(
                &self,
                destination: &str,
            ) -> Result<Option<crate::image::ImageMetadata>, String> {
                if destination == "corrupt.png" {
                    return Err("corrupt".into());
                }
                if destination == "missing.png" {
                    return Ok(None);
                }
                Ok(Some(crate::image::ImageMetadata {
                    format: crate::image::EncodedImageFormat::Png,
                    width: 64,
                    height: 64,
                }))
            }

            fn load(&self, destination: &str) -> Result<Option<Vec<u8>>, String> {
                self.loads.set(self.loads.get() + 1);
                if destination == "corrupt.png" {
                    return Ok(Some(b"corrupt".to_vec()));
                }
                if destination == "missing.png" {
                    return Ok(None);
                }
                let index = destination
                    .strip_prefix("image-")
                    .and_then(|value| value.strip_suffix(".png"))
                    .and_then(|value| value.parse::<u8>().ok())
                    .unwrap_or(0);
                Ok(Some(
                    crate::image::prepare_rgba_image(64, 64, vec![index; 64 * 64 * 4])
                        .unwrap()
                        .bytes()
                        .to_vec(),
                ))
            }
        }

        let mut source = String::from("![bad](corrupt.png)\n\n![gone](missing.png)\n\n");
        for index in 0..100 {
            source.push_str(&format!("![image-{index}](image-{index}.png)\n\n"));
        }
        let images = VaryingImages {
            loads: Cell::new(0),
        };
        let mut pipeline = PreviewPipeline::new();
        let frame = pipeline
            .build_with_image_source(
                &snapshot(&source, Generation::initial()),
                300,
                120,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&images),
            )
            .unwrap();
        assert!(frame.index().text().contains("bad"));
        assert!(frame.index().text().contains("gone"));
        assert!(pipeline.image_cache_entries() > 0);
        assert!(pipeline.image_cache_entries() < 100);
        assert!(images.loads.get() < 100);
        assert!(pipeline.image_cache_bytes() <= crate::image::IMAGE_CACHE_BUDGET_BYTES);
        assert_eq!(pipeline.counters().parses, 1);
    }

    #[test]
    fn phase11_overscroll_decodes_images_at_the_clamped_bottom_viewport() {
        let prepared = crate::image::prepare_rgba_image(64, 64, vec![160; 64 * 64 * 4]).unwrap();
        let images = MemoryImages {
            bytes: prepared.bytes().to_vec(),
            loads: Cell::new(0),
        };
        let mut source = String::new();
        for index in 0..40 {
            source.push_str(&format!("![image-{index}](image.png)\n\n"));
        }
        let generation = Generation::initial();
        let mut pipeline = PreviewPipeline::new();
        pipeline
            .build_with_image_source(
                &snapshot(&source, generation),
                300,
                120,
                1.0,
                0.0,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&images),
            )
            .unwrap();
        let initial_loads = images.loads.get();
        assert!(initial_loads < 40);

        let frame = pipeline
            .paint_with_image_source(
                generation,
                120,
                f32::MAX,
                PreviewSelection::default(),
                PreviewTheme::Light,
                Some(&images),
            )
            .unwrap();

        assert!(frame.scroll_y().is_finite());
        assert!(images.loads.get() > initial_loads);
        assert!(
            pipeline
                .layout
                .as_ref()
                .and_then(|layout| layout.blocks.last())
                .is_some_and(|block| block.chunks.iter().any(|chunk| matches!(
                    chunk.content,
                    crate::preview::layout::LayoutContent::Image(_)
                ))),
            "the final visible image must be decoded after scroll clamps to the document bottom"
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

        pipeline.release_raster_caches();
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
    fn phase11_large_fixture_retains_absolute_source_offsets() {
        let source = fixture(1024 * 1024);
        let owned = PreviewParser
            .parse(&snapshot(&source, Generation::initial()))
            .unwrap();
        let tree = RenderTreeBuilder.build(&owned);
        let mixed_blocks = tree
            .blocks
            .iter()
            .filter(|block| block.spans.iter().any(|span| span.math.is_some()))
            .count();
        let late_mixed_blocks = tree
            .blocks
            .iter()
            .filter(|block| block.spans.iter().any(|span| span.math.is_some()))
            .filter(|block| {
                block
                    .spans
                    .iter()
                    .any(|span| span.source_range.is_some_and(|range| range.end > 64 * 1024))
            })
            .count();

        assert!(mixed_blocks > 1_000);
        assert!(late_mixed_blocks * 10 > mixed_blocks * 9);
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
            LayoutResources {
                font_system: &mut font_system,
                fonts: &fonts,
                math_engine: &mut math_engine,
                image_source: None,
                image_cache: &mut crate::image::DecodedImageCache::default(),
                image_band: (0.0, f32::MAX),
            },
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
                LayoutResources {
                    font_system: &mut font_system,
                    fonts: &fonts,
                    math_engine: &mut math_engine,
                    image_source: None,
                    image_cache: &mut crate::image::DecodedImageCache::default(),
                    image_band: (0.0, f32::MAX),
                },
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
                LayoutResources {
                    font_system: &mut font_system,
                    fonts: &fonts,
                    math_engine: &mut math_engine,
                    image_source: None,
                    image_cache: &mut crate::image::DecodedImageCache::default(),
                    image_band: (0.0, f32::MAX),
                },
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
        let seed = include_str!("../../../../tests/fixtures/performance/typical-note-seed.md");
        let mut source = String::with_capacity(bytes + seed.len());
        while source.len() < bytes {
            source.push_str(seed);
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
