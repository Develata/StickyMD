//! Viewport-culling software paint for native preview blocks.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use super::copy_rail::CopyRails;
use super::end_room::PreviewEnd;
use std::sync::Arc;

use cosmic_text::{FontSystem, SwashCache};
use tiny_skia::{Pixmap, PixmapPaint, PixmapRef, Transform};

mod drawing;
mod frame;
use super::horizontal::{clip_box, intersect};
use super::{PreviewHorizontalOffsets, PreviewHorizontalTarget, PreviewRect};
use drawing::*;
pub use frame::PreviewFrame;

use super::layout::{
    DecorationRole, LaidOutBlock, LaidOutDocument, LayoutContent, LayoutDecoration,
};
use super::{PreviewSelection, PreviewTextIndex};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PreviewTheme {
    #[default]
    Light,
    Dark,
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

#[allow(clippy::too_many_arguments)]
pub(super) fn paint_document(
    resources: PaintResources<'_>,
    document: &mut LaidOutDocument,
    height: u32,
    requested_scroll_y: f32,
    notice_bar_px: f32,
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
    let scroll_end = PreviewEnd::new(document.height_px, height as f32, notice_bar_px);
    let scroll_y = scroll_end.clamp(requested_scroll_y);
    let viewport_bottom = scroll_y + height as f32;
    // A block counts as visible while its background does: it reaches past the block's top and
    // bottom, and its copy rail with it.
    let outset = super::layout::BACKGROUND_OUTSET_DIP * document.scale;
    let start = document
        .blocks
        .partition_point(|block| block.bottom + outset < scroll_y);
    let end = document
        .blocks
        .partition_point(|block| block.top - outset <= viewport_bottom);

    let viewport = PreviewRect {
        x: 0.0,
        y: scroll_y,
        width: document.width_px as f32,
        height: height as f32,
    };
    // The horizontal target of the visible block at `index` in `start..end`, for the box,
    // decoration and content passes and the frame. Allocation-free and cheap (O(1) without stored
    // offsets, else one O(log n) lookup in the offset map), so each pass derives it again instead
    // of keeping a per-frame list.
    let (generation, width_px, scale) = (document.generation, document.width_px, document.scale);
    let target_of = |index: usize, block: &LaidOutBlock| {
        block.horizontal.map(|layout| {
            layout.target(
                start + index,
                generation,
                width_px,
                scale,
                horizontal_offsets,
            )
        })
    };
    let mut visible_boxes = Vec::new();
    for (index, block) in document.blocks[start..end].iter().enumerate() {
        let target = target_of(index, block);
        // Clip, then clamp: both kinds of atomic box (formula and image objects, and the text
        // of a failed formula or image placeholder) decide drag endpoints by what is visible.
        let finish_box = |item| {
            if target.is_some() {
                item
            } else {
                visible_bounds(item)
            }
        };
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
                .filter_map(|item| clip_box(item, offset, clip))
                .map(finish_box),
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
                    .filter_map(|item| clip_box(item, 0.0, clip))
                    .map(finish_box),
                );
            }
        }
    }
    let mut index =
        PreviewTextIndex::from_document(Arc::clone(&document.projection), visible_boxes);
    // Formulas the painter could not draw, by their atomic box's selection range, with why.
    let mut failed_formulas: Vec<(std::ops::Range<usize>, Arc<str>)> = Vec::new();
    // At most one viewport-sized alpha mask, reused for every overflow block.
    let mut mask = BlockMask::default();
    for (index, block) in document.blocks[start..end].iter().enumerate() {
        let target = target_of(index, block);
        let offset = target.map_or(0.0, |target| target.offset());
        // Only a decoration that scrolls with the block needs the clip; code and math
        // backgrounds stay fixed, so a block with nothing else builds no mask here.
        let needs_mask = block
            .decorations
            .iter()
            .any(|decoration| !decoration.role.is_block_background());
        let clip = target
            .map(|target| target.content_rect())
            .filter(|_| needs_mask);
        let clip_mask = mask.get(document.width_px, height, clip, scroll_y)?;
        for decoration in &block.decorations {
            let mut decoration = *decoration;
            if decoration.role.is_block_background() {
                paint_decoration(&mut pixmap, decoration, scroll_y, &palette, None);
            } else {
                decoration.rect.x -= offset;
                paint_decoration(&mut pixmap, decoration, scroll_y, &palette, clip_mask);
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
        let target = target_of(index, block);
        let offset = target.map_or(0.0, |target| target.offset());
        let clip = target.map_or(viewport, |target| target.content_rect());
        let clip_mask = mask.get(
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
                    let drawn = math_engine.draw(
                        Arc::clone(&formula.source),
                        formula.kind,
                        formula.font_size,
                        super::layout::math_foreground(theme),
                        |raster| {
                            if let Some(source) =
                                PixmapRef::from_bytes(&raster.pixels, raster.width, raster.height)
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
                        },
                    );
                    if let Err(error) = drawn {
                        // A formula the painter cannot draw stays an error on that formula
                        // (plan 06 `math-raster-lifecycle`): its box gets the math-error marker
                        // and the reason as its tooltip, the rest of the frame paints, and its
                        // source stays copyable. A later repaint tries it again.
                        let rect = PreviewRect {
                            x: chunk.x - offset,
                            y: chunk.y,
                            width: formula.metrics.width as f32,
                            height: formula.metrics.height as f32,
                        };
                        paint_decoration(
                            &mut pixmap,
                            LayoutDecoration {
                                rect,
                                role: DecorationRole::MathError,
                            },
                            scroll_y,
                            &palette,
                            clip_mask,
                        );
                        failed_formulas
                            .push((formula.selection.clone(), Arc::from(error.to_string())));
                    }
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

    let horizontal: Vec<PreviewHorizontalTarget> = document.blocks[start..end]
        .iter()
        .enumerate()
        .filter_map(|(index, block)| target_of(index, block))
        .collect();
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
    // The index was built before painting; give each failed formula's own atomic box (matched
    // by selection range, never by geometry) its reason, in place.
    index.set_atomic_tooltips(&mut failed_formulas);
    let index = Arc::new(index);
    let rails = CopyRails::new(
        &document.blocks[start..end],
        start,
        document.generation,
        document.width_px,
        document.scale,
        palette.rail,
        |block| index.copy_object(block).is_ok(),
    );
    Ok(PreviewFrame {
        generation: document.generation,
        width: document.width_px,
        height,
        document_height: document.height_px,
        end: scroll_end,
        scroll_y,
        rgba: pixmap.take(),
        index,
        visible_blocks: end.saturating_sub(start),
        horizontal,
        rails,
    })
}

/// An atomic object in a block that cannot scroll horizontally, clipped to the frame, decides a
/// drag endpoint by the part a reader can see: a pointer held inside the pane can reach the
/// object's visible right edge but never its off-pane right end.
fn visible_bounds(mut item: super::PreviewTextBox) -> super::PreviewTextBox {
    if item.atomic {
        let (left, right) = (item.rect.x, item.rect.right());
        item.start_x = item.start_x.clamp(left, right);
        item.end_x = item.end_x.clamp(left, right);
    }
    item
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
        let mut font_system = crate::system_fonts::new_font_system();
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

    /// A blit is clipped to its target: placed wholly to the right of it or below it, it copies
    /// nothing; placed across its edge, only the overlap.
    #[test]
    fn a_blit_copies_only_what_lands_on_its_target() {
        let (mut font_system, mut document) = document("paragraph text\n\n", 4);
        let frame = paint_document(
            PaintResources {
                font_system: &mut font_system,
                swash_cache: &mut SwashCache::new(),
                math_engine: &mut crate::math::MathEngine::new(),
            },
            &mut document,
            120,
            0.0,
            0.0,
            PreviewSelection::default(),
            PreviewTheme::Light,
            &crate::preview::PreviewHorizontalOffsets::default(),
        )
        .unwrap();
        let untouched = Pixmap::new(1, 1).unwrap();
        for (x, y) in [(2, 0), (0, 2), (1, 1)] {
            let mut target = untouched.clone();
            frame.blit_to(&mut target, x, y);
            assert_eq!(target.data(), untouched.data(), "placed at ({x}, {y})");
        }
        let mut target = Pixmap::new(10, 10).unwrap();
        frame.blit_to(&mut target, 6, 7);
        let painted = |x: u32, y: u32| target.pixel(x, y).unwrap().alpha() != 0;
        assert!(painted(6, 7) && painted(9, 9));
        assert!(!painted(5, 9) && !painted(9, 6));
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
            0.0,
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
