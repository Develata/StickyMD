//! Opt-in input-latency diagnostic for the VM driver (`stickymd-smoke mac-latency`). It is off
//! unless `STICKYMD_DIAGNOSTIC_INPUT_LATENCY` names an output file, and then records timestamps
//! only, never a key's identity or the note's text:
//!
//! - every key-down: the event's own timestamp, and when the main thread saw it (a local event
//!   monitor runs before AppKit dispatches the event);
//! - every presented frame: how long drawing it took, when `present` returned, and when Core
//!   Animation finished committing that transaction (its completion block).
//!
//! The buffers are allocated once; a full buffer counts what it drops and never grows, so the key
//! path does no I/O and no allocation. SIGUSR1 writes the file on the main thread, atomically;
//! nothing is written at exit. Times are nanoseconds of `CLOCK_UPTIME_RAW`, the
//! `mach_absolute_time` clock event timestamps use too.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

use std::cell::RefCell;
use std::ffi::c_void;
use std::fmt::Write as _;
use std::path::PathBuf;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{class, msg_send};

/// The environment variable naming the output file.
pub const ENV: &str = "STICKYMD_DIAGNOSTIC_INPUT_LATENCY";
/// Samples kept per kind; a measurement run types a few hundred keys.
const CAPACITY: usize = 8192;
/// `NSEventMaskKeyDown`.
const KEY_DOWN_MASK: u64 = 1 << 10;

struct Recorder {
    path: PathBuf,
    /// `(event, seen)` per key-down.
    keys: Vec<(u64, u64)>,
    /// `(presented, committed, drawn)` per frame: `committed` is 0 until the commit is seen,
    /// `drawn` is the drawing time in nanoseconds.
    frames: Vec<(u64, u64, u64)>,
    dropped_keys: u64,
    dropped_frames: u64,
    /// Kept alive for the process: removing it would stop the recording.
    _monitor: Option<Retained<AnyObject>>,
}

thread_local! {
    /// Main thread only: the monitor, `after_present`, the completion blocks and the signal
    /// handler all run there.
    static RECORDER: RefCell<Option<Recorder>> = const { RefCell::new(None) };
}

unsafe extern "C" {
    fn clock_gettime_nsec_np(clock: libc::clockid_t) -> u64;
}

fn now_ns() -> u64 {
    // SAFETY: reads a monotonic clock by its id; no preconditions (0 only for an invalid id).
    unsafe { clock_gettime_nsec_np(libc::CLOCK_UPTIME_RAW) }
}

/// Starts recording when the environment asks for it. Call once, on the main thread, after the
/// application object exists. Without the variable it does nothing.
pub fn install_from_environment() {
    let Some(path) = std::env::var_os(ENV).filter(|value| !value.is_empty()) else {
        return;
    };
    if RECORDER.with(|recorder| recorder.borrow().is_some()) {
        return;
    }
    if now_ns() == 0 {
        eprintln!("input latency diagnostic disabled: no uptime clock");
        return;
    }
    let block = RcBlock::new(move |event: *mut AnyObject| -> *mut AnyObject {
        if !event.is_null() {
            // SAFETY: AppKit hands the monitor a live NSEvent for the duration of the call.
            let stamp: f64 = unsafe { msg_send![event, timestamp] };
            let seen = now_ns();
            RECORDER.with(|recorder| {
                if let Some(recorder) = recorder.borrow_mut().as_mut() {
                    if recorder.keys.len() < CAPACITY {
                        recorder.keys.push(((stamp * 1e9) as u64, seen));
                    } else {
                        recorder.dropped_keys += 1;
                    }
                }
            });
        }
        // Returning the event lets AppKit dispatch it as usual.
        event
    });
    // SAFETY: a class method taking an event mask and a block it copies, on the main thread.
    let monitor: Option<Retained<AnyObject>> = unsafe {
        msg_send![
            class!(NSEvent),
            addLocalMonitorForEventsMatchingMask: KEY_DOWN_MASK,
            handler: &*block
        ]
    };
    if monitor.is_none() {
        eprintln!("input latency diagnostic disabled: no event monitor");
        return;
    }
    RECORDER.with(|recorder| {
        *recorder.borrow_mut() = Some(Recorder {
            path: PathBuf::from(path),
            keys: Vec::with_capacity(CAPACITY),
            frames: Vec::with_capacity(CAPACITY),
            dropped_keys: 0,
            dropped_frames: 0,
            _monitor: monitor,
        });
    });
    if let Err(error) = install_flush_signal() {
        eprintln!("input latency diagnostic cannot be written: {error}");
    }
}

/// A frame reached the window: its present time now, its commit time when Core Animation has
/// committed the transaction it belongs to.
pub fn frame_presented(draw: std::time::Duration) {
    RECORDER.with(|cell| {
        let mut borrowed = cell.borrow_mut();
        let Some(recorder) = borrowed.as_mut() else {
            return;
        };
        if recorder.frames.len() >= CAPACITY {
            recorder.dropped_frames += 1;
            return;
        }
        recorder.frames.push((
            now_ns(),
            0,
            u64::try_from(draw.as_nanos()).unwrap_or(u64::MAX),
        ));
        drop(borrowed);
        let block = RcBlock::new(move || {
            let committed = now_ns();
            RECORDER.with(|cell| {
                if let Some(recorder) = cell.borrow_mut().as_mut() {
                    // One transaction can carry several frames, and only its last completion
                    // block runs: every frame still waiting is committed now.
                    for frame in recorder.frames.iter_mut().rev() {
                        if frame.1 != 0 {
                            break;
                        }
                        frame.1 = committed;
                    }
                }
            });
        });
        // SAFETY: a class method on the current (implicit) transaction, on the main thread; the
        // transaction copies the block.
        let _: () = unsafe { msg_send![class!(CATransaction), setCompletionBlock: &*block] };
    });
}

fn write_file() {
    RECORDER.with(|cell| {
        let borrowed = cell.borrow();
        let Some(recorder) = borrowed.as_ref() else {
            return;
        };
        let mut text = String::with_capacity(64 * (recorder.keys.len() + recorder.frames.len()));
        let _ = writeln!(text, "stickymd-input-latency 2");
        let _ = writeln!(
            text,
            "dropped {} {}",
            recorder.dropped_keys, recorder.dropped_frames
        );
        for (event, seen) in &recorder.keys {
            let _ = writeln!(text, "key {event} {seen}");
        }
        for (presented, committed, drawn) in &recorder.frames {
            let _ = writeln!(text, "frame {presented} {committed} {drawn}");
        }
        let temporary = recorder.path.with_extension("partial");
        let result = std::fs::write(&temporary, text.as_bytes())
            .and_then(|()| std::fs::rename(&temporary, &recorder.path));
        if let Err(error) = result {
            eprintln!("input latency diagnostic not written: {error}");
        }
    });
}

unsafe extern "C" {
    static _dispatch_source_type_signal: c_void;
    static _dispatch_main_q: c_void;
    fn dispatch_source_create(
        kind: *const c_void,
        handle: usize,
        mask: usize,
        queue: *const c_void,
    ) -> *mut c_void;
    fn dispatch_source_set_event_handler_f(
        source: *mut c_void,
        handler: extern "C" fn(*mut c_void),
    );
    fn dispatch_resume(object: *mut c_void);
}

extern "C" fn on_flush(_context: *mut c_void) {
    write_file();
}

/// SIGUSR1, delivered through a dispatch source on the main queue, writes the file. The default
/// action (terminate) is ignored only once the source exists.
fn install_flush_signal() -> Result<(), String> {
    // SAFETY: creates a signal source on the main queue with a plain C handler; the source lives
    // for the process. Ignoring the signal's default action only after it exists means a signal
    // sent before still ends the process as before.
    unsafe {
        let source = dispatch_source_create(
            &raw const _dispatch_source_type_signal,
            libc::SIGUSR1 as usize,
            0,
            &raw const _dispatch_main_q,
        );
        if source.is_null() {
            return Err("dispatch_source_create failed for SIGUSR1".into());
        }
        dispatch_source_set_event_handler_f(source, on_flush);
        dispatch_resume(source);
        if libc::signal(libc::SIGUSR1, libc::SIG_IGN) == libc::SIG_ERR {
            return Err(std::io::Error::last_os_error().to_string());
        }
    }
    Ok(())
}
