//! Per-layout/refresh reuse of image facts; never a cross-refresh file cache.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use crate::image::{
    DecodedImageCache, DecodedImageRaster, ImageCacheKey, ImageMetadata, PreviewImageSource,
    decode_scaled_image_owned, inspect_encoded_image,
};

pub(super) struct ImageRequests<'a> {
    source: &'a dyn PreviewImageSource,
    metadata: HashMap<Box<str>, Option<ImageMetadata>>,
    // Only facts/failed target sizes: no encoded file buffers or extra raster leases.
    content: HashMap<Box<str>, Option<ContentFact>>,
}

struct ContentFact {
    metadata: ImageMetadata,
    hash: stickymd_core::Hash32,
    failed_targets: HashSet<(u32, u32)>,
}

impl<'a> ImageRequests<'a> {
    pub fn new(source: &'a dyn PreviewImageSource) -> Self {
        Self {
            source,
            metadata: HashMap::new(),
            content: HashMap::new(),
        }
    }

    pub fn inspect(&mut self, destination: &str) -> Option<ImageMetadata> {
        if let Some(metadata) = self.metadata.get(destination) {
            return *metadata;
        }
        let metadata = self.source.inspect(destination).ok().flatten();
        self.metadata.insert(destination.into(), metadata);
        metadata
    }

    pub fn load(
        &mut self,
        destination: &str,
        max_width: u32,
        max_height: u32,
        cache: &mut DecodedImageCache,
    ) -> Option<Arc<DecodedImageRaster>> {
        if let Some(fact) = self.content.get(destination) {
            let fact = fact.as_ref()?;
            let (width, height) = image_target(&fact.metadata, max_width, max_height);
            if fact.failed_targets.contains(&(width, height)) {
                return None;
            }
            let key = ImageCacheKey {
                source_hash: fact.hash,
                width,
                height,
            };
            if let Some(raster) = cache.get(&key) {
                return Some(raster);
            }
            // A different target size or an evicted raster requires decoding again.
        }
        let previous = self.content.insert(destination.into(), None).flatten();
        let bytes = self.source.load(destination).ok().flatten()?;
        let metadata = inspect_encoded_image(&bytes).ok()?;
        let (width, height) = image_target(&metadata, max_width, max_height);
        let key = ImageCacheKey {
            source_hash: stickymd_core::hash_bytes(&bytes),
            width,
            height,
        };
        let hash = key.source_hash;
        let raster = cache.get(&key).or_else(|| {
            cache.insert(
                key.clone(),
                decode_scaled_image_owned(bytes, width, height).ok()?,
            )
        });
        let mut failed_targets = previous
            .filter(|fact| fact.hash == hash)
            .map_or_else(HashSet::new, |fact| fact.failed_targets);
        if raster.is_none() {
            failed_targets.insert((width, height));
        }
        self.content.insert(
            destination.into(),
            Some(ContentFact {
                metadata,
                hash,
                failed_targets,
            }),
        );
        self.metadata.insert(destination.into(), Some(metadata));
        raster
    }
}

pub(super) fn image_target(
    metadata: &ImageMetadata,
    max_width: u32,
    max_height: u32,
) -> (u32, u32) {
    let scale = (max_width as f64 / metadata.width as f64)
        .min(max_height as f64 / metadata.height as f64)
        .min(1.0);
    (
        ((metadata.width as f64 * scale).floor() as u32).max(1),
        ((metadata.height as f64 * scale).floor() as u32).max(1),
    )
}
