//! Opt-in input-latency diagnostic for the VM driver (`stickymd-smoke mac-latency`). It is off
//! unless `STICKYMD_DIAGNOSTIC_INPUT_LATENCY` names an output file, and then records timestamps
//! only, never a key's identity or the note's text:
//!
//! - every key-down: the event's own timestamp, and when the main thread saw it (a local event
//!   monitor runs before AppKit dispatches the event);
//! - every presented frame: when `present` returned, and how long drawing it took.
//!
//! What happens after `present` (Core Animation's commit, the compositor, the display) is not
//! measured: a transaction completion block follows the animations' lifetime (the caret's blink
//! animation delays it), so there is no reliable per-frame commit or display receipt yet.
//!
//! The buffers are allocated once; a full buffer counts what it drops and never grows, so neither
//! the key path nor the frame path does I/O or allocates. SIGUSR1 writes the file on the main
//! thread, atomically; nothing is written at exit. Times are nanoseconds of `CLOCK_UPTIME_RAW`,
//! the `mach_absolute_time` clock event timestamps use too.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

use std::cell::RefCell;
use std::ffi::c_void;
use std::fmt::Write as _;
use std::io::Write as _;
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
    /// `(presented, drawn)` per frame; `drawn` is the drawing time in nanoseconds.
    frames: Vec<(u64, u64)>,
    dropped_keys: u64,
    dropped_frames: u64,
    /// Kept alive for the process: removing it would stop the recording.
    _monitor: Option<Retained<AnyObject>>,
}

thread_local! {
    /// Main thread only: the monitor, `after_present` and the signal handler all run there.
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

/// A frame reached the window, `draw` after its drawing started.
pub fn frame_presented(draw: std::time::Duration) {
    RECORDER.with(|cell| {
        if let Some(recorder) = cell.borrow_mut().as_mut() {
            if recorder.frames.len() < CAPACITY {
                let drawn = u64::try_from(draw.as_nanos()).unwrap_or(u64::MAX);
                recorder.frames.push((now_ns(), drawn));
            } else {
                recorder.dropped_frames += 1;
            }
        }
    });
}

fn write_file() {
    RECORDER.with(|cell| {
        let borrowed = cell.borrow();
        let Some(recorder) = borrowed.as_ref() else {
            return;
        };
        let mut text = String::with_capacity(48 * (recorder.keys.len() + recorder.frames.len()));
        let _ = writeln!(text, "stickymd-input-latency 3");
        let _ = writeln!(
            text,
            "dropped {} {}",
            recorder.dropped_keys, recorder.dropped_frames
        );
        for (event, seen) in &recorder.keys {
            let _ = writeln!(text, "key {event} {seen}");
        }
        for (presented, drawn) in &recorder.frames {
            let _ = writeln!(text, "frame {presented} {drawn}");
        }
        if let Err(error) = publish(&recorder.path, text.as_bytes()) {
            eprintln!("input latency diagnostic not written: {error}");
        }
    });
}

/// Writes a new, uniquely named file beside `path`, syncs it and renames it over `path`. The
/// temporary file is created exclusively, so no existing file (or link) is ever written into, and
/// only a file this call created is removed on failure.
fn publish(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("output path has no file name"))?;
    let mut temporary_name = name.to_os_string();
    temporary_name.push(format!(".{}.{}.tmp", std::process::id(), now_ns()));
    let temporary = path.with_file_name(temporary_name);
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    let written = file
        .write_all(bytes)
        .and_then(|()| file.sync_all())
        .and_then(|()| std::fs::rename(&temporary, path));
    if written.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    written
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
    fn dispatch_source_set_registration_handler_f(
        source: *mut c_void,
        handler: extern "C" fn(*mut c_void),
    );
    fn dispatch_resume(object: *mut c_void);
}

extern "C" fn on_flush(_context: *mut c_void) {
    write_file();
}

/// The source is registered: from now on SIGUSR1 reaches `on_flush`, so its default action
/// (terminate) can be ignored. A signal sent before this still ends the process, visibly.
extern "C" fn on_registered(_context: *mut c_void) {
    // SAFETY: sets this process's disposition for SIGUSR1; no other preconditions.
    if unsafe { libc::signal(libc::SIGUSR1, libc::SIG_IGN) } == libc::SIG_ERR {
        eprintln!(
            "input latency diagnostic: SIGUSR1 stays fatal: {}",
            std::io::Error::last_os_error()
        );
    }
}

/// SIGUSR1, delivered through a dispatch source on the main queue, writes the file.
fn install_flush_signal() -> Result<(), String> {
    // SAFETY: creates a signal source on the main queue with plain C handlers; the source lives
    // for the process.
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
        dispatch_source_set_registration_handler_f(source, on_registered);
        dispatch_resume(source);
    }
    Ok(())
}
