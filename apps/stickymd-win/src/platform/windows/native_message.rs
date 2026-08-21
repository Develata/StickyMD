//! Read-only translation of relevant Win32 messages into shell facts.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use std::ffi::c_void;

use windows::Win32::UI::WindowsAndMessaging::{
    MSG, PBT_APMRESUMEAUTOMATIC, WM_DISPLAYCHANGE, WM_ENTERSIZEMOVE, WM_EXITSIZEMOVE,
    WM_POWERBROADCAST,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWindowSignal {
    MoveSizeStarted,
    MoveSizeFinished,
    DisplayTopologyChanged,
}

/// Reads one message while winit still owns and dispatches it.
///
/// This hook never consumes the message and never calls application logic.
pub fn translate_message(message: *const c_void) -> Option<NativeWindowSignal> {
    if message.is_null() {
        return None;
    }
    // SAFETY: winit's `with_msg_hook` contract supplies a non-null pointer to a
    // live Win32 MSG for the duration of this callback. MSG is only read here;
    // the pointer is neither retained nor mutated.
    let message = unsafe { &*(message.cast::<MSG>()) };
    match message.message {
        WM_ENTERSIZEMOVE => Some(NativeWindowSignal::MoveSizeStarted),
        WM_EXITSIZEMOVE => Some(NativeWindowSignal::MoveSizeFinished),
        WM_DISPLAYCHANGE => Some(NativeWindowSignal::DisplayTopologyChanged),
        WM_POWERBROADCAST if message.wParam.0 as u32 == PBT_APMRESUMEAUTOMATIC => {
            Some(NativeWindowSignal::DisplayTopologyChanged)
        }
        _ => None,
    }
}

#[cfg(test)]
mod phase8_native_message_tests {
    use super::*;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};

    fn message(kind: u32, wparam: usize) -> MSG {
        MSG {
            hwnd: HWND::default(),
            message: kind,
            wParam: WPARAM(wparam),
            lParam: LPARAM::default(),
            ..Default::default()
        }
    }

    #[test]
    fn phase8_native_message_hook_emits_only_shell_facts() {
        let start = message(WM_ENTERSIZEMOVE, 0);
        let finish = message(WM_EXITSIZEMOVE, 0);
        let topology = message(WM_DISPLAYCHANGE, 0);
        assert_eq!(
            translate_message((&raw const start).cast()),
            Some(NativeWindowSignal::MoveSizeStarted)
        );
        assert_eq!(
            translate_message((&raw const finish).cast()),
            Some(NativeWindowSignal::MoveSizeFinished)
        );
        assert_eq!(
            translate_message((&raw const topology).cast()),
            Some(NativeWindowSignal::DisplayTopologyChanged)
        );
        assert_eq!(translate_message(std::ptr::null()), None);
    }
}
