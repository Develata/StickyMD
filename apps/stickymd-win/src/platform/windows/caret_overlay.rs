//! Low-cost transient caret overlay for the Win32 software surface.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use std::ffi::c_void;

use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle};
use thiserror::Error;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{DSTINVERT, GetDC, PatBlt, ReleaseDC};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaretOverlayRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Error)]
pub enum CaretOverlayError {
    #[error("window handle is unavailable: {0}")]
    Handle(#[from] HandleError),
    #[error("window handle is not a Win32 HWND")]
    UnsupportedHandle,
    #[error("caret rectangle must have positive dimensions")]
    EmptyRect,
    #[error("GetDC failed for the caret overlay")]
    DeviceContext,
    #[error("PatBlt(DSTINVERT) failed for the caret overlay")]
    Paint,
    #[error("ReleaseDC failed for the caret overlay")]
    Release,
}

/// Inverts one client-area rectangle without touching the retained software
/// framebuffer. Applying the operation twice restores the exact pixels.
///
/// The normal full-frame present remains authoritative. This adapter is only
/// a transient presentation optimization for the 550 ms caret blink; it owns
/// neither editor state nor document content.
pub fn toggle_caret_overlay(
    window: &impl HasWindowHandle,
    rect: CaretOverlayRect,
) -> Result<(), CaretOverlayError> {
    if rect.width <= 0 || rect.height <= 0 {
        return Err(CaretOverlayError::EmptyRect);
    }
    let handle = window.window_handle()?;
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        return Err(CaretOverlayError::UnsupportedHandle);
    };
    let hwnd = HWND(handle.hwnd.get() as *mut c_void);
    // SAFETY: `hwnd` belongs to the live winit window on the UI thread. GetDC
    // returns a borrowed client DC whose lifetime ends at the paired ReleaseDC.
    let device = unsafe { GetDC(Some(hwnd)) };
    if device.0.is_null() {
        return Err(CaretOverlayError::DeviceContext);
    }
    // SAFETY: the client DC is live and exclusively used by this synchronous
    // UI-thread call. The positive rectangle is copied by GDI; no pointer or
    // resource ownership is retained.
    let painted =
        unsafe { PatBlt(device, rect.x, rect.y, rect.width, rect.height, DSTINVERT) }.as_bool();
    // SAFETY: releases exactly the borrowed DC returned by GetDC for `hwnd`;
    // the adapter never stores or transfers either handle.
    let released = unsafe { ReleaseDC(Some(hwnd), device) } != 0;
    if !painted {
        return Err(CaretOverlayError::Paint);
    }
    if !released {
        return Err(CaretOverlayError::Release);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase9_caret_overlay_rejects_empty_rect_before_touching_win32() {
        struct NoHandle;
        impl HasWindowHandle for NoHandle {
            fn window_handle(
                &self,
            ) -> Result<raw_window_handle::WindowHandle<'_>, raw_window_handle::HandleError>
            {
                Err(raw_window_handle::HandleError::Unavailable)
            }
        }

        assert!(matches!(
            toggle_caret_overlay(
                &NoHandle,
                CaretOverlayRect {
                    x: 0,
                    y: 0,
                    width: 0,
                    height: 10,
                },
            ),
            Err(CaretOverlayError::EmptyRect)
        ));
    }
}
