//! CPU-pixmap primitives shared by source projection painting.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use cosmic_text::Color;
use tiny_skia::{Paint, Pixmap, Rect, Transform};

pub(crate) fn fill(pixmap: &mut Pixmap, color: (u8, u8, u8, u8)) {
    pixmap.fill(tiny_skia::Color::from_rgba8(
        color.0, color.1, color.2, color.3,
    ));
}

pub(crate) fn rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: (u8, u8, u8, u8),
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let Some(rectangle) = Rect::from_xywh(x, y, width, height) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color_rgba8(color.0, color.1, color.2, color.3);
    pixmap.fill_rect(rectangle, &paint, Transform::identity(), None);
}

pub(crate) fn blend_glyph_rect(
    pixmap: &mut Pixmap,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    color: Color,
) {
    let pixmap_width = pixmap.width() as i32;
    let pixmap_height = pixmap.height() as i32;
    let source_alpha = color.a() as u32;
    if source_alpha == 0 {
        return;
    }

    let source = (color.r() as u32, color.g() as u32, color.b() as u32);
    let data = pixmap.data_mut();
    for row in y..y.saturating_add(height as i32) {
        if row < 0 || row >= pixmap_height {
            continue;
        }
        for column in x..x.saturating_add(width as i32) {
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
