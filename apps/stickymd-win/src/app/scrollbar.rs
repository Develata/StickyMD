//! Shared scrollbar geometry and gesture state for both document panes.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars

use tiny_skia::{Paint, Pixmap, Rect, Transform};
use winit::dpi::PhysicalPosition;

use super::preview_runtime::PaneRect;

pub(super) const GUTTER_DIP: f32 = 20.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ScrollPane {
    Source,
    Preview,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct Scrollbar {
    pub pane: ScrollPane,
    pub track: PaneRect,
    pub thumb_top: f64,
    pub thumb_height: f64,
}

impl Scrollbar {
    pub fn new(
        pane: ScrollPane,
        content: PaneRect,
        dpi: f32,
        fraction: f64,
        visible: f64,
        reserve_split_control: bool,
    ) -> Option<Self> {
        if !fraction.is_finite() || !visible.is_finite() || visible >= 1.0 {
            return None;
        }
        let inset = (8.0 * dpi).round().max(1.0) as u32;
        let top_inset = if reserve_split_control {
            (28.0 * dpi).round() as u32
        } else {
            inset
        };
        let track = PaneRect {
            x: content.x + content.width + (2.0 * dpi).round() as u32,
            y: content.y + top_inset,
            width: (10.0 * dpi).round().max(1.0) as u32,
            height: content.height.saturating_sub(top_inset + inset),
        };
        if track.height < 2 {
            return None;
        }
        let height = f64::from(track.height);
        let thumb_height = (height * visible.clamp(0.0, 1.0))
            .max(f64::from(24.0 * dpi))
            .min(height - 1.0);
        Some(Self {
            pane,
            track,
            thumb_top: f64::from(track.y) + fraction.clamp(0.0, 1.0) * (height - thumb_height),
            thumb_height,
        })
    }

    pub fn contains(self, position: PhysicalPosition<f64>) -> bool {
        self.track.contains(position.x as f32, position.y as f32)
    }

    pub fn press(self, y: f64) -> ScrollbarPress {
        if y < self.thumb_top {
            ScrollbarPress::Page(-1.0)
        } else if y >= self.thumb_top + self.thumb_height {
            ScrollbarPress::Page(1.0)
        } else {
            ScrollbarPress::Drag(ScrollbarDrag {
                pane: self.pane,
                grab: (y - self.thumb_top) / self.thumb_height,
            })
        }
    }

    pub fn fraction_at(self, drag: ScrollbarDrag, y: f64) -> f64 {
        ((y - f64::from(self.track.y) - drag.grab * self.thumb_height)
            / (f64::from(self.track.height) - self.thumb_height))
            .clamp(0.0, 1.0)
    }

    pub fn paint(self, pixmap: &mut Pixmap, emphasized: bool, dark: bool) {
        let track_width = self.track.width as f32;
        let width = track_width * if emphasized { 0.7 } else { 0.3 };
        let Some(rect) = Rect::from_xywh(
            self.track.x as f32 + (track_width - width) / 2.0,
            self.thumb_top as f32,
            width,
            self.thumb_height as f32,
        ) else {
            return;
        };
        let mut paint = Paint::default();
        let shade = match (dark, emphasized) {
            (true, true) => 190,
            (true, false) => 120,
            (false, true) => 106,
            (false, false) => 161,
        };
        paint.set_color_rgba8(shade, shade, shade, 255);
        pixmap.fill_rect(rect, &paint, Transform::identity(), None);
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ScrollbarDrag {
    pub pane: ScrollPane,
    grab: f64,
}

pub(super) enum ScrollbarPress {
    Drag(ScrollbarDrag),
    Page(f32),
}

#[derive(Default)]
pub(super) struct ScrollbarInteraction {
    pub drag: Option<ScrollbarDrag>,
    pub hovered: Option<ScrollPane>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrollbar_drag_keeps_grab_offset_and_clamps_outside_window() {
        let pane = PaneRect {
            x: 0,
            y: 34,
            width: 500,
            height: 646,
        };
        let bar = Scrollbar::new(ScrollPane::Source, pane, 1.0, 0.4, 0.001, false).unwrap();
        assert_eq!(bar.thumb_height, 24.0);
        let y = bar.thumb_top + 3.0;
        let ScrollbarPress::Drag(drag) = bar.press(y) else {
            panic!("thumb drag")
        };
        assert!((bar.fraction_at(drag, y) - 0.4).abs() < 1e-9);
        assert_eq!(bar.fraction_at(drag, -500.0), 0.0);
        assert_eq!(bar.fraction_at(drag, 5000.0), 1.0);
        assert!(matches!(
            bar.press(bar.thumb_top - 1.0),
            ScrollbarPress::Page(-1.0)
        ));
        assert!(matches!(
            bar.press(bar.thumb_top + bar.thumb_height),
            ScrollbarPress::Page(1.0)
        ));
        assert!(Scrollbar::new(ScrollPane::Source, pane, 1.0, 0.0, 1.0, false).is_none());
    }

    #[test]
    fn scrollbar_paints_inside_gutter_and_avoids_resize_border_at_each_dpi() {
        for dpi in [1.0, 1.25, 1.5, 2.0] {
            let width = (520.0_f32 * dpi).round() as u32;
            let height = (680.0_f32 * dpi).round() as u32;
            let pane = PaneRect {
                x: 0,
                y: 0,
                width: width - (GUTTER_DIP * dpi).round() as u32,
                height,
            };
            let bar = Scrollbar::new(ScrollPane::Preview, pane, dpi, 1.0, 0.1, false).unwrap();
            let split_bar = Scrollbar::new(ScrollPane::Source, pane, dpi, 0.0, 0.1, true).unwrap();
            assert!(split_bar.track.y as f32 > pane.y as f32 + 26.0 * dpi);
            assert!(bar.track.x + bar.track.width < width - (6.0 * dpi) as u32);
            let mut thin = Pixmap::new(width, height).unwrap();
            let mut thick = thin.clone();
            bar.paint(&mut thin, false, false);
            bar.paint(&mut thick, true, false);
            let thin_count = thin.pixels().iter().filter(|p| p.alpha() > 0).count();
            let thick_count = thick.pixels().iter().filter(|p| p.alpha() > 0).count();
            assert!(thick_count > thin_count);
            for y in 0..height {
                assert!(
                    thick.pixels()[(y * width) as usize..(y * width + pane.width) as usize]
                        .iter()
                        .all(|p| p.alpha() == 0)
                );
            }
        }
    }
}
