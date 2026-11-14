//! Watches the pointer while it rests in the menu-bar strip above a top-docked window, where the
//! window hears nothing. It costs nothing while the pointer is still, and ends once it has
//! reported.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use std::cell::RefCell;
use std::rc::Rc;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use stickymd_desktop::host::{DesktopEvent, EventSink, HostSignal};

use super::window::PointerProbe;

/// `NSEventMaskMouseMoved`, the three `Dragged` masks and the three `Up` masks: a drag out of the
/// strip moves no mouse in AppKit's terms, and it may end with no further movement.
const POINTER_EVENTS: u64 =
    (1 << 5) | (1 << 6) | (1 << 7) | (1 << 27) | (1 << 2) | (1 << 4) | (1 << 26);

/// The installed monitor, shared with its own handler so that the handler can remove it.
type MonitorSlot = Rc<RefCell<Option<Retained<AnyObject>>>>;

#[derive(Default)]
pub struct PointerBandWatch {
    monitor: MonitorSlot,
}

fn remove(slot: &MonitorSlot) {
    if let Some(monitor) = slot.borrow_mut().take() {
        // SAFETY: `monitor` came from addGlobalMonitor and, having left the slot, is removed once.
        let _: () = unsafe { msg_send![class!(NSEvent), removeMonitor: &*monitor] };
    }
}

impl PointerBandWatch {
    /// Posts `PointerLeft` once the pointer has left both the strip and the window.
    pub fn start(&mut self, probe: PointerProbe, sink: EventSink) {
        self.stop();
        let slot = Rc::clone(&self.monitor);
        let block = RcBlock::new(move |_event: *mut AnyObject| {
            if !probe.in_band_above() && !probe.inside_window() {
                remove(&slot);
                sink.post(DesktopEvent::Host(HostSignal::PointerLeft));
            }
        });
        // SAFETY: a class method taking an event mask and a block it copies, on the main thread.
        let monitor: Option<Retained<AnyObject>> = unsafe {
            msg_send![
                class!(NSEvent),
                addGlobalMonitorForEventsMatchingMask: POINTER_EVENTS,
                handler: &*block
            ]
        };
        *self.monitor.borrow_mut() = monitor;
    }

    pub fn stop(&mut self) {
        remove(&self.monitor);
    }
}

impl Drop for PointerBandWatch {
    fn drop(&mut self) {
        self.stop();
    }
}
