//! Thin Win32 persistence adapter for the Phase 1E spike.
//!
//! plan_ref: docs/plan/05_document_persistence.md#atomic-save
//!
//! This module is the ONLY place in this spike that touches Win32 directly.
//! Portable file operations (create dir / write bytes / delete) go through
//! `std::fs`; only the platform-specific primitives live here:
//!
//! - `GetFinalPathNameByHandleW`: resolve junction/symlink/reparse to a canonical
//!   NT path (needed for the directory-identity hash).
//! - `CreateMutexW` / named event: single-instance ownership + activate signal.
//! - `FlushFileBuffers`: durable write before atomic replace.
//! - `ReplaceFileW` (+ `MoveFileExW` fallback): atomic replacement of note.md.
//!
//! Production adapter design (std-only vs. `windows` crate vs. third-party) is
//! decided after Phase 1 review.
#![allow(clippy::missing_safety_doc)]

use std::io;
use std::os::windows::io::AsRawHandle;
use std::path::Path;

use windows::Win32::Foundation::{HANDLE, ERROR_ALREADY_EXISTS};
use windows::Win32::Storage::FileSystem::{
    CreateFileW, FlushFileBuffers, GetFinalPathNameByHandleW, MoveFileExW, ReplaceFileW,
    FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH, OPEN_EXISTING,
};
use windows::Win32::System::Threading::{
    CreateEventW, CreateMutexW, OpenEventW, SetEvent, WaitForSingleObject,
};
use windows::core::PCWSTR;

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn from_wide(v: &[u16]) -> String {
    let len = v.iter().position(|&c| c == 0).unwrap_or(v.len());
    String::from_utf16_lossy(&v[..len])
}

/// RAII guard that closes a Win32 HANDLE on drop.
pub struct OwnedHandle(HANDLE);
impl OwnedHandle {
    pub fn raw(&self) -> HANDLE {
        self.0
    }
}
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        // SAFETY: we own the handle and close it exactly once.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}

/// Resolve a directory to its canonical NT path (`\\?\C:\...`), expanding
/// junctions / symlinks / reparse points. This is the stable identity input.
pub fn canonical_dir(dir: &Path) -> io::Result<String> {
    let dirw = wide(&dir.to_string_lossy());
    // SAFETY: we open the directory read-only with backup semantics (required to
    // open a directory), query its final path, then close the handle.
    unsafe {
        let h = CreateFileW(
            PCWSTR(dirw.as_ptr()),
            0u32,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
        .map_err(|e| io::Error::other(format!("CreateFileW(dir): {e}")))?;
        let h = OwnedHandle(h);

        use windows::Win32::Storage::FileSystem::GETFINALPATHNAMEBYHANDLE_FLAGS;
        const VOLUME_NAME_DOS: GETFINALPATHNAMEBYHANDLE_FLAGS =
            GETFINALPATHNAMEBYHANDLE_FLAGS(0);
        // First call with an empty buffer returns the required length (incl. NUL).
        let need = GetFinalPathNameByHandleW(h.raw(), &mut [], VOLUME_NAME_DOS);
        if need == 0 {
            return Err(io::Error::last_os_error());
        }
        let mut buf = vec![0u16; need as usize];
        let written = GetFinalPathNameByHandleW(h.raw(), &mut buf, VOLUME_NAME_DOS);
        if written == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(from_wide(&buf))
    }
}

/// Attempt to acquire the single-instance named mutex.
///
/// - `Ok(guard)` → this process is the FIRST instance for this directory identity;
///   hold the guard for the process lifetime.
/// - `Err(())` → a first instance already holds the mutex; caller is the SECOND.
pub fn try_acquire_instance(name: &str) -> Result<OwnedHandle, ()> {
    let nw = wide(name);
    // SAFETY: CreateMutexW with a unique local name; we inspect GetLastError to
    // distinguish "created" from "already existed".
    unsafe {
        let h = CreateMutexW(None, false, PCWSTR(nw.as_ptr())).map_err(|_| ())?;
        if windows::Win32::Foundation::GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = windows::Win32::Foundation::CloseHandle(h);
            return Err(());
        }
        Ok(OwnedHandle(h))
    }
}

/// Create the named "activate" event that the first instance listens on.
pub fn create_activate_event(name: &str) -> io::Result<OwnedHandle> {
    let nw = wide(name);
    // SAFETY: auto-reset event, initially non-signaled, unique local name.
    unsafe {
        let h = CreateEventW(None, false, false, PCWSTR(nw.as_ptr()))
            .map_err(|e| io::Error::other(format!("CreateEventW: {e}")))?;
        Ok(OwnedHandle(h))
    }
}

/// Second instance: signal the first instance's activate event, if it exists.
pub fn signal_activate_event(name: &str) -> io::Result<()> {
    let nw = wide(name);
    // SAFETY: open an existing event for modify-state and set it; close after.
    unsafe {
        let h = OpenEventW(
            windows::Win32::System::Threading::EVENT_MODIFY_STATE,
            false,
            PCWSTR(nw.as_ptr()),
        )
        .map_err(|e| io::Error::new(io::ErrorKind::NotFound, format!("OpenEventW: {e}")))?;
        let h = OwnedHandle(h);
        SetEvent(h.raw()).map_err(|e| io::Error::other(format!("SetEvent: {e}")))
    }
}

/// First instance: wait (bounded) for the activate event. Returns true if signaled.
pub fn wait_activate_event(ev: &OwnedHandle, timeout_ms: u32) -> bool {
    // SAFETY: waiting on a valid event handle we own.
    unsafe { WaitForSingleObject(ev.raw(), timeout_ms) == windows::Win32::Foundation::WAIT_OBJECT_0 }
}

/// Call `FlushFileBuffers` on an already-open `std::fs::File` to force durability.
pub fn flush_file_buffers(file: &std::fs::File) -> io::Result<()> {
    let raw = file.as_raw_handle();
    // SAFETY: the handle is valid for the lifetime of `file`.
    unsafe {
        FlushFileBuffers(HANDLE(raw)).map_err(|e| {
            io::Error::other(format!("FlushFileBuffers: {e}"))
        })
    }
}

/// Atomically replace `target` with the already-written `temp` file.
///
/// Tries `ReplaceFileW` first; on failure falls back to
/// `MoveFileExW(REPLACE_EXISTING | WRITE_THROUGH)` (plan 05 step 6).
pub fn atomic_replace(target: &Path, temp: &Path) -> io::Result<()> {
    let tw = wide(&target.to_string_lossy());
    let mw = wide(&temp.to_string_lossy());
    // SAFETY: both paths are NUL-terminated wide strings; the temp file exists
    // and is fully flushed before this call.
    unsafe {
        let replaced = ReplaceFileW(
            PCWSTR(tw.as_ptr()),
            PCWSTR(mw.as_ptr()),
            PCWSTR::null(),
            windows::Win32::Storage::FileSystem::REPLACE_FILE_FLAGS(0),
            None,
            None,
        );
        if replaced.is_ok() {
            return Ok(());
        }
        let err = replaced.err().unwrap();
        // Fallback path.
        MoveFileExW(
            PCWSTR(mw.as_ptr()),
            PCWSTR(tw.as_ptr()),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
        .map_err(|e| {
            io::Error::other(
                format!("ReplaceFileW failed ({err}); MoveFileExW also failed: {e}"),
            )
        })
    }
}
