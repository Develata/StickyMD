//! CGEvent posting, noninteractive cancellation and per-event evidence.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::super::{
    input::{Event, Sender},
    model::{Point, Trace},
};
use super::cf::{Cf, Owned};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
static CANCELLED: AtomicBool = AtomicBool::new(false);
#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGEventCreateKeyboardEvent(source: Cf, key: u16, down: bool) -> Cf;
    fn CGEventKeyboardSetUnicodeString(event: Cf, len: usize, text: *const u16);
    fn CGEventSetFlags(event: Cf, flags: u64);
    fn CGEventCreateMouseEvent(source: Cf, kind: u32, at: Point, button: u32) -> Cf;
    fn CGEventPost(tap: u32, event: Cf);
    fn CGEventSourceKeyState(state: i32, key: u16) -> bool;
    fn CGEventSourceButtonState(state: i32, button: u32) -> bool;
}
#[link(name = "System")]
unsafe extern "C" {
    fn signal(number: i32, handler: usize) -> usize;
}
extern "C" fn cancel(_: i32) {
    CANCELLED.store(true, Ordering::Relaxed);
}
pub(in crate::mac_gui) fn install_cancel() -> Result<(), String> {
    // SAFETY: signal handler only stores a lock-free atomic; function lives for process lifetime.
    unsafe {
        for n in [2, 15] {
            if signal(n, cancel as *const () as usize) == usize::MAX {
                return Err("cannot install cancellation handler".into());
            }
        }
    }
    Ok(())
}
pub(in crate::mac_gui) fn running() -> Result<(), String> {
    if CANCELLED.load(Ordering::Relaxed) {
        Err("cancelled by SIGINT/SIGTERM; releasing owned input".into())
    } else {
        Ok(())
    }
}
pub(in crate::mac_gui) fn wait(ms: u64) -> Result<(), String> {
    let end = Instant::now() + Duration::from_millis(ms);
    while Instant::now() < end {
        running()?;
        std::thread::sleep(
            end.saturating_duration_since(Instant::now())
                .min(Duration::from_millis(20)),
        );
    }
    running()
}
pub(in crate::mac_gui) struct Events {
    pub started: Instant,
    pub last_key_ms: Option<u64>,
    pub trace: Vec<Trace>,
}
impl Events {
    pub fn new() -> Self {
        Self {
            started: Instant::now(),
            last_key_ms: None,
            trace: Vec::new(),
        }
    }
}
impl Sender for Events {
    fn pause(&mut self, ms: u64) {
        std::thread::sleep(Duration::from_millis(ms));
    }
    fn send(&mut self, event: Event) -> Result<(), String> {
        let release = matches!(
            event,
            Event::Key { down: false, .. } | Event::Button { down: false, .. }
        );
        if !release {
            running()?;
        }
        self.trace.push(Trace {
            ms: self.started.elapsed().as_millis() as u64,
            event: format!("inject {event:?}"),
        });
        // SAFETY: event constructors copy their inputs; the owned event is released once after
        // posting. Unicode points to a live u16 during the copying setter. HID tap is 0.
        unsafe {
            let native = Owned::take(match event {
                Event::Key {
                    code,
                    text,
                    down,
                    flags,
                } => {
                    let e = CGEventCreateKeyboardEvent(std::ptr::null(), code, down);
                    if !e.is_null() {
                        CGEventSetFlags(e, flags);
                        if let Some(unit) = text {
                            CGEventKeyboardSetUnicodeString(e, 1, &unit);
                        }
                    }
                    e
                }
                Event::Button { at, down } => {
                    CGEventCreateMouseEvent(std::ptr::null(), if down { 1 } else { 2 }, at, 0)
                }
                Event::Move { at, dragging } => {
                    CGEventCreateMouseEvent(std::ptr::null(), if dragging { 6 } else { 5 }, at, 0)
                }
            })?;
            if matches!(event, Event::Key { down: true, .. }) {
                self.last_key_ms = Some(self.started.elapsed().as_millis() as u64);
            }
            CGEventPost(0, native.0);
            // Post has no receipt. Release checks combined-session state, not target delivery.
            if release {
                let end = Instant::now() + Duration::from_millis(250);
                loop {
                    let down = match event {
                        Event::Key { code, .. } => CGEventSourceKeyState(0, code),
                        Event::Button { .. } => CGEventSourceButtonState(0, 0),
                        _ => false,
                    };
                    if !down {
                        break;
                    }
                    if Instant::now() >= end {
                        return Err("CGEvent release still reads down after 250ms".into());
                    }
                    std::thread::sleep(Duration::from_millis(5));
                }
            }
        }
        Ok(())
    }
}
