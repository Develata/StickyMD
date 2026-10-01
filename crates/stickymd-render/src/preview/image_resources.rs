//! Stable image geometry and bounded raster refresh on the Preview worker.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#image-safety-limits

use std::sync::Arc;

use crate::image::{
    DecodedImageCache, DecodedImageRaster, ImageCacheKey, PreviewImageSource,
    decode_scaled_image_owned, inspect_encoded_image,
};

use super::ImageKind;
use super::layout::{LaidOutBlock, LayoutContent};
use super::render_tree::RenderImage;

pub(super) struct LaidOutImage {
    destination: Box<str>,
    max_width: u32,
    max_height: u32,
    // Preserve initial layout admission, including the block origin used by
    // inline images. Refresh must agree with a full layout of the same input.
    admission_y: f32,
    pub width: u32,
    pub height: u32,
    pub raster: Option<Arc<DecodedImageRaster>>,
}

impl LaidOutImage {
    pub fn build(
        image: &RenderImage,
        max_width: u32,
        max_height: u32,
        admission_y: f32,
        source: Option<&dyn PreviewImageSource>,
        cache: &mut DecodedImageCache,
        band: (f32, f32),
    ) -> Option<Box<Self>> {
        if !matches!(
            image.kind,
            ImageKind::LocalRelative | ImageKind::LocalAbsolute
        ) {
            return None;
        }
        let source = source?;
        let metadata = source.inspect(&image.destination).ok().flatten()?;
        let (width, height) = image_target(&metadata, max_width, max_height);
        let mut slot = Self {
            destination: image.destination.clone().into_boxed_str(),
            max_width,
            max_height,
            admission_y,
            width,
            height,
            raster: None,
        };
        if slot.in_band(band) {
            // The file can change between metadata inspection and byte read.
            // Initial layout must use the dimensions of the actual decoded bytes.
            let raster = slot.load(source, cache)?;
            slot.width = raster.width;
            slot.height = raster.height;
            slot.raster = Some(raster);
        }
        Some(Box::new(slot))
    }

    fn in_band(&self, band: (f32, f32)) -> bool {
        self.admission_y + self.height as f32 >= band.0 && self.admission_y <= band.1
    }

    fn load(
        &self,
        source: &dyn PreviewImageSource,
        cache: &mut DecodedImageCache,
    ) -> Option<Arc<DecodedImageRaster>> {
        let bytes = source.load(&self.destination).ok().flatten()?;
        let metadata = inspect_encoded_image(&bytes).ok()?;
        let (width, height) = image_target(&metadata, self.max_width, self.max_height);
        let key = ImageCacheKey {
            source_hash: stickymd_core::hash_bytes(&bytes),
            width,
            height,
        };
        cache.get(&key).or_else(|| {
            let raster = decode_scaled_image_owned(bytes, width, height).ok()?;
            cache.insert(key, raster)
        })
    }
}

/// Only successfully laid-out local images have slots. If any semantic image
/// fell back to text, a full layout must retry it before geometry can be reused.
pub(super) struct ImageSlots {
    locations: Vec<(usize, usize)>,
    complete: bool,
}

impl ImageSlots {
    pub fn new(blocks: &[LaidOutBlock], local_image_count: usize) -> Self {
        if local_image_count == 0 {
            return Self {
                locations: Vec::new(),
                complete: true,
            };
        }
        let locations: Vec<_> = blocks
            .iter()
            .enumerate()
            .flat_map(|(block_index, block)| {
                block
                    .chunks
                    .iter()
                    .enumerate()
                    .filter_map(move |(chunk_index, chunk)| {
                        matches!(chunk.content, LayoutContent::Image(_))
                            .then_some((block_index, chunk_index))
                    })
            })
            .collect();
        Self {
            complete: locations.len() == local_image_count,
            locations,
        }
    }

    /// Returns false when source changes, decode failures or cache admission
    /// can alter geometry. The caller then discards this layout and rebuilds it.
    pub fn refresh(
        &self,
        blocks: &mut [LaidOutBlock],
        source: &dyn PreviewImageSource,
        cache: &mut DecodedImageCache,
        band: (f32, f32),
    ) -> bool {
        if !self.complete {
            return false;
        }
        // Inspect even off-band images, as full layout does: deletion or a new
        // size above the viewport may shift every subsequent block and anchor.
        for &location in &self.locations {
            let slot = image_at(blocks, location);
            let Ok(Some(metadata)) = source.inspect(&slot.destination) else {
                return false;
            };
            if image_target(&metadata, slot.max_width, slot.max_height) != (slot.width, slot.height)
            {
                return false;
            }
        }
        // Release all old leases before admitting the new band. Retaining
        // off-screen Arcs would prevent LRU eviction under the live-byte budget.
        for &location in &self.locations {
            image_at(blocks, location).raster = None;
        }
        for &location in &self.locations {
            let slot = image_at(blocks, location);
            if slot.in_band(band) {
                let Some(raster) = slot.load(source, cache) else {
                    return false;
                };
                if (raster.width, raster.height) != (slot.width, slot.height) {
                    return false;
                }
                slot.raster = Some(raster);
            }
        }
        true
    }
}

fn image_at(blocks: &mut [LaidOutBlock], (block, chunk): (usize, usize)) -> &mut LaidOutImage {
    match &mut blocks[block].chunks[chunk].content {
        LayoutContent::Image(image) => image,
        _ => unreachable!("image slot index belongs to this immutable layout geometry"),
    }
}

fn image_target(
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
