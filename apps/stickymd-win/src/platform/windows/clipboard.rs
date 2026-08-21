//! Windows clipboard adapter with explicit image-format priority.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#paste

use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

use windows::Win32::Foundation::HGLOBAL;
use windows::Win32::Graphics::Gdi::{BITMAP, GetObjectW, HGDIOBJ};
use windows::Win32::System::DataExchange::{
    CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    RegisterClipboardFormatW,
};
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{CF_BITMAP, CF_DIB, CF_DIBV5, CF_HDROP};
use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};
use windows::core::PCWSTR;

use crate::flow::{ClipboardError, ClipboardPaste, ClipboardPort};

const MAX_CLIPBOARD_IMAGE_BYTES: usize = 64 * 1024 * 1024;
const MAX_CLIPBOARD_FILES: u32 = 256;

pub struct ArboardClipboard {
    inner: Option<arboard::Clipboard>,
}

impl ArboardClipboard {
    pub const fn new() -> Self {
        Self { inner: None }
    }

    fn clipboard(&mut self) -> Result<&mut arboard::Clipboard, ClipboardError> {
        if self.inner.is_none() {
            self.inner = Some(
                arboard::Clipboard::new()
                    .map_err(|error| ClipboardError::Unavailable(error.to_string()))?,
            );
        }
        self.inner
            .as_mut()
            .ok_or_else(|| ClipboardError::Unavailable("initialization failed".to_owned()))
    }
}

impl Default for ArboardClipboard {
    fn default() -> Self {
        Self::new()
    }
}

impl ClipboardPort for ArboardClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        match self.clipboard()?.get_text() {
            Ok(text) => Ok(Some(text)),
            Err(arboard::Error::ContentNotAvailable) => Ok(None),
            Err(error) => Err(ClipboardError::Unavailable(error.to_string())),
        }
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.clipboard()?
            .set_text(text.to_owned())
            .map_err(|error| ClipboardError::Unavailable(error.to_string()))
    }

    fn read_paste(&mut self) -> Result<Option<ClipboardPaste>, ClipboardError> {
        let clipboard = ClipboardGuard::open()?;
        if format_available(u32::from(CF_HDROP.0)) {
            let files = read_file_drop()?;
            if !files.is_empty() {
                drop(clipboard);
                return Ok(Some(classify_file_drop(files)));
            }
        }
        for name in [
            "PNG",
            "image/png",
            "JFIF",
            "JPEG",
            "image/jpeg",
            "WebP",
            "image/webp",
        ] {
            let format = register_format(name);
            if format != 0 && format_available(format) {
                let bytes = read_global_bytes(format)?;
                drop(clipboard);
                return Ok(Some(ClipboardPaste::EncodedImage(bytes)));
            }
        }
        for format in [u32::from(CF_DIBV5.0), u32::from(CF_DIB.0)] {
            if format_available(format) {
                let bytes = read_dib_bytes(format)?;
                drop(clipboard);
                return Ok(Some(ClipboardPaste::Dib(bytes)));
            }
        }
        if format_available(u32::from(CF_BITMAP.0)) {
            validate_bitmap_dimensions()?;
            drop(clipboard);
            return self.read_bitmap().map(Some);
        }
        drop(clipboard);
        self.read_text().map(|text| text.map(ClipboardPaste::Text))
    }
}

impl ArboardClipboard {
    fn read_bitmap(&mut self) -> Result<ClipboardPaste, ClipboardError> {
        let image = self
            .clipboard()?
            .get_image()
            .map_err(|error| ClipboardError::Unavailable(error.to_string()))?;
        let width = u32::try_from(image.width)
            .map_err(|_| ClipboardError::Unavailable("bitmap width overflow".into()))?;
        let height = u32::try_from(image.height)
            .map_err(|_| ClipboardError::Unavailable("bitmap height overflow".into()))?;
        check_dimensions(width, height)?;
        let expected = usize::try_from(u64::from(width) * u64::from(height) * 4)
            .map_err(|_| ClipboardError::Unavailable("bitmap byte length overflow".into()))?;
        if image.bytes.len() != expected || expected > MAX_CLIPBOARD_IMAGE_BYTES {
            return Err(ClipboardError::Unavailable(
                "bitmap RGBA payload length is unsafe".into(),
            ));
        }
        Ok(ClipboardPaste::Rgba {
            width,
            height,
            bytes: image.bytes.into_owned(),
        })
    }
}

struct ClipboardGuard;

impl ClipboardGuard {
    fn open() -> Result<Self, ClipboardError> {
        let waits = [0, 10, 30, 80];
        let mut last_error = None;
        for wait in waits {
            if wait > 0 {
                thread::sleep(Duration::from_millis(wait));
            }
            // SAFETY: null owner is explicitly supported; this thread closes
            // every successful open through ClipboardGuard::drop.
            match unsafe { OpenClipboard(None) } {
                Ok(()) => return Ok(Self),
                Err(error) => last_error = Some(error),
            }
        }
        Err(ClipboardError::Unavailable(last_error.map_or_else(
            || "open failed".into(),
            |error| error.to_string(),
        )))
    }
}

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: this guard is created only after OpenClipboard succeeded on
        // the same thread and owns the matching close operation.
        let _ = unsafe { CloseClipboard() };
    }
}

fn format_available(format: u32) -> bool {
    // SAFETY: format is either a documented constant or a value returned by
    // RegisterClipboardFormatW.
    unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

fn register_format(name: &str) -> u32 {
    let mut wide = name.encode_utf16().collect::<Vec<_>>();
    wide.push(0);
    // SAFETY: wide is NUL terminated and remains alive for the call.
    unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) }
}

fn read_global_bytes(format: u32) -> Result<Vec<u8>, ClipboardError> {
    // SAFETY: clipboard is open and format availability was checked by caller.
    let handle = unsafe { GetClipboardData(format) }
        .map_err(|error| ClipboardError::Unavailable(error.to_string()))?;
    let global = HGLOBAL(handle.0);
    // SAFETY: handle belongs to the open clipboard and is not freed or retained.
    let size = unsafe { GlobalSize(global) };
    if size == 0 || size > MAX_CLIPBOARD_IMAGE_BYTES {
        return Err(ClipboardError::Unavailable(format!(
            "clipboard image payload size {size} is invalid"
        )));
    }
    // SAFETY: a clipboard HGLOBAL may be locked for read-only access; the
    // resulting pointer is used only for `size` bytes before GlobalUnlock.
    let pointer = unsafe { GlobalLock(global) };
    if pointer.is_null() {
        return Err(ClipboardError::Unavailable("GlobalLock failed".into()));
    }
    // SAFETY: GlobalSize established the accessible allocation length and the
    // clipboard remains open and locked during this copy.
    let bytes = unsafe { std::slice::from_raw_parts(pointer.cast::<u8>(), size) }.to_vec();
    // SAFETY: global was successfully locked above and is still valid.
    let _ = unsafe { GlobalUnlock(global) };
    Ok(bytes)
}

fn read_dib_bytes(format: u32) -> Result<Vec<u8>, ClipboardError> {
    let bytes = read_global_bytes(format)?;
    if bytes.len() < 40 {
        return Err(ClipboardError::Unavailable(
            "DIB header is truncated".into(),
        ));
    }
    let width = i32::from_le_bytes(bytes[4..8].try_into().unwrap_or([0; 4]));
    let height = i32::from_le_bytes(bytes[8..12].try_into().unwrap_or([0; 4]));
    let width = width.unsigned_abs();
    let height = height.unsigned_abs();
    check_dimensions(width, height)?;
    Ok(bytes)
}

fn validate_bitmap_dimensions() -> Result<(), ClipboardError> {
    // SAFETY: clipboard is open and CF_BITMAP availability was checked. The
    // returned bitmap remains clipboard-owned and is only queried synchronously.
    let handle = unsafe { GetClipboardData(u32::from(CF_BITMAP.0)) }
        .map_err(|error| ClipboardError::Unavailable(error.to_string()))?;
    let mut bitmap = BITMAP::default();
    // SAFETY: `bitmap` is a live, properly aligned BITMAP output buffer and the
    // clipboard-owned GDI handle remains valid while the clipboard is open.
    let copied = unsafe {
        GetObjectW(
            HGDIOBJ(handle.0),
            i32::try_from(std::mem::size_of::<BITMAP>()).unwrap_or(i32::MAX),
            Some((&raw mut bitmap).cast()),
        )
    };
    if copied != i32::try_from(std::mem::size_of::<BITMAP>()).unwrap_or(i32::MAX) {
        return Err(ClipboardError::Unavailable(
            "cannot inspect clipboard bitmap dimensions".into(),
        ));
    }
    check_dimensions(
        bitmap.bmWidth.unsigned_abs(),
        bitmap.bmHeight.unsigned_abs(),
    )
}

fn check_dimensions(width: u32, height: u32) -> Result<(), ClipboardError> {
    let pixels = u64::from(width).checked_mul(u64::from(height));
    let rgba_bytes = pixels.and_then(|count| count.checked_mul(4));
    if width == 0
        || height == 0
        || width > 16_384
        || height > 16_384
        || pixels.is_none_or(|count| count > 40_000_000)
        || rgba_bytes.is_none_or(|bytes| bytes > MAX_CLIPBOARD_IMAGE_BYTES as u64)
    {
        Err(ClipboardError::Unavailable(format!(
            "clipboard image dimension {width}x{height} is unsafe"
        )))
    } else {
        Ok(())
    }
}

fn classify_file_drop(paths: Vec<PathBuf>) -> ClipboardPaste {
    if paths.iter().all(|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "ico"
                )
            })
    }) {
        ClipboardPaste::Files(paths)
    } else {
        ClipboardPaste::Text(
            paths
                .iter()
                .map(|path| path.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\r\n"),
        )
    }
}

fn read_file_drop() -> Result<Vec<PathBuf>, ClipboardError> {
    // SAFETY: clipboard is open and CF_HDROP availability was checked.
    let handle = unsafe { GetClipboardData(u32::from(CF_HDROP.0)) }
        .map_err(|error| ClipboardError::Unavailable(error.to_string()))?;
    let drop = HDROP(handle.0);
    // SAFETY: HDROP remains owned by the open clipboard; 0xFFFF_FFFF queries count.
    let count = validate_file_count(unsafe { DragQueryFileW(drop, u32::MAX, None) })?;
    let mut paths = Vec::with_capacity(count as usize);
    for index in 0..count {
        // SAFETY: first call queries the UTF-16 length for this valid drop index.
        let length = unsafe { DragQueryFileW(drop, index, None) };
        if length == 0 {
            continue;
        }
        let mut wide = vec![0u16; length as usize + 1];
        // SAFETY: buffer includes room for the terminating NUL and HDROP is valid.
        let copied = unsafe { DragQueryFileW(drop, index, Some(&mut wide)) };
        if copied > 0 {
            paths.push(PathBuf::from(OsString::from_wide(&wide[..copied as usize])));
        }
    }
    Ok(paths)
}

fn validate_file_count(count: u32) -> Result<u32, ClipboardError> {
    if count > MAX_CLIPBOARD_FILES {
        Err(ClipboardError::Unavailable(format!(
            "clipboard file list contains {count} entries; limit is {MAX_CLIPBOARD_FILES}"
        )))
    } else {
        Ok(count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn oversized_file_drop_is_rejected_instead_of_partially_pasted() {
        assert_eq!(validate_file_count(MAX_CLIPBOARD_FILES).unwrap(), 256);
        assert!(validate_file_count(MAX_CLIPBOARD_FILES + 1).is_err());
    }

    #[test]
    fn mixed_file_drop_falls_back_to_path_text() {
        let mixed = classify_file_drop(vec![PathBuf::from("a.png"), PathBuf::from("b.txt")]);
        assert_eq!(mixed, ClipboardPaste::Text("a.png\r\nb.txt".to_owned()));
        assert!(matches!(
            classify_file_drop(vec![PathBuf::from("a.PNG"), PathBuf::from("b.jpeg")]),
            ClipboardPaste::Files(_)
        ));
    }

    #[test]
    fn bitmap_dimensions_are_rejected_before_large_rgba_capture() {
        assert!(check_dimensions(4096, 4096).is_ok());
        assert!(check_dimensions(5000, 5000).is_err());
    }
}
