//! Named-event signal used only by opt-in startup diagnostics.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

use std::path::Path;

use thiserror::Error;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{EVENT_MODIFY_STATE, OpenEventW, SetEvent};
use windows::core::HSTRING;

#[derive(Debug, Error)]
pub enum DiagnosticEventError {
    #[error("cannot open readiness event: {0}")]
    Open(windows::core::Error),
    #[error("cannot signal readiness event: {0}")]
    Signal(windows::core::Error),
    #[error("cannot write diagnostic trace {}: {source}", path.display())]
    TraceWrite {
        path: Box<Path>,
        source: std::io::Error,
    },
}

pub fn signal_named_event(name: &str) -> Result<(), DiagnosticEventError> {
    let name = HSTRING::from(name);
    // SAFETY: `name` remains a valid NUL-terminated HSTRING for the call. The
    // returned handle is owned by this function and closed on every branch.
    let handle = unsafe { OpenEventW(EVENT_MODIFY_STATE, false, &name) }
        .map_err(DiagnosticEventError::Open)?;
    // SAFETY: `handle` is a live event handle opened with EVENT_MODIFY_STATE.
    let result = unsafe { SetEvent(handle) }.map_err(DiagnosticEventError::Signal);
    close_handle(handle);
    result
}

fn close_handle(handle: HANDLE) {
    // SAFETY: the handle is owned by this adapter and is closed exactly once.
    let _ = unsafe { CloseHandle(handle) };
}

pub fn write_startup_trace(path: &Path, bytes: &[u8]) -> Result<(), DiagnosticEventError> {
    std::fs::write(path, bytes).map_err(|source| DiagnosticEventError::TraceWrite {
        path: path.into(),
        source,
    })
}
