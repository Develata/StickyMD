//! The caret as a layer over the window: showing it produces no frame, and the system compositor
//! blinks it, so the app is not woken for each blink.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

use objc2::encode::{Encoding, RefEncode};
use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use objc2_foundation::{NSPoint, NSRect, NSSize, NSString};
use stickymd_app::flow::window::geometry::PhysicalRect;
use stickymd_desktop::host::CARET_BLINK;
use winit::window::Window;

use super::window::ns_view;

// Linking the framework is what makes `CALayer` available to `class!`.
#[link(name = "QuartzCore", kind = "framework")]
unsafe extern "C" {
    /// `CAAnimationCalculationMode` for key frames that switch rather than interpolate.
    static kCAAnimationDiscrete: &'static NSString;
}

/// The key the blink animation is added under, so showing again restarts it.
const BLINK_KEY: &str = "stickymd.caret.blink";

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

/// Shown for `CARET_BLINK`, then hidden for `CARET_BLINK`, repeated by the system compositor.
/// Discrete key frames switch the opacity, so the caret never fades. Added when the caret is
/// shown, it starts shown: each new frame or caret move restarts the cycle, as typing should.
fn blink_animation() -> Retained<AnyObject> {
    let half = CARET_BLINK.as_secs_f64();
    // SAFETY: Core Animation and Foundation class methods and setters with matching argument
    // types, on the main thread. The numbers and arrays are autoreleased and retained by the
    // animation they are set on; the animation itself is returned retained.
    unsafe {
        let path = NSString::from_str("opacity");
        let animation: Retained<AnyObject> =
            msg_send![class!(CAKeyframeAnimation), animationWithKeyPath: &*path];
        let shown: *mut AnyObject = msg_send![class!(NSNumber), numberWithFloat: 1.0f32];
        let hidden: *mut AnyObject = msg_send![class!(NSNumber), numberWithFloat: 0.0f32];
        let values = [shown, hidden];
        let values: *mut AnyObject =
            msg_send![class!(NSArray), arrayWithObjects: values.as_ptr(), count: values.len()];
        // Discrete mode takes one more key time than values: each value holds until the next.
        let times: [*mut AnyObject; 3] = [
            msg_send![class!(NSNumber), numberWithDouble: 0.0f64],
            msg_send![class!(NSNumber), numberWithDouble: 0.5f64],
            msg_send![class!(NSNumber), numberWithDouble: 1.0f64],
        ];
        let times: *mut AnyObject =
            msg_send![class!(NSArray), arrayWithObjects: times.as_ptr(), count: times.len()];
        let _: () = msg_send![&*animation, setValues: values];
        let _: () = msg_send![&*animation, setKeyTimes: times];
        let _: () = msg_send![&*animation, setCalculationMode: kCAAnimationDiscrete];
        let _: () = msg_send![&*animation, setDuration: 2.0 * half];
        let _: () = msg_send![&*animation, setRepeatCount: f32::INFINITY];
        let _: () = msg_send![&*animation, setRemovedOnCompletion: Bool::NO];
        animation
    }
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
            within_transaction(|| hide(&layer));
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
        let blink = blink_animation();
        let key = NSString::from_str(BLINK_KEY);
        // SAFETY: a live layer and animation on the main thread; replacing the animation under
        // the same key restarts the blink from shown.
        within_transaction(|| unsafe {
            let _: () = msg_send![&*layer, setFrame: frame];
            let _: () = msg_send![&*layer, setHidden: Bool::NO];
            let _: () = msg_send![&*layer, removeAnimationForKey: &*key];
            let _: () = msg_send![&*layer, addAnimation: &*blink, forKey: &*key];
        });
        true
    }

    /// The window repainted the whole frame, which no longer includes this rectangle.
    pub fn release(&mut self) {
        self.shown = None;
        if let Some(layer) = &self.layer {
            within_transaction(|| hide(layer));
        }
    }
}

/// Hides the layer and stops its blink, inside the caller's transaction.
fn hide(layer: &AnyObject) {
    let key = NSString::from_str(BLINK_KEY);
    // SAFETY: a live layer on the main thread.
    unsafe {
        let _: () = msg_send![layer, removeAnimationForKey: &*key];
        let _: () = msg_send![layer, setHidden: Bool::YES];
    }
}
