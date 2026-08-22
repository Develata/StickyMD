//! Fixed native top-control layout and transient popup projection.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use winit::dpi::{PhysicalPosition, PhysicalSize};

pub(super) const TOOLBAR_HEIGHT_DIP: f64 = 34.0;
const CONTROL_DIP: f64 = 28.0;
const CONTROL_GAP_DIP: f64 = 4.0;
const EDGE_DIP: f64 = 5.0;
const COMPACT_EDGE_DIP: f64 = 3.0;
const COMPACT_GAP_DIP: f64 = 2.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ControlId {
    Source,
    Split,
    Preview,
    Topmost,
    Theme,
    Opacity,
    Collapse,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct ControlRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl ControlRect {
    pub fn contains(self, point: PhysicalPosition<f64>) -> bool {
        point.x >= self.x
            && point.x < self.x + self.width
            && point.y >= self.y
            && point.y < self.y + self.height
    }
}

#[derive(Debug, Clone)]
pub(super) struct ControlLayout {
    controls: [(ControlId, ControlRect); 8],
    pub toolbar: ControlRect,
    pub opacity_popup: ControlRect,
    pub opacity_slider: ControlRect,
    pub opacity_input: ControlRect,
}

impl ControlLayout {
    pub fn new(size: PhysicalSize<u32>, scale: f64) -> Self {
        let scale = scale.max(0.5);
        let toolbar_height = TOOLBAR_HEIGHT_DIP * scale;
        let regular_required = 2.0 * EDGE_DIP + 8.0 * CONTROL_DIP + 7.0 * CONTROL_GAP_DIP;
        let compact = size.width as f64 / scale < regular_required;
        let edge = if compact { COMPACT_EDGE_DIP } else { EDGE_DIP } * scale;
        let gap = if compact {
            COMPACT_GAP_DIP
        } else {
            CONTROL_GAP_DIP
        } * scale;
        let available = (size.width as f64 - 2.0 * edge - 7.0 * gap).max(8.0 * scale);
        let control = (available / 8.0).min(CONTROL_DIP * scale);
        let y = (toolbar_height - control) * 0.5;
        let left = [ControlId::Source, ControlId::Split, ControlId::Preview];
        let right = [
            ControlId::Topmost,
            ControlId::Theme,
            ControlId::Opacity,
            ControlId::Collapse,
            ControlId::Close,
        ];
        let mut controls = [(
            ControlId::Source,
            ControlRect {
                x: 0.0,
                y,
                width: control,
                height: control,
            },
        ); 8];
        for (index, id) in left.into_iter().enumerate() {
            controls[index] = (
                id,
                ControlRect {
                    x: edge + index as f64 * (control + gap),
                    y,
                    width: control,
                    height: control,
                },
            );
        }
        let right_width = right.len() as f64 * control + (right.len() - 1) as f64 * gap;
        let right_origin = size.width as f64 - edge - right_width;
        for (offset, id) in right.into_iter().enumerate() {
            controls[3 + offset] = (
                id,
                ControlRect {
                    x: right_origin + offset as f64 * (control + gap),
                    y,
                    width: control,
                    height: control,
                },
            );
        }
        let popup_width = (230.0 * scale).min(size.width as f64);
        let popup_height = 58.0 * scale;
        let opacity = controls[5].1;
        let popup_x = (opacity.x + opacity.width - popup_width)
            .clamp(0.0, (size.width as f64 - popup_width).max(0.0));
        let opacity_popup = ControlRect {
            x: popup_x,
            y: toolbar_height,
            width: popup_width,
            height: popup_height,
        };
        let opacity_slider = ControlRect {
            x: popup_x + 12.0 * scale,
            y: toolbar_height + 19.0 * scale,
            width: 150.0 * scale,
            height: 20.0 * scale,
        };
        let opacity_input = ControlRect {
            x: popup_x + 172.0 * scale,
            y: toolbar_height + 13.0 * scale,
            width: 45.0 * scale,
            height: 30.0 * scale,
        };
        Self {
            controls,
            toolbar: ControlRect {
                x: 0.0,
                y: 0.0,
                width: size.width as f64,
                height: toolbar_height,
            },
            opacity_popup,
            opacity_slider,
            opacity_input,
        }
    }

    pub fn control_at(&self, point: PhysicalPosition<f64>) -> Option<ControlId> {
        self.controls
            .iter()
            .find_map(|(id, rect)| rect.contains(point).then_some(*id))
    }

    pub fn rect(&self, id: ControlId) -> ControlRect {
        self.controls
            .iter()
            .find_map(|(candidate, rect)| (*candidate == id).then_some(*rect))
            .unwrap_or(self.toolbar)
    }

    pub fn is_drag_region(&self, point: PhysicalPosition<f64>) -> bool {
        self.toolbar.contains(point) && self.control_at(point).is_none()
    }

    pub fn opacity_at(&self, x: f64) -> u8 {
        let ratio = ((x - self.opacity_slider.x) / self.opacity_slider.width).clamp(0.0, 1.0);
        (40.0 + ratio * 60.0).round() as u8
    }
}

#[derive(Debug, Clone)]
pub(super) struct ControlState {
    pub opacity_popup_open: bool,
    pub opacity_dragging: bool,
    pub opacity_preview: u8,
    pub opacity_input: String,
    pub opacity_input_focused: bool,
}

impl ControlState {
    pub fn new(opacity: u8) -> Self {
        let opacity = opacity.clamp(40, 100);
        Self {
            opacity_popup_open: false,
            opacity_dragging: false,
            opacity_preview: opacity,
            opacity_input: opacity.to_string(),
            opacity_input_focused: false,
        }
    }

    pub fn preview(&mut self, opacity: u8) -> u8 {
        self.opacity_preview = opacity.clamp(40, 100);
        self.opacity_input = self.opacity_preview.to_string();
        self.opacity_preview
    }

    pub fn replace_input(&mut self, input: String) {
        if input.len() <= 3 && input.chars().all(|character| character.is_ascii_digit()) {
            self.opacity_input = input;
        }
    }

    pub fn commit_input(&mut self) -> Option<u8> {
        let parsed = self.opacity_input.parse::<u16>().ok()?;
        let opacity = parsed.clamp(40, 100) as u8;
        self.preview(opacity);
        Some(opacity)
    }
}

#[cfg(test)]
mod phase8_control_tests {
    use std::time::Instant;

    use super::*;

    #[test]
    fn phase8_controls_share_layout_between_hit_testing_and_painting() {
        let layout = ControlLayout::new(PhysicalSize::new(520, 680), 1.0);
        for id in [
            ControlId::Source,
            ControlId::Split,
            ControlId::Preview,
            ControlId::Topmost,
            ControlId::Theme,
            ControlId::Opacity,
            ControlId::Collapse,
            ControlId::Close,
        ] {
            let rect = layout.rect(id);
            let center =
                PhysicalPosition::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
            assert_eq!(layout.control_at(center), Some(id));
        }
    }

    #[test]
    fn phase8_opacity_slider_and_input_clamp_only_on_commit() {
        let layout = ControlLayout::new(PhysicalSize::new(520, 680), 1.0);
        assert_eq!(layout.opacity_at(layout.opacity_slider.x - 100.0), 40);
        assert_eq!(
            layout.opacity_at(layout.opacity_slider.x + layout.opacity_slider.width + 100.0),
            100
        );
        let mut state = ControlState::new(96);
        state.replace_input("7".into());
        assert_eq!(state.opacity_input, "7");
        assert_eq!(state.opacity_preview, 96);
        assert_eq!(state.commit_input(), Some(40));
        state.replace_input("x".into());
        assert_eq!(state.opacity_input, "40");
    }

    #[test]
    fn phase10_compact_layout_keeps_every_control_inside_without_overlap() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            let width = (220.0 * scale) as u32;
            let layout =
                ControlLayout::new(PhysicalSize::new(width, (120.0 * scale) as u32), scale);
            let mut previous_right = 0.0;
            for id in [
                ControlId::Source,
                ControlId::Split,
                ControlId::Preview,
                ControlId::Topmost,
                ControlId::Theme,
                ControlId::Opacity,
                ControlId::Collapse,
                ControlId::Close,
            ] {
                let rect = layout.rect(id);
                assert!(rect.x >= previous_right);
                assert!(rect.x + rect.width <= f64::from(width) + f64::EPSILON);
                previous_right = rect.x + rect.width;
            }
        }
    }

    #[test]
    #[ignore = "Release-only Phase 8 control-layout performance receipt"]
    fn phase8_performance_control_layout_and_hit_test() {
        let mut samples = Vec::with_capacity(25);
        for _ in 0..25 {
            let started = Instant::now();
            let mut hits = 0usize;
            for index in 0..100_000 {
                let layout = ControlLayout::new(
                    PhysicalSize::new(520 + (index % 400), 680),
                    [1.0, 1.25, 1.5, 2.0][index as usize % 4],
                );
                hits += layout
                    .control_at(PhysicalPosition::new((index % 520) as f64, 17.0))
                    .is_some() as usize;
            }
            std::hint::black_box(hits);
            samples.push(started.elapsed());
        }
        samples.sort_unstable();
        eprintln!(
            "phase8 control_layout_100k median={:?} p95={:?} max={:?}",
            samples[samples.len() / 2],
            samples[samples.len() * 95 / 100],
            samples[samples.len() - 1]
        );
    }
}
