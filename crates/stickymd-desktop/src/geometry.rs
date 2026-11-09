//! View geometry: the toolbar, the panes and their scrollbar gutters for one window size and DPI.
//!
//! plan_ref: docs/plan/09_windows_shell.md#window-geometry

use stickymd_app::config::ViewMode;
use winit::dpi::PhysicalSize;

/// The only toolbar height. The geometry below rounds it in `f32`; the control layout widens it to
/// `f64` first. 34 is exact in both, so each path keeps its own arithmetic.
pub const TOOLBAR_HEIGHT_DIP: f32 = 34.0;

/// The scrollbar band right of each pane.
pub const GUTTER_DIP: f32 = 20.0;

const SPLIT_DIVIDER_DIP: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaneRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PaneRect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x as f32
            && x < self.x.saturating_add(self.width) as f32
            && y >= self.y as f32
            && y < self.y.saturating_add(self.height) as f32
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ViewGeometry {
    pub toolbar_height: u32,
    pub source: Option<PaneRect>,
    pub preview: Option<PaneRect>,
    pub divider_x: Option<u32>,
}

pub fn geometry(mode: ViewMode, size: PhysicalSize<u32>, scale: f32) -> ViewGeometry {
    let toolbar_height = (TOOLBAR_HEIGHT_DIP * scale.max(0.5)).round().max(1.0) as u32;
    let content_y = toolbar_height.min(size.height);
    let content_height = size.height.saturating_sub(content_y).max(1);
    let full = PaneRect {
        x: 0,
        y: content_y,
        width: size.width.max(1),
        height: content_height,
    };
    let mut geometry = match mode {
        ViewMode::Source => ViewGeometry {
            toolbar_height,
            source: Some(full),
            preview: None,
            divider_x: None,
        },
        ViewMode::Preview => ViewGeometry {
            toolbar_height,
            source: None,
            preview: Some(full),
            divider_x: None,
        },
        ViewMode::Split => {
            let divider = (SPLIT_DIVIDER_DIP * scale.max(0.5)).round().max(1.0) as u32;
            let available = size.width.saturating_sub(divider);
            let source_width = available / 2;
            let preview_width = available.saturating_sub(source_width);
            ViewGeometry {
                toolbar_height,
                source: Some(PaneRect {
                    width: source_width.max(1),
                    ..full
                }),
                preview: Some(PaneRect {
                    x: source_width.saturating_add(divider),
                    width: preview_width.max(1),
                    ..full
                }),
                divider_x: Some(source_width),
            }
        }
    };
    let gutter = (GUTTER_DIP * scale.max(0.5)).round() as u32;
    for pane in [&mut geometry.source, &mut geometry.preview]
        .into_iter()
        .flatten()
    {
        pane.width = pane.width.saturating_sub(gutter).max(1);
    }
    geometry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_is_fixed_fifty_fifty_while_semantic_scroll_mapping_stays_separate() {
        let geometry = geometry(ViewMode::Split, PhysicalSize::new(901, 700), 1.0);
        let source = geometry.source.unwrap();
        let preview = geometry.preview.unwrap();
        assert_eq!(geometry.divider_x, Some(source.width + GUTTER_DIP as u32));
        assert!((source.width as i64 - preview.width as i64).abs() <= 1);
        assert_eq!(source.y, preview.y);
        assert_eq!(source.height, preview.height);
    }

    #[test]
    fn toolbar_is_reserved_in_every_view_mode() {
        for mode in [ViewMode::Source, ViewMode::Split, ViewMode::Preview] {
            let geometry = geometry(mode, PhysicalSize::new(520, 680), 1.5);
            for pane in [geometry.source, geometry.preview].into_iter().flatten() {
                assert_eq!(pane.y, geometry.toolbar_height);
                assert_eq!(pane.height + pane.y, 680);
            }
        }
    }

    /// The two copies this constant replaced: an `f32` rounded here, an `f64` in the control
    /// layout. Halves round away from zero, and the widening loses nothing.
    #[test]
    fn the_toolbar_height_keeps_its_rounding_and_its_exact_widening() {
        for (scale, expected) in [(0.25_f32, 17), (1.0, 34), (1.25, 43), (1.5, 51), (1.75, 60)] {
            let view = geometry(ViewMode::Source, PhysicalSize::new(520, 680), scale);
            assert_eq!(view.toolbar_height, expected, "{scale}");
        }
        assert_eq!(f64::from(TOOLBAR_HEIGHT_DIP), 34.0);
    }

    #[test]
    fn phase10_minimum_window_keeps_all_modes_operable_and_split_is_fifty_fifty() {
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            let size = PhysicalSize::new(
                (220.0 * scale).round() as u32,
                (120.0 * scale).round() as u32,
            );
            let source = geometry(ViewMode::Source, size, scale)
                .source
                .expect("Source pane");
            let preview = geometry(ViewMode::Preview, size, scale)
                .preview
                .expect("Preview pane");
            assert!(source.width > 0 && source.height > 0);
            assert!(preview.width > 0 && preview.height > 0);

            let split = geometry(ViewMode::Split, size, scale);
            let left = split.source.expect("Split source pane");
            let right = split.preview.expect("Split preview pane");
            let divider = (SPLIT_DIVIDER_DIP * scale).round().max(1.0) as u32;
            let gutter = (GUTTER_DIP * scale).round() as u32;
            assert_eq!(right.x, left.width + gutter + divider);
            assert!((left.width as i64 - right.width as i64).abs() <= 1);
            assert_eq!(left.width + divider + right.width + 2 * gutter, size.width);
            assert!(left.height > 0 && right.height > 0);
        }
    }
}
