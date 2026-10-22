//! Block-local horizontal geometry and immutable session snapshots.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-horizontal-scroll

use std::collections::BTreeMap;
use std::sync::Arc;

use stickymd_core::Generation;

use super::{PreviewRect, PreviewTextBox};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PreviewBlockId(usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LayoutScope {
    generation: Generation,
    width: u32,
    scale: u32,
}

/// Session data only. Block identities are valid for one document/width/scale.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PreviewHorizontalOffsets {
    scope: Option<LayoutScope>,
    values: Option<Arc<BTreeMap<PreviewBlockId, f32>>>,
}

impl PreviewHorizontalOffsets {
    pub fn for_layout(&self, generation: Generation, width: u32, scale: f32) -> Self {
        if self.scope == Some(LayoutScope::new(generation, width, scale)) {
            self.clone()
        } else {
            Self::default()
        }
    }

    pub fn with_offset(&self, target: PreviewHorizontalTarget, offset: f32) -> Self {
        if !offset.is_finite() {
            return self.clone();
        }
        let mut next = if self.scope == Some(target.scope) {
            self.clone()
        } else {
            Self {
                scope: Some(target.scope),
                ..Self::default()
            }
        };
        let offset = offset.clamp(0.0, target.max_offset());
        if offset == 0.0 {
            let Some(values) = &mut next.values else {
                return Self::default();
            };
            Arc::make_mut(values).remove(&target.id);
            if values.is_empty() {
                return Self::default();
            }
        } else {
            Arc::make_mut(next.values.get_or_insert_with(Default::default))
                .insert(target.id, offset);
        }
        next
    }

    // Internal callers normalize the snapshot with for_layout before slot refresh.
    pub(super) fn block_offset(&self, index: usize, maximum: f32) -> f32 {
        self.values
            .as_ref()
            .and_then(|values| values.get(&PreviewBlockId(index)))
            .copied()
            .unwrap_or(0.0)
            .clamp(0.0, maximum)
    }

    pub fn offset(&self, target: PreviewHorizontalTarget) -> f32 {
        if self.scope == Some(target.scope) {
            self.block_offset(target.id.0, target.max_offset())
        } else {
            0.0
        }
    }
}

impl LayoutScope {
    fn new(generation: Generation, width: u32, scale: f32) -> Self {
        Self {
            generation,
            width,
            scale: scale.max(0.5).to_bits(),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct HorizontalLayout {
    pub viewport: PreviewRect,
    pub content_width: f32,
    pub bar_height: f32,
}

impl HorizontalLayout {
    pub fn new(
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        content_width: f32,
        scale: f32,
    ) -> Option<Self> {
        (content_width > width + 0.5).then_some(Self {
            viewport: PreviewRect {
                x,
                y,
                width,
                height,
            },
            content_width,
            bar_height: 10.0 * scale,
        })
    }

    pub fn target(
        self,
        index: usize,
        generation: Generation,
        width: u32,
        scale: f32,
        offsets: &PreviewHorizontalOffsets,
    ) -> PreviewHorizontalTarget {
        let mut target = PreviewHorizontalTarget {
            id: PreviewBlockId(index),
            scope: LayoutScope::new(generation, width, scale),
            layout: self,
            offset: 0.0,
        };
        target.offset = offsets.offset(target);
        target
    }
}

/// Semantic hit/drag surface; coordinates are in the preview document.
#[derive(Debug, Clone, Copy)]
pub struct PreviewHorizontalTarget {
    id: PreviewBlockId,
    scope: LayoutScope,
    layout: HorizontalLayout,
    offset: f32,
}

impl PreviewHorizontalTarget {
    pub fn matches_layout(self, generation: Generation, width: u32, scale: f32) -> bool {
        self.scope == LayoutScope::new(generation, width, scale)
    }
    pub fn content_rect(self) -> PreviewRect {
        self.layout.viewport
    }
    pub fn offset(self) -> f32 {
        self.offset
    }
    pub fn max_offset(self) -> f32 {
        self.layout.content_width - self.layout.viewport.width
    }
    pub fn track(self) -> PreviewRect {
        PreviewRect {
            y: self.layout.viewport.bottom(),
            height: self.layout.bar_height,
            ..self.layout.viewport
        }
    }
    pub fn thumb(self) -> PreviewRect {
        let track = self.track();
        let width = (track.width * track.width / self.layout.content_width)
            .max(self.layout.bar_height * 2.0)
            .min(track.width);
        PreviewRect {
            x: track.x + (track.width - width) * self.offset / self.max_offset(),
            width,
            ..track
        }
    }
    pub fn offset_after_drag(self, delta_x: f32) -> f32 {
        let travel = self.track().width - self.thumb().width;
        if travel > 0.0 {
            (self.offset + delta_x * self.max_offset() / travel).clamp(0.0, self.max_offset())
        } else {
            self.offset
        }
    }
    pub fn offset_after_page(self, x: f32) -> f32 {
        (self.offset
            + if x < self.thumb().x {
                -self.track().width
            } else {
                self.track().width
            })
        .clamp(0.0, self.max_offset())
    }
}

pub(super) fn clip_box(
    mut item: PreviewTextBox,
    offset: f32,
    clip: PreviewRect,
) -> Option<PreviewTextBox> {
    item.rect.x -= offset;
    item.start_x -= offset;
    item.end_x -= offset;
    item.rect = intersect(item.rect, clip)?;
    Some(item)
}

pub(super) fn intersect(rect: PreviewRect, clip: PreviewRect) -> Option<PreviewRect> {
    let x = rect.x.max(clip.x);
    let y = rect.y.max(clip.y);
    let right = rect.right().min(clip.right());
    let bottom = rect.bottom().min(clip.bottom());
    (right > x && bottom > y).then_some(PreviewRect {
        x,
        y,
        width: right - x,
        height: bottom - y,
    })
}
