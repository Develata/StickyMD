//! Stable Windows display identity projection.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use std::mem::size_of;

use sha2::{Digest, Sha256};
use thiserror::Error;
use windows::Win32::Devices::Display::{
    DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME, DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
    DISPLAYCONFIG_DEVICE_INFO_HEADER, DISPLAYCONFIG_MODE_INFO, DISPLAYCONFIG_PATH_INFO,
    DISPLAYCONFIG_SOURCE_DEVICE_NAME, DISPLAYCONFIG_TARGET_DEVICE_NAME, DisplayConfigGetDeviceInfo,
    GetDisplayConfigBufferSizes, QDC_ONLY_ACTIVE_PATHS, QueryDisplayConfig,
};
use windows::Win32::Foundation::{
    ERROR_INSUFFICIENT_BUFFER, ERROR_SUCCESS, GetLastError, RECT, WIN32_ERROR,
};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, HMONITOR, MONITORINFO};

const MAX_TOPOLOGY_RETRIES: usize = 4;

/// Stable hash of the Windows display device path.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct StableDisplayIdentity([u8; 32]);

impl StableDisplayIdentity {
    pub fn as_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Platform facts for mapping a winit monitor name to stable Windows identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActiveDisplay {
    pub gdi_device_name: String,
    pub friendly_name: String,
    pub stable_identity: Option<StableDisplayIdentity>,
}

/// Taskbar-excluded monitor work area in signed physical screen coordinates.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkAreaRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

#[derive(Debug, Error)]
pub enum MonitorIdentityError {
    #[error("GetDisplayConfigBufferSizes failed with Win32 error {0}")]
    BufferSizes(u32),
    #[error("display topology changed too often while it was being enumerated")]
    TopologyUnstable,
    #[error("QueryDisplayConfig failed with Win32 error {0}")]
    Query(u32),
    #[error("DisplayConfigGetDeviceInfo({kind}) failed with Win32 error {code}")]
    DeviceInfo { kind: &'static str, code: u32 },
    #[error("GetMonitorInfoW failed with Win32 error {0}")]
    WorkArea(u32),
}

/// Enumerates active display paths without deciding placement or fallback.
///
/// `gdi_device_name` maps to winit's Windows `MonitorHandle::native_id()`.
/// A missing device path is represented honestly as `None`; the coordinator,
/// not this adapter, decides whether to fall back to the primary monitor.
pub fn enumerate_active_displays() -> Result<Vec<ActiveDisplay>, MonitorIdentityError> {
    let paths = query_active_paths()?;
    paths
        .into_iter()
        .map(|path| {
            let source = source_name(&path)?;
            let target = target_name(&path)?;
            let device_path = utf16_field(&target.monitorDevicePath);
            Ok(ActiveDisplay {
                gdi_device_name: utf16_field(&source.viewGdiDeviceName),
                friendly_name: utf16_field(&target.monitorFriendlyDeviceName),
                stable_identity: (!device_path.is_empty())
                    .then(|| stable_identity_for_device_path(&device_path)),
            })
        })
        .collect()
}

/// Reads the taskbar-excluded work area for a winit Windows `hmonitor()`.
///
/// This adapter returns only the signed Windows geometry fact. The window
/// coordinator owns all DPI conversion, placement, and fallback decisions.
pub fn work_area(hmonitor: isize) -> Result<WorkAreaRect, MonitorIdentityError> {
    let mut info = MONITORINFO {
        cbSize: size_of::<MONITORINFO>() as u32,
        ..Default::default()
    };
    // SAFETY: `info` is a live, correctly sized MONITORINFO allocation and
    // `HMONITOR` is the opaque value supplied by winit. The API writes only to
    // `info` during this call and retains neither value.
    if unsafe { GetMonitorInfoW(HMONITOR(hmonitor as *mut _), &mut info) }.as_bool() {
        Ok(work_area_rect(info.rcWork))
    } else {
        // SAFETY: GetLastError reads thread-local error state immediately
        // after the failed Win32 call and has no pointer or lifetime contract.
        Err(MonitorIdentityError::WorkArea(unsafe { GetLastError() }.0))
    }
}

fn work_area_rect(rect: RECT) -> WorkAreaRect {
    WorkAreaRect {
        left: rect.left,
        top: rect.top,
        right: rect.right,
        bottom: rect.bottom,
    }
}

fn query_active_paths() -> Result<Vec<DISPLAYCONFIG_PATH_INFO>, MonitorIdentityError> {
    for _ in 0..MAX_TOPOLOGY_RETRIES {
        let mut path_count = 0;
        let mut mode_count = 0;
        // SAFETY: both count pointers are valid for writes during the call and
        // no buffers are supplied at this sizing stage.
        let status = unsafe {
            GetDisplayConfigBufferSizes(QDC_ONLY_ACTIVE_PATHS, &mut path_count, &mut mode_count)
        };
        status_ok(status).map_err(MonitorIdentityError::BufferSizes)?;

        let mut paths = vec![DISPLAYCONFIG_PATH_INFO::default(); path_count as usize];
        let mut modes = vec![DISPLAYCONFIG_MODE_INFO::default(); mode_count as usize];
        // SAFETY: the vectors have capacities described by their mutable count
        // variables. QueryDisplayConfig writes at most those counts and does
        // not retain either buffer after returning.
        let status = unsafe {
            QueryDisplayConfig(
                QDC_ONLY_ACTIVE_PATHS,
                &mut path_count,
                paths.as_mut_ptr(),
                &mut mode_count,
                modes.as_mut_ptr(),
                None,
            )
        };
        if status == ERROR_INSUFFICIENT_BUFFER {
            continue;
        }
        status_ok(status).map_err(MonitorIdentityError::Query)?;
        paths.truncate(path_count as usize);
        return Ok(paths);
    }
    Err(MonitorIdentityError::TopologyUnstable)
}

fn source_name(
    path: &DISPLAYCONFIG_PATH_INFO,
) -> Result<DISPLAYCONFIG_SOURCE_DEVICE_NAME, MonitorIdentityError> {
    let mut packet = DISPLAYCONFIG_SOURCE_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_SOURCE_NAME,
            size: size_of::<DISPLAYCONFIG_SOURCE_DEVICE_NAME>() as u32,
            adapterId: path.sourceInfo.adapterId,
            id: path.sourceInfo.id,
        },
        ..Default::default()
    };
    // SAFETY: `header` is the first field of the correctly sized packet. The
    // API uses its type/size to fill that same stack allocation and retains no
    // pointer after returning.
    let status = unsafe { DisplayConfigGetDeviceInfo(&mut packet.header) };
    status_ok(WIN32_ERROR(status as u32)).map_err(|code| MonitorIdentityError::DeviceInfo {
        kind: "source-name",
        code,
    })?;
    Ok(packet)
}

fn target_name(
    path: &DISPLAYCONFIG_PATH_INFO,
) -> Result<DISPLAYCONFIG_TARGET_DEVICE_NAME, MonitorIdentityError> {
    let mut packet = DISPLAYCONFIG_TARGET_DEVICE_NAME {
        header: DISPLAYCONFIG_DEVICE_INFO_HEADER {
            r#type: DISPLAYCONFIG_DEVICE_INFO_GET_TARGET_NAME,
            size: size_of::<DISPLAYCONFIG_TARGET_DEVICE_NAME>() as u32,
            adapterId: path.targetInfo.adapterId,
            id: path.targetInfo.id,
        },
        ..Default::default()
    };
    // SAFETY: identical packet-layout and lifetime argument as `source_name`.
    let status = unsafe { DisplayConfigGetDeviceInfo(&mut packet.header) };
    status_ok(WIN32_ERROR(status as u32)).map_err(|code| MonitorIdentityError::DeviceInfo {
        kind: "target-name",
        code,
    })?;
    Ok(packet)
}

fn status_ok(status: WIN32_ERROR) -> Result<(), u32> {
    if status == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(status.0)
    }
}

fn utf16_field(field: &[u16]) -> String {
    let end = field
        .iter()
        .position(|unit| *unit == 0)
        .unwrap_or(field.len());
    String::from_utf16_lossy(&field[..end])
}

fn stable_identity_for_device_path(device_path: &str) -> StableDisplayIdentity {
    let mut hasher = Sha256::new();
    for unit in device_path.to_uppercase().encode_utf16() {
        hasher.update(unit.to_le_bytes());
    }
    StableDisplayIdentity(hasher.finalize().into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase8_device_path_identity_uses_windows_case_insensitive_semantics() {
        let upper = stable_identity_for_device_path(r"\\?\DISPLAY#ACME123#INSTANCE");
        let lower = stable_identity_for_device_path(r"\\?\display#acme123#instance");
        assert_eq!(upper, lower);
        assert_ne!(
            upper,
            stable_identity_for_device_path(r"\\?\DISPLAY#OTHER#INSTANCE")
        );
    }

    #[test]
    fn phase8_utf16_fields_stop_at_nul() {
        assert_eq!(
            utf16_field(&['A' as u16, '中' as u16, 0, 'Z' as u16]),
            "A中"
        );
    }

    #[test]
    fn phase8_work_area_rect_preserves_negative_virtual_screen_coordinates() {
        assert_eq!(
            work_area_rect(RECT {
                left: -2560,
                top: -240,
                right: 0,
                bottom: 1200,
            }),
            WorkAreaRect {
                left: -2560,
                top: -240,
                right: 0,
                bottom: 1200,
            }
        );
    }

    #[test]
    fn phase8_active_display_enumeration_returns_mappable_windows_facts() {
        let displays =
            enumerate_active_displays().expect("active display topology should enumerate");
        assert!(!displays.is_empty(), "Windows reported no active displays");
        assert!(
            displays
                .iter()
                .all(|display| !display.gdi_device_name.is_empty()),
            "every active path must expose its GDI device name"
        );
    }
}
