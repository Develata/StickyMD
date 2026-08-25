//! Generation-local semantic scroll anchor shared by Source and Preview projections.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#split-scroll-sync

/// A non-authoritative position used to align two projections of one document.
///
/// `source_byte..source_end` is a UTF-8 source range in the current snapshot.
/// A point anchor uses equal endpoints. `block_fraction` keeps a stable
/// relative position inside visually tall blocks without binding unrelated
/// scrollbar percentages.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ScrollAnchor {
    pub source_byte: usize,
    pub source_end: usize,
    pub block_fraction: f32,
}

impl ScrollAnchor {
    pub fn new(source_byte: usize, source_end: usize, block_fraction: f32) -> Self {
        Self {
            source_byte,
            source_end: source_end.max(source_byte),
            block_fraction: block_fraction.clamp(0.0, 1.0),
        }
    }

    pub fn point(source_byte: usize) -> Self {
        Self::new(source_byte, source_byte, 0.0)
    }
}
