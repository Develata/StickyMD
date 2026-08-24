//! RaTeX semantic adapter and bounded two-level formula cache.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#ratex-native-math

use std::sync::Arc;

use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_types::color::Color;
use ratex_types::display_item::DisplayList;
use ratex_types::math_style::MathStyle;
use thiserror::Error;

use super::cache::{ByteLru, EntryLru};
use super::painter::{MAX_RASTER_BYTES, MathPaintError, MathPainter, rasterize};

pub(crate) const MAX_FORMULA_SOURCE_BYTES: usize = 64 * 1024;
pub(crate) const MAX_DOCUMENT_FORMULAS: usize = 2_000;
const MAX_LAYOUT_ENTRIES: usize = 512;
const RASTER_ENTRY_METADATA_ESTIMATE: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MathKind {
    Inline,
    Display,
}

#[derive(Debug, Clone)]
pub(crate) struct MathRaster {
    pub width: u32,
    pub height: u32,
    pub baseline: f32,
    pub pixels: Arc<[u8]>,
}

impl MathRaster {
    pub(crate) fn byte_len(&self) -> usize {
        self.pixels.len()
    }
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub(crate) enum MathError {
    #[error("formula source exceeds the 64 KiB safety limit")]
    SourceTooLong,
    #[error("document contains more than 2000 formulas")]
    TooManyFormulas,
    #[error("formula parse failed: {0}")]
    Parse(String),
    #[error("formula geometry is not finite or non-negative")]
    InvalidGeometry,
    #[error(transparent)]
    Paint(#[from] MathPaintError),
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MathEngineCounters {
    pub parse_layout_calls: u64,
    pub rasterizations: u64,
    pub layout_hits: u64,
    pub layout_misses: u64,
    pub layout_evictions: u64,
    pub raster_hits: u64,
    pub raster_misses: u64,
    pub raster_evictions: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct LayoutKey {
    source: Arc<str>,
    kind: MathKind,
    foreground: [u8; 4],
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RasterKey {
    layout: LayoutKey,
    font_size_bits: u32,
}

struct MathLayout {
    display_list: DisplayList,
}

pub(crate) struct MathEngine {
    layouts: EntryLru<LayoutKey, Arc<MathLayout>>,
    rasters: ByteLru<RasterKey, Arc<MathRaster>>,
    painter: MathPainter,
    counters: MathEngineCounters,
    raster_scale_bits: Option<u32>,
    raster_theme: Option<[u8; 4]>,
}

impl Default for MathEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl MathEngine {
    pub(crate) fn new() -> Self {
        Self {
            layouts: EntryLru::new(MAX_LAYOUT_ENTRIES),
            rasters: ByteLru::new(MAX_RASTER_BYTES),
            painter: MathPainter::new(),
            counters: MathEngineCounters::default(),
            raster_scale_bits: None,
            raster_theme: None,
        }
    }

    pub(crate) fn prepare_projection(&mut self, scale: f32, foreground: [u8; 4]) {
        let scale_bits = scale.to_bits();
        if self
            .raster_scale_bits
            .is_some_and(|previous| previous != scale_bits)
            || self
                .raster_theme
                .is_some_and(|previous| previous != foreground)
        {
            self.rasters.clear();
        }
        self.raster_scale_bits = Some(scale_bits);
        self.raster_theme = Some(foreground);
    }

    pub(crate) fn render(
        &mut self,
        source: &str,
        kind: MathKind,
        font_size_px: f32,
        foreground: [u8; 4],
    ) -> Result<Arc<MathRaster>, MathError> {
        if source.len() > MAX_FORMULA_SOURCE_BYTES {
            return Err(MathError::SourceTooLong);
        }
        if !font_size_px.is_finite() || font_size_px <= 0.0 {
            return Err(MathError::InvalidGeometry);
        }
        let layout_key = LayoutKey {
            source: Arc::from(source),
            kind,
            foreground,
        };
        let layout = if let Some(layout) = self.layouts.get(&layout_key) {
            self.counters.layout_hits = self.counters.layout_hits.saturating_add(1);
            layout
        } else {
            self.counters.layout_misses = self.counters.layout_misses.saturating_add(1);
            self.counters.parse_layout_calls = self.counters.parse_layout_calls.saturating_add(1);
            let parsed = ratex_parser::parse(source)
                .map_err(|error| MathError::Parse(sanitized_parse_error(&error.to_string())))?;
            let options = LayoutOptions {
                style: match kind {
                    MathKind::Inline => MathStyle::Text,
                    MathKind::Display => MathStyle::Display,
                },
                color: rgba_color(foreground),
                ..LayoutOptions::default()
            };
            let display_list = to_display_list(&layout(&parsed, &options));
            if !valid_display_list(&display_list) {
                return Err(MathError::InvalidGeometry);
            }
            let layout = Arc::new(MathLayout { display_list });
            if self.layouts.insert(layout_key.clone(), Arc::clone(&layout)) {
                self.counters.layout_evictions = self.counters.layout_evictions.saturating_add(1);
            }
            layout
        };

        let raster_key = RasterKey {
            layout: layout_key,
            font_size_bits: font_size_px.to_bits(),
        };
        if let Some(raster) = self.rasters.get(&raster_key) {
            self.counters.raster_hits = self.counters.raster_hits.saturating_add(1);
            return Ok(raster);
        }
        self.counters.raster_misses = self.counters.raster_misses.saturating_add(1);
        self.counters.rasterizations = self.counters.rasterizations.saturating_add(1);
        let raster = Arc::new(rasterize(
            &mut self.painter,
            &layout.display_list,
            font_size_px,
        )?);
        let evicted = self.rasters.insert(
            raster_key,
            Arc::clone(&raster),
            raster
                .byte_len()
                .saturating_add(source.len())
                .saturating_add(RASTER_ENTRY_METADATA_ESTIMATE),
        );
        self.counters.raster_evictions = self
            .counters
            .raster_evictions
            .saturating_add(evicted as u64);
        Ok(raster)
    }

    pub(crate) fn release_rasters(&mut self) {
        self.rasters.clear();
    }

    pub(crate) const fn counters(&self) -> MathEngineCounters {
        self.counters
    }

    #[cfg(test)]
    pub(crate) fn cache_sizes(&self) -> (usize, usize, usize, usize) {
        (
            self.layouts.len(),
            self.rasters.len(),
            self.rasters.bytes(),
            self.painter.outline_bytes(),
        )
    }
}

fn rgba_color(color: [u8; 4]) -> Color {
    Color::new(
        f32::from(color[0]) / 255.0,
        f32::from(color[1]) / 255.0,
        f32::from(color[2]) / 255.0,
        f32::from(color[3]) / 255.0,
    )
}

fn valid_display_list(display: &DisplayList) -> bool {
    [display.width, display.height, display.depth]
        .into_iter()
        .all(|value| value.is_finite() && value >= 0.0)
}

fn sanitized_parse_error(error: &str) -> String {
    const MAX_ERROR_CHARS: usize = 160;
    let mut result = error.chars().take(MAX_ERROR_CHARS).collect::<String>();
    if error.chars().count() > MAX_ERROR_CHARS {
        result.push('…');
    }
    result
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::*;

    const BLACK: [u8; 4] = [40, 38, 34, 255];

    #[test]
    fn repeated_formula_hits_both_bounded_cache_levels() {
        let mut engine = MathEngine::new();
        let first = engine
            .render(r"\frac{a}{b}", MathKind::Inline, 17.0, BLACK)
            .unwrap();
        for _ in 0..100 {
            let repeated = engine
                .render(r"\frac{a}{b}", MathKind::Inline, 17.0, BLACK)
                .unwrap();
            assert!(Arc::ptr_eq(&first, &repeated));
        }
        assert_eq!(engine.counters().parse_layout_calls, 1);
        assert_eq!(engine.counters().rasterizations, 1);
        assert_eq!(engine.counters().layout_hits, 100);
        assert_eq!(engine.counters().raster_hits, 100);
    }

    #[test]
    fn raster_budget_accounts_for_source_and_entry_metadata() {
        let mut engine = MathEngine::new();
        let source = r"\frac{a}{b}";
        let raster = engine
            .render(source, MathKind::Inline, 17.0, BLACK)
            .unwrap();
        let (_, _, accounted_bytes, _) = engine.cache_sizes();
        assert!(accounted_bytes >= raster.byte_len() + source.len());
        assert!(accounted_bytes <= MAX_RASTER_BYTES);
    }

    #[test]
    fn malformed_and_oversized_sources_are_isolated_errors() {
        let mut engine = MathEngine::new();
        assert!(matches!(
            engine.render(r"\frac{", MathKind::Display, 17.0, BLACK),
            Err(MathError::Parse(_))
        ));
        let oversized = "x".repeat(MAX_FORMULA_SOURCE_BYTES + 1);
        assert!(matches!(
            engine.render(&oversized, MathKind::Inline, 17.0, BLACK),
            Err(MathError::SourceTooLong)
        ));
    }

    #[test]
    fn unique_formulas_keep_both_caches_bounded() {
        let mut engine = MathEngine::new();
        for index in 0..600 {
            engine
                .render(&format!("x_{{{index}}}"), MathKind::Inline, 17.0, BLACK)
                .unwrap();
        }
        let (layouts, _rasters, raster_bytes, outline_bytes) = engine.cache_sizes();
        assert_eq!(layouts, MAX_LAYOUT_ENTRIES);
        assert!(raster_bytes <= MAX_RASTER_BYTES);
        assert!(outline_bytes <= 4 * 1024 * 1024);
        assert!(engine.counters().layout_evictions > 0);
    }

    #[test]
    fn scale_or_theme_change_releases_only_raster_cache() {
        let mut engine = MathEngine::new();
        engine.prepare_projection(1.0, BLACK);
        engine.render("x", MathKind::Inline, 17.0, BLACK).unwrap();
        assert_eq!(engine.cache_sizes().0, 1);
        assert_eq!(engine.cache_sizes().1, 1);
        engine.prepare_projection(2.0, BLACK);
        assert_eq!(engine.cache_sizes().0, 1);
        assert_eq!(engine.cache_sizes().1, 0);
    }

    #[test]
    fn mixed_cjk_text_formula_is_safe_without_a_platform_cjk_font() {
        let mut engine = MathEngine::new();
        let raster = engine
            .render(r"\text{中文 Rust}", MathKind::Inline, 17.0, BLACK)
            .unwrap();
        assert!(raster.width > 0 && raster.height > 0);
        assert_eq!(
            raster.pixels.len(),
            raster.width as usize * raster.height as usize * 4
        );
        // Linux portable-core CI intentionally does not install a CJK font.
        // The Latin run must still rasterize while missing native glyphs stay
        // a safe projection concern rather than a panic or malformed buffer.
        assert!(raster.pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn cjk_text_formula_uses_windows_native_fallback() {
        let mut engine = MathEngine::new();
        let raster = engine
            .render(r"\text{中文}", MathKind::Inline, 17.0, BLACK)
            .unwrap();
        assert!(raster.width > 0 && raster.height > 0);
        assert!(raster.pixels.chunks_exact(4).any(|pixel| pixel[3] != 0));
    }

    #[test]
    fn ten_thousand_deterministic_formula_inputs_never_panic() {
        const VALID: [&str; 12] = [
            "x^2",
            r"\frac{a}{b}",
            r"\sqrt{x}",
            r"\sum_{n=1}^{\infty}",
            r"\int_0^1 x^2\,dx",
            r"\left(\frac{x}{y}\right)",
            r"\begin{matrix}a&b\\c&d\end{matrix}",
            r"\begin{cases}x,&x>0\\-x,&x<0\end{cases}",
            r"\mathbb{R}",
            r"\mathbf{x}",
            r"\operatorname{rank}(A)",
            r"\text{中文 Rust}",
        ];
        const MALFORMED: [&str; 6] = [
            r"\frac{",
            r"\sqrt{",
            r"\begin{matrix}",
            r"\left(",
            r"x^{",
            "}",
        ];
        let options = LayoutOptions {
            style: MathStyle::Text,
            color: rgba_color(BLACK),
            ..LayoutOptions::default()
        };
        let mut seed = 0x51A6_EC0D_1234_5678u64;
        for index in 0..10_000 {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            let source = if index % 5 == 0 {
                MALFORMED[(seed as usize) % MALFORMED.len()]
            } else {
                VALID[(seed as usize) % VALID.len()]
            };
            if let Ok(parsed) = ratex_parser::parse(source) {
                let display = to_display_list(&layout(&parsed, &options));
                assert!(
                    valid_display_list(&display),
                    "invalid geometry for {source:?}"
                );
            }
        }
    }

    #[test]
    #[ignore = "Release-only Phase 6 formula performance receipt"]
    fn phase6_formula_release_baseline() {
        let source = r"\begin{pmatrix}\frac{a}{b}&\sqrt{x}\\\sum_{n=1}^{\infty}n^{-2}&\int_0^1x^2\,dx\end{pmatrix}";
        let options = LayoutOptions {
            style: MathStyle::Display,
            color: rgba_color(BLACK),
            ..LayoutOptions::default()
        };
        let total_started = Instant::now();
        let parse_started = Instant::now();
        let parsed = ratex_parser::parse(source).unwrap();
        let parse = parse_started.elapsed();
        let layout_started = Instant::now();
        let laid_out = layout(&parsed, &options);
        let layout_time = layout_started.elapsed();
        let display_started = Instant::now();
        let display = to_display_list(&laid_out);
        let display_time = display_started.elapsed();
        let raster_started = Instant::now();
        let mut painter = MathPainter::new();
        let _raster = rasterize(&mut painter, &display, 17.0).unwrap();
        let raster = raster_started.elapsed();
        let cold_total = total_started.elapsed();

        let simple = measure_warm("x^2", 100);
        let fraction = measure_warm(r"\frac{a}{b}", 100);
        let matrix = measure_warm(r"\begin{matrix}a&b\\c&d\end{matrix}", 100);
        let complex = measure_warm(source, 100);
        println!(
            "phase6 cold_first parse={parse:?} layout={layout_time:?} display={display_time:?} raster_font={raster:?} total={cold_total:?} warm_simple={:?}/{:?}/{:?} warm_fraction={:?}/{:?}/{:?} warm_matrix={:?}/{:?}/{:?} warm_complex={:?}/{:?}/{:?}",
            percentile(&simple, 50),
            percentile(&simple, 95),
            max(&simple),
            percentile(&fraction, 50),
            percentile(&fraction, 95),
            max(&fraction),
            percentile(&matrix, 50),
            percentile(&matrix, 95),
            max(&matrix),
            percentile(&complex, 50),
            percentile(&complex, 95),
            max(&complex),
        );
        assert!(cold_total < Duration::from_millis(200));
        assert!(percentile(&simple, 95) < Duration::from_millis(5));
        assert!(percentile(&fraction, 95) < Duration::from_millis(10));
        assert!(percentile(&matrix, 95) < Duration::from_millis(20));
        assert!(percentile(&complex, 95) < Duration::from_millis(20));
    }

    fn measure_warm(source: &str, repeats: usize) -> Vec<Duration> {
        let mut engine = MathEngine::new();
        engine
            .render(source, MathKind::Display, 17.0, BLACK)
            .unwrap();
        (0..repeats)
            .map(|_| {
                let started = Instant::now();
                engine
                    .render(source, MathKind::Display, 17.0, BLACK)
                    .unwrap();
                started.elapsed()
            })
            .collect()
    }

    fn percentile(samples: &[Duration], percentile: usize) -> Duration {
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        sorted[(sorted.len().saturating_sub(1) * percentile) / 100]
    }

    fn max(samples: &[Duration]) -> Duration {
        samples.iter().copied().max().unwrap_or_default()
    }
}
