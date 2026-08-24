//! Read-only translation of relevant Win32 messages into shell facts.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use std::ffi::c_void;

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::WindowsAndMessaging::{
    MSG, PBT_APMRESUMEAUTOMATIC, SendMessageW, WM_DISPLAYCHANGE, WM_NCLBUTTONDOWN,
    WM_POWERBROADCAST,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeWindowSignal {
    DisplayTopologyChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct NativeMessageAction {
    pub signal: Option<NativeWindowSignal>,
    pub handled: bool,
}

/// Reads one message while winit still owns and dispatches it.
///
/// Application facts are translated without consuming the source message.
/// winit's malformed synthetic drag message is consumed and synchronously
/// redispatched once with a valid coordinate payload so winit still owns the
/// native drag lifecycle and its `WM_EXITSIZEMOVE` cleanup. Synchronous
/// dispatch starts the modal move loop before another thread can move the
/// cursor away from this message's repaired anchor.
pub fn handle_message(message: *const c_void) -> NativeMessageAction {
    if message.is_null() {
        return NativeMessageAction::default();
    }
    // SAFETY: winit's `with_msg_hook` contract supplies a non-null pointer to
    // a live Win32 MSG for this callback. It is read only and not kept.
    let message = unsafe { &*(message.cast::<MSG>()) };
    if message.message == WM_NCLBUTTONDOWN {
        // A queued MSG owns the screen coordinate recorded when the message
        // was posted. It is the race-free replacement for winit's invalid
        // pointer payload and needs no second desktop query.
        let lparam = repair_non_client_drag_lparam(pack_screen_point(message.pt));
        // SAFETY: all scalar values are copied from the live queued MSG; the
        // target HWND remains owned by winit. Same-thread SendMessageW performs
        // a nested synchronous dispatch and retains no Rust data.
        unsafe {
            SendMessageW(
                message.hwnd,
                message.message,
                Some(message.wParam),
                Some(lparam),
            );
        }
        return NativeMessageAction {
            signal: None,
            handled: true,
        };
    }
    NativeMessageAction {
        signal: translate_message(message),
        handled: false,
    }
}

fn translate_message(message: &MSG) -> Option<NativeWindowSignal> {
    match message.message {
        WM_DISPLAYCHANGE => Some(NativeWindowSignal::DisplayTopologyChanged),
        WM_POWERBROADCAST if message.wParam.0 as u32 == PBT_APMRESUMEAUTOMATIC => {
            Some(NativeWindowSignal::DisplayTopologyChanged)
        }
        _ => None,
    }
}

/// Constructs a correctly packed replacement for a non-client drag message.
///
/// winit 0.30.13 (and the currently published neighbouring versions) casts a
/// stack `POINTS` address to `LPARAM` in its Windows `handle_os_dragging`
/// implementation. Win32 instead requires signed screen x/y values packed in
/// the low/high 16-bit words. Repairing the queued message here keeps winit's
/// native dragging state and `WM_EXITSIZEMOVE` cleanup authoritative while
/// avoiding a duplicated window-move state machine in StickyMD.
fn repair_non_client_drag_lparam(packed_position: u32) -> windows::Win32::Foundation::LPARAM {
    windows::Win32::Foundation::LPARAM((packed_position as i32) as isize)
}

fn pack_screen_point(point: POINT) -> u32 {
    u32::from(point.x as i16 as u16) | (u32::from(point.y as i16 as u16) << 16)
}

#[cfg(test)]
mod phase8_native_message_tests {
    use super::*;
    use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
    use windows::Win32::UI::WindowsAndMessaging::WM_EXITSIZEMOVE;

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
        let finish = message(WM_EXITSIZEMOVE, 0);
        let topology = message(WM_DISPLAYCHANGE, 0);
        assert_eq!(translate_message(&finish), None);
        assert_eq!(
            translate_message(&topology),
            Some(NativeWindowSignal::DisplayTopologyChanged)
        );
        assert_eq!(
            handle_message(std::ptr::null()),
            NativeMessageAction::default()
        );
    }

    #[test]
    fn phase8_native_drag_repair_packs_signed_screen_coordinates_in_lparam() {
        let x = -32_i16;
        let y = -15_i16;
        let packed = u32::from(x as u16) | (u32::from(y as u16) << 16);
        let repaired = repair_non_client_drag_lparam(packed);

        let actual = repaired.0 as u32;
        assert_eq!(actual as u16 as i16, x);
        assert_eq!((actual >> 16) as u16 as i16, y);
    }
}
