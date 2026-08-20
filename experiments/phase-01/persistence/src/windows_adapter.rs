//! Thin Windows adapter for the Phase 1 persistence spike.
//!
//! plan_ref: docs/plan/05_document_persistence.md#atomic-save
#![deny(unsafe_op_in_unsafe_fn)]

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, HANDLE, WAIT_OBJECT_0};
use windows::Win32::Storage::FileSystem::{
    MOVEFILE_WRITE_THROUGH, MoveFileExW, REPLACE_FILE_FLAGS, ReplaceFileW,
};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, EVENT_MODIFY_STATE, OpenEventW, SetEvent, WaitForSingleObject,
};
use windows::core::PCWSTR;

fn wide(value: &OsStr) -> Vec<u16> {
    value.encode_wide().chain(std::iter::once(0)).collect()
}

pub struct OwnedHandle(HANDLE);

impl OwnedHandle {
    fn raw(&self) -> HANDLE {
        self.0
    }
}

impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns the valid handle and closes it exactly once.
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Replaces an existing target with `ReplaceFileW`; first creation uses a non-replacing,
/// write-through move. Unknown replacement failures are returned, never hidden by an
/// unconditional fallback.
pub fn replace_flushed_temp(target: &Path, temp: &Path) -> io::Result<()> {
    let target_w = wide(target.as_os_str());
    let temp_w = wide(temp.as_os_str());
    // SAFETY: both buffers are NUL-terminated and live for the duration of the call;
    // the caller has closed the fully flushed temp file and both paths share a directory.
    unsafe {
        if target.exists() {
            ReplaceFileW(
                PCWSTR(target_w.as_ptr()),
                PCWSTR(temp_w.as_ptr()),
                PCWSTR::null(),
                REPLACE_FILE_FLAGS(0),
                None,
                None,
            )
            .map_err(|error| io::Error::other(format!("ReplaceFileW: {error}")))
        } else {
            MoveFileExW(
                PCWSTR(temp_w.as_ptr()),
                PCWSTR(target_w.as_ptr()),
                MOVEFILE_WRITE_THROUGH,
            )
            .map_err(|error| io::Error::other(format!("MoveFileExW(first create): {error}")))
        }
    }
}

pub fn canonical_directory(path: &Path) -> io::Result<String> {
    std::fs::canonicalize(path).map(|path| path.to_string_lossy().into_owned())
}

pub fn acquire_first_instance(name: &str) -> io::Result<Option<OwnedHandle>> {
    let name_w = wide(OsStr::new(name));
    // SAFETY: the name is NUL-terminated; the returned handle is immediately wrapped or closed.
    unsafe {
        let handle = CreateMutexW(None, false, PCWSTR(name_w.as_ptr()))
            .map_err(|error| io::Error::other(format!("CreateMutexW: {error}")))?;
        if windows::Win32::Foundation::GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            Ok(None)
        } else {
            Ok(Some(OwnedHandle(handle)))
        }
    }
}

pub fn create_activation_event(name: &str) -> io::Result<OwnedHandle> {
    let name_w = wide(OsStr::new(name));
    // SAFETY: the name is NUL-terminated and the returned handle is owned by the wrapper.
    unsafe {
        CreateEventW(None, false, false, PCWSTR(name_w.as_ptr()))
            .map(OwnedHandle)
            .map_err(|error| io::Error::other(format!("CreateEventW: {error}")))
    }
}

pub fn signal_activation_event(name: &str) -> io::Result<()> {
    let name_w = wide(OsStr::new(name));
    // SAFETY: the name is NUL-terminated; the opened handle is wrapped and closed exactly once.
    unsafe {
        let event = OpenEventW(EVENT_MODIFY_STATE, false, PCWSTR(name_w.as_ptr()))
            .map(OwnedHandle)
            .map_err(|error| io::Error::other(format!("OpenEventW: {error}")))?;
        SetEvent(event.raw()).map_err(|error| io::Error::other(format!("SetEvent: {error}")))
    }
}

pub fn wait_for_activation(event: &OwnedHandle, timeout_ms: u32) -> bool {
    // SAFETY: `event` owns a valid event handle for the duration of the wait.
    unsafe { WaitForSingleObject(event.raw(), timeout_ms) == WAIT_OBJECT_0 }
}
