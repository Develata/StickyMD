//! The system pasteboard, read in the priority plan 08 fixes.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#macos-clipboard-and-assets

use std::path::PathBuf;

use objc2::runtime::{AnyClass, AnyObject, Bool};
use objc2::{class, msg_send};
use objc2_foundation::NSString;
use stickymd_app::flow::{ClipboardError, ClipboardPaste, ClipboardPort};

const MAX_CLIPBOARD_IMAGE_BYTES: usize = 64 * 1024 * 1024;
const TEXT_TYPE: &str = "public.utf8-plain-text";
/// In the order an image is taken; each is the original encoding.
const ENCODED_IMAGE_TYPES: [&str; 3] = ["public.png", "public.jpeg", "org.webmproject.webp"];
const TIFF_TYPE: &str = "public.tiff";
/// `NSBitmapImageFileTypePNG`.
const PNG_FILE_TYPE: usize = 4;

#[link(name = "AppKit", kind = "framework")]
unsafe extern "C" {
    static NSPasteboardURLReadingFileURLsOnlyKey: *const AnyObject;
}

#[derive(Default)]
pub struct MacClipboard;

fn unavailable(message: &str) -> ClipboardError {
    ClipboardError::Unavailable(message.to_owned())
}

fn general_pasteboard() -> *mut AnyObject {
    // SAFETY: a class method without preconditions, on the main thread.
    unsafe { msg_send![class!(NSPasteboard), generalPasteboard] }
}

fn string_from(value: *mut AnyObject) -> Option<String> {
    if value.is_null() {
        return None;
    }
    // SAFETY: callers pass an NSString the pasteboard returned.
    Some(unsafe { (*value.cast::<NSString>()).to_string() })
}

fn bytes_of(data: *mut AnyObject) -> Result<Vec<u8>, ClipboardError> {
    // SAFETY: `data` is a live NSData; `bytes` is valid for `length` while it lives.
    unsafe {
        let length: usize = msg_send![data, length];
        if length > MAX_CLIPBOARD_IMAGE_BYTES {
            return Err(unavailable("clipboard image exceeds the 64 MiB bound"));
        }
        let bytes: *const std::ffi::c_void = msg_send![data, bytes];
        Ok(if length == 0 || bytes.is_null() {
            Vec::new()
        } else {
            std::slice::from_raw_parts(bytes.cast::<u8>(), length).to_vec()
        })
    }
}

fn data_for(pasteboard: *mut AnyObject, kind: &str) -> *mut AnyObject {
    let kind = NSString::from_str(kind);
    // SAFETY: a live pasteboard and string, on the main thread.
    unsafe { msg_send![pasteboard, dataForType: &*kind] }
}

fn file_paths(pasteboard: *mut AnyObject) -> Vec<PathBuf> {
    // SAFETY: Foundation and AppKit calls with valid objects, on the main thread. The class is
    // passed as an object, which is what `NSArray` stores.
    unsafe {
        let url_class: *const AnyClass = class!(NSURL);
        let classes: *mut AnyObject =
            msg_send![class!(NSArray), arrayWithObject: url_class.cast::<AnyObject>()];
        let yes: *mut AnyObject = msg_send![class!(NSNumber), numberWithBool: Bool::YES];
        let options: *mut AnyObject = msg_send![
            class!(NSDictionary),
            dictionaryWithObject: yes,
            forKey: NSPasteboardURLReadingFileURLsOnlyKey
        ];
        let urls: *mut AnyObject =
            msg_send![pasteboard, readObjectsForClasses: classes, options: options];
        if urls.is_null() {
            return Vec::new();
        }
        let count: usize = msg_send![urls, count];
        (0..count)
            .filter_map(|index| {
                let url: *mut AnyObject = msg_send![urls, objectAtIndex: index];
                let path: *mut AnyObject = msg_send![url, path];
                string_from(path).map(PathBuf::from)
            })
            .collect()
    }
}

/// A TIFF has no decoder in the product, and no original encoding worth keeping: screenshots
/// and bitmaps arrive this way.
fn tiff_to_png(tiff: *mut AnyObject) -> Result<Vec<u8>, ClipboardError> {
    // SAFETY: AppKit calls with valid objects; the returned objects are autoreleased and used
    // before this function returns.
    unsafe {
        let rep: *mut AnyObject = msg_send![class!(NSBitmapImageRep), imageRepWithData: tiff];
        if rep.is_null() {
            return Err(unavailable("the clipboard image could not be decoded"));
        }
        let properties: *mut AnyObject = msg_send![class!(NSDictionary), dictionary];
        let png: *mut AnyObject =
            msg_send![rep, representationUsingType: PNG_FILE_TYPE, properties: properties];
        if png.is_null() {
            return Err(unavailable("the clipboard image could not be converted"));
        }
        bytes_of(png)
    }
}

/// What the priority chooses from the formats present, without touching their contents.
#[derive(Debug, PartialEq, Eq)]
enum Choice {
    Files,
    Encoded(&'static str),
    Tiff,
    Text,
}

fn choose(has_files: bool, present: impl Fn(&str) -> bool) -> Choice {
    if has_files {
        return Choice::Files;
    }
    if let Some(kind) = ENCODED_IMAGE_TYPES.iter().find(|kind| present(kind)) {
        return Choice::Encoded(kind);
    }
    if present(TIFF_TYPE) {
        return Choice::Tiff;
    }
    Choice::Text
}

/// Only images become assets; any other file pastes as its path, as on Windows.
fn classify_files(paths: Vec<PathBuf>) -> ClipboardPaste {
    let all_images = paths.iter().all(|path| {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                matches!(
                    extension.to_ascii_lowercase().as_str(),
                    "png" | "jpg" | "jpeg" | "webp" | "gif" | "bmp" | "ico"
                )
            })
    });
    if all_images {
        ClipboardPaste::Files(paths)
    } else {
        ClipboardPaste::Text(
            paths
                .iter()
                .map(|path| path.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\n"),
        )
    }
}

impl ClipboardPort for MacClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        Ok(text_from(general_pasteboard()))
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        let kind = NSString::from_str(TEXT_TYPE);
        let value = NSString::from_str(text);
        let pasteboard = general_pasteboard();
        // SAFETY: a live pasteboard and strings, on the main thread.
        let written: bool = unsafe {
            let _: isize = msg_send![pasteboard, clearContents];
            msg_send![pasteboard, setString: &*value, forType: &*kind]
        };
        if written {
            Ok(())
        } else {
            Err(unavailable("the pasteboard refused the text"))
        }
    }

    fn read_paste(&mut self) -> Result<Option<ClipboardPaste>, ClipboardError> {
        paste_from(general_pasteboard())
    }
}

fn paste_from(pasteboard: *mut AnyObject) -> Result<Option<ClipboardPaste>, ClipboardError> {
    let files = file_paths(pasteboard);
    let choice = choose(!files.is_empty(), |kind| {
        !data_for(pasteboard, kind).is_null()
    });
    Ok(Some(match choice {
        Choice::Files => classify_files(files),
        Choice::Encoded(kind) => {
            ClipboardPaste::EncodedImage(bytes_of(data_for(pasteboard, kind))?)
        }
        Choice::Tiff => ClipboardPaste::EncodedImage(tiff_to_png(data_for(pasteboard, TIFF_TYPE))?),
        Choice::Text => return Ok(text_from(pasteboard).map(ClipboardPaste::Text)),
    }))
}

fn text_from(pasteboard: *mut AnyObject) -> Option<String> {
    let kind = NSString::from_str(TEXT_TYPE);
    // SAFETY: a live pasteboard and string, on the main thread.
    let text: *mut AnyObject = unsafe { msg_send![pasteboard, stringForType: &*kind] };
    string_from(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A pasteboard of its own, so a test run leaves the user's clipboard alone.
    struct PrivatePasteboard(*mut AnyObject);

    impl PrivatePasteboard {
        fn new() -> Self {
            // SAFETY: class and instance methods without preconditions; the pasteboard is released
            // in `drop`.
            unsafe {
                let pasteboard: *mut AnyObject =
                    msg_send![class!(NSPasteboard), pasteboardWithUniqueName];
                let _: isize = msg_send![pasteboard, clearContents];
                Self(pasteboard)
            }
        }

        fn put_data(&self, kind: &str, bytes: &[u8]) {
            let kind = NSString::from_str(kind);
            // SAFETY: `bytes` is valid for its length and the data object copies it.
            unsafe {
                let data: *mut AnyObject = msg_send![
                    class!(NSData),
                    dataWithBytes: bytes.as_ptr().cast::<std::ffi::c_void>(),
                    length: bytes.len()
                ];
                let _: isize = msg_send![self.0, addTypes: &*array_of(&kind), owner: std::ptr::null_mut::<AnyObject>()];
                let written: bool = msg_send![self.0, setData: data, forType: &*kind];
                assert!(written);
            }
        }

        fn put_string(&self, kind: &str, value: &str) {
            let kind = NSString::from_str(kind);
            let value = NSString::from_str(value);
            // SAFETY: a live pasteboard and strings.
            unsafe {
                let _: isize = msg_send![self.0, addTypes: &*array_of(&kind), owner: std::ptr::null_mut::<AnyObject>()];
                let written: bool = msg_send![self.0, setString: &*value, forType: &*kind];
                assert!(written);
            }
        }
    }

    impl Drop for PrivatePasteboard {
        fn drop(&mut self) {
            // SAFETY: a pasteboard this test created and no longer uses.
            let _: () = unsafe { msg_send![self.0, releaseGlobally] };
        }
    }

    fn array_of(kind: &NSString) -> objc2::rc::Retained<objc2_foundation::NSArray<NSString>> {
        objc2_foundation::NSArray::from_slice(&[kind])
    }

    const PNG_BYTES: &[u8] = b"\x89PNG\r\n\x1a\nnot decoded on this path";

    /// A 2x1 uncompressed RGB TIFF: one red pixel, one blue pixel.
    fn small_tiff() -> Vec<u8> {
        const ENTRIES: u16 = 9;
        let pixels_at = 8 + 2 + u32::from(ENTRIES) * 12 + 4 + 6;
        let bits_at = 8 + 2 + u32::from(ENTRIES) * 12 + 4;
        let mut tiff = vec![b'I', b'I', 42, 0, 8, 0, 0, 0];
        tiff.extend_from_slice(&ENTRIES.to_le_bytes());
        let entries: [(u16, u16, u32, u32); ENTRIES as usize] = [
            (256, 4, 1, 2),
            (257, 4, 1, 1),
            (258, 3, 3, bits_at),
            (259, 3, 1, 1),
            (262, 3, 1, 2),
            (273, 4, 1, pixels_at),
            (277, 3, 1, 3),
            (278, 4, 1, 1),
            (279, 4, 1, 6),
        ];
        for (tag, kind, count, value) in entries {
            tiff.extend_from_slice(&tag.to_le_bytes());
            tiff.extend_from_slice(&kind.to_le_bytes());
            tiff.extend_from_slice(&count.to_le_bytes());
            tiff.extend_from_slice(&value.to_le_bytes());
        }
        tiff.extend_from_slice(&0u32.to_le_bytes());
        for _ in 0..3 {
            tiff.extend_from_slice(&8u16.to_le_bytes());
        }
        tiff.extend_from_slice(&[255, 0, 0, 0, 0, 255]);
        tiff
    }

    /// P16-C01.
    #[test]
    fn a_real_pasteboard_gives_files_before_image_and_text() {
        let pasteboard = PrivatePasteboard::new();
        pasteboard.put_string("public.file-url", "file:///tmp/picture.png");
        pasteboard.put_data("public.png", PNG_BYTES);
        pasteboard.put_string(TEXT_TYPE, "ignored");
        assert_eq!(
            paste_from(pasteboard.0).unwrap(),
            Some(ClipboardPaste::Files(vec![PathBuf::from(
                "/tmp/picture.png"
            )]))
        );
    }

    /// P16-C02.
    #[test]
    fn a_real_pasteboard_gives_the_original_encoding_before_tiff() {
        let pasteboard = PrivatePasteboard::new();
        pasteboard.put_data("public.png", PNG_BYTES);
        pasteboard.put_data(TIFF_TYPE, &small_tiff());
        pasteboard.put_string(TEXT_TYPE, "ignored");
        assert_eq!(
            paste_from(pasteboard.0).unwrap(),
            Some(ClipboardPaste::EncodedImage(PNG_BYTES.to_vec()))
        );
    }

    /// P16-C03.
    #[test]
    fn a_real_pasteboard_converts_tiff_to_png_keeping_its_pixels() {
        let pasteboard = PrivatePasteboard::new();
        pasteboard.put_data(TIFF_TYPE, &small_tiff());
        pasteboard.put_string(TEXT_TYPE, "ignored");
        let Some(ClipboardPaste::EncodedImage(png)) = paste_from(pasteboard.0).unwrap() else {
            panic!("expected an image");
        };
        let decoded = image::load_from_memory_with_format(&png, image::ImageFormat::Png)
            .unwrap()
            .to_rgb8();
        assert_eq!(decoded.dimensions(), (2, 1));
        assert_eq!(decoded.get_pixel(0, 0).0, [255, 0, 0]);
        assert_eq!(decoded.get_pixel(1, 0).0, [0, 0, 255]);
    }

    #[test]
    fn a_real_pasteboard_with_only_text_gives_the_text() {
        let pasteboard = PrivatePasteboard::new();
        pasteboard.put_string(TEXT_TYPE, "公式 $x$");
        assert_eq!(
            paste_from(pasteboard.0).unwrap(),
            Some(ClipboardPaste::Text("公式 $x$".to_owned()))
        );
    }

    fn present(types: &'static [&'static str]) -> impl Fn(&str) -> bool {
        move |kind| types.contains(&kind)
    }

    #[test]
    fn only_a_selection_of_images_pastes_as_files() {
        let images = vec![PathBuf::from("/a/x.PNG"), PathBuf::from("/a/y.webp")];
        assert!(matches!(
            classify_files(images),
            ClipboardPaste::Files(paths) if paths.len() == 2
        ));
        let mixed = vec![PathBuf::from("/a/x.png"), PathBuf::from("/a/report.pdf")];
        assert!(matches!(
            classify_files(mixed),
            ClipboardPaste::Text(text) if text == "/a/x.png\n/a/report.pdf"
        ));
    }

    #[test]
    fn files_win_over_every_image_and_text() {
        assert_eq!(
            choose(true, present(&["public.png", "public.tiff", TEXT_TYPE])),
            Choice::Files
        );
    }

    #[test]
    fn an_original_encoding_wins_over_tiff_and_text() {
        assert_eq!(
            choose(false, present(&["public.tiff", "public.png", TEXT_TYPE])),
            Choice::Encoded("public.png")
        );
        assert_eq!(
            choose(false, present(&["public.tiff", "public.jpeg"])),
            Choice::Encoded("public.jpeg")
        );
        assert_eq!(
            choose(false, present(&["public.jpeg", "public.png"])),
            Choice::Encoded("public.png")
        );
    }

    #[test]
    fn tiff_wins_over_text_and_text_is_last() {
        assert_eq!(
            choose(false, present(&["public.tiff", TEXT_TYPE])),
            Choice::Tiff
        );
        assert_eq!(choose(false, present(&[TEXT_TYPE])), Choice::Text);
        assert_eq!(choose(false, present(&[])), Choice::Text);
    }
}
