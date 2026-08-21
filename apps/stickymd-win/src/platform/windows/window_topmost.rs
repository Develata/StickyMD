//! Non-activating effective topmost projection for the native window.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use std::ffi::c_void;

use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle};
use thiserror::Error;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
};

#[derive(Debug, Error)]
pub enum WindowTopmostError {
    #[error("window handle is unavailable: {0}")]
    Handle(#[from] HandleError),
    #[error("window handle is not a Win32 HWND")]
    UnsupportedHandle,
    #[error("SetWindowPos failed: {0}")]
    Apply(#[from] windows::core::Error),
}

/// Changes effective z-order without activating or moving the window.
pub fn set_window_topmost_no_activate(
    window: &impl HasWindowHandle,
    topmost: bool,
) -> Result<(), WindowTopmostError> {
    let handle = window.window_handle()?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err(WindowTopmostError::UnsupportedHandle);
    };
    let hwnd = HWND(handle.hwnd.get() as *mut c_void);
    let insert_after = if topmost {
        HWND_TOPMOST
    } else {
        HWND_NOTOPMOST
    };
    // SAFETY: `hwnd` is borrowed from a live winit top-level window on its UI
    // thread. The call changes only z-order, retains no pointers, transfers no
    // ownership, and `SWP_NOACTIVATE` prevents focus activation.
    unsafe {
        SetWindowPos(
            hwnd,
            Some(insert_after),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
    }?;
    Ok(())
}
