//! Where a pointer is over the view, without a window: pane-local positions, the cursor each pane
//! shows and the drag threshold that tells a link click from a selection.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use stickymd_render::preview::PreviewFrame;
use winit::dpi::PhysicalPosition;
use winit::window::CursorIcon;

use crate::geometry::{PaneRect, ViewGeometry};

const MEANINGFUL_DRAG_DIP: f64 = 3.0;

/// `position` relative to `pane`, when it lies on the pane.
pub fn pane_local(pane: PaneRect, position: PhysicalPosition<f64>) -> Option<(f32, f32)> {
    let x = position.x as f32;
    let y = position.y as f32;
    pane.contains(x, y)
        .then_some((x - pane.x as f32, y - pane.y as f32))
}

/// The cursor at `position`, off the shell's own surface (`ControlLayout::hit_test`). `frame` is the Preview frame whose hit geometry is current, asked for
/// only over the Preview pane; without one the pane shows the text cursor.
pub fn view_cursor<'a>(
    geometry: ViewGeometry,
    position: PhysicalPosition<f64>,
    frame: impl FnOnce() -> Option<&'a PreviewFrame>,
) -> CursorIcon {
    if let Some((x, y)) = geometry.preview.and_then(|pane| pane_local(pane, position)) {
        let Some(frame) = frame() else {
            return CursorIcon::Text;
        };
        let y = y + frame.scroll_y();
        return if frame
            .horizontal_target_at(x, y)
            .is_some_and(|target| target.track().contains(x, y))
        {
            CursorIcon::Default
        } else if frame.action_at(x, y).is_some() {
            CursorIcon::Pointer
        } else {
            CursorIcon::Text
        };
    }
    if geometry
        .source
        .is_some_and(|pane| pane.contains(position.x as f32, position.y as f32))
    {
        CursorIcon::Text
    } else {
        CursorIcon::Default
    }
}

pub fn meaningful_preview_drag(
    start: PhysicalPosition<f64>,
    current: PhysicalPosition<f64>,
    scale: f64,
) -> bool {
    let threshold = MEANINGFUL_DRAG_DIP * scale.max(0.5);
    let dx = current.x - start.x;
    let dy = current.y - start.y;
    dx.mul_add(dx, dy * dy) >= threshold * threshold
}

#[cfg(test)]
mod tests {
    use stickymd_app::config::ViewMode;
    use winit::dpi::PhysicalSize;

    use super::*;
    use crate::geometry::geometry;

    #[test]
    fn link_click_tolerates_jitter_but_selection_drag_crosses_dip_threshold() {
        let start = PhysicalPosition::new(100.0, 100.0);
        assert!(!meaningful_preview_drag(
            start,
            PhysicalPosition::new(102.0, 101.0),
            1.0
        ));
        assert!(meaningful_preview_drag(
            start,
            PhysicalPosition::new(104.0, 100.0),
            1.0
        ));
        assert!(!meaningful_preview_drag(
            start,
            PhysicalPosition::new(104.0, 100.0),
            2.0
        ));
    }

    /// A pane answers from its own top-left corner up to, not including, its far edges.
    #[test]
    fn a_position_is_local_to_the_pane_it_lies_on() {
        let pane = PaneRect {
            x: 10,
            y: 34,
            width: 100,
            height: 50,
        };
        let at = |x, y| pane_local(pane, PhysicalPosition::new(x, y));
        assert_eq!(at(10.0, 34.0), Some((0.0, 0.0)));
        assert_eq!(at(109.5, 83.5), Some((99.5, 49.5)));
        assert_eq!(at(110.0, 40.0), None);
        assert_eq!(at(20.0, 84.0), None);
        assert_eq!(at(9.5, 40.0), None);
    }

    /// Without a frame: each pane shows the text cursor and the gutter and whatever lies above the
    /// panes the arrow. The frame is asked for only over the Preview pane.
    #[test]
    fn each_part_of_the_view_shows_its_cursor() {
        let view = geometry(ViewMode::Split, PhysicalSize::new(520, 680), 1.0);
        let at = |x, y| {
            view_cursor(view, PhysicalPosition::new(x, y), || {
                assert!(view.preview.unwrap().contains(x as f32, y as f32));
                None
            })
        };
        assert_eq!(at(100.0, 17.0), CursorIcon::Default);
        assert_eq!(at(100.0, 300.0), CursorIcon::Text);
        assert_eq!(at(400.0, 300.0), CursorIcon::Text);
        assert_eq!(at(245.0, 300.0), CursorIcon::Default);
        assert_eq!(at(510.0, 300.0), CursorIcon::Default);
    }
}
