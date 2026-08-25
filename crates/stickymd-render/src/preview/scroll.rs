//! Preview-side semantic scroll index for Split synchronization.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#split-scroll-sync

use std::sync::Arc;

use crate::scroll::ScrollAnchor;

use super::SourceRange;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct PreviewScrollAnchor {
    pub source_range: SourceRange,
    pub top: f32,
    pub bottom: f32,
}

#[derive(Debug, Clone, Default)]
pub(super) struct PreviewScrollIndex {
    anchors: Arc<[PreviewScrollAnchor]>,
}

impl PreviewScrollIndex {
    pub fn new(mut anchors: Vec<PreviewScrollAnchor>) -> Self {
        anchors.sort_by(|left, right| {
            left.source_range
                .start
                .cmp(&right.source_range.start)
                .then_with(|| left.top.total_cmp(&right.top))
        });
        Self {
            anchors: anchors.into(),
        }
    }

    pub fn anchor_at_y(&self, y: f32) -> Option<ScrollAnchor> {
        let index = self.anchors.partition_point(|anchor| anchor.bottom <= y);
        let anchor = self.anchors.get(index).or_else(|| self.anchors.last())?;
        let height = (anchor.bottom - anchor.top).max(1.0);
        Some(ScrollAnchor::new(
            anchor.source_range.start,
            anchor.source_range.end,
            ((y - anchor.top) / height).clamp(0.0, 1.0),
        ))
    }

    pub fn y_for_anchor(&self, anchor: ScrollAnchor) -> Option<f32> {
        let index = self
            .anchors
            .partition_point(|item| item.source_range.end <= anchor.source_byte);
        let item = self.anchors.get(index).or_else(|| self.anchors.last())?;
        let fraction = if anchor.source_end > anchor.source_byte {
            anchor.block_fraction
        } else {
            let source_span = item
                .source_range
                .end
                .saturating_sub(item.source_range.start);
            if source_span == 0 {
                0.0
            } else {
                anchor
                    .source_byte
                    .saturating_sub(item.source_range.start)
                    .min(source_span) as f32
                    / source_span as f32
            }
        };
        Some(item.top + (item.bottom - item.top).max(0.0) * fraction)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn semantic_scroll_mapping_uses_source_ranges_instead_of_height_ratio() {
        let index = PreviewScrollIndex::new(vec![
            PreviewScrollAnchor {
                source_range: SourceRange::new(0, 10).unwrap(),
                top: 20.0,
                bottom: 120.0,
            },
            PreviewScrollAnchor {
                source_range: SourceRange::new(100, 120).unwrap(),
                top: 200.0,
                bottom: 240.0,
            },
        ]);
        let anchor = index.anchor_at_y(220.0).unwrap();
        assert_eq!(anchor.source_byte, 100);
        assert_eq!(anchor.source_end, 120);
        assert_eq!(anchor.block_fraction, 0.5);
        assert_eq!(index.y_for_anchor(anchor), Some(220.0));
        assert_eq!(index.y_for_anchor(ScrollAnchor::point(105)), Some(210.0));
    }
}
