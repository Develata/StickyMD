//! Prefer ordinary foreground activation before winit's compatibility input path.
//!
//! plan_ref: docs/plan/09_windows_shell.md#tool-window-identity

use std::ffi::c_void;

use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, IsIconic, IsWindowVisible, SetForegroundWindow,
};
use winit::window::Window;

/// Called for the initial, already shown window. A refused ordinary activation
/// retains winit's existing fallback, including its foreground restrictions.
pub(crate) fn focus_on_startup(window: &Window) {
    let Ok(handle) = window.window_handle() else {
        window.focus_window();
        return;
    };
    let RawWindowHandle::Win32(handle) = handle.as_raw() else {
        window.focus_window();
        return;
    };
    request_focus(&mut NativeFocus {
        window,
        hwnd: HWND(handle.hwnd.get() as *mut c_void),
    });
}

trait FocusTarget {
    fn can_focus(&self) -> bool;
    fn is_foreground(&self) -> bool;
    fn request_foreground(&mut self) -> bool;
    fn compatibility_focus(&mut self);
}

fn request_focus(target: &mut impl FocusTarget) {
    if !target.can_focus() || target.is_foreground() {
        return;
    }
    // A successful API return alone must not be mistaken for observed focus.
    if !target.request_foreground() || !target.is_foreground() {
        target.compatibility_focus();
    }
}

struct NativeFocus<'a> {
    window: &'a Window,
    hwnd: HWND,
}

impl FocusTarget for NativeFocus<'_> {
    fn can_focus(&self) -> bool {
        // SAFETY: the borrowed HWND belongs to the live UI-thread-owned winit
        // Window; both calls only query its current visibility/minimized state.
        unsafe { IsWindowVisible(self.hwnd).as_bool() && !IsIconic(self.hwnd).as_bool() }
    }

    fn is_foreground(&self) -> bool {
        // SAFETY: this pointer-free query returns a borrowed HWND scalar.
        unsafe { GetForegroundWindow() == self.hwnd }
    }

    fn request_foreground(&mut self) -> bool {
        // SAFETY: the HWND remains owned by the live winit Window. This normal
        // activation request retains no Rust data and obeys Windows policy.
        unsafe { SetForegroundWindow(self.hwnd).as_bool() }
    }

    fn compatibility_focus(&mut self) {
        self.window.focus_window();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Target {
        available: bool,
        foreground: bool,
        accepted: bool,
        activated: bool,
        requests: usize,
        fallbacks: usize,
    }

    impl FocusTarget for Target {
        fn can_focus(&self) -> bool {
            self.available
        }

        fn is_foreground(&self) -> bool {
            self.foreground
        }

        fn request_foreground(&mut self) -> bool {
            self.requests += 1;
            self.foreground = self.activated;
            self.accepted
        }

        fn compatibility_focus(&mut self) {
            self.fallbacks += 1;
        }
    }

    fn target() -> Target {
        Target {
            available: true,
            foreground: false,
            accepted: true,
            activated: true,
            requests: 0,
            fallbacks: 0,
        }
    }

    #[test]
    fn ordinary_activation_requires_observed_foreground_before_skipping_fallback() {
        let mut target = target();
        request_focus(&mut target);
        assert!(target.foreground);
        assert_eq!((target.requests, target.fallbacks), (1, 0));
        request_focus(&mut target);
        assert_eq!((target.requests, target.fallbacks), (1, 0));
    }

    #[test]
    fn rejected_or_unconfirmed_activation_retains_the_compatibility_path() {
        for (accepted, activated) in [(false, false), (true, false), (false, true)] {
            let mut target = Target {
                accepted,
                activated,
                ..target()
            };
            request_focus(&mut target);
            assert_eq!((target.requests, target.fallbacks), (1, 1));
        }
    }

    #[test]
    fn hidden_or_minimized_window_is_not_shown_or_forced_into_focus() {
        let mut target = Target {
            available: false,
            ..target()
        };
        request_focus(&mut target);
        assert!(!target.foreground);
        assert_eq!((target.requests, target.fallbacks), (0, 0));
    }
}
