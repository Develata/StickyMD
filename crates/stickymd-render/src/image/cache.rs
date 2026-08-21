//! Bounded decoded-image projection cache.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#image-safety-limits

use std::collections::HashMap;
use std::sync::Arc;

use stickymd_core::Hash32;

use super::{
    DecodedImageRaster, IMAGE_CACHE_BUDGET_BYTES, IMAGE_CACHE_ENTRY_OVERHEAD_BYTES,
    IMAGE_CACHE_MAX_ENTRIES,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ImageCacheKey {
    pub source_hash: Hash32,
    pub width: u32,
    pub height: u32,
}

struct CacheEntry {
    raster: Arc<DecodedImageRaster>,
    last_used: u64,
    accounted_bytes: usize,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ImageCacheCounters {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

/// Strict byte/entry-budget LRU. Hits are average O(1); the bounded map is
/// scanned only when an eviction is required.
pub struct DecodedImageCache {
    entries: HashMap<ImageCacheKey, CacheEntry>,
    bytes: usize,
    clock: u64,
    budget: usize,
    counters: ImageCacheCounters,
}

impl Default for DecodedImageCache {
    fn default() -> Self {
        Self::new(IMAGE_CACHE_BUDGET_BYTES)
    }
}

impl DecodedImageCache {
    pub fn new(budget: usize) -> Self {
        Self {
            entries: HashMap::new(),
            bytes: 0,
            clock: 0,
            budget,
            counters: ImageCacheCounters::default(),
        }
    }

    pub fn get(&mut self, key: &ImageCacheKey) -> Option<Arc<DecodedImageRaster>> {
        self.clock = self.clock.saturating_add(1);
        match self.entries.get_mut(key) {
            Some(entry) => {
                self.counters.hits = self.counters.hits.saturating_add(1);
                entry.last_used = self.clock;
                Some(Arc::clone(&entry.raster))
            }
            None => {
                self.counters.misses = self.counters.misses.saturating_add(1);
                None
            }
        }
    }

    pub fn insert(
        &mut self,
        key: ImageCacheKey,
        raster: DecodedImageRaster,
    ) -> Option<Arc<DecodedImageRaster>> {
        // The key contains the source digest and exact target dimensions, so
        // an existing entry is already the same deterministic projection.
        // Reuse it rather than dropping the cache's accounting reference while
        // a layout may still lease that raster.
        if let Some(existing) = self.entries.get_mut(&key) {
            self.clock = self.clock.saturating_add(1);
            existing.last_used = self.clock;
            return Some(Arc::clone(&existing.raster));
        }
        let size = raster
            .byte_len()
            .checked_add(IMAGE_CACHE_ENTRY_OVERHEAD_BYTES)?;
        if size > self.budget {
            return None;
        }
        while self.bytes.saturating_add(size) > self.budget {
            // A raster borrowed by the currently applied or currently built
            // layout is still live memory. It must remain accounted here;
            // evicting only our map reference would make the cache byte counter
            // lie while the Arc stayed alive in a LayoutChunk.
            self.evict_oldest_unleased()?;
        }
        while self.entries.len() >= IMAGE_CACHE_MAX_ENTRIES {
            if self.evict_oldest_unleased().is_none() {
                break;
            }
        }
        if self.entries.len() >= IMAGE_CACHE_MAX_ENTRIES {
            return None;
        }
        self.clock = self.clock.saturating_add(1);
        let raster = Arc::new(raster);
        self.bytes = self.bytes.saturating_add(size);
        self.entries.insert(
            key,
            CacheEntry {
                raster: Arc::clone(&raster),
                last_used: self.clock,
                accounted_bytes: size,
            },
        );
        Some(raster)
    }

    pub fn clear(&mut self) {
        self.entries.clear();
        self.bytes = 0;
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    pub fn entry_count(&self) -> usize {
        self.entries.len()
    }

    pub const fn counters(&self) -> ImageCacheCounters {
        self.counters
    }

    fn evict_oldest_unleased(&mut self) -> Option<()> {
        let oldest = self
            .entries
            .iter()
            .filter(|(_, entry)| Arc::strong_count(&entry.raster) == 1)
            .min_by_key(|(_, entry)| entry.last_used)
            .map(|(key, _)| key.clone())?;
        let old = self.entries.remove(&oldest)?;
        self.bytes = self.bytes.saturating_sub(old.accounted_bytes);
        self.counters.evictions = self.counters.evictions.saturating_add(1);
        Some(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn raster(value: u8, bytes: usize) -> DecodedImageRaster {
        DecodedImageRaster {
            width: 2,
            height: 2,
            rgba: vec![value; bytes].into(),
        }
    }

    #[test]
    fn cache_never_exceeds_budget_and_counts_hits_misses_and_evictions() {
        let budget = 4 * (16 + IMAGE_CACHE_ENTRY_OVERHEAD_BYTES);
        let mut cache = DecodedImageCache::new(budget);
        for index in 0..10u8 {
            let key = ImageCacheKey {
                source_hash: Hash32::new([index; 32]),
                width: 2,
                height: 2,
            };
            assert!(cache.insert(key, raster(index, 16)).is_some());
            assert!(cache.bytes() <= budget);
        }
        assert_eq!(cache.bytes(), budget);
        assert_eq!(cache.counters().evictions, 6);

        let key = ImageCacheKey {
            source_hash: Hash32::new([9; 32]),
            width: 2,
            height: 2,
        };
        assert!(cache.get(&key).is_some());
        assert!(cache.get(&ImageCacheKey { width: 3, ..key }).is_none());
        assert_eq!(cache.counters().hits, 1);
        assert_eq!(cache.counters().misses, 1);
    }

    #[test]
    fn cache_metadata_count_is_bounded_for_tiny_images() {
        let mut cache = DecodedImageCache::default();
        for index in 0..600_u32 {
            let mut hash = [0_u8; 32];
            hash[..4].copy_from_slice(&index.to_le_bytes());
            cache.insert(
                ImageCacheKey {
                    source_hash: Hash32::new(hash),
                    width: 1,
                    height: 1,
                },
                raster(0, 4),
            );
        }
        assert_eq!(cache.entry_count(), IMAGE_CACHE_MAX_ENTRIES);
    }

    #[test]
    fn live_layout_rasters_remain_accounted_and_prevent_overcommit() {
        let entry_bytes = 16 + IMAGE_CACHE_ENTRY_OVERHEAD_BYTES;
        let mut cache = DecodedImageCache::new(entry_bytes);
        let first_key = ImageCacheKey {
            source_hash: Hash32::new([1; 32]),
            width: 2,
            height: 2,
        };
        let leased = cache.insert(first_key, raster(1, 16)).unwrap();
        let second_key = ImageCacheKey {
            source_hash: Hash32::new([2; 32]),
            width: 2,
            height: 2,
        };
        assert!(cache.insert(second_key.clone(), raster(2, 16)).is_none());
        assert_eq!(cache.bytes(), entry_bytes);
        assert_eq!(cache.entry_count(), 1);

        drop(leased);
        assert!(cache.insert(second_key, raster(2, 16)).is_some());
        assert_eq!(cache.bytes(), entry_bytes);
        assert_eq!(cache.entry_count(), 1);
    }

    #[test]
    fn reinserting_a_leased_key_reuses_the_accounted_raster() {
        let entry_bytes = 16 + IMAGE_CACHE_ENTRY_OVERHEAD_BYTES;
        let mut cache = DecodedImageCache::new(entry_bytes);
        let key = ImageCacheKey {
            source_hash: Hash32::new([3; 32]),
            width: 2,
            height: 2,
        };
        let first = cache.insert(key.clone(), raster(3, 16)).unwrap();
        let second = cache.insert(key, raster(9, 16)).unwrap();
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(cache.bytes(), entry_bytes);
        assert_eq!(cache.entry_count(), 1);
    }
}
