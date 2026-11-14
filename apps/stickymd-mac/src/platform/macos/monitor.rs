//! A display's work area and stable identity, from AppKit.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use std::ffi::c_void;

use objc2::msg_send;
use objc2::runtime::AnyObject;
use objc2_foundation::{NSRect, NSString};
use winit::monitor::MonitorHandle;
use winit::platform::macos::MonitorHandleExtMacOS;

#[link(name = "ColorSync", kind = "framework")]
unsafe extern "C" {
    fn CGDisplayCreateUUIDFromDisplayID(display: u32) -> *const c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFUUIDCreateString(allocator: *const c_void, uuid: *const c_void) -> *const c_void;
    fn CFRelease(object: *const c_void);
}

/// How much of a display, in points, the menu bar and the Dock take from each side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DisplayInsets {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

pub fn display_insets(monitor: &MonitorHandle) -> Option<DisplayInsets> {
    let screen = monitor.ns_screen()?.cast::<AnyObject>();
    if screen.is_null() {
        return None;
    }
    // SAFETY: winit returned a live NSScreen for this monitor, used on the main thread.
    let (frame, visible): (NSRect, NSRect) =
        unsafe { (msg_send![screen, frame], msg_send![screen, visibleFrame]) };
    Some(insets_between(frame, visible))
}

fn insets_between(frame: NSRect, visible: NSRect) -> DisplayInsets {
    DisplayInsets {
        left: visible.origin.x - frame.origin.x,
        top: (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height),
        right: (frame.origin.x + frame.size.width) - (visible.origin.x + visible.size.width),
        bottom: visible.origin.y - frame.origin.y,
    }
}

/// The display's UUID, which survives reboots and reconnection; the display number does not.
pub fn stable_identity(monitor: &MonitorHandle) -> Option<String> {
    // SAFETY: the functions take a display number and return owned Core Foundation objects, each
    // released here once. A CFString is toll-free bridged to NSString.
    unsafe {
        let uuid = CGDisplayCreateUUIDFromDisplayID(monitor.native_id());
        if uuid.is_null() {
            return None;
        }
        let text = CFUUIDCreateString(std::ptr::null(), uuid);
        CFRelease(uuid);
        if text.is_null() {
            return None;
        }
        let value = (*text.cast::<NSString>()).to_string();
        CFRelease(text);
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use objc2_foundation::{NSPoint, NSSize};

    fn rect(x: f64, y: f64, width: f64, height: f64) -> NSRect {
        NSRect::new(NSPoint::new(x, y), NSSize::new(width, height))
    }

    /// AppKit's y grows upward: the menu bar is the top inset, the Dock the bottom one.
    #[test]
    fn insets_come_from_the_difference_of_the_two_frames() {
        let insets = insets_between(
            rect(0.0, 0.0, 1512.0, 982.0),
            rect(0.0, 90.0, 1512.0, 859.0),
        );
        assert_eq!(
            insets,
            DisplayInsets {
                left: 0.0,
                top: 33.0,
                right: 0.0,
                bottom: 90.0
            }
        );
    }
}
