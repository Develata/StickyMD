//! What the running application hears from the system: being opened again and being asked to end.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use std::cell::{Cell, OnceCell};

use objc2::rc::Retained;
use objc2::runtime::{AnyClass, AnyObject, Bool, Imp, NSObject, Sel};
use objc2::{AllocAnyThread, class, define_class, msg_send, sel};
use objc2_foundation::{NSArray, NSString};
use stickymd_app::ports::TrayCommand;
use stickymd_desktop::host::{DesktopEvent, EventSink, HostSignal};

/// `NSTerminateCancel`.
const TERMINATE_CANCEL: usize = 0;
/// `NSTerminateLater`.
const TERMINATE_LATER: usize = 2;
/// The system gives a logout about two minutes; answer before it stops waiting.
const QUIT_ANSWER_DEADLINE_SECONDS: f64 = 100.0;
/// `kCoreEventClass` and `kAEReopenApplication`.
const CORE_EVENT_CLASS: u32 = u32::from_be_bytes(*b"aevt");
const REOPEN_APPLICATION: u32 = u32::from_be_bytes(*b"rapp");
/// Displays, the menu bar or the Dock changed the space there is.
const SCREEN_PARAMETERS_CHANGED: &str = "NSApplicationDidChangeScreenParametersNotification";

thread_local! {
    static BRIDGE: OnceCell<Retained<AppBridge>> = const { OnceCell::new() };
    static SINK: OnceCell<EventSink> = const { OnceCell::new() };
    static TERMINATION_PENDING: Cell<bool> = const { Cell::new(false) };
}

unsafe extern "C" {
    fn object_getClass(object: *const AnyObject) -> *const AnyClass;
    fn class_addMethod(
        class: *const AnyClass,
        name: Sel,
        imp: Imp,
        types: *const std::ffi::c_char,
    ) -> Bool;
}

fn post(signal: HostSignal) {
    SINK.with(|sink| {
        if let Some(sink) = sink.get() {
            sink.post(DesktopEvent::Host(signal));
        }
    });
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "StickyMDAppBridge"]
    struct AppBridge;

    impl AppBridge {
        #[unsafe(method(handleReopen:withReply:))]
        fn handle_reopen(&self, _event: *mut AnyObject, _reply: *mut AnyObject) {
            post(HostSignal::ShowRequested);
        }

        #[unsafe(method(screenParametersChanged:))]
        fn screen_parameters_changed(&self, _notification: *mut AnyObject) {
            post(HostSignal::DisplayTopologyChanged);
        }

        #[unsafe(method(quitDeadlinePassed:))]
        fn quit_deadline_passed(&self, _ignored: *mut AnyObject) {
            post(HostSignal::QuitDeadlinePassed);
        }

        #[unsafe(method(answerYes:))]
        fn answer_yes(&self, _ignored: *mut AnyObject) {
            answer_termination(true);
        }

        #[unsafe(method(answerNo:))]
        fn answer_no(&self, _ignored: *mut AnyObject) {
            answer_termination(false);
        }
    }
);

pub fn shared_application() -> *mut AnyObject {
    // SAFETY: a class method without preconditions, on the main thread.
    unsafe { msg_send![class!(NSApplication), sharedApplication] }
}

const PRESS_AND_HOLD: &str = "ApplePressAndHoldEnabled";

/// A key held down repeats as the keyboard settings say instead of opening the accent popup,
/// whose replacement needs the range winit drops (plan 07). The value goes into the argument
/// domain, which outranks the user's own and global settings and is never written to disk.
/// Returns whether the popup is now off, read back as AppKit will read it.
pub fn disable_press_and_hold() -> bool {
    let key = NSString::from_str(PRESS_AND_HOLD);
    let argument_domain = NSString::from_str("NSArgumentDomain");
    // SAFETY: Foundation calls with valid objects. `mutableCopy` and `new` return owned objects,
    // which `Retained` releases.
    unsafe {
        let defaults: *mut AnyObject = msg_send![class!(NSUserDefaults), standardUserDefaults];
        let current: *mut AnyObject = msg_send![defaults, volatileDomainForName: &*argument_domain];
        let domain: Retained<AnyObject> = if current.is_null() {
            msg_send![class!(NSMutableDictionary), new]
        } else {
            msg_send![current, mutableCopy]
        };
        let off: *mut AnyObject = msg_send![class!(NSNumber), numberWithBool: Bool::NO];
        let _: () = msg_send![&*domain, setObject: off, forKey: &*key];
        let _: () = msg_send![defaults, setVolatileDomain: &*domain, forName: &*argument_domain];
        let enabled: bool = msg_send![defaults, boolForKey: &*key];
        !enabled
    }
}

fn bridge() -> Retained<AppBridge> {
    BRIDGE.with(|cell| {
        cell.get_or_init(|| {
            // SAFETY: plain allocation and init of our own NSObject subclass.
            unsafe { msg_send![AppBridge::alloc(), init] }
        })
        .clone()
    })
}

fn run_loop_modes() -> Retained<NSArray<NSString>> {
    // While an answer is pending AppKit runs the loop in the modal-panel mode.
    NSArray::from_retained_slice(&[
        NSString::from_str("kCFRunLoopDefaultMode"),
        NSString::from_str("NSModalPanelRunLoopMode"),
    ])
}

fn perform_later(selector: Sel, delay: f64) {
    let bridge = bridge();
    let modes = run_loop_modes();
    let nil: *mut AnyObject = std::ptr::null_mut();
    // SAFETY: the bridge implements `selector` and lives for the process.
    let _: () = unsafe {
        msg_send![
            &*bridge,
            performSelector: selector,
            withObject: nil,
            afterDelay: delay,
            inModes: &*modes
        ]
    };
}

fn cancel_deadline() {
    let bridge = bridge();
    let nil: *mut AnyObject = std::ptr::null_mut();
    // SAFETY: cancelling a perform request on our own object is always valid.
    let _: () = unsafe {
        msg_send![
            class!(NSObject),
            cancelPreviousPerformRequestsWithTarget: &*bridge,
            selector: sel!(quitDeadlinePassed:),
            object: nil
        ]
    };
}

fn answer_termination(terminate: bool) {
    if !TERMINATION_PENDING.with(Cell::get) {
        return;
    }
    TERMINATION_PENDING.with(|pending| pending.set(false));
    cancel_deadline();
    let app = shared_application();
    // SAFETY: the shared application on the main thread, with an answer outstanding.
    let _: () = unsafe { msg_send![app, replyToApplicationShouldTerminate: Bool::new(terminate)] };
}

/// The answer that lets the process end. It is sent after the event handler has returned, because
/// ending re-enters winit, which refuses to run inside its own handler.
pub fn termination_finished() {
    if TERMINATION_PENDING.with(Cell::get) {
        perform_later(sel!(answerYes:), 0.0);
    }
}

pub fn termination_abandoned() {
    if TERMINATION_PENDING.with(Cell::get) {
        perform_later(sel!(answerNo:), 0.0);
    }
}

extern "C-unwind" fn should_terminate(
    _this: *mut AnyObject,
    _selector: Sel,
    _sender: *mut AnyObject,
) -> usize {
    let app = shared_application();
    // SAFETY: the shared application on the main thread.
    let modal: *mut AnyObject = unsafe { msg_send![app, modalWindow] };
    if !modal.is_null() {
        // The dialog sits on the stack inside the event handler. Answering "later" would start a
        // nested loop above it, so it could never unwind and the quit could never reach the shell.
        // The request ends here instead: the quit runs once the dialog has returned.
        // SAFETY: as above, with a modal session running.
        let _: () = unsafe { msg_send![app, abortModal] };
        post(HostSignal::Tray(TrayCommand::Quit));
        return TERMINATE_CANCEL;
    }
    if !TERMINATION_PENDING.with(Cell::get) {
        TERMINATION_PENDING.with(|pending| pending.set(true));
        post(HostSignal::Tray(TrayCommand::Quit));
        perform_later(sel!(quitDeadlinePassed:), QUIT_ANSWER_DEADLINE_SECONDS);
    }
    TERMINATE_LATER
}

/// Connects the reopen event and the termination request. Runs once, after launch has finished:
/// AppKit registers its own reopen handler during launch.
pub fn install_hooks(sink: EventSink) {
    if SINK.with(|cell| cell.set(sink)).is_err() {
        return;
    }
    let bridge = bridge();
    let app = shared_application();
    // SAFETY: Objective-C runtime calls on live objects, on the main thread. The added method's
    // encoding matches `should_terminate`, and the transmute only erases its signature for the
    // runtime, which calls it with exactly those arguments.
    unsafe {
        let manager: *mut AnyObject =
            msg_send![class!(NSAppleEventManager), sharedAppleEventManager];
        let _: () = msg_send![
            manager,
            setEventHandler: &*bridge,
            andSelector: sel!(handleReopen:withReply:),
            forEventClass: CORE_EVENT_CLASS,
            andEventID: REOPEN_APPLICATION
        ];
        let center: *mut AnyObject = msg_send![class!(NSNotificationCenter), defaultCenter];
        let name = NSString::from_str(SCREEN_PARAMETERS_CHANGED);
        let _: () = msg_send![
            center,
            addObserver: &*bridge,
            selector: sel!(screenParametersChanged:),
            name: &*name,
            object: app
        ];
        let delegate: *mut AnyObject = msg_send![app, delegate];
        if !delegate.is_null() {
            let class = object_getClass(delegate.cast_const());
            let imp: Imp = std::mem::transmute(should_terminate as *const ());
            let added = class_addMethod(
                class,
                sel!(applicationShouldTerminate:),
                imp,
                c"Q@:@".as_ptr(),
            );
            if !added.as_bool() {
                eprintln!("cannot intercept system termination: the delegate already handles it");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A value registered or passed on the command line before stays where it was; ours wins.
    #[test]
    fn press_and_hold_is_off_whatever_was_registered() {
        let key = NSString::from_str(PRESS_AND_HOLD);
        let other = NSString::from_str("StickyMDProbeArgument");
        let argument_domain = NSString::from_str("NSArgumentDomain");
        // SAFETY: Foundation calls with valid objects; only volatile, in-process domains change.
        unsafe {
            let defaults: *mut AnyObject = msg_send![class!(NSUserDefaults), standardUserDefaults];
            let on: *mut AnyObject = msg_send![class!(NSNumber), numberWithBool: Bool::YES];
            let registered: *mut AnyObject =
                msg_send![class!(NSDictionary), dictionaryWithObject: on, forKey: &*key];
            let _: () = msg_send![defaults, registerDefaults: registered];
            let arguments: *mut AnyObject =
                msg_send![class!(NSDictionary), dictionaryWithObject: on, forKey: &*other];
            let _: () =
                msg_send![defaults, setVolatileDomain: arguments, forName: &*argument_domain];
            assert!(disable_press_and_hold());
            let kept: bool = msg_send![defaults, boolForKey: &*other];
            assert!(kept);
        }
    }
}
