//! Pixel primitives and the reusable overflow clip owned by preview paint.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-horizontal-scroll

use super::{DecorationRole, LayoutDecoration, PreviewPaintError, PreviewRect, PreviewTheme};
use cosmic_text::Color;
use tiny_skia::{Mask, Paint, PathBuilder, Pixmap, Rect, Stroke, Transform};

pub(super) fn block_mask(
    mask: &mut Option<Mask>,
    width: u32,
    height: u32,
    clip: Option<PreviewRect>,
    scroll_y: f32,
) -> Result<Option<&Mask>, PreviewPaintError> {
    let Some(clip) = clip else {
        return Ok(None);
    };
    if mask.is_none() {
        *mask =
            Some(Mask::new(width, height).ok_or(PreviewPaintError::Allocation { width, height })?);
    }
    let mask = mask.as_mut().expect("allocated mask");
    mask.data_mut().fill(0);
    if let Some(rect) = Rect::from_xywh(clip.x, clip.y - scroll_y, clip.width, clip.height) {
        mask.fill_path(
            &PathBuilder::from_rect(rect),
            tiny_skia::FillRule::Winding,
            false,
            Transform::identity(),
        );
    }
    Ok(Some(mask))
}

#[derive(Clone, Copy)]
pub(super) struct Palette {
    pub(super) background: tiny_skia::Color,
    pub(super) text: Color,
    pub(super) selection: tiny_skia::Color,
    pub(super) quote: tiny_skia::Color,
    pub(super) code: tiny_skia::Color,
    pub(super) math: tiny_skia::Color,
    pub(super) rule: tiny_skia::Color,
    pub(super) table: tiny_skia::Color,
    pub(super) table_header: tiny_skia::Color,
    pub(super) table_border: tiny_skia::Color,
    pub(super) math_error: tiny_skia::Color,
}

impl Palette {
    pub(super) fn for_theme(theme: PreviewTheme) -> Self {
        match theme {
            PreviewTheme::Light => Self {
                background: rgba(248, 246, 239, 255),
                text: Color::rgb(40, 38, 34),
                selection: rgba(176, 207, 243, 210),
                quote: rgba(148, 142, 126, 255),
                code: rgba(238, 235, 226, 255),
                math: rgba(243, 239, 228, 255),
                rule: rgba(180, 176, 164, 255),
                table: rgba(248, 246, 239, 255),
                table_header: rgba(235, 232, 222, 255),
                table_border: rgba(186, 181, 168, 255),
                math_error: rgba(190, 73, 55, 255),
            },
            PreviewTheme::Dark => Self {
                background: rgba(35, 35, 33, 255),
                text: Color::rgb(226, 223, 214),
                selection: rgba(68, 101, 142, 220),
                quote: rgba(130, 130, 124, 255),
                code: rgba(48, 48, 45, 255),
                math: rgba(52, 50, 44, 255),
                rule: rgba(86, 85, 80, 255),
                table: rgba(35, 35, 33, 255),
                table_header: rgba(49, 49, 46, 255),
                table_border: rgba(87, 86, 80, 255),
                math_error: rgba(232, 120, 102, 255),
            },
        }
    }
}

pub(super) fn paint_decoration(
    pixmap: &mut Pixmap,
    decoration: LayoutDecoration,
    scroll_y: f32,
    palette: Palette,
    mask: Option<&Mask>,
) {
    let rect = decoration.rect;
    let y = rect.y - scroll_y;
    match decoration.role {
        DecorationRole::QuoteBar => fill_rect(
            pixmap,
            rect.x,
            y,
            rect.width,
            rect.height,
            palette.quote,
            mask,
        ),
        DecorationRole::CodeBackground => fill_rect(
            pixmap,
            rect.x,
            y,
            rect.width,
            rect.height,
            palette.code,
            mask,
        ),
        DecorationRole::MathBackground => fill_rect(
            pixmap,
            rect.x,
            y,
            rect.width,
            rect.height,
            palette.math,
            mask,
        ),
        DecorationRole::MathError => {
            stroke_rect(
                pixmap,
                rect.x,
                y,
                rect.width,
                rect.height,
                palette.math_error,
                mask,
            );
            // Keep a compact error marker visible even when a long formula
            // literal makes the subtle border easy to miss.
            fill_rect(
                pixmap,
                rect.right() - 5.0,
                y + 1.0,
                4.0,
                4.0,
                palette.math_error,
                mask,
            );
        }
        DecorationRole::Rule => fill_rect(
            pixmap,
            rect.x,
            y,
            rect.width,
            rect.height,
            palette.rule,
            mask,
        ),
        DecorationRole::TableCell | DecorationRole::TableHeader => {
            let fill = if decoration.role == DecorationRole::TableHeader {
                palette.table_header
            } else {
                palette.table
            };
            fill_rect(pixmap, rect.x, y, rect.width, rect.height, fill, mask);
            stroke_rect(
                pixmap,
                rect.x,
                y,
                rect.width,
                rect.height,
                palette.table_border,
                mask,
            );
        }
    }
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(r, g, b, a)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn fill_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: tiny_skia::Color,
    mask: Option<&Mask>,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let Some(rect) = Rect::from_xywh(x, y, width, height) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color(color);
    pixmap.fill_rect(rect, &paint, Transform::identity(), mask);
}

#[allow(clippy::too_many_arguments)]
fn stroke_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: tiny_skia::Color,
    mask: Option<&Mask>,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let Some(rect) = Rect::from_xywh(x, y, width, height) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color(color);
    let stroke = Stroke {
        width: 1.0,
        ..Stroke::default()
    };
    let path = PathBuilder::from_rect(rect);
    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), mask);
}

pub(super) fn blend_glyph_rect(
    pixmap: &mut Pixmap,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    color: Color,
    clip: PreviewRect,
) {
    let pixmap_width = pixmap.width() as i32;
    let pixmap_height = pixmap.height() as i32;
    let source_alpha = color.a() as u32;
    if source_alpha == 0 {
        return;
    }
    let source = (color.r() as u32, color.g() as u32, color.b() as u32);
    let data = pixmap.data_mut();
    for row in y.max(clip.y.ceil() as i32)
        ..y.saturating_add(height as i32)
            .min(clip.bottom().ceil() as i32)
    {
        if row < 0 || row >= pixmap_height {
            continue;
        }
        for column in x.max(clip.x.ceil() as i32)
            ..x.saturating_add(width as i32)
                .min(clip.right().ceil() as i32)
        {
            if column < 0 || column >= pixmap_width {
                continue;
            }
            let offset = ((row * pixmap_width + column) * 4) as usize;
            let inverse = 255 - source_alpha;
            data[offset] = ((source.0 * source_alpha + data[offset] as u32 * inverse) / 255) as u8;
            data[offset + 1] =
                ((source.1 * source_alpha + data[offset + 1] as u32 * inverse) / 255) as u8;
            data[offset + 2] =
                ((source.2 * source_alpha + data[offset + 2] as u32 * inverse) / 255) as u8;
            data[offset + 3] = 255;
        }
    }
}
