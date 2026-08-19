//! Thin Win32 adapter for the Phase 1 window spike.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-实现映射adapter-内细节可替换
//!
//! This module is the ONLY place in this spike that touches Win32. Each API
//! records why the cross-platform abstraction (winit 0.30) was insufficient:
//!
//! - `SetLayeredWindowAttributes`: winit 0.30 exposes no whole-window alpha.
//! - `DwmSetWindowAttribute`: winit 0.30 exposes no Win11 corner preference.
//!
//! These are spike-level probes; the production adapter design is decided
//! after Phase 1 review.
#![allow(clippy::missing_safety_doc)]

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Graphics::Dwm::{
        DwmSetWindowAttribute, DWMWA_WINDOW_CORNER_PREFERENCE,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetWindowLongPtrW, SetLayeredWindowAttributes, SetWindowLongPtrW, GWL_EXSTYLE,
        LWA_ALPHA, WS_EX_LAYERED,
    };

    /// Apply whole-window opacity (70..=100 percent) via WS_EX_LAYERED.
    ///
    /// # Safety contract
    /// `hwnd` must be a valid window handle owned by this process and must
    /// stay valid for the duration of the call.
    pub unsafe fn set_opacity_percent(hwnd: HWND, percent: u8) -> Result<(), String> {
        let percent = percent.clamp(70, 100);
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | WS_EX_LAYERED.0 as isize);
            let alpha = ((percent as u32 * 255 + 50) / 100) as u8;
            SetLayeredWindowAttributes(hwnd, Default::default(), alpha, LWA_ALPHA)
                .map_err(|e| format!("SetLayeredWindowAttributes failed: {e}"))
        }
    }

    /// Enable Windows 11 rounded corners (DWMWCP_ROUND).
    ///
    /// # Safety contract
    /// Same HWND validity contract as `set_opacity_percent`.
    pub unsafe fn enable_rounded_corners(hwnd: HWND) -> Result<(), String> {
        const DWMWCP_ROUND: u32 = 2;
        let value = DWMWCP_ROUND;
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                &value as *const u32 as *const core::ffi::c_void,
                size_of::<u32>() as u32,
            )
            .map_err(|e| format!("DwmSetWindowAttribute failed: {e}"))
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub struct Hwnd(pub *mut core::ffi::c_void);
    pub unsafe fn set_opacity_percent(_hwnd: Hwnd, _percent: u8) -> Result<(), String> {
        Err("not supported on this platform".into())
    }
    pub unsafe fn enable_rounded_corners(_hwnd: Hwnd) -> Result<(), String> {
        Err("not supported on this platform".into())
    }
}

pub use imp::*;

#[cfg(windows)]
pub fn hwnd_from_window(window: &winit::window::Window) -> Option<windows::Win32::Foundation::HWND> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    let handle = window.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(h) => Some(windows::Win32::Foundation::HWND(
            h.hwnd.get() as *mut core::ffi::c_void,
        )),
        _ => None,
    }
}
