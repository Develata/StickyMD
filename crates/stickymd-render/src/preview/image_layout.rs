//! Standalone native image layout and target-size calculation.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

use crate::image::{
    DecodedImageCache, ImageCacheKey, PreviewImageSource, decode_scaled_image_owned,
    inspect_encoded_image,
};

use super::layout::{BlockBuild, LayoutChunk, LayoutContent};
use super::{ImageKind, PreviewRect, PreviewTextBox, RenderBlock};

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
    if !matches!(
        image.kind,
        ImageKind::LocalRelative | ImageKind::LocalAbsolute
    ) {
        return None;
    }
    let image_source = image_source?;
    let metadata = image_source.inspect(&image.destination).ok().flatten()?;
    let max_width = width.floor().max(1.0) as u32;
    let max_height = (900.0 * scale).floor().max(1.0) as u32;
    let (mut target_width, mut target_height) = image_target(&metadata, max_width, max_height);
    let in_decode_band = y + target_height as f32 >= image_band.0 && y <= image_band.1;
    let content = if in_decode_band {
        let bytes = image_source.load(&image.destination).ok().flatten()?;
        let current_metadata = inspect_encoded_image(&bytes).ok()?;
        (target_width, target_height) = image_target(&current_metadata, max_width, max_height);
        let key = ImageCacheKey {
            source_hash: stickymd_core::hash_bytes(&bytes),
            width: target_width,
            height: target_height,
        };
        let raster = if let Some(raster) = image_cache.get(&key) {
            raster
        } else {
            decode_scaled_image_owned(bytes, target_width, target_height)
                .ok()
                .and_then(|raster| image_cache.insert(key, raster))?
        };
        LayoutContent::Image(raster)
    } else {
        LayoutContent::ImagePlaceholder {
            width: target_width,
            height: target_height,
        }
    };
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
            action: span.action.clone(),
            tooltip: None,
            atomic: true,
        }],
    })
}

pub(super) fn image_target(
    metadata: &crate::image::ImageMetadata,
    max_width: u32,
    max_height: u32,
) -> (u32, u32) {
    let target_scale = (max_width as f64 / metadata.width as f64)
        .min(max_height as f64 / metadata.height as f64)
        .min(1.0);
    (
        ((metadata.width as f64 * target_scale).floor() as u32).max(1),
        ((metadata.height as f64 * target_scale).floor() as u32).max(1),
    )
}
