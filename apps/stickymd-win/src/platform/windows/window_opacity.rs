//! Whole-window alpha for the native StickyMD window.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use std::ffi::c_void;

use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle};
use thiserror::Error;
use windows::Win32::Foundation::{COLORREF, GetLastError, HWND, SetLastError, WIN32_ERROR};
use windows::Win32::UI::WindowsAndMessaging::{
    GWL_EXSTYLE, GetWindowLongPtrW, LWA_ALPHA, SetLayeredWindowAttributes, SetWindowLongPtrW,
    WS_EX_LAYERED,
};

pub const MIN_OPACITY_PERCENT: u8 = 40;
pub const MAX_OPACITY_PERCENT: u8 = 100;

#[derive(Debug, Error)]
pub enum WindowOpacityError {
    #[error("opacity must be in the inclusive range 40..=100, got {0}")]
    OutOfRange(u8),
    #[error("window handle is unavailable: {0}")]
    Handle(#[from] HandleError),
    #[error("window handle is not a Win32 HWND")]
    UnsupportedHandle,
    #[error("GetWindowLongPtrW(GWL_EXSTYLE) failed with Win32 error {0}")]
    ReadStyle(u32),
    #[error("SetWindowLongPtrW(GWL_EXSTYLE) failed with Win32 error {0}")]
    WriteStyle(u32),
    #[error(
        "SetLayeredWindowAttributes failed: {source}; style rollback error: {rollback_error:?}"
    )]
    SetAlpha {
        source: windows::core::Error,
        rollback_error: Option<u32>,
    },
}

/// Applies full-window opacity while preserving every unrelated extended style.
///
/// At 100%, StickyMD removes `WS_EX_LAYERED` instead of leaving an unnecessary
/// layered-window path enabled. The window must be owned by the calling UI
/// thread, matching winit's Win32 handle-affinity contract.
pub fn set_window_opacity(
    window: &impl HasWindowHandle,
    opacity_percent: u8,
) -> Result<(), WindowOpacityError> {
    if !(MIN_OPACITY_PERCENT..=MAX_OPACITY_PERCENT).contains(&opacity_percent) {
        return Err(WindowOpacityError::OutOfRange(opacity_percent));
    }

    let hwnd = win32_hwnd(window)?;
    let previous_style = read_extended_style(hwnd)?;
    let layered_style = WS_EX_LAYERED.0 as isize;
    let desired_style = if opacity_percent == MAX_OPACITY_PERCENT {
        previous_style & !layered_style
    } else {
        previous_style | layered_style
    };

    if desired_style != previous_style {
        try_write_extended_style(hwnd, desired_style).map_err(WindowOpacityError::WriteStyle)?;
    }
    if opacity_percent == MAX_OPACITY_PERCENT {
        return Ok(());
    }

    let alpha = alpha_from_percent(opacity_percent);
    // SAFETY: `hwnd` comes from a live winit `Window` on its owner thread. The
    // adapter has established `WS_EX_LAYERED`; color key is ignored under
    // `LWA_ALPHA`, and the call neither borrows nor transfers the handle.
    if let Err(source) = unsafe { SetLayeredWindowAttributes(hwnd, COLORREF(0), alpha, LWA_ALPHA) }
    {
        let rollback_error = if desired_style != previous_style {
            try_write_extended_style(hwnd, previous_style).err()
        } else {
            None
        };
        return Err(WindowOpacityError::SetAlpha {
            source,
            rollback_error,
        });
    }
    Ok(())
}

fn win32_hwnd(window: &impl HasWindowHandle) -> Result<HWND, WindowOpacityError> {
    let handle = window.window_handle()?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err(WindowOpacityError::UnsupportedHandle);
    };
    Ok(HWND(handle.hwnd.get() as *mut c_void))
}

fn read_extended_style(hwnd: HWND) -> Result<isize, WindowOpacityError> {
    // SAFETY: `hwnd` is a live top-level window handle. Clearing last-error is
    // required because a zero style is valid and otherwise indistinguishable
    // from the API's failure sentinel.
    unsafe { SetLastError(WIN32_ERROR(0)) };
    // SAFETY: the handle is valid and `GWL_EXSTYLE` reads process-owned window
    // metadata without retaining pointers or transferring ownership.
    let style = unsafe { GetWindowLongPtrW(hwnd, GWL_EXSTYLE) };
    if style == 0 {
        // SAFETY: reads the calling thread's last-error value immediately after
        // `GetWindowLongPtrW`, before any intervening Win32 call.
        let error = unsafe { GetLastError() };
        if error != WIN32_ERROR(0) {
            return Err(WindowOpacityError::ReadStyle(error.0));
        }
    }
    Ok(style)
}

fn try_write_extended_style(hwnd: HWND, style: isize) -> Result<(), u32> {
    // SAFETY: see `read_extended_style`; zero is also a valid previous style,
    // so last-error must be cleared and inspected around the setter.
    unsafe { SetLastError(WIN32_ERROR(0)) };
    // SAFETY: the handle is live and belongs to this UI thread. `style` was
    // derived from its existing `GWL_EXSTYLE`, changing only `WS_EX_LAYERED`.
    let previous = unsafe { SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style) };
    if previous == 0 {
        // SAFETY: reads the setter's last-error value without an intervening
        // Win32 call.
        let error = unsafe { GetLastError() };
        if error != WIN32_ERROR(0) {
            return Err(error.0);
        }
    }
    Ok(())
}

const fn alpha_from_percent(percent: u8) -> u8 {
    (((percent as u16) * 255 + 50) / 100) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase8_alpha_conversion_rounds_to_nearest_byte() {
        assert_eq!(alpha_from_percent(40), 102);
        assert_eq!(alpha_from_percent(70), 179);
        assert_eq!(alpha_from_percent(85), 217);
        assert_eq!(alpha_from_percent(96), 245);
        assert_eq!(alpha_from_percent(100), 255);
    }
}
