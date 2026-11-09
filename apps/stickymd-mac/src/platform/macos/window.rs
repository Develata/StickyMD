//! The AppKit window: what winit does not reach.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use std::sync::Arc;

use objc2::msg_send;
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2_foundation::{NSPoint, NSRect, NSSize};
use stickymd_app::flow::window::geometry::PhysicalRect;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

use super::app::shared_application;

/// `NSWindowCollectionBehaviorCanJoinAllSpaces | NSWindowCollectionBehaviorFullScreenAuxiliary`.
const ALL_DESKTOPS: usize = 1 | (1 << 8);
/// Above the menu bar (`kCGMainMenuWindowLevel` is 24).
pub const ABOVE_MENU_BAR_LEVEL: isize = 27;
pub const FLOATING_LEVEL: isize = 3;
pub const NORMAL_LEVEL: isize = 0;

pub fn ns_view(window: &Window) -> Option<*mut AnyObject> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    Some(handle.ns_view.as_ptr().cast())
}

pub fn ns_window(window: &Window) -> Option<*mut AnyObject> {
    let view = ns_view(window)?;
    // SAFETY: the view belongs to the live winit window, on the main thread.
    let window: *mut AnyObject = unsafe { msg_send![view, window] };
    (!window.is_null()).then_some(window)
}

/// Keeps the window on every desktop: without this, a wake on another desktop focuses a window
/// the user cannot see (plan 09).
pub fn join_all_desktops(window: &Window) {
    if let Some(native) = ns_window(window) {
        // SAFETY: a live NSWindow on the main thread.
        let _: () = unsafe { msg_send![native, setCollectionBehavior: ALL_DESKTOPS] };
    }
}

pub fn set_alpha(window: &Window, percent: u8) -> bool {
    let Some(native) = ns_window(window) else {
        return false;
    };
    let alpha = f64::from(percent.min(100)) / 100.0;
    // SAFETY: a live NSWindow on the main thread.
    let _: () = unsafe { msg_send![native, setAlphaValue: alpha] };
    true
}

pub fn set_level(window: &Window, level: isize) {
    if let Some(native) = ns_window(window) {
        // SAFETY: a live NSWindow on the main thread.
        let _: () = unsafe { msg_send![native, setLevel: level] };
    }
}

pub fn level(window: &Window) -> Option<isize> {
    let native = ns_window(window)?;
    // SAFETY: a live NSWindow on the main thread.
    Some(unsafe { msg_send![native, level] })
}

/// The screen edges in winit's coordinates, where y grows downward from the primary screen's top.
fn primary_screen_height() -> f64 {
    // SAFETY: class methods without preconditions, on the main thread.
    unsafe {
        let screens: *mut AnyObject = msg_send![objc2::class!(NSScreen), screens];
        let count: usize = msg_send![screens, count];
        if count == 0 {
            return 0.0;
        }
        let primary: *mut AnyObject = msg_send![screens, objectAtIndex: 0usize];
        let frame: NSRect = msg_send![primary, frame];
        frame.size.height
    }
}

/// One atomic move and resize: separate winit calls would show a frame of each half.
pub fn set_frame(window: &Window, target: PhysicalRect) -> bool {
    let Some(native) = ns_window(window) else {
        return false;
    };
    let scale = window.scale_factor();
    let (width, height) = (
        f64::from(target.width) / scale,
        f64::from(target.height) / scale,
    );
    let origin = NSPoint::new(
        f64::from(target.x) / scale,
        primary_screen_height() - f64::from(target.y) / scale - height,
    );
    let frame = NSRect::new(origin, NSSize::new(width, height));
    // SAFETY: a live NSWindow on the main thread.
    let _: () = unsafe { msg_send![native, setFrame: frame, display: Bool::NO] };
    true
}

/// Gives the app a Dock icon: the way back when the menu-bar icon could not be made.
pub fn use_regular_activation_policy() {
    let app = shared_application();
    // `NSApplicationActivationPolicyRegular`.
    // SAFETY: the shared application on the main thread.
    let _: bool = unsafe { msg_send![app, setActivationPolicy: 0isize] };
}

/// The pointer's place relative to a window's frame, for the strip above a top-docked window.
pub struct PointerProbe {
    window: Retained<AnyObject>,
}

impl PointerProbe {
    pub fn new(window: &Arc<Window>) -> Option<Self> {
        let native = ns_window(window)?;
        // SAFETY: a live NSWindow; retaining it keeps it valid for this probe.
        let window = unsafe { Retained::retain(native) }?;
        Some(Self { window })
    }

    /// Whether the window starts at the top of the work area and the pointer is in the bar above
    /// it, within the window's width.
    pub fn in_band_above(&self) -> bool {
        // SAFETY: the retained NSWindow and class methods, on the main thread.
        unsafe {
            let frame: NSRect = msg_send![&*self.window, frame];
            let screen: *mut AnyObject = msg_send![&*self.window, screen];
            if screen.is_null() {
                return false;
            }
            let visible: NSRect = msg_send![screen, visibleFrame];
            let whole: NSRect = msg_send![screen, frame];
            let pointer: NSPoint = msg_send![objc2::class!(NSEvent), mouseLocation];
            let window_top = frame.origin.y + frame.size.height;
            let work_top = visible.origin.y + visible.size.height;
            (window_top - work_top).abs() <= 1.0
                && pointer.x >= frame.origin.x
                && pointer.x <= frame.origin.x + frame.size.width
                && pointer.y >= window_top
                && pointer.y <= whole.origin.y + whole.size.height
        }
    }

    pub fn inside_window(&self) -> bool {
        // SAFETY: the retained NSWindow and a class method, on the main thread.
        unsafe {
            let frame: NSRect = msg_send![&*self.window, frame];
            let pointer: NSPoint = msg_send![objc2::class!(NSEvent), mouseLocation];
            pointer.x >= frame.origin.x
                && pointer.x <= frame.origin.x + frame.size.width
                && pointer.y >= frame.origin.y
                && pointer.y <= frame.origin.y + frame.size.height
        }
    }
}
