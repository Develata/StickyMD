//! Fixed native top-control layout.
//!
//! plan_ref: docs/plan/09_windows_shell.md#window-geometry

use stickymd_app::config::ViewMode;
use stickymd_app::instruction::WindowResizeEdge;
use winit::dpi::{PhysicalPosition, PhysicalSize};

use crate::geometry::{TOOLBAR_HEIGHT_DIP, geometry};

const CONTROL_DIP: f64 = 28.0;
const CONTROL_GAP_DIP: f64 = 4.0;
const EDGE_DIP: f64 = 5.0;
// The compact edge stays outside the 6 DIP native resize hit region. At the
// 220 DIP minimum, removing inter-control gaps leaves nine approximately
// 23.1 DIP hit targets instead of shrinking them below usable size.
const COMPACT_EDGE_DIP: f64 = 6.0;
const COMPACT_GAP_DIP: f64 = 0.0;
const RESIZE_BORDER_DIP: f64 = 6.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlId {
    Source,
    Split,
    Preview,
    ConvertMath,
    Topmost,
    Theme,
    Opacity,
    Collapse,
    Close,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlRect {
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

/// Where a press lands on the open opacity popup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PopupHit {
    Slider,
    Field,
    /// The popup's own surface: it covers what lies under it, so the press goes nowhere else.
    Blank,
}

/// What a press or a still pointer finds on the shell's own surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlHit {
    Popup(PopupHit),
    Resize(WindowResizeEdge),
    SplitSync,
    Button(ControlId),
    /// The toolbar outside its buttons, which moves the window.
    DragWindow,
}

/// The shell's one geometry for a window size, DPI and view: paint, hover and press read it.
#[derive(Debug, Clone)]
pub struct ControlLayout {
    size: PhysicalSize<u32>,
    scale: f64,
    controls: [(ControlId, ControlRect); 9],
    pub toolbar: ControlRect,
    pub opacity_popup: ControlRect,
    pub opacity_slider: ControlRect,
    pub opacity_input: ControlRect,
    /// In a Split, the control on the divider.
    pub split_sync: Option<ControlRect>,
}

impl ControlLayout {
    pub fn new(mode: ViewMode, size: PhysicalSize<u32>, scale: f64) -> Self {
        let scale = scale.max(0.5);
        let toolbar_height = f64::from(TOOLBAR_HEIGHT_DIP) * scale;
        let regular_required = 2.0 * EDGE_DIP + 9.0 * CONTROL_DIP + 8.0 * CONTROL_GAP_DIP;
        let compact = size.width as f64 / scale < regular_required;
        let edge = if compact { COMPACT_EDGE_DIP } else { EDGE_DIP } * scale;
        let gap = if compact {
            COMPACT_GAP_DIP
        } else {
            CONTROL_GAP_DIP
        } * scale;
        let available = (size.width as f64 - 2.0 * edge - 8.0 * gap).max(9.0 * scale);
        let control = (available / 9.0).min(CONTROL_DIP * scale);
        let y = (toolbar_height - control) * 0.5;
        let left = [
            ControlId::Source,
            ControlId::Split,
            ControlId::Preview,
            ControlId::ConvertMath,
        ];
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
        ); 9];
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
            controls[4 + offset] = (
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
        let opacity = controls
            .iter()
            .find_map(|(id, rect)| (*id == ControlId::Opacity).then_some(*rect))
            .unwrap_or(controls[0].1);
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
        let view = geometry(mode, size, scale as f32);
        let split_sync = view
            .divider_x
            .map(|divider_x| split_sync_rect(divider_x, view.toolbar_height, scale));
        Self {
            size,
            scale,
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
            split_sync,
        }
    }

    /// The first of, in this order: the open popup, which covers what lies under it; the resize
    /// border, which covers the buttons' outer rows; the Split control; a button; the toolbar.
    pub fn hit_test(&self, point: PhysicalPosition<f64>, popup_open: bool) -> Option<ControlHit> {
        if popup_open && let Some(hit) = self.popup_hit(point) {
            return Some(ControlHit::Popup(hit));
        }
        if let Some(edge) = resize_direction(self.size, point, self.scale) {
            return Some(ControlHit::Resize(edge));
        }
        if self.split_sync.is_some_and(|rect| rect.contains(point)) {
            return Some(ControlHit::SplitSync);
        }
        if let Some(id) = self.control_at(point) {
            return Some(ControlHit::Button(id));
        }
        self.toolbar
            .contains(point)
            .then_some(ControlHit::DragWindow)
    }

    /// What is left of `rect` once the resize border has taken its part (`hit_test`): the surface a
    /// press on it reaches. The rectangle may come out empty.
    pub fn inside_resize_border(&self, rect: ControlRect) -> ControlRect {
        let border = RESIZE_BORDER_DIP * self.scale;
        let left = rect.x.max(border);
        let top = rect.y.max(border);
        let right = (rect.x + rect.width).min(self.size.width as f64 - border);
        let bottom = (rect.y + rect.height).min(self.size.height as f64 - border);
        ControlRect {
            x: left,
            y: top,
            width: right - left,
            height: bottom - top,
        }
    }

    /// The DPI the layout was built for.
    pub fn scale(&self) -> f64 {
        self.scale
    }

    /// The nine buttons in toolbar order.
    pub fn controls(&self) -> &[(ControlId, ControlRect); 9] {
        &self.controls
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

    /// Where a press lands on the open opacity popup; `None` outside it.
    pub fn popup_hit(&self, point: PhysicalPosition<f64>) -> Option<PopupHit> {
        if self.opacity_slider.contains(point) {
            Some(PopupHit::Slider)
        } else if self.opacity_input.contains(point) {
            Some(PopupHit::Field)
        } else if self.opacity_popup.contains(point) {
            Some(PopupHit::Blank)
        } else {
            None
        }
    }

    pub fn opacity_at(&self, x: f64) -> u8 {
        let ratio = ((x - self.opacity_slider.x) / self.opacity_slider.width).clamp(0.0, 1.0);
        (40.0 + ratio * 60.0).round() as u8
    }
}

/// The window edge or corner under `point`, inside the scaled border. The far edges are inclusive,
/// so a captured pointer on the window's last column or row still finds them.
pub fn resize_direction(
    size: PhysicalSize<u32>,
    point: PhysicalPosition<f64>,
    scale: f64,
) -> Option<WindowResizeEdge> {
    let border = RESIZE_BORDER_DIP * scale.max(0.5);
    let left = point.x >= 0.0 && point.x < border;
    let right = point.x <= size.width as f64 && point.x >= size.width as f64 - border;
    let top = point.y >= 0.0 && point.y < border;
    let bottom = point.y <= size.height as f64 && point.y >= size.height as f64 - border;
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(WindowResizeEdge::NorthWest),
        (_, true, true, _) => Some(WindowResizeEdge::NorthEast),
        (true, _, _, true) => Some(WindowResizeEdge::SouthWest),
        (_, true, _, true) => Some(WindowResizeEdge::SouthEast),
        (true, _, _, _) => Some(WindowResizeEdge::West),
        (_, true, _, _) => Some(WindowResizeEdge::East),
        (_, _, true, _) => Some(WindowResizeEdge::North),
        (_, _, _, true) => Some(WindowResizeEdge::South),
        _ => None,
    }
}

/// Split-only synchronization control centered over the fixed divider.
///
/// Keeping this control out of the nine-item toolbar preserves the 23 DIP
/// minimum hit target at the 220 DIP compact-window boundary.
pub fn split_sync_rect(divider_x: u32, toolbar_height: u32, scale: f64) -> ControlRect {
    let size = 22.0 * scale.max(0.5);
    ControlRect {
        x: f64::from(divider_x) - size * 0.5,
        y: f64::from(toolbar_height) + 4.0 * scale.max(0.5),
        width: size,
        height: size,
    }
}
