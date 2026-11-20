//! The copy rail beside a code block, Raw HTML block or display formula: where a frame shows
//! each one, how it is painted over the frame, and which object a click on it copies. Rails are
//! produced per frame from the visible blocks, as horizontal scroll targets are, and add nothing
//! to the layout (plan 06 #preview-copy-rail).
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-rail

use stickymd_core::Generation;
use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};

use super::PreviewRect;
use super::layout::LaidOutBlock;

/// The rail's width in content DIP, scaled with the content; provisional until visual acceptance.
pub const COPY_RAIL_WIDTH_DIP: f32 = 8.0;

/// One visible rail, in Preview document coordinates. It names its frame's generation and its
/// render block, so a rail kept from an earlier frame is recognised as not this frame's.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewCopyRail {
    generation: Generation,
    block: usize,
    rect: PreviewRect,
}

impl PreviewCopyRail {
    pub const fn rect(self) -> PreviewRect {
        self.rect
    }

    pub(super) const fn block(self) -> usize {
        self.block
    }
}

/// A rail's colours in one theme, kept in the Preview palette (`paint::drawing::Palette`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct RailColors {
    pub rest: Color,
    pub hovered: Color,
}

/// The rails one frame shows, in the colours of the theme it was painted in.
#[derive(Debug, Clone)]
pub(super) struct CopyRails {
    rails: Vec<PreviewCopyRail>,
    colors: RailColors,
}

impl CopyRails {
    /// The rail of every block in `blocks` (render blocks `first..`) that has a code or math
    /// background: the background's right edge out to the rail width, at the background's height
    /// (so never beside the horizontal track below it). A rail is kept only when all of it lies in
    /// `0..width_px` and its object can be copied (`copyable`). O(visible blocks · decorations).
    ///
    /// Postcondition: the rails are in display order and pairwise disjoint, at most one per block,
    /// since each stands beside its own block's background.
    pub(super) fn new(
        blocks: &[LaidOutBlock],
        first: usize,
        generation: Generation,
        width_px: u32,
        scale: f32,
        colors: RailColors,
        copyable: impl Fn(usize) -> bool,
    ) -> Self {
        let width = COPY_RAIL_WIDTH_DIP * scale;
        let rails = blocks
            .iter()
            .enumerate()
            .filter_map(|(offset, block)| {
                let background = block
                    .decorations
                    .iter()
                    .find(|decoration| decoration.role.is_block_background())?;
                let rect = PreviewRect {
                    x: background.rect.right(),
                    y: background.rect.y,
                    width,
                    height: background.rect.height,
                };
                let block = first + offset;
                (rect.x >= 0.0 && rect.right() <= width_px as f32 && copyable(block)).then_some(
                    PreviewCopyRail {
                        generation,
                        block,
                        rect,
                    },
                )
            })
            .collect();
        Self { rails, colors }
    }

    pub(super) fn all(&self) -> &[PreviewCopyRail] {
        &self.rails
    }

    /// The rail at document position `(x, y)`: its visible rectangle, half-open. A linear scan,
    /// since a frame holds at most one rail per visible code or math block.
    pub(super) fn at(&self, x: f32, y: f32) -> Option<PreviewCopyRail> {
        self.rails.iter().copied().find(|rail| {
            let rect = rail.rect;
            x >= rect.x && x < rect.right() && y >= rect.y && y < rect.bottom()
        })
    }

    pub(super) fn owns(&self, rail: PreviewCopyRail) -> bool {
        self.rails.contains(&rail)
    }

    /// Paints the rails over a frame of `width` × `height` pixels blitted at `origin`, scrolled to
    /// `scroll_y`, clipped to the frame; `hovered` is painted with more contrast.
    pub(super) fn paint(
        &self,
        target: &mut Pixmap,
        origin: (u32, u32),
        (width, height): (u32, u32),
        scroll_y: f32,
        hovered: Option<PreviewCopyRail>,
    ) {
        let (origin_x, origin_y) = (origin.0 as f32, origin.1 as f32);
        for rail in &self.rails {
            let rect = rail.rect;
            let Some(clipped) = Rect::from_ltrb(
                origin_x + rect.x.max(0.0),
                origin_y + (rect.y - scroll_y).max(0.0),
                origin_x + rect.right().min(width as f32),
                origin_y + (rect.bottom() - scroll_y).min(height as f32),
            ) else {
                continue;
            };
            let mut paint = Paint::default();
            paint.set_color(if hovered == Some(*rail) {
                self.colors.hovered
            } else {
                self.colors.rest
            });
            target.fill_rect(clipped, &paint, Transform::identity(), None);
        }
    }
}
