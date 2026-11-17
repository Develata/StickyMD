//! A painted Preview viewport and what the shell asks of it: its pixels, the hit testing, the
//! selection and copy semantics of its text index, its horizontal targets and copy rails.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use std::sync::Arc;

use tiny_skia::Pixmap;

use crate::preview::copy_rail::{CopyRails, PreviewCopyRail};
use crate::preview::end_room::PreviewEnd;
use crate::preview::{PreviewHorizontalTarget, PreviewSelection, PreviewTextIndex};

#[derive(Debug, Clone)]
pub struct PreviewFrame {
    pub(super) generation: stickymd_core::Generation,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) document_height: f32,
    /// Where scrolling this frame ends (plan 09 #vertical-scrollbars).
    pub(super) end: PreviewEnd,
    pub(super) scroll_y: f32,
    pub(super) rgba: Vec<u8>,
    pub(super) index: Arc<PreviewTextIndex>,
    pub(super) visible_blocks: usize,
    pub(super) horizontal: Vec<PreviewHorizontalTarget>,
    pub(super) rails: CopyRails,
}

impl PreviewFrame {
    pub fn horizontal_target_at(&self, x: f32, document_y: f32) -> Option<PreviewHorizontalTarget> {
        self.horizontal.iter().copied().find(|target| {
            let rect = target.content_rect();
            x >= rect.x
                && x < rect.right()
                && document_y >= rect.y
                && document_y < target.track().bottom()
        })
    }

    /// The copy rails this frame shows, in document coordinates (plan 06 #preview-copy-rail).
    pub fn copy_rails(&self) -> &[PreviewCopyRail] {
        self.rails.all()
    }

    pub fn copy_rail_at(&self, x: f32, document_y: f32) -> Option<PreviewCopyRail> {
        self.rails.at(x, document_y)
    }

    /// What a click on `rail` copies: its object's whole content by rule 1 of plan 06
    /// #preview-copy-semantics, read from this frame's own snapshot. `None` when `rail` is not
    /// one of this frame's.
    pub fn copy_rail_text(&self, rail: PreviewCopyRail) -> Option<String> {
        if !self.rails.owns(rail) {
            return None;
        }
        self.index.copy_object(rail.block()).ok().map(str::to_owned)
    }

    /// Paints this frame's rails over it once blitted at `(origin_x, origin_y)`, `hovered`
    /// with more contrast. The rails are not in `rgba`, so hovering repaints only them.
    pub fn paint_copy_rails(
        &self,
        target: &mut Pixmap,
        origin_x: u32,
        origin_y: u32,
        hovered: Option<PreviewCopyRail>,
    ) {
        self.rails.paint(
            target,
            (origin_x, origin_y),
            (self.width, self.height),
            self.scroll_y,
            hovered,
        );
    }

    pub const fn generation(&self) -> stickymd_core::Generation {
        self.generation
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn document_height(&self) -> f32 {
        self.document_height
    }

    /// The scroll end every reader of this frame shares: painting clamped to it, and the wheel,
    /// the thumb, a view change and Split sync read it here (plan 09 #vertical-scrollbars).
    pub const fn end(&self) -> PreviewEnd {
        self.end
    }

    pub const fn scroll_y(&self) -> f32 {
        self.scroll_y
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn index(&self) -> &Arc<PreviewTextIndex> {
        &self.index
    }

    pub fn text(&self) -> &str {
        self.index.text()
    }

    pub fn select_all(&self) -> PreviewSelection {
        self.index.select_all()
    }

    /// The display text the selection covers, as painted.
    pub fn selected_text(&self, selection: PreviewSelection) -> Option<&str> {
        self.index.selected_text(selection)
    }

    /// What Ctrl+C copies for `selection` (plan 06 #preview-copy-semantics): read from this
    /// frame's own source snapshot, so a frame that lags the document copies what it shows.
    pub fn copy_text(
        &self,
        selection: PreviewSelection,
    ) -> Result<String, crate::preview::PreviewCopyError> {
        self.index.copy_text(selection)
    }

    pub fn hit_test(&self, x: f32, document_y: f32) -> usize {
        self.index.hit_test(x, document_y)
    }

    pub fn action_at(&self, x: f32, document_y: f32) -> Option<&crate::preview::SpanAction> {
        self.index.action_at(x, document_y)
    }

    pub fn tooltip_at(&self, x: f32, document_y: f32) -> Option<&str> {
        self.index.tooltip_at(x, document_y)
    }

    pub fn scroll_anchor_at_y(&self, document_y: f32) -> Option<crate::scroll::ScrollAnchor> {
        self.index.scroll_anchor_at_y(document_y)
    }

    pub fn y_for_scroll_anchor(&self, anchor: crate::scroll::ScrollAnchor) -> Option<f32> {
        self.index.y_for_scroll_anchor(anchor)
    }

    pub const fn visible_blocks(&self) -> usize {
        self.visible_blocks
    }

    /// O(viewport pixels) clipped copy into the shared software framebuffer.
    pub fn blit_to(&self, target: &mut Pixmap, origin_x: u32, origin_y: u32) {
        let copy_width = self.width.min(target.width().saturating_sub(origin_x));
        let copy_height = self.height.min(target.height().saturating_sub(origin_y));
        let target_stride = target.width() as usize * 4;
        let source_stride = self.width as usize * 4;
        let row_bytes = copy_width as usize * 4;
        for row in 0..copy_height as usize {
            let source_start = row * source_stride;
            let target_start = (origin_y as usize + row) * target_stride + origin_x as usize * 4;
            target.data_mut()[target_start..target_start + row_bytes]
                .copy_from_slice(&self.rgba[source_start..source_start + row_bytes]);
        }
    }
}
