//! Stable image geometry and bounded raster refresh on the Preview worker.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#image-safety-limits

use std::sync::Arc;

use crate::image::{DecodedImageCache, DecodedImageRaster, PreviewImageSource};

use super::ImageKind;
use super::image_requests::{ImageRequests, image_target};
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
        source: Option<&mut ImageRequests<'_>>,
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
        let metadata = source.inspect(&image.destination)?;
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
        source: &mut ImageRequests<'_>,
        cache: &mut DecodedImageCache,
    ) -> Option<Arc<DecodedImageRaster>> {
        source.load(&self.destination, self.max_width, self.max_height, cache)
    }
}

/// Only successfully laid-out local images have slots. If any semantic image
/// fell back to text, a full layout must retry it before geometry can be reused.
pub(super) struct ImageSlots {
    locations: Vec<(usize, usize)>,
    complete: bool,
}

impl ImageSlots {
    pub fn horizontal_changed(
        &self,
        blocks: &[LaidOutBlock],
        previous: &super::PreviewHorizontalOffsets,
        next: &super::PreviewHorizontalOffsets,
    ) -> bool {
        self.locations.iter().any(|&(index, _)| {
            blocks[index].horizontal.is_some_and(|layout| {
                let maximum = layout.content_width - layout.viewport.width;
                previous.block_offset(index, maximum) != next.block_offset(index, maximum)
            })
        })
    }

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
        horizontal: &super::PreviewHorizontalOffsets,
    ) -> bool {
        if !self.complete {
            return false;
        }
        let mut source = ImageRequests::new(source);
        // Inspect even off-band images, as full layout does: deletion or a new
        // size above the viewport may shift every subsequent block and anchor.
        for &location in &self.locations {
            let slot = image_at(blocks, location);
            let Some(metadata) = source.inspect(&slot.destination) else {
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
            let in_horizontal_view = horizontal_visible(blocks, location, horizontal);
            let slot = image_at(blocks, location);
            if slot.in_band(band) && in_horizontal_view {
                let Some(raster) = slot.load(&mut source, cache) else {
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

fn horizontal_visible(
    blocks: &[LaidOutBlock],
    (block_index, chunk_index): (usize, usize),
    offsets: &super::PreviewHorizontalOffsets,
) -> bool {
    let block = &blocks[block_index];
    let Some(horizontal) = block.horizontal else {
        return true;
    };
    let chunk = &block.chunks[chunk_index];
    let LayoutContent::Image(image) = &chunk.content else {
        return false;
    };
    let x = chunk.x
        - offsets.block_offset(
            block_index,
            horizontal.content_width - horizontal.viewport.width,
        );
    x + image.width as f32 >= horizontal.viewport.x && x <= horizontal.viewport.right()
}
