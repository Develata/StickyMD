//! Minimal pre-window fatal-error presentation.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use windows::Win32::UI::WindowsAndMessaging::{MB_ICONERROR, MB_OK, MessageBoxW};
use windows::core::HSTRING;

pub fn show_error(title: &str, message: &str) {
    let title = HSTRING::from(title);
    let message = HSTRING::from(message);
    // SAFETY: both HSTRING values remain live for this synchronous call. A null
    // owner is intentional because startup failed before a window existed.
    let _ = unsafe { MessageBoxW(None, &message, &title, MB_OK | MB_ICONERROR) };
}
