//! The copy rail beside a code block, Raw HTML block or display formula: where a frame shows
//! each one, how it is painted over the frame, and which object a click on it copies. Rails are
//! produced per frame from the visible blocks, as horizontal scroll targets are, and add nothing
//! to the layout (plan 06 #preview-copy-rail).
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-copy-rail

use stickymd_core::Generation;
use tiny_skia::{Color, Paint, Pixmap, Rect, Transform};

use super::layout::{DecorationRole, LaidOutBlock};
use super::{PreviewRect, PreviewTheme};

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

/// The rails one frame shows, and the theme it was painted in.
#[derive(Debug, Clone, Default)]
pub(super) struct CopyRails {
    rails: Vec<PreviewCopyRail>,
    theme: PreviewTheme,
}

impl CopyRails {
    /// The rail of every block in `blocks` (render blocks `first..`) that has a code or math
    /// background: the background's right edge out to the rail width, at the background's height
    /// (so never beside the horizontal track below it). A rail is kept only when all of it lies in
    /// `0..width_px` and its object can be copied (`copyable`). O(visible blocks · decorations).
    pub(super) fn new(
        blocks: &[LaidOutBlock],
        first: usize,
        generation: Generation,
        width_px: u32,
        scale: f32,
        theme: PreviewTheme,
        copyable: impl Fn(usize) -> bool,
    ) -> Self {
        let width = COPY_RAIL_WIDTH_DIP * scale;
        let rails = blocks
            .iter()
            .enumerate()
            .filter_map(|(offset, block)| {
                let background = block.decorations.iter().find(|decoration| {
                    matches!(
                        decoration.role,
                        DecorationRole::CodeBackground | DecorationRole::MathBackground
                    )
                })?;
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
        Self { rails, theme }
    }

    pub(super) fn all(&self) -> &[PreviewCopyRail] {
        &self.rails
    }

    /// The rail at document position `(x, y)`: its visible rectangle, half-open.
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
    /// `scroll_y`, clipped to the frame; `hovered` is painted brighter.
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
            paint.set_color(rail_color(self.theme, hovered == Some(*rail)));
            target.fill_rect(clipped, &paint, Transform::identity(), None);
        }
    }
}

/// Low contrast against the code and math backgrounds (`paint::drawing::Palette`): at rest it
/// reads as a slightly thicker right edge of the box; hovered it stands out more.
fn rail_color(theme: PreviewTheme, hovered: bool) -> Color {
    let (r, g, b) = match (theme, hovered) {
        (PreviewTheme::Light, false) => (224, 220, 209),
        (PreviewTheme::Light, true) => (198, 193, 180),
        (PreviewTheme::Dark, false) => (64, 63, 59),
        (PreviewTheme::Dark, true) => (92, 91, 86),
    };
    Color::from_rgba8(r, g, b, 255)
}
