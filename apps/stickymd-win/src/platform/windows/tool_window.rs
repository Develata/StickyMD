//! Windows tool-window identity for the tray-owned desktop paper.
//!
//! plan_ref: docs/plan/09_windows_shell.md#tool-window-identity

use std::ffi::c_void;

use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle};
use thiserror::Error;
use windows::Win32::Foundation::{GetLastError, HWND, SetLastError, WIN32_ERROR};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, SetWindowLongPtrW, WS_EX_APPWINDOW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW,
};

#[derive(Debug, Error)]
pub enum ToolWindowError {
    #[error("window handle is unavailable: {0}")]
    Handle(#[from] HandleError),
    #[error("window handle is not a Win32 HWND")]
    UnsupportedHandle,
    #[error("GetWindowLongPtrW(GWL_EXSTYLE) failed with Win32 error {0}")]
    ReadStyle(u32),
    #[error("SetWindowLongPtrW(GWL_EXSTYLE) failed with Win32 error {0}")]
    WriteStyle(u32),
    #[error("tool-window identity unexpectedly enabled WS_EX_NOACTIVATE")]
    NoActivateRegression,
    #[error("tool-window identity was not applied; observed extended style {0:#x}")]
    IdentityNotApplied(isize),
}

/// Hides the primary window from shell switchers without making it non-activating.
pub fn apply_tool_window_identity(window: &impl HasWindowHandle) -> Result<(), ToolWindowError> {
    let handle = window.window_handle()?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err(ToolWindowError::UnsupportedHandle);
    };
    let hwnd = HWND(handle.hwnd.get() as *mut c_void);
    let previous = read_extended_style(hwnd)?;
    let desired = (previous | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);
    if desired & WS_EX_NOACTIVATE.0 as isize != 0 {
        return Err(ToolWindowError::NoActivateRegression);
    }
    if desired != previous {
        write_extended_style(hwnd, desired)?;
    }
    let observed = read_extended_style(hwnd)?;
    if !is_activating_tool_window(observed) {
        return Err(ToolWindowError::IdentityNotApplied(observed));
    }
    Ok(())
}

const fn is_activating_tool_window(style: isize) -> bool {
    style & WS_EX_TOOLWINDOW.0 as isize != 0
        && style & WS_EX_APPWINDOW.0 as isize == 0
        && style & WS_EX_NOACTIVATE.0 as isize == 0
}

fn read_extended_style(hwnd: HWND) -> Result<isize, ToolWindowError> {
    // SAFETY: `hwnd` is a live winit-owned top-level window. Zero is a valid
    // style, so last-error is cleared and checked around the getter.
    unsafe { SetLastError(WIN32_ERROR(0)) };
    // SAFETY: this reads process-owned window metadata and retains no pointer.
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    if style == 0 {
        // SAFETY: no Win32 call intervenes between the getter and this read.
        let error = unsafe { GetLastError() };
        if error != WIN32_ERROR(0) {
            return Err(ToolWindowError::ReadStyle(error.0));
        }
    }
    Ok(style)
}

fn write_extended_style(hwnd: HWND, style: isize) -> Result<(), ToolWindowError> {
    // SAFETY: zero is a valid previous style, so last-error is cleared before
    // changing only the shell-identity bits of this UI-thread-owned window.
    unsafe { SetLastError(WIN32_ERROR(0)) };
    // SAFETY: `hwnd` is live and owned by the current UI thread; the call does
    // not transfer ownership or retain `style`.
    let previous = unsafe { SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style) };
    if previous == 0 {
        // SAFETY: no Win32 call intervenes between the setter and this read.
        let error = unsafe { GetLastError() };
        if error != WIN32_ERROR(0) {
            return Err(ToolWindowError::WriteStyle(error.0));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase10_tool_style_keeps_activation_and_removes_app_window_identity() {
        let existing = WS_EX_APPWINDOW.0 as isize;
        let desired = (existing | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);
        assert!(is_activating_tool_window(desired));
    }
}
