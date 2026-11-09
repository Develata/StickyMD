//! The caret's blink as a layer over the window: toggling it produces no frame.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use objc2::encode::{Encoding, RefEncode};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use stickymd_app::flow::window::geometry::PhysicalRect;
use winit::window::Window;

use super::window::ns_view;

// Linking the framework is what makes `CALayer` available to `class!`.
#[link(name = "QuartzCore", kind = "framework")]
unsafe extern "C" {}

/// Opaque Core Graphics color, for the messages that pass one.
#[repr(C)]
struct CgColor {
    _private: [u8; 0],
}

// SAFETY: `CGColorRef` is a pointer to an opaque struct of this name.
unsafe impl RefEncode for CgColor {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("CGColor", &[]));
}

/// Shows or hides one inverted rectangle above the software frame.
#[derive(Default)]
pub struct CaretLayer {
    layer: Option<Retained<AnyObject>>,
    shown: Option<PhysicalRect>,
}

fn within_transaction(change: impl FnOnce()) {
    // SAFETY: class methods on CATransaction, on the main thread.
    unsafe {
        let _: () = msg_send![class!(CATransaction), begin];
        let _: () = msg_send![class!(CATransaction), setDisableActions: Bool::YES];
    }
    change();
    // SAFETY: as above, closing the transaction opened here.
    let _: () = unsafe { msg_send![class!(CATransaction), commit] };
}

impl CaretLayer {
    /// Made on first use: the software surface's own layer has to exist below it.
    fn layer(&mut self, window: &Window) -> Option<Retained<AnyObject>> {
        if self.layer.is_none() {
            let view = ns_view(window)?;
            // SAFETY: AppKit and Core Animation calls on live objects, on the main thread.
            unsafe {
                let root: *mut AnyObject = msg_send![view, layer];
                if root.is_null() {
                    return None;
                }
                let layer: Retained<AnyObject> = msg_send![class!(CALayer), new];
                let filter = NSString::from_str("differenceBlendMode");
                let _: () = msg_send![&*layer, setCompositingFilter: &*filter];
                let white: *mut AnyObject = msg_send![class!(NSColor), whiteColor];
                let color: *mut CgColor = msg_send![white, CGColor];
                let _: () = msg_send![&*layer, setBackgroundColor: color];
                let _: () = msg_send![&*layer, setZPosition: 1.0f64];
                let _: () = msg_send![&*layer, setHidden: Bool::YES];
                let _: () = msg_send![root, addSublayer: &*layer];
                self.layer = Some(layer);
            }
        }
        self.layer.clone()
    }

    /// The same rectangle again takes it away, as the overlay contract asks.
    pub fn toggle(&mut self, window: &Window, rect: PhysicalRect) -> bool {
        let Some(layer) = self.layer(window) else {
            return false;
        };
        if self.shown == Some(rect) {
            self.shown = None;
            // SAFETY: a live layer on the main thread.
            within_transaction(|| {
                let _: () = unsafe { msg_send![&*layer, setHidden: Bool::YES] };
            });
            return true;
        }
        let Some(view) = ns_view(window) else {
            return false;
        };
        let scale = window.scale_factor();
        // SAFETY: the live view's layer, on the main thread.
        let (root_height, flipped) = unsafe {
            let root: *mut AnyObject = msg_send![view, layer];
            let bounds: NSRect = msg_send![root, bounds];
            let flipped: bool = msg_send![root, isGeometryFlipped];
            (bounds.size.height, flipped)
        };
        let height = f64::from(rect.height) / scale;
        let top = f64::from(rect.y) / scale;
        let y = if flipped {
            top
        } else {
            root_height - top - height
        };
        let frame = NSRect::new(
            NSPoint::new(f64::from(rect.x) / scale, y),
            NSSize::new(f64::from(rect.width) / scale, height),
        );
        self.shown = Some(rect);
        // SAFETY: a live layer on the main thread.
        within_transaction(|| unsafe {
            let _: () = msg_send![&*layer, setFrame: frame];
            let _: () = msg_send![&*layer, setHidden: Bool::NO];
        });
        true
    }

    /// The window repainted the whole frame, which no longer includes this rectangle.
    pub fn release(&mut self) {
        self.shown = None;
        if let Some(layer) = &self.layer {
            // SAFETY: a live layer on the main thread.
            within_transaction(|| {
                let _: () = unsafe { msg_send![&**layer, setHidden: Bool::YES] };
            });
        }
    }
}
