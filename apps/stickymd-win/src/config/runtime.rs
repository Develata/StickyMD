//! Versioned v1 portable configuration.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

use serde::{Deserialize, Serialize};

pub const CONFIG_VERSION: u32 = 1;
const MAX_WINDOW_DIP: u32 = 16_384;
pub const MIN_WINDOW_WIDTH_DIP: u32 = 220;
pub const MIN_WINDOW_HEIGHT_DIP: u32 = 120;

/// Durable integer percentage used by every document-content projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentZoomPercent(u16);

impl ContentZoomPercent {
    pub const MIN: u16 = 50;
    pub const MAX: u16 = 300;
    pub const DEFAULT: u16 = 100;

    pub const fn new_clamped(value: u16) -> Self {
        if value < Self::MIN {
            Self(Self::MIN)
        } else if value > Self::MAX {
            Self(Self::MAX)
        } else {
            Self(value)
        }
    }

    pub const fn value(self) -> u16 {
        self.0
    }

    pub fn factor(self) -> f32 {
        f32::from(self.value()) / 100.0
    }

    pub fn stepped(self, delta: i16) -> Self {
        let next = i32::from(self.0) + i32::from(delta);
        Self::new_clamped(next.clamp(i32::from(Self::MIN), i32::from(Self::MAX)) as u16)
    }

    const fn is_valid(self) -> bool {
        self.0 >= Self::MIN && self.0 <= Self::MAX
    }
}

impl Default for ContentZoomPercent {
    fn default() -> Self {
        Self(Self::DEFAULT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Light,
    System,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    #[default]
    Source,
    Split,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DockEdge {
    #[default]
    None,
    Left,
    Right,
    Top,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub width_dip: u32,
    pub height_dip: u32,
    pub monitor_id: String,
    pub dock_edge: DockEdge,
    pub dock_offset_ratio: f32,
    pub floating_x_ratio: f32,
    pub floating_y_ratio: f32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width_dip: 520,
            height_dip: 680,
            monitor_id: String::new(),
            dock_edge: DockEdge::None,
            dock_offset_ratio: 0.5,
            floating_x_ratio: 0.5,
            floating_y_ratio: 0.25,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeConfig {
    pub version: u32,
    pub theme: ThemeMode,
    pub opacity: u8,
    pub content_zoom_percent: ContentZoomPercent,
    pub split_scroll_sync: bool,
    pub always_on_top: bool,
    pub view_mode: ViewMode,
    pub window: WindowConfig,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            theme: ThemeMode::Light,
            opacity: 96,
            content_zoom_percent: ContentZoomPercent::default(),
            split_scroll_sync: true,
            always_on_top: false,
            view_mode: ViewMode::Source,
            window: WindowConfig::default(),
        }
    }
}

impl RuntimeConfig {
    pub(super) fn is_semantically_valid(&self) -> bool {
        (40..=100).contains(&self.opacity)
            && self.content_zoom_percent.is_valid()
            && (MIN_WINDOW_WIDTH_DIP..=MAX_WINDOW_DIP).contains(&self.window.width_dip)
            && (MIN_WINDOW_HEIGHT_DIP..=MAX_WINDOW_DIP).contains(&self.window.height_dip)
            && valid_ratio(self.window.dock_offset_ratio)
            && valid_ratio(self.window.floating_x_ratio)
            && valid_ratio(self.window.floating_y_ratio)
    }
}

fn valid_ratio(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase10_zoom_defaults_and_bounds_are_stable() {
        let default = RuntimeConfig::default();
        assert_eq!(default.content_zoom_percent.value(), 100);
        assert_eq!(ContentZoomPercent::new_clamped(1).value(), 50);
        assert_eq!(ContentZoomPercent::new_clamped(999).value(), 300);
        assert_eq!(ContentZoomPercent::default().stepped(10).value(), 110);
        assert_eq!(ContentZoomPercent::new_clamped(50).stepped(-10).value(), 50);
    }

    #[test]
    fn config_defaults_keep_phase10_and_phase14_preferences_stable() {
        let default = RuntimeConfig::default();
        assert_eq!(default.content_zoom_percent.value(), 100);
        assert!(default.split_scroll_sync);
    }
}
