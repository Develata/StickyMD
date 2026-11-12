//! SIGTERM as a quit request. A logout asks StickyMD to quit without waiting for it, and launchd
//! sends SIGTERM to an instance it manages (launched by Finder or `open`) still running 1 to 3 s
//! later (report RISK-macos-logout-ui-element); at its default disposition the signal ends it,
//! whatever the quit barrier has left to save. A dispatch source on the main queue delivers the
//! signal instead, and once it is registered the disposition becomes "ignore". The main run loop
//! serves that queue in its common modes, a modal dialog's included, so the handler runs on the
//! main thread like any other event and can end the dialog.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use std::ffi::c_void;

unsafe extern "C" {
    /// `DISPATCH_SOURCE_TYPE_SIGNAL` is the address of this symbol.
    static _dispatch_source_type_signal: c_void;
    /// `dispatch_get_main_queue()` is the address of this symbol.
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

extern "C" fn on_sigterm(_context: *mut c_void) {
    super::app::quit_requested_by_signal();
}

/// Registration is asynchronous: only once the source listens may the default disposition go,
/// so no SIGTERM is ever ignored without being delivered. Until then it still ends the process.
extern "C" fn on_registered(_context: *mut c_void) {
    // SAFETY: plain disposition change for one signal; the handler value is `SIG_IGN`.
    if unsafe { libc::signal(libc::SIGTERM, libc::SIG_IGN) } == libc::SIG_ERR {
        eprintln!(
            "cannot ignore SIGTERM; a logout may end StickyMD before it saves: {}",
            std::io::Error::last_os_error()
        );
    }
}

static INSTALLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Called on the main thread after the event sink is in place; once it has succeeded, a later
/// call does nothing. The source is never released: it serves the process to the end.
pub fn install() -> Result<(), String> {
    if INSTALLED.load(std::sync::atomic::Ordering::SeqCst) {
        return Ok(());
    }
    // SAFETY: libdispatch calls with the documented signal source type and the main queue, both
    // static for the process; the handlers are plain functions without context. SIGTERM's
    // disposition changes only in the registration handler, after the source listens.
    unsafe {
        let source = dispatch_source_create(
            &raw const _dispatch_source_type_signal,
            libc::SIGTERM as usize,
            0,
            &raw const _dispatch_main_q,
        );
        if source.is_null() {
            return Err("dispatch_source_create failed for SIGTERM".into());
        }
        dispatch_source_set_event_handler_f(source, on_sigterm);
        dispatch_source_set_registration_handler_f(source, on_registered);
        dispatch_resume(source);
    }
    INSTALLED.store(true, std::sync::atomic::Ordering::SeqCst);
    Ok(())
}
