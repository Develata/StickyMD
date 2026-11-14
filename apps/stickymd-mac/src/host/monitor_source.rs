//! The displays macOS reports, in the identity the config stores.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use stickymd_app::flow::window::geometry::{MonitorGeometry, MonitorIdentity, PhysicalRect};
use stickymd_app::ports::{FailureKind, MonitorSnapshot, MonitorSource};
use winit::monitor::MonitorHandle;

use super::MacHost;
use crate::platform::macos::monitor::{DisplayInsets, display_insets, stable_identity};

impl MonitorSource for MacHost {
    fn snapshot(&self) -> Result<MonitorSnapshot, FailureKind> {
        let window = self.window.as_ref().ok_or(FailureKind::Unavailable)?;
        let primary = window.primary_monitor();
        let current = window
            .current_monitor()
            .map(|monitor| identity_of(&monitor));
        let displays: Vec<_> = window
            .available_monitors()
            .map(|monitor| {
                let is_primary = primary.as_ref() == Some(&monitor);
                geometry_of(&monitor, is_primary)
            })
            .collect();
        // Every display asleep reads as no displays at all. That is not a layout to recover to:
        // the window stays where it is until they wake and the change is reported again.
        if displays.is_empty() {
            return Err(FailureKind::Unavailable);
        }
        Ok(MonitorSnapshot { displays, current })
    }
}

fn identity_of(monitor: &MonitorHandle) -> MonitorIdentity {
    MonitorIdentity::new(
        stable_identity(monitor).unwrap_or_else(|| format!("display:{:?}", monitor.name())),
    )
}

fn geometry_of(monitor: &MonitorHandle, primary: bool) -> MonitorGeometry {
    let position = monitor.position();
    let size = monitor.size();
    let scale = monitor.scale_factor();
    let insets = display_insets(monitor).unwrap_or(DisplayInsets {
        left: 0.0,
        top: 0.0,
        right: 0.0,
        bottom: 0.0,
    });
    let px = |points: f64| (points * scale).round() as i32;
    let work_area = PhysicalRect::new(
        position.x + px(insets.left),
        position.y + px(insets.top),
        (size.width as i32 - px(insets.left) - px(insets.right)).max(1) as u32,
        (size.height as i32 - px(insets.top) - px(insets.bottom)).max(1) as u32,
    );
    // The pointer stops at the screen's edge, not under the menu bar (plan 09).
    MonitorGeometry::new(identity_of(monitor), work_area, scale, primary)
        .with_top_sensor_edge(position.y)
}
