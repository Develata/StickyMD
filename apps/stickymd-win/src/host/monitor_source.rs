//! The displays Windows reports, in the identity the config stores.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use stickymd_app::flow::window::geometry::{MonitorGeometry, MonitorIdentity, PhysicalRect};
use stickymd_app::ports::{FailureKind, MonitorSnapshot, MonitorSource};
use winit::platform::windows::MonitorHandleExtWindows;
use winit::window::Window;

use super::WinHost;
use crate::platform::windows::monitor::{ActiveDisplay, enumerate_active_displays, work_area};

impl MonitorSource for WinHost {
    fn snapshot(&self) -> Result<MonitorSnapshot, FailureKind> {
        let window = self.window.as_ref().ok_or(FailureKind::Unavailable)?;
        Ok(monitor_snapshot(window))
    }
}

pub(super) fn monitor_snapshot(window: &Window) -> MonitorSnapshot {
    let active = enumerate_active_displays().unwrap_or_default();
    let current = window
        .current_monitor()
        .map(|monitor| identity_for_native(&active, &monitor.native_id()));
    let primary_id = window.primary_monitor().map(|monitor| monitor.native_id());
    let displays = window
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
    MonitorSnapshot { displays, current }
}

fn identity_for_native(active: &[ActiveDisplay], native: &str) -> MonitorIdentity {
    active
        .iter()
        .find(|display| display.gdi_device_name.eq_ignore_ascii_case(native))
        .and_then(|display| display.stable_identity)
        .map(|identity| MonitorIdentity::new(hex_identity(identity.as_bytes())))
        .unwrap_or_else(|| MonitorIdentity::new(format!("gdi:{}", native.to_uppercase())))
}

/// The config stores this text, so its format is part of the data.
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

    #[test]
    fn phase15_a_stable_identity_is_lowercase_hex_of_all_its_bytes() {
        let mut bytes = [0; 32];
        bytes[0] = 0xab;
        bytes[31] = 0x0f;
        let text = hex_identity(bytes);
        assert_eq!(text.len(), 64);
        assert!(text.starts_with("ab00"));
        assert!(text.ends_with("000f"));
    }
}
