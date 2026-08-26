//! DisplayList painter for StickyMD.
//!
//! Adapted narrowly from RaTeX 0.1.14's MIT-licensed `ratex-render`
//! renderer. StickyMD retains only direct tiny-skia painting; PNG encoding,
//! background composition, and math parsing/layout are intentionally absent.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#ratex-native-math

use std::collections::HashMap;
use std::sync::Arc;

use ab_glyph::{Font, FontRef, OutlineCurve};
use ratex_font::FontId;
use ratex_font_loader::FontSet;
use ratex_types::color::Color;
use ratex_types::display_item::{DisplayItem, DisplayList};
use tiny_skia::{FillRule, Paint, PathBuilder, Pixmap, Rect, Transform};

use super::cache::ByteLru;
use super::engine::{DEFAULT_COLOR_SENTINEL, MathRaster};
use super::path_painter::paint_path;

pub(super) const MAX_RASTER_BYTES: usize = 8 * 1024 * 1024;
const MAX_RASTER_SIDE: u32 = 16_384;
const PADDING_DIP: f32 = 2.0;
const BASE_FONT_DIP: f32 = 17.0;
const MAX_OUTLINE_CACHE_BYTES: usize = 4 * 1024 * 1024;
const OUTLINE_ENTRY_METADATA_ESTIMATE: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font: FontId,
    character: char,
}

#[derive(Clone)]
struct CachedOutline {
    curves: Arc<[OutlineCurve]>,
    units_per_em: f32,
}

pub(super) struct MathPainter {
    outlines: ByteLru<GlyphKey, Arc<CachedOutline>>,
}

impl MathPainter {
    pub(super) fn new() -> Self {
        Self {
            outlines: ByteLru::new(MAX_OUTLINE_CACHE_BYTES),
        }
    }

    #[cfg(test)]
    pub(super) const fn outline_bytes(&self) -> usize {
        self.outlines.bytes()
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub(crate) enum MathPaintError {
    #[error("formula raster geometry is invalid")]
    InvalidGeometry,
    #[error("formula raster exceeds the 8 MiB safety budget")]
    RasterTooLarge,
    #[error("formula raster allocation failed for {width}x{height}")]
    Allocation { width: u32, height: u32 },
    #[error("formula font loading failed: {0}")]
    FontLoad(String),
    #[error("formula font data is invalid: {0}")]
    InvalidFont(String),
}

pub(super) fn rasterize(
    painter: &mut MathPainter,
    display: &DisplayList,
    font_size_px: f32,
    foreground: [u8; 4],
) -> Result<MathRaster, MathPaintError> {
    let padding_px = PADDING_DIP * (font_size_px / BASE_FONT_DIP);
    let width = raster_dimension(display.width, font_size_px, padding_px)?;
    let height = raster_dimension(display.total_height(), font_size_px, padding_px)?;
    let bytes = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or(MathPaintError::RasterTooLarge)?;
    if width > MAX_RASTER_SIDE || height > MAX_RASTER_SIDE || bytes > MAX_RASTER_BYTES {
        return Err(MathPaintError::RasterTooLarge);
    }
    let mut pixmap =
        Pixmap::new(width, height).ok_or(MathPaintError::Allocation { width, height })?;
    let fonts = ratex_font_loader::load_fonts_for_items("", &display.items)
        .map_err(MathPaintError::FontLoad)?;
    let font_refs = build_font_refs(&fonts)?;
    paint_display_list(
        painter,
        &mut pixmap,
        display,
        &font_refs,
        font_size_px,
        padding_px,
        foreground,
    );
    Ok(MathRaster {
        width,
        height,
        baseline: padding_px + display.height as f32 * font_size_px,
        pixels: Arc::from(pixmap.data()),
    })
}

fn raster_dimension(em: f64, font_size_px: f32, padding_px: f32) -> Result<u32, MathPaintError> {
    let pixels = em * f64::from(font_size_px) + f64::from(padding_px * 2.0);
    if !pixels.is_finite() || pixels < 0.0 || pixels > f64::from(u32::MAX) {
        return Err(MathPaintError::InvalidGeometry);
    }
    Ok((pixels.ceil() as u32).max(1))
}

fn build_font_refs(data: &FontSet) -> Result<HashMap<FontId, FontRef<'_>>, MathPaintError> {
    let mut fonts = HashMap::new();
    for (id, bytes) in data.iter() {
        let font = FontRef::try_from_slice_and_index(bytes, collection_index(*id))
            .map_err(|error| MathPaintError::InvalidFont(format!("{}: {error}", id.as_str())))?;
        fonts.insert(*id, font);
    }
    if !fonts.contains_key(&FontId::MainRegular) {
        return Err(MathPaintError::FontLoad(
            "Main-Regular font not found".to_owned(),
        ));
    }
    Ok(fonts)
}

fn collection_index(id: FontId) -> u32 {
    match id {
        FontId::CjkRegular => ratex_unicode_font::unicode_font_face_index().unwrap_or(0),
        FontId::CjkFallback => ratex_unicode_font::fallback_font_face_index().unwrap_or(0),
        FontId::EmojiFallback => ratex_unicode_font::emoji_font_face_index().unwrap_or(0),
        _ => 0,
    }
}

fn paint_display_list(
    painter: &mut MathPainter,
    pixmap: &mut Pixmap,
    display: &DisplayList,
    fonts: &HashMap<FontId, FontRef<'_>>,
    em: f32,
    padding: f32,
    foreground: [u8; 4],
) {
    let foreground = rgba_color(foreground);
    for item in &display.items {
        match item {
            DisplayItem::GlyphPath {
                x,
                y,
                scale,
                font,
                char_code,
                color,
            } => {
                let color = effective_color(color, &foreground);
                paint_glyph(
                    painter,
                    pixmap,
                    padding + *x as f32 * em,
                    padding + *y as f32 * em,
                    FontId::parse(font).unwrap_or(FontId::MainRegular),
                    *char_code,
                    color,
                    fonts,
                    em * *scale as f32,
                )
            }
            DisplayItem::Line {
                x,
                y,
                width,
                thickness,
                color,
                dashed,
            } => {
                let color = effective_color(color, &foreground);
                paint_line(
                    pixmap,
                    padding + *x as f32 * em,
                    padding + *y as f32 * em,
                    *width as f32 * em,
                    *thickness as f32 * em,
                    color,
                    *dashed,
                )
            }
            DisplayItem::Rect {
                x,
                y,
                width,
                height,
                color,
            } => {
                let color = effective_color(color, &foreground);
                paint_rect(
                    pixmap,
                    padding + *x as f32 * em,
                    padding + *y as f32 * em,
                    *width as f32 * em,
                    *height as f32 * em,
                    color,
                )
            }
            DisplayItem::Path {
                x,
                y,
                commands,
                fill,
                color,
            } => {
                let color = effective_color(color, &foreground);
                paint_path(
                    pixmap,
                    padding + *x as f32 * em,
                    padding + *y as f32 * em,
                    commands,
                    *fill,
                    color,
                    em,
                )
            }
        }
    }
}

fn effective_color<'a>(color: &'a Color, foreground: &'a Color) -> &'a Color {
    if *color == DEFAULT_COLOR_SENTINEL {
        foreground
    } else {
        color
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

#[allow(clippy::too_many_arguments)]
fn paint_glyph(
    painter: &mut MathPainter,
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    requested: FontId,
    char_code: u32,
    color: &Color,
    fonts: &HashMap<FontId, FontRef<'_>>,
    em: f32,
) {
    let ch = ratex_font::katex_ttf_glyph_char(requested, char_code);
    let candidates = [
        requested,
        FontId::MainRegular,
        FontId::CjkRegular,
        FontId::CjkFallback,
        FontId::EmojiFallback,
    ];
    for id in candidates {
        let Some(font) = fonts.get(&id) else {
            continue;
        };
        let key = GlyphKey {
            font: id,
            character: ch,
        };
        let outline = if let Some(outline) = painter.outlines.get(&key) {
            outline
        } else {
            let glyph = font.glyph_id(ch);
            if glyph.0 == 0 {
                continue;
            }
            let Some(outline) = font.outline(glyph) else {
                continue;
            };
            if outline.curves.is_empty() {
                continue;
            }
            let cached = Arc::new(CachedOutline {
                units_per_em: font.units_per_em().unwrap_or(1_000.0),
                curves: Arc::from(outline.curves),
            });
            let bytes = cached
                .curves
                .len()
                .saturating_mul(std::mem::size_of::<OutlineCurve>())
                .saturating_add(OUTLINE_ENTRY_METADATA_ESTIMATE);
            painter.outlines.insert(key, Arc::clone(&cached), bytes);
            cached
        };
        if paint_outline(pixmap, x, y, &outline, color, em) {
            return;
        }
    }
}

fn paint_outline(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    outline: &CachedOutline,
    color: &Color,
    em: f32,
) -> bool {
    let scale = em / outline.units_per_em;
    let mut builder = PathBuilder::new();
    let mut previous_end: Option<(f32, f32)> = None;
    for curve in outline.curves.iter() {
        let (start, end) = curve_ends(curve, x, y, scale);
        let begins_contour = previous_end.is_none_or(|previous| {
            (previous.0 - start.0).abs() > 0.01 || (previous.1 - start.1).abs() > 0.01
        });
        if begins_contour {
            if previous_end.is_some() {
                builder.close();
            }
            builder.move_to(start.0, start.1);
        }
        match curve {
            OutlineCurve::Line(_, p1) => builder.line_to(x + p1.x * scale, y - p1.y * scale),
            OutlineCurve::Quad(_, p1, p2) => builder.quad_to(
                x + p1.x * scale,
                y - p1.y * scale,
                x + p2.x * scale,
                y - p2.y * scale,
            ),
            OutlineCurve::Cubic(_, p1, p2, p3) => builder.cubic_to(
                x + p1.x * scale,
                y - p1.y * scale,
                x + p2.x * scale,
                y - p2.y * scale,
                x + p3.x * scale,
                y - p3.y * scale,
            ),
        }
        previous_end = Some(end);
    }
    if previous_end.is_some() {
        builder.close();
    }
    let Some(path) = builder.finish() else {
        return false;
    };
    let mut paint = paint_for(color);
    paint.anti_alias = true;
    pixmap.fill_path(
        &path,
        &paint,
        FillRule::Winding,
        Transform::identity(),
        None,
    );
    true
}

fn curve_ends(curve: &OutlineCurve, x: f32, y: f32, scale: f32) -> ((f32, f32), (f32, f32)) {
    let (start, end) = match curve {
        OutlineCurve::Line(start, end) | OutlineCurve::Quad(start, _, end) => (start, end),
        OutlineCurve::Cubic(start, _, _, end) => (start, end),
    };
    (
        (x + start.x * scale, y - start.y * scale),
        (x + end.x * scale, y - end.y * scale),
    )
}

fn paint_line(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    thickness: f32,
    color: &Color,
    dashed: bool,
) {
    let thickness = thickness.max(1.0);
    if dashed {
        let dash = (4.0 * thickness).max(2.0);
        let period = dash * 2.0;
        let mut current = x;
        while current < x + width {
            let segment = dash.min(x + width - current).max(0.0);
            fill_rect(
                pixmap,
                current,
                y - thickness / 2.0,
                segment,
                thickness,
                color,
            );
            current += period;
        }
    } else {
        fill_rect(pixmap, x, y - thickness / 2.0, width, thickness, color);
    }
}

fn paint_rect(pixmap: &mut Pixmap, x: f32, y: f32, width: f32, height: f32, color: &Color) {
    if width < 2.0 || height < 2.0 {
        let Some(rect) = Rect::from_xywh(x, y, width, height) else {
            return;
        };
        let mut paint = paint_for(color);
        paint.anti_alias = true;
        pixmap.fill_path(
            &PathBuilder::from_rect(rect),
            &paint,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    } else {
        fill_rect(pixmap, x, y, width, height, color);
    }
}

fn fill_rect(pixmap: &mut Pixmap, x: f32, y: f32, width: f32, height: f32, color: &Color) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let Some(rect) = Rect::from_xywh(x, y, width, height) else {
        return;
    };
    pixmap.fill_rect(rect, &paint_for(color), Transform::identity(), None);
}

fn paint_for(color: &Color) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color_rgba8(
        channel(color.r),
        channel(color.g),
        channel(color.b),
        channel(color.a),
    );
    paint
}

fn channel(value: f32) -> u8 {
    if value.is_finite() {
        (value.clamp(0.0, 1.0) * 255.0).round() as u8
    } else {
        0
    }
}

#[cfg(test)]
mod tests {
    use ratex_layout::{LayoutOptions, layout, to_display_list};
    use ratex_types::color::Color;
    use ratex_types::math_style::MathStyle;
    use ratex_types::path_command::PathCommand;

    use super::*;

    #[test]
    fn painter_covers_every_display_item_variant_without_png_roundtrip() {
        let display = DisplayList {
            width: 4.0,
            height: 1.0,
            depth: 0.4,
            items: vec![
                DisplayItem::GlyphPath {
                    x: 0.0,
                    y: 1.0,
                    scale: 1.0,
                    font: FontId::MainRegular.as_str().to_owned(),
                    char_code: 'x' as u32,
                    color: Color::BLACK,
                },
                DisplayItem::Line {
                    x: 1.0,
                    y: 0.5,
                    width: 1.0,
                    thickness: 0.05,
                    color: Color::BLACK,
                    dashed: true,
                },
                DisplayItem::Rect {
                    x: 2.0,
                    y: 0.3,
                    width: 0.4,
                    height: 0.2,
                    color: Color::BLACK,
                },
                DisplayItem::Path {
                    x: 2.6,
                    y: 0.2,
                    commands: vec![
                        PathCommand::MoveTo { x: 0.0, y: 0.0 },
                        PathCommand::LineTo { x: 0.8, y: 0.0 },
                        PathCommand::QuadTo {
                            x1: 1.0,
                            y1: 0.4,
                            x: 0.8,
                            y: 0.8,
                        },
                        PathCommand::CubicTo {
                            x1: 0.6,
                            y1: 1.0,
                            x2: 0.2,
                            y2: 1.0,
                            x: 0.0,
                            y: 0.8,
                        },
                        PathCommand::Close,
                    ],
                    fill: true,
                    color: Color::BLACK,
                },
            ],
        };
        let mut painter = MathPainter::new();
        let raster = rasterize(&mut painter, &display, 20.0, [0, 0, 0, 255]).unwrap();
        assert!(raster.width > 0 && raster.height > 0);
        assert!(raster.pixels.iter().any(|value| *value != 0));
    }

    #[test]
    fn allocation_guard_rejects_pathological_geometry_before_allocating() {
        let display = DisplayList {
            width: 1_000_000.0,
            height: 1.0,
            depth: 0.0,
            items: Vec::new(),
        };
        assert!(matches!(
            rasterize(&mut MathPainter::new(), &display, 40.0, [0, 0, 0, 255]),
            Err(MathPaintError::RasterTooLarge)
        ));
    }

    #[test]
    fn embedded_font_raster_golden_covers_core_display_geometry() {
        let fixtures = [
            r"\frac{a}{b}",
            r"\sqrt{x^2+y^2}",
            r"\begin{matrix}a&b\\c&d\end{matrix}",
            r"\left(\frac{x}{y}\right)",
            r"\color{red}{x^2}+y^2",
        ];
        let mut painter = MathPainter::new();
        let actual = fixtures.map(|source| {
            let parsed = ratex_parser::parse(source).expect("golden formula parses");
            let display = to_display_list(&layout(
                &parsed,
                &LayoutOptions {
                    style: MathStyle::Display,
                    color: Color::BLACK,
                    ..LayoutOptions::default()
                },
            ));
            let raster = rasterize(&mut painter, &display, 17.0, [0, 0, 0, 255])
                .expect("golden raster paints");
            let alpha_coverage = raster
                .pixels
                .chunks_exact(4)
                .filter(|pixel| pixel[3] != 0)
                .count();
            let hash = raster
                .pixels
                .iter()
                .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                    (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
                });
            (
                raster.width,
                raster.height,
                raster.baseline.to_bits(),
                alpha_coverage,
                hash,
            )
        });
        const EXPECTED: [(u32, u32, u32, usize, u64); 5] = [
            (18, 35, 1_101_439_183, 122, 15_705_868_895_198_505_677),
            (75, 26, 1_100_540_308, 394, 8_165_363_465_731_752_547),
            (39, 45, 1_104_491_316, 194, 13_362_824_739_131_903_429),
            (43, 45, 1_104_491_316, 341, 12_213_990_901_879_308_804),
            (58, 22, 1_099_269_321, 209, 14_105_165_786_631_227_363),
        ];
        assert_eq!(actual, EXPECTED);
    }
}
