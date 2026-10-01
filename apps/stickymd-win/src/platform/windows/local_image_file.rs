//! Read-only local image handles without network or reparse traversal.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

mod path;
#[cfg(test)]
mod tests;

use std::ffi::OsString;
use std::fs::File;
use std::io;
use std::os::windows::ffi::{OsStrExt, OsStringExt};
use std::os::windows::fs::MetadataExt;
use std::os::windows::io::FromRawHandle;
use std::path::Path;

use windows::Wdk::Foundation::OBJECT_ATTRIBUTES;
use windows::Wdk::Storage::FileSystem::{
    FILE_NON_DIRECTORY_FILE, FILE_OPEN, FILE_OPEN_NO_RECALL, FILE_SYNCHRONOUS_IO_NONALERT,
    NtCreateFile,
};
use windows::Win32::Foundation::{
    HANDLE, OBJ_CASE_INSENSITIVE, OBJ_DONT_REPARSE, RtlNtStatusToDosError, UNICODE_STRING,
};
use windows::Win32::Storage::FileSystem::{
    FILE_ATTRIBUTE_NORMAL, FILE_ATTRIBUTE_OFFLINE, FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS,
    FILE_ATTRIBUTE_RECALL_ON_OPEN, FILE_ATTRIBUTE_REPARSE_POINT, FILE_GENERIC_READ,
    FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE, QueryDosDeviceW,
};
use windows::Win32::System::IO::IO_STATUS_BLOCK;
use windows::core::{PCWSTR, PWSTR};

/// Resolve DOS aliases in memory, then open the observed local device directly.
/// OBJ_DONT_REPARSE rejects reparse points anywhere in the path in the same
/// kernel operation that obtains the handle. A preflight symlink_metadata walk
/// followed by File::open would permit replacement races and remote traversal.
pub(crate) fn open(path: &Path) -> io::Result<File> {
    let native = path::native_path(path, query_device)?;
    let mut name: Vec<u16> = native.encode_wide().collect();
    let byte_len = name
        .len()
        .checked_mul(2)
        .and_then(|n| u16::try_from(n).ok())
        .ok_or_else(|| denied("local image path is too long"))?;
    let name = UNICODE_STRING {
        Length: byte_len,
        MaximumLength: byte_len,
        Buffer: PWSTR(name.as_mut_ptr()),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: std::mem::size_of::<OBJECT_ATTRIBUTES>() as u32,
        ObjectName: &name,
        Attributes: OBJ_CASE_INSENSITIVE | OBJ_DONT_REPARSE,
        ..Default::default()
    };
    let mut handle = HANDLE::default();
    let mut status_block = IO_STATUS_BLOCK::default();
    // SAFETY: All counted UTF-16 buffers and output structures live through the
    // synchronous call. FILE_OPEN cannot create/overwrite; the validated native
    // device path and OBJ_DONT_REPARSE prohibit following a redirected target.
    let status = unsafe {
        NtCreateFile(
            &mut handle,
            FILE_GENERIC_READ,
            &attributes,
            &mut status_block,
            None,
            FILE_ATTRIBUTE_NORMAL,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            FILE_OPEN,
            FILE_NON_DIRECTORY_FILE | FILE_SYNCHRONOUS_IO_NONALERT | FILE_OPEN_NO_RECALL,
            None,
            0,
        )
    };
    if status.is_err() {
        // SAFETY: Conversion accepts the returned NTSTATUS value and borrows no pointers.
        return Err(io::Error::from_raw_os_error(
            unsafe { RtlNtStatusToDosError(status) } as i32,
        ));
    }
    // SAFETY: Successful synchronous NtCreateFile returns one owned file handle;
    // File assumes that ownership exactly once and closes it on every exit path.
    let file = unsafe { File::from_raw_handle(handle.0) };
    let metadata = file.metadata()?;
    let unavailable = FILE_ATTRIBUTE_REPARSE_POINT
        | FILE_ATTRIBUTE_OFFLINE
        | FILE_ATTRIBUTE_RECALL_ON_OPEN
        | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS;
    if !metadata.is_file() || metadata.file_attributes() & unavailable.0 != 0 {
        return Err(denied(
            "local image must be an ordinary, locally available file",
        ));
    }
    Ok(file)
}

fn query_device(drive: u8) -> io::Result<OsString> {
    let name = [u16::from(drive), u16::from(b':'), 0];
    let mut target = [0u16; 1024];
    // SAFETY: name is NUL-terminated, target is a live output buffer. This only
    // queries the DOS device namespace; it does not open the mapped destination.
    let count = unsafe { QueryDosDeviceW(PCWSTR(name.as_ptr()), Some(&mut target)) };
    if count == 0 {
        return Err(io::Error::last_os_error());
    }
    let end = target[..count as usize]
        .iter()
        .position(|unit| *unit == 0)
        .ok_or_else(|| denied("DOS device query returned no terminated target"))?;
    // The first string is the current mapping; later strings are old mappings.
    Ok(OsString::from_wide(&target[..end]))
}

fn denied(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}
