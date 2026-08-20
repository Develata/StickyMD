//! Stable identity and change facts for an already-open Windows file.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-file-watch

use std::os::windows::io::AsRawHandle;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::Storage::FileSystem::{BY_HANDLE_FILE_INFORMATION, GetFileInformationByHandle};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenFileObservation {
    volume_serial: u32,
    file_index: u64,
    file_size: u64,
    last_write: u64,
}

pub fn observe_open_file(file: &std::fs::File) -> std::io::Result<OpenFileObservation> {
    let mut information = BY_HANDLE_FILE_INFORMATION::default();
    let handle = HANDLE(file.as_raw_handle());
    // SAFETY: `handle` is borrowed from a live `File`; `information` is valid,
    // writable storage for the duration of the synchronous Win32 call. The
    // function neither takes ownership of the handle nor retains the pointer.
    unsafe { GetFileInformationByHandle(handle, &mut information) }
        .map_err(std::io::Error::other)?;
    Ok(OpenFileObservation {
        volume_serial: information.dwVolumeSerialNumber,
        file_index: u64::from(information.nFileIndexHigh) << 32
            | u64::from(information.nFileIndexLow),
        file_size: u64::from(information.nFileSizeHigh) << 32 | u64::from(information.nFileSizeLow),
        last_write: u64::from(information.ftLastWriteTime.dwHighDateTime) << 32
            | u64::from(information.ftLastWriteTime.dwLowDateTime),
    })
}
