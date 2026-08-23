//! Named-event signal used only by opt-in startup diagnostics.
//!
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;
use windows::Win32::Foundation::{CloseHandle, HANDLE};
use windows::Win32::System::Threading::{EVENT_MODIFY_STATE, OpenEventW, SetEvent};
use windows::core::HSTRING;

use super::atomic_file::{AtomicPublishError, prepare_temporary_exclusive, publish_prepared_new};

#[derive(Debug, Error)]
pub enum DiagnosticEventError {
    #[error("cannot open readiness event: {0}")]
    Open(windows::core::Error),
    #[error("cannot signal readiness event: {0}")]
    Signal(windows::core::Error),
    #[error("diagnostic startup trace path has no file name: {0}")]
    InvalidTracePath(PathBuf),
    #[error("cannot atomically publish diagnostic startup trace {}: {source}", path.display())]
    TracePublish {
        path: Box<Path>,
        source: AtomicPublishError,
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
    let file_name = path
        .file_name()
        .ok_or_else(|| DiagnosticEventError::InvalidTracePath(path.to_path_buf()))?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    let mut temporary_name = OsString::from(".");
    temporary_name.push(file_name);
    temporary_name.push(format!(".{}-{nonce}.tmp", std::process::id()));
    let temporary = path.with_file_name(temporary_name);

    if let Err(source) = prepare_temporary_exclusive(path, &temporary, bytes) {
        if !matches!(source, AtomicPublishError::TempCreate(_)) {
            let _ = std::fs::remove_file(&temporary);
        }
        return Err(DiagnosticEventError::TracePublish {
            path: path.into(),
            source,
        });
    }
    if let Err(source) = publish_prepared_new(path, &temporary) {
        let _ = std::fs::remove_file(&temporary);
        return Err(DiagnosticEventError::TracePublish {
            path: path.into(),
            source,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::write_startup_trace;
    use crate::test_support::unique_temp_path;
    use std::fs::{self, OpenOptions};
    use std::io::Write as _;

    fn unique_trace_path(label: &str) -> std::path::PathBuf {
        unique_temp_path(label).with_extension("trace")
    }

    #[test]
    fn startup_trace_is_created_exclusively() {
        let path = unique_trace_path("create");
        write_startup_trace(&path, b"ready=1\n").expect("create diagnostic trace");
        assert_eq!(
            fs::read(&path).expect("read diagnostic trace"),
            b"ready=1\n"
        );
        fs::remove_file(path).expect("remove diagnostic trace");
    }

    #[test]
    fn startup_trace_never_overwrites_an_existing_file() {
        let path = unique_trace_path("preserve");
        let mut existing = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .expect("create protected target");
        existing
            .write_all(b"user-data")
            .expect("seed protected target");
        existing.sync_all().expect("flush protected target");
        drop(existing);

        assert!(write_startup_trace(&path, b"diagnostic-data").is_err());
        assert_eq!(
            fs::read(&path).expect("read protected target"),
            b"user-data"
        );
        fs::remove_file(path).expect("remove protected target");
    }
}
