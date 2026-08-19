//! Thin Win32 clipboard text adapter for the Phase 1 text/IME spike.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-实现映射adapter-内细节可替换
//!
//! This module is the ONLY place in this spike that touches the Win32 clipboard.
//! winit 0.30 does not provide clipboard access, so a thin adapter is required.
//! Production design (arboard vs. hand-rolled) is decided after Phase 1 review.
#![allow(clippy::missing_safety_doc)]

#[cfg(windows)]
mod imp {
    use windows::Win32::Foundation::{HANDLE, HWND};
    use windows::Win32::System::DataExchange::{
        CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
    };
    use windows::Win32::System::Memory::{GMEM_MOVEABLE, GlobalAlloc, GlobalLock, GlobalUnlock};
    use windows::Win32::System::Ole::CF_UNICODETEXT;

    /// Read UTF-16 text from the clipboard, if present.
    pub fn get_text() -> Option<String> {
        // SAFETY: Open/Close are balanced on every path. GetClipboardData returns
        // a handle owned by the clipboard; we only read while it is open and do
        // not free it. GlobalLock/GlobalUnlock are balanced.
        unsafe {
            if OpenClipboard(None).is_err() {
                return None;
            }
            let mut out = None;
            if let Ok(handle) = GetClipboardData(CF_UNICODETEXT.0 as u32) {
                let ptr = GlobalLock(windows::Win32::Foundation::HGLOBAL(handle.0));
                if !ptr.is_null() {
                    let base = ptr as *const u16;
                    let mut len = 0usize;
                    while *base.add(len) != 0 {
                        len += 1;
                    }
                    let slice = core::slice::from_raw_parts(base, len);
                    out = String::from_utf16(slice).ok();
                    let _ = GlobalUnlock(windows::Win32::Foundation::HGLOBAL(handle.0));
                }
            }
            let _ = CloseClipboard();
            out
        }
    }

    /// Write `text` to the clipboard as UTF-16 (CF_UNICODETEXT).
    pub fn set_text(text: &str) -> Result<(), String> {
        let mut wide: Vec<u16> = text.encode_utf16().collect();
        wide.push(0);
        let bytes = wide.len() * 2;

        // SAFETY: Open/Close balanced. GlobalAlloc returns a movable handle that
        // we transfer to SetClipboardData on success (the clipboard then owns it),
        // so we must NOT free it after a successful SetClipboardData. GlobalLock /
        // GlobalUnlock are balanced around the copy.
        unsafe {
            OpenClipboard(None).map_err(|e| format!("OpenClipboard failed: {e}"))?;
            let _ = EmptyClipboard();
            let result = (|| -> Result<(), String> {
                let hmem =
                    GlobalAlloc(GMEM_MOVEABLE, bytes).map_err(|e| format!("GlobalAlloc: {e}"))?;
                let ptr = GlobalLock(hmem);
                if ptr.is_null() {
                    return Err("GlobalLock returned null".into());
                }
                core::ptr::copy_nonoverlapping(wide.as_ptr(), ptr as *mut u16, wide.len());
                let _ = GlobalUnlock(hmem);
                SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(hmem.0)))
                    .map_err(|e| format!("SetClipboardData: {e}"))?;
                Ok(())
            })();
            let _ = CloseClipboard();
            result
        }
    }

    /// Whether a Win32 clipboard adapter is available (always true on Windows).
    pub const fn available() -> bool {
        true
    }

    #[allow(dead_code)]
    fn _assert_hwnd_unused(_: HWND) {}
}

#[cfg(not(windows))]
mod imp {
    pub fn get_text() -> Option<String> {
        None
    }
    pub fn set_text(_text: &str) -> Result<(), String> {
        Err("clipboard adapter not available on this platform".into())
    }
    pub const fn available() -> bool {
        false
    }
}

pub use imp::*;
