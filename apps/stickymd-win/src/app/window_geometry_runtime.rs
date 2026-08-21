//! Projects monitor facts and native move completion into window-domain geometry.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::platform::windows::MonitorHandleExtWindows;

use super::StickyApp;
use crate::flow::window::geometry::{MonitorGeometry, MonitorIdentity, PhysicalRect};
use crate::flow::window::state::{VisibilityState, WindowIntent};
use crate::platform::windows::monitor::{enumerate_active_displays, work_area};

impl StickyApp {
    pub(super) fn complete_window_drag(&mut self) {
        if !self.move_resize_active {
            return;
        }
        self.move_resize_active = false;
        let completed = self.window.as_ref().and_then(|window| {
            let position = window.outer_position().ok()?;
            let size = window.outer_size();
            let frame = PhysicalRect::new(position.x, position.y, size.width, size.height);
            let (monitors, current_identity) = monitor_snapshot(window);
            let monitor = current_identity
                .and_then(|identity| {
                    monitors
                        .iter()
                        .find(|candidate| candidate.identity == identity)
                        .cloned()
                })
                .or_else(|| configured_monitor(&monitors, ""))?;
            Some((frame, monitor))
        });
        if let Some((frame, monitor)) = completed {
            self.dispatch_window_intent(
                None,
                WindowIntent::DragEnded {
                    frame,
                    monitor,
                    now_ms: self.timestamp_ms(),
                },
            );
        }
        // Native geometry queries may transiently fail during topology churn.
        // The drag guard is a separate fact and must always be released.
        self.refresh_window_guards(None);
    }

    pub(super) fn recover_display_topology(&mut self) {
        let Some(window) = &self.window else { return };
        self.dispatch_window_intent(
            None,
            WindowIntent::DisplayTopologyChanged {
                monitors: monitor_geometries(window),
            },
        );
    }

    pub(super) fn window_next_deadline(&self) -> Option<u64> {
        self.window_flow
            .as_ref()
            .and_then(|flow| flow.state().next_deadline_ms())
    }

    pub(super) fn window_is_hidden_to_tray(&self) -> bool {
        self.window_flow.as_ref().is_some_and(|flow| {
            matches!(
                flow.state().visibility(),
                VisibilityState::HiddenToTray { .. }
            )
        })
    }

    pub(super) fn window_accepts_editor_mutation(&self) -> bool {
        self.shell_input_enabled
            && self
                .window_flow
                .as_ref()
                .is_some_and(|flow| flow.state().accepts_editor_mutation())
    }
}

pub(super) fn apply_frame(window: &winit::window::Window, frame: PhysicalRect) {
    window.set_outer_position(PhysicalPosition::new(frame.x, frame.y));
    let _ = window.request_inner_size(PhysicalSize::new(frame.width, frame.height));
}

pub(super) fn configured_monitor(
    monitors: &[MonitorGeometry],
    configured: &str,
) -> Option<MonitorGeometry> {
    monitors
        .iter()
        .find(|monitor| !configured.is_empty() && monitor.identity.as_str() == configured)
        .or_else(|| monitors.iter().find(|monitor| monitor.primary))
        .or_else(|| monitors.first())
        .cloned()
}

pub(super) fn monitor_geometries(window: &winit::window::Window) -> Vec<MonitorGeometry> {
    monitor_snapshot(window).0
}

fn monitor_snapshot(
    window: &winit::window::Window,
) -> (Vec<MonitorGeometry>, Option<MonitorIdentity>) {
    let active = enumerate_active_displays().unwrap_or_default();
    let current_identity = window
        .current_monitor()
        .map(|monitor| identity_for_native(&active, &monitor.native_id()));
    let primary_id = window.primary_monitor().map(|monitor| monitor.native_id());
    let geometries = window
        .available_monitors()
        .map(|monitor| {
            let native = monitor.native_id();
            let display = active
                .iter()
                .find(|display| display.gdi_device_name.eq_ignore_ascii_case(&native));
            let identity = display
                .and_then(|display| display.stable_identity)
                .map(|identity| hex_identity(identity.as_bytes()))
                .unwrap_or_else(|| format!("gdi:{}", native.to_uppercase()));
            let area = work_area(monitor.hmonitor()).ok();
            let position = monitor.position();
            let size = monitor.size();
            let work_area = area.map_or_else(
                || PhysicalRect::new(position.x, position.y, size.width, size.height),
                |area| {
                    PhysicalRect::new(
                        area.left,
                        area.top,
                        area.right.saturating_sub(area.left) as u32,
                        area.bottom.saturating_sub(area.top) as u32,
                    )
                },
            );
            MonitorGeometry::new(
                MonitorIdentity::new(identity),
                work_area,
                monitor.scale_factor(),
                primary_id
                    .as_ref()
                    .is_some_and(|primary| primary.eq_ignore_ascii_case(&native)),
            )
        })
        .collect();
    (geometries, current_identity)
}

fn identity_for_native(
    active: &[crate::platform::windows::monitor::ActiveDisplay],
    native: &str,
) -> MonitorIdentity {
    active
        .iter()
        .find(|display| display.gdi_device_name.eq_ignore_ascii_case(native))
        .and_then(|display| display.stable_identity)
        .map(|identity| MonitorIdentity::new(hex_identity(identity.as_bytes())))
        .unwrap_or_else(|| MonitorIdentity::new(format!("gdi:{}", native.to_uppercase())))
}

fn hex_identity(bytes: [u8; 32]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(64);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

#[cfg(test)]
mod phase8_monitor_projection_tests {
    use super::*;

    #[test]
    fn phase8_monitor_identity_fallback_is_case_stable_without_hmonitor_authority() {
        assert_eq!(
            identity_for_native(&[], "\\\\.\\display2"),
            MonitorIdentity::new("gdi:\\\\.\\DISPLAY2")
        );
    }
}
