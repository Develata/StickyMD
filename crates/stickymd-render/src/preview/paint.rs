//! Viewport-culling software paint for native preview blocks.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use std::sync::Arc;

use cosmic_text::{Color, FontSystem, SwashCache};
use tiny_skia::{Paint, PathBuilder, Pixmap, PixmapPaint, PixmapRef, Rect, Stroke, Transform};

use super::layout::{DecorationRole, LaidOutDocument, LayoutContent, LayoutDecoration};
use super::{PreviewSelection, PreviewTextIndex};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PreviewTheme {
    #[default]
    Light,
    Dark,
}

#[derive(Debug, Clone)]
pub struct PreviewFrame {
    generation: stickymd_core::Generation,
    width: u32,
    height: u32,
    document_height: f32,
    scroll_y: f32,
    rgba: Vec<u8>,
    index: Arc<PreviewTextIndex>,
    visible_blocks: usize,
}

impl PreviewFrame {
    pub const fn generation(&self) -> stickymd_core::Generation {
        self.generation
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn document_height(&self) -> f32 {
        self.document_height
    }

    pub const fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn index(&self) -> &Arc<PreviewTextIndex> {
        &self.index
    }

    pub const fn visible_blocks(&self) -> usize {
        self.visible_blocks
    }

    /// O(viewport pixels) clipped copy into the shared software framebuffer.
    pub fn blit_to(&self, target: &mut Pixmap, origin_x: u32, origin_y: u32) {
        let copy_width = self.width.min(target.width().saturating_sub(origin_x));
        let copy_height = self.height.min(target.height().saturating_sub(origin_y));
        let target_stride = target.width() as usize * 4;
        let source_stride = self.width as usize * 4;
        let row_bytes = copy_width as usize * 4;
        for row in 0..copy_height as usize {
            let source_start = row * source_stride;
            let target_start = (origin_y as usize + row) * target_stride + origin_x as usize * 4;
            target.data_mut()[target_start..target_start + row_bytes]
                .copy_from_slice(&self.rgba[source_start..source_start + row_bytes]);
        }
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum PreviewPaintError {
    #[error("preview viewport must have non-zero width and height")]
    InvalidViewport,
    #[error("preview pixmap allocation failed for {width}x{height}")]
    Allocation { width: u32, height: u32 },
}

pub(super) fn paint_document(
    font_system: &mut FontSystem,
    swash_cache: &mut SwashCache,
    document: &mut LaidOutDocument,
    height: u32,
    requested_scroll_y: f32,
    selection: PreviewSelection,
    theme: PreviewTheme,
) -> Result<PreviewFrame, PreviewPaintError> {
    if document.width_px == 0 || height == 0 {
        return Err(PreviewPaintError::InvalidViewport);
    }
    let mut pixmap =
        Pixmap::new(document.width_px, height).ok_or(PreviewPaintError::Allocation {
            width: document.width_px,
            height,
        })?;
    let palette = Palette::for_theme(theme);
    pixmap.fill(palette.background);
    let max_scroll = (document.height_px - height as f32).max(0.0);
    let scroll_y = requested_scroll_y.clamp(0.0, max_scroll);
    let viewport_bottom = scroll_y + height as f32;
    let start = document
        .blocks
        .partition_point(|block| block.bottom < scroll_y);
    let end = document
        .blocks
        .partition_point(|block| block.top <= viewport_bottom);

    for block in &document.blocks[start..end] {
        for decoration in &block.decorations {
            paint_decoration(&mut pixmap, *decoration, scroll_y, palette);
        }
    }
    if !selection.is_collapsed() {
        for rectangle in document.index.selection_rects(selection) {
            if rectangle.bottom() >= scroll_y && rectangle.y <= viewport_bottom {
                fill_rect(
                    &mut pixmap,
                    rectangle.x,
                    rectangle.y - scroll_y,
                    rectangle.width,
                    rectangle.height,
                    palette.selection,
                );
            }
        }
    }
    for block in &mut document.blocks[start..end] {
        for chunk in &mut block.chunks {
            let origin_x = chunk.x.round() as i32;
            let origin_y = (chunk.y - scroll_y).round() as i32;
            match &mut chunk.content {
                LayoutContent::Text(buffer) => buffer.draw(
                    font_system,
                    swash_cache,
                    palette.text,
                    |x, y, width, height, color| {
                        blend_glyph_rect(
                            &mut pixmap,
                            x + origin_x,
                            y + origin_y,
                            width,
                            height,
                            color,
                        );
                    },
                ),
                LayoutContent::Math(raster) => {
                    if let Some(source) =
                        PixmapRef::from_bytes(&raster.pixels, raster.width, raster.height)
                    {
                        pixmap.draw_pixmap(
                            origin_x,
                            origin_y,
                            source,
                            &PixmapPaint::default(),
                            Transform::identity(),
                            None,
                        );
                    }
                }
            }
        }
    }

    Ok(PreviewFrame {
        generation: document.generation,
        width: document.width_px,
        height,
        document_height: document.height_px,
        scroll_y,
        rgba: pixmap.data().to_vec(),
        index: Arc::clone(&document.index),
        visible_blocks: end.saturating_sub(start),
    })
}

#[derive(Clone, Copy)]
struct Palette {
    background: tiny_skia::Color,
    text: Color,
    selection: tiny_skia::Color,
    quote: tiny_skia::Color,
    code: tiny_skia::Color,
    math: tiny_skia::Color,
    rule: tiny_skia::Color,
    table: tiny_skia::Color,
    table_header: tiny_skia::Color,
    table_border: tiny_skia::Color,
    math_error: tiny_skia::Color,
}

impl Palette {
    fn for_theme(theme: PreviewTheme) -> Self {
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

fn paint_decoration(
    pixmap: &mut Pixmap,
    decoration: LayoutDecoration,
    scroll_y: f32,
    palette: Palette,
) {
    let rect = decoration.rect;
    let y = rect.y - scroll_y;
    match decoration.role {
        DecorationRole::QuoteBar => {
            fill_rect(pixmap, rect.x, y, rect.width, rect.height, palette.quote)
        }
        DecorationRole::CodeBackground => {
            fill_rect(pixmap, rect.x, y, rect.width, rect.height, palette.code)
        }
        DecorationRole::MathBackground => {
            fill_rect(pixmap, rect.x, y, rect.width, rect.height, palette.math)
        }
        DecorationRole::MathError => {
            stroke_rect(
                pixmap,
                rect.x,
                y,
                rect.width,
                rect.height,
                palette.math_error,
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
            );
        }
        DecorationRole::Rule => fill_rect(pixmap, rect.x, y, rect.width, rect.height, palette.rule),
        DecorationRole::TableCell | DecorationRole::TableHeader => {
            let fill = if decoration.role == DecorationRole::TableHeader {
                palette.table_header
            } else {
                palette.table
            };
            fill_rect(pixmap, rect.x, y, rect.width, rect.height, fill);
            stroke_rect(
                pixmap,
                rect.x,
                y,
                rect.width,
                rect.height,
                palette.table_border,
            );
        }
    }
}

fn rgba(r: u8, g: u8, b: u8, a: u8) -> tiny_skia::Color {
    tiny_skia::Color::from_rgba8(r, g, b, a)
}

fn fill_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: tiny_skia::Color,
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }
    let Some(rect) = Rect::from_xywh(x, y, width, height) else {
        return;
    };
    let mut paint = Paint::default();
    paint.set_color(color);
    pixmap.fill_rect(rect, &paint, Transform::identity(), None);
}

fn stroke_rect(
    pixmap: &mut Pixmap,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: tiny_skia::Color,
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
    pixmap.stroke_path(&path, &paint, &stroke, Transform::identity(), None);
}

fn blend_glyph_rect(pixmap: &mut Pixmap, x: i32, y: i32, width: u32, height: u32, color: Color) {
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cosmic_text::{FontSystem, SwashCache};
    use stickymd_core::{DocumentSnapshot, Generation, LineEnding};

    use super::*;
    use crate::preview::layout::layout_document;
    use crate::preview::{PreviewParser, RenderTreeBuilder};
    use crate::source::FontSelection;

    fn document(source: &str, blocks: usize) -> (FontSystem, LaidOutDocument) {
        let snapshot = DocumentSnapshot {
            text: Arc::from(source.repeat(blocks)),
            generation: Generation::initial(),
            line_ending: LineEnding::Lf,
        };
        let owned = PreviewParser.parse(&snapshot).unwrap();
        let tree = RenderTreeBuilder.build(&owned);
        let mut font_system = FontSystem::new();
        let fonts = FontSelection::resolve(&mut font_system);
        let layout = layout_document(
            &mut font_system,
            &fonts,
            &mut crate::math::MathEngine::new(),
            &tree,
            520,
            1.0,
            PreviewTheme::Light,
        );
        (font_system, layout)
    }

    #[test]
    fn paint_culls_blocks_outside_the_viewport() {
        let (mut font_system, mut document) = document("paragraph text\n\n", 200);
        let total = document.blocks.len();
        let frame = paint_document(
            &mut font_system,
            &mut SwashCache::new(),
            &mut document,
            180,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
        assert!(frame.visible_blocks() > 0);
        assert!(frame.visible_blocks() < total);
        assert_eq!(frame.rgba().len(), 520 * 180 * 4);
    }

    #[test]
    fn scroll_and_theme_paint_keep_generation_and_index() {
        let (mut font_system, mut document) = document("中文 **bold**\n\n", 40);
        let mut cache = SwashCache::new();
        let first = paint_document(
            &mut font_system,
            &mut cache,
            &mut document,
            200,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
        )
        .unwrap();
        let second = paint_document(
            &mut font_system,
            &mut cache,
            &mut document,
            200,
            120.0,
            first.index().select_all(),
            PreviewTheme::Dark,
        )
        .unwrap();
        assert_eq!(first.generation(), second.generation());
        assert_eq!(first.index().text(), second.index().text());
        assert_ne!(first.rgba(), second.rgba());
    }
}
