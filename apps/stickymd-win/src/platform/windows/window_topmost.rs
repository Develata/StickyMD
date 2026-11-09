//! Non-activating effective topmost projection for the native window.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use std::ffi::c_void;

use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle};
use thiserror::Error;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    HWND_NOTOPMOST, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
    WS_EX_TOPMOST,
};

use super::tool_window::read_extended_style;

#[derive(Debug, Error)]
pub enum WindowTopmostError {
    #[error("window handle is unavailable: {0}")]
    Handle(#[from] HandleError),
    #[error("window handle is not a Win32 HWND")]
    UnsupportedHandle,
    #[error("SetWindowPos failed: {0}")]
    Apply(#[from] windows::core::Error),
}

/// Changes effective z-order without activating or moving the window and reads back whether the
/// window now has the topmost bit (`None` when the style cannot be read: the host cannot observe
/// it, and the z-order change itself succeeded). The bit is what this proves, not z-order against
/// other windows or focus.
pub fn set_window_topmost_no_activate(
    window: &impl HasWindowHandle,
    topmost: bool,
) -> Result<Option<bool>, WindowTopmostError> {
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
    Ok(topmost_read_back(read_extended_style(hwnd)))
}

fn topmost_read_back<E>(style: Result<isize, E>) -> Option<bool> {
    style.ok().map(has_topmost_bit)
}

const fn has_topmost_bit(style: isize) -> bool {
    style & WS_EX_TOPMOST.0 as isize != 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unreadable_style_is_a_fact_the_host_cannot_observe() {
        assert_eq!(topmost_read_back::<()>(Err(())), None);
        assert_eq!(topmost_read_back::<()>(Ok(0)), Some(false));
        assert_eq!(
            topmost_read_back::<()>(Ok(WS_EX_TOPMOST.0 as isize)),
            Some(true)
        );
    }

    #[test]
    fn only_the_topmost_bit_decides() {
        assert!(has_topmost_bit(WS_EX_TOPMOST.0 as isize));
        assert!(has_topmost_bit(WS_EX_TOPMOST.0 as isize | 0x100));
        assert!(!has_topmost_bit(0));
        assert!(!has_topmost_bit(0x100));
    }
}
