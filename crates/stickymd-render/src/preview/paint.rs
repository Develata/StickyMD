//! Viewport-culling software paint for native preview blocks.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use std::sync::Arc;

use cosmic_text::{FontSystem, SwashCache};
use tiny_skia::{Pixmap, PixmapPaint, PixmapRef, Transform};

mod drawing;
use super::horizontal::{clip_box, intersect};
use super::{PreviewHorizontalOffsets, PreviewHorizontalTarget, PreviewRect};
use drawing::*;

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
    horizontal: Vec<PreviewHorizontalTarget>,
}

impl PreviewFrame {
    pub fn horizontal_target_at(&self, x: f32, document_y: f32) -> Option<PreviewHorizontalTarget> {
        self.horizontal.iter().copied().find(|target| {
            let rect = target.content_rect();
            x >= rect.x
                && x < rect.right()
                && document_y >= rect.y
                && document_y < target.track().bottom()
        })
    }

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

    pub fn text(&self) -> &str {
        self.index.text()
    }

    pub fn select_all(&self) -> PreviewSelection {
        self.index.select_all()
    }

    pub fn copy_selection(&self, selection: PreviewSelection) -> Option<&str> {
        self.index.copy(selection)
    }

    pub fn hit_test(&self, x: f32, document_y: f32) -> usize {
        self.index.hit_test(x, document_y)
    }

    pub fn action_at(&self, x: f32, document_y: f32) -> Option<&super::SpanAction> {
        self.index.action_at(x, document_y)
    }

    pub fn tooltip_at(&self, x: f32, document_y: f32) -> Option<&str> {
        self.index.tooltip_at(x, document_y)
    }

    pub fn scroll_anchor_at_y(&self, document_y: f32) -> Option<crate::scroll::ScrollAnchor> {
        self.index.scroll_anchor_at_y(document_y)
    }

    pub fn y_for_scroll_anchor(&self, anchor: crate::scroll::ScrollAnchor) -> Option<f32> {
        self.index.y_for_scroll_anchor(anchor)
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
    #[error("formula rasterization failed: {0}")]
    Math(String),
}

pub(super) struct PaintResources<'a> {
    pub font_system: &'a mut FontSystem,
    pub swash_cache: &'a mut SwashCache,
    pub math_engine: &'a mut crate::math::MathEngine,
}

pub(super) fn paint_document(
    resources: PaintResources<'_>,
    document: &mut LaidOutDocument,
    height: u32,
    requested_scroll_y: f32,
    selection: PreviewSelection,
    theme: PreviewTheme,
    horizontal_offsets: &PreviewHorizontalOffsets,
) -> Result<PreviewFrame, PreviewPaintError> {
    let PaintResources {
        font_system,
        swash_cache,
        math_engine,
    } = resources;
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

    let viewport = PreviewRect {
        x: 0.0,
        y: scroll_y,
        width: document.width_px as f32,
        height: height as f32,
    };
    let horizontal: Vec<_> = document.blocks[start..end]
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            block.horizontal.map(|layout| {
                layout.target(
                    start + index,
                    document.generation,
                    document.width_px,
                    document.scale,
                    horizontal_offsets,
                )
            })
        })
        .collect();
    let mut visible_boxes = Vec::new();
    for (index, block) in document.blocks[start..end].iter().enumerate() {
        let target = block.horizontal.map(|layout| {
            layout.target(
                start + index,
                document.generation,
                document.width_px,
                document.scale,
                horizontal_offsets,
            )
        });
        let offset = target.map_or(0.0, |target| target.offset());
        let Some(clip) = target.map_or(Some(viewport), |target| {
            intersect(target.content_rect(), viewport)
        }) else {
            continue;
        };
        visible_boxes.extend(
            block
                .boxes
                .iter()
                .cloned()
                .filter_map(|item| clip_box(item, offset, clip)),
        );
        for chunk in &block.chunks {
            let (offset, clip) = if chunk.scroll_with_block {
                (offset, clip)
            } else {
                (0.0, viewport)
            };
            if let LayoutContent::Text(layout) = &chunk.content {
                visible_boxes.extend(
                    super::text_layout::project_text_boxes_in_rect(
                        layout,
                        chunk.x - offset,
                        chunk.y,
                        clip,
                    )
                    .into_iter()
                    .filter_map(|item| clip_box(item, 0.0, clip)),
                );
            }
        }
    }
    let index = Arc::new(PreviewTextIndex::from_document(
        Arc::clone(&document.projection),
        visible_boxes,
    ));
    // At most one viewport-sized alpha mask, reused for every overflow block.
    let mut mask = None;
    for (index, block) in document.blocks[start..end].iter().enumerate() {
        let target = block.horizontal.map(|layout| {
            layout.target(
                start + index,
                document.generation,
                document.width_px,
                document.scale,
                horizontal_offsets,
            )
        });
        let offset = target.map_or(0.0, |target| target.offset());
        let clip = target.map(|target| target.content_rect());
        let clip_mask = block_mask(&mut mask, document.width_px, height, clip, scroll_y)?;
        for decoration in &block.decorations {
            let mut decoration = *decoration;
            if matches!(
                decoration.role,
                DecorationRole::CodeBackground | DecorationRole::MathBackground
            ) {
                paint_decoration(&mut pixmap, decoration, scroll_y, palette, None);
            } else {
                decoration.rect.x -= offset;
                paint_decoration(&mut pixmap, decoration, scroll_y, palette, clip_mask);
            }
        }
    }
    if !selection.is_collapsed() {
        for rectangle in index.selection_rects_in_y_range(selection, scroll_y, viewport_bottom) {
            fill_rect(
                &mut pixmap,
                rectangle.x,
                rectangle.y - scroll_y,
                rectangle.width,
                rectangle.height,
                palette.selection,
                None,
            );
        }
    }
    for (index, block) in document.blocks[start..end].iter_mut().enumerate() {
        let target = block.horizontal.map(|layout| {
            layout.target(
                start + index,
                document.generation,
                document.width_px,
                document.scale,
                horizontal_offsets,
            )
        });
        let offset = target.map_or(0.0, |target| target.offset());
        let clip = target.map_or(viewport, |target| target.content_rect());
        let clip_mask = block_mask(
            &mut mask,
            document.width_px,
            height,
            target.map(|target| target.content_rect()),
            scroll_y,
        )?;
        for chunk in &mut block.chunks {
            let (offset, clip, clip_mask) = if chunk.scroll_with_block {
                (offset, clip, clip_mask)
            } else {
                (0.0, viewport, None)
            };
            let origin_x = (chunk.x - offset).round() as i32;
            let origin_y = (chunk.y - scroll_y).round() as i32;
            match &mut chunk.content {
                LayoutContent::Text(layout) => layout.draw_in_rect(
                    font_system,
                    swash_cache,
                    palette.text,
                    PreviewRect {
                        x: clip.x - origin_x as f32,
                        y: scroll_y - chunk.y,
                        width: clip.width,
                        height: height as f32,
                    },
                    |x, y, width, height, color| {
                        blend_glyph_rect(
                            &mut pixmap,
                            x + origin_x,
                            y + origin_y,
                            width,
                            height,
                            color,
                            PreviewRect {
                                y: clip.y - scroll_y,
                                ..clip
                            },
                        );
                    },
                ),
                LayoutContent::Math(formula) => {
                    if chunk.y + (formula.metrics.height as f32) < scroll_y
                        || chunk.y > viewport_bottom
                        || chunk.x - offset + (formula.metrics.width as f32) < clip.x
                        || chunk.x - offset > clip.right()
                    {
                        continue;
                    }
                    math_engine
                        .draw(
                            Arc::clone(&formula.source),
                            formula.kind,
                            formula.font_size,
                            super::layout::math_foreground(theme),
                            |raster| {
                                if let Some(source) = PixmapRef::from_bytes(
                                    &raster.pixels,
                                    raster.width,
                                    raster.height,
                                ) {
                                    pixmap.draw_pixmap(
                                        origin_x,
                                        origin_y,
                                        source,
                                        &PixmapPaint::default(),
                                        Transform::identity(),
                                        clip_mask,
                                    );
                                }
                            },
                        )
                        .map_err(|error| PreviewPaintError::Math(error.to_string()))?;
                }
                LayoutContent::Image(image) => {
                    if let Some(raster) = &image.raster {
                        if let Some(source) =
                            PixmapRef::from_bytes(&raster.rgba, raster.width, raster.height)
                        {
                            pixmap.draw_pixmap(
                                origin_x,
                                origin_y,
                                source,
                                &PixmapPaint::default(),
                                Transform::identity(),
                                clip_mask,
                            );
                        }
                    } else {
                        fill_rect(
                            &mut pixmap,
                            origin_x as f32,
                            origin_y as f32,
                            image.width as f32,
                            image.height as f32,
                            palette.code,
                            clip_mask,
                        );
                    }
                }
            }
        }
    }

    for target in &horizontal {
        for (rect, color) in [
            (target.track(), palette.code),
            (target.thumb(), palette.quote),
        ] {
            fill_rect(
                &mut pixmap,
                rect.x,
                rect.y - scroll_y,
                rect.width,
                rect.height,
                color,
                None,
            );
        }
    }
    Ok(PreviewFrame {
        generation: document.generation,
        width: document.width_px,
        height,
        document_height: document.height_px,
        scroll_y,
        rgba: pixmap.take(),
        index,
        visible_blocks: end.saturating_sub(start),
        horizontal,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use cosmic_text::{FontSystem, SwashCache};
    use stickymd_core::{DocumentSnapshot, Generation, LineEnding};

    use super::*;
    use crate::preview::layout::{LayoutResources, layout_document};
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
            LayoutResources {
                horizontal_offsets: &crate::preview::PreviewHorizontalOffsets::default(),
                font_system: &mut font_system,
                fonts: &fonts,
                math_engine: &mut crate::math::MathEngine::new(),
                image_source: None,
                image_cache: &mut crate::image::DecodedImageCache::default(),
                image_band: (0.0, f32::MAX),
            },
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
            PaintResources {
                font_system: &mut font_system,
                swash_cache: &mut SwashCache::new(),
                math_engine: &mut crate::math::MathEngine::new(),
            },
            &mut document,
            180,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
            &crate::preview::PreviewHorizontalOffsets::default(),
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
            PaintResources {
                font_system: &mut font_system,
                swash_cache: &mut cache,
                math_engine: &mut crate::math::MathEngine::new(),
            },
            &mut document,
            200,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
            &crate::preview::PreviewHorizontalOffsets::default(),
        )
        .unwrap();
        let second = paint_document(
            PaintResources {
                font_system: &mut font_system,
                swash_cache: &mut cache,
                math_engine: &mut crate::math::MathEngine::new(),
            },
            &mut document,
            200,
            120.0,
            first.index().select_all(),
            PreviewTheme::Dark,
            &crate::preview::PreviewHorizontalOffsets::default(),
        )
        .unwrap();
        assert_eq!(first.generation(), second.generation());
        assert_eq!(first.index().text(), second.index().text());
        assert_ne!(first.rgba(), second.rgba());
    }
}
