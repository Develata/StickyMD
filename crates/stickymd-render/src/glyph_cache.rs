//! Local pressure relief for regenerable glyph pixels; never shares font ownership.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#preview-resource-assessment

use cosmic_text::SwashCache;

const RETAINED_GLYPH_BYTES: usize = 8 * 1024 * 1024;
const ENTRY_OVERHEAD: usize = 128;

pub(crate) struct GlyphCacheBudget {
    observed_entries: usize,
    limit: usize,
}

impl Default for GlyphCacheBudget {
    fn default() -> Self {
        Self {
            observed_entries: 0,
            limit: RETAINED_GLYPH_BYTES,
        }
    }
}

impl GlyphCacheBudget {
    /// Image entries are immutable after insertion. Unchanged paints are O(1).
    /// This bounds retention between paints, not the temporary current frame.
    pub fn after_paint(&mut self, cache: &mut SwashCache) {
        let entries = cache.image_cache.len();
        if entries == self.observed_entries {
            return;
        }
        self.observed_entries = entries;
        let bytes = cache.image_cache.values().fold(0usize, |bytes, image| {
            bytes
                .saturating_add(ENTRY_OVERHEAD)
                .saturating_add(image.as_ref().map_or(0, |image| image.data.capacity()))
        });
        if bytes > self.limit {
            *cache = SwashCache::new();
            self.observed_entries = 0;
        }
    }

    #[cfg(test)]
    pub fn for_test(limit: usize) -> Self {
        Self {
            limit,
            ..Self::default()
        }
    }
}
