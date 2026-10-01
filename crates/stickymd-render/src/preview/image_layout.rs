//! Standalone native image placement and selection geometry.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

use std::sync::Arc;

use crate::image::{DecodedImageCache, PreviewImageSource};

use super::image_resources::LaidOutImage;
use super::layout::{BlockBuild, LayoutChunk, LayoutContent};
use super::{PreviewRect, PreviewTextBox, RenderBlock};

#[allow(clippy::too_many_arguments)]
pub(super) fn layout_image_block(
    block: &RenderBlock,
    x: f32,
    y: f32,
    width: f32,
    scale: f32,
    selection_text: &mut String,
    image_source: Option<&dyn PreviewImageSource>,
    image_cache: &mut DecodedImageCache,
    image_band: (f32, f32),
) -> Option<BlockBuild> {
    let [span] = block.spans.as_slice() else {
        return None;
    };
    let image = span.image.as_ref()?;
    let max_width = width.floor().max(1.0) as u32;
    let max_height = (900.0 * scale).floor().max(1.0) as u32;
    let slot = LaidOutImage::build(
        image,
        max_width,
        max_height,
        y,
        image_source,
        image_cache,
        image_band,
    )?;
    let (target_width, target_height) = (slot.width, slot.height);
    let content = LayoutContent::Image(slot);
    let selection_start = selection_text.len();
    selection_text.push_str(&span.copy_text);
    let selection_end = selection_text.len();
    let rect = PreviewRect {
        x,
        y,
        width: target_width as f32,
        height: target_height as f32,
    };
    Some(BlockBuild {
        height: rect.height.max(1.0),
        chunks: vec![LayoutChunk { content, x, y }],
        decorations: Vec::new(),
        boxes: vec![PreviewTextBox {
            selection_range: selection_start..selection_end,
            source_range: span.source_range,
            rect,
            action: span.action.clone().map(Arc::new),
            tooltip: None,
            atomic: true,
            start_x: rect.x,
            end_x: rect.right(),
        }],
    })
}
