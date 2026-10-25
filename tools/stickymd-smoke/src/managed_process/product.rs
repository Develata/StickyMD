//! A process's product, read from its image's version resource rather than its file name.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

use std::ffi::c_void;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;

/// No version resource, or none with the queried string.
const ERROR_RESOURCE_DATA_NOT_FOUND: i32 = 1812;
const ERROR_RESOURCE_TYPE_NOT_FOUND: i32 = 1813;
/// The product name the StickyMD build embeds (`apps/stickymd-win/build.rs`); a renamed copy
/// keeps it.
const STICKYMD_PRODUCT: &str = "StickyMD";

#[link(name = "version")]
unsafe extern "system" {
    fn GetFileVersionInfoSizeW(path: *const u16, handle: *mut u32) -> u32;
    fn GetFileVersionInfoW(path: *const u16, handle: u32, length: u32, data: *mut c_void) -> i32;
    fn VerQueryValueW(
        block: *const c_void,
        sub_block: *const u16,
        buffer: *mut *mut c_void,
        length: *mut u32,
    ) -> i32;
}

/// What a process's image says it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Product {
    /// The version resource's ProductName.
    Named(String),
    /// The image has no version resource at all.
    Absent,
    /// The process or its image could not be read, so nothing is proven.
    Unreadable(String),
}

impl Product {
    /// Whether the process may be StickyMD: its ProductName says so, or it is unreadable.
    /// An image without a version resource is not a StickyMD build for Windows, which always
    /// embeds one. A present resource whose ProductName is not found is unreadable, not absent:
    /// a failed lookup cannot tell a missing key from an invalid resource.
    pub(crate) fn may_be_stickymd(&self) -> bool {
        match self {
            Self::Named(name) => name.trim().eq_ignore_ascii_case(STICKYMD_PRODUCT),
            Self::Absent => false,
            Self::Unreadable(_) => true,
        }
    }
}

/// The product of `pid`, from the version resource of its image file.
pub(crate) fn process_product(pid: u32) -> Product {
    match super::query_executable(pid) {
        Ok(Some(image)) => image_product(&image),
        Ok(None) => Product::Unreadable("the process exited".to_owned()),
        Err(error) => Product::Unreadable(error),
    }
}

fn image_product(image: &Path) -> Product {
    let path: Vec<u16> = image.as_os_str().encode_wide().chain([0]).collect();
    let mut ignored = 0;
    // SAFETY: `path` is a NUL-terminated UTF-16 buffer alive through the call; `ignored` is
    // writable DWORD storage. Neither pointer is retained.
    let size = unsafe { GetFileVersionInfoSizeW(path.as_ptr(), &raw mut ignored) };
    if size == 0 {
        let error = std::io::Error::last_os_error();
        return match error.raw_os_error() {
            Some(ERROR_RESOURCE_DATA_NOT_FOUND | ERROR_RESOURCE_TYPE_NOT_FOUND) => Product::Absent,
            _ => Product::Unreadable(format!(
                "cannot read the version resource of {}: {error}",
                image.display()
            )),
        };
    }
    let mut block = vec![0_u8; size as usize];
    // SAFETY: `block` is writable for `size` bytes, the length passed; `path` is as above.
    if unsafe { GetFileVersionInfoW(path.as_ptr(), 0, size, block.as_mut_ptr().cast()) } == 0 {
        return Product::Unreadable(format!(
            "cannot read the version resource of {}: {}",
            image.display(),
            std::io::Error::last_os_error()
        ));
    }
    match product_name(query(&block, "\\VarFileInfo\\Translation"), |key| {
        query(&block, key)
    }) {
        Ok(name) => Product::Named(name),
        Err(why) => Product::Unreadable(format!(
            "the version resource of {} names no product: {why}",
            image.display()
        )),
    }
}

/// The ProductName of a present version resource: `translations` is its
/// `\VarFileInfo\Translation` value and `lookup` reads a string by key. Anything short of a
/// found name (no or malformed translations, every lookup failing) is an error, never absence.
fn product_name<'a>(
    translations: Option<&[u8]>,
    lookup: impl Fn(&str) -> Option<&'a [u8]>,
) -> Result<String, &'static str> {
    let translations = translations.ok_or("no translation table")?;
    if translations.is_empty() || !translations.len().is_multiple_of(4) {
        return Err("a malformed translation table");
    }
    // Each translation is a little-endian (language, code page) pair of u16.
    for pair in translations.chunks_exact(4) {
        let language = u16::from_le_bytes([pair[0], pair[1]]);
        let code_page = u16::from_le_bytes([pair[2], pair[3]]);
        let key = format!("\\StringFileInfo\\{language:04x}{code_page:04x}\\ProductName");
        if let Some(bytes) = lookup(&key) {
            let units: Vec<u16> = bytes
                .chunks_exact(2)
                .map(|unit| u16::from_le_bytes([unit[0], unit[1]]))
                .take_while(|&unit| unit != 0)
                .collect();
            return Ok(String::from_utf16_lossy(&units));
        }
    }
    Err("no translation has a readable ProductName")
}

/// The bytes of `key` in a version-resource `block`; strings are UTF-16 code units.
fn query<'a>(block: &'a [u8], key: &str) -> Option<&'a [u8]> {
    let key: Vec<u16> = key.encode_utf16().chain([0]).collect();
    let mut value: *mut c_void = std::ptr::null_mut();
    let mut length = 0_u32;
    // SAFETY: `block` holds a version resource filled by GetFileVersionInfoW and outlives the
    // returned pointer, which points into it; `key` is NUL-terminated; both outputs are
    // writable.
    if unsafe {
        VerQueryValueW(
            block.as_ptr().cast(),
            key.as_ptr(),
            &raw mut value,
            &raw mut length,
        )
    } == 0
        || value.is_null()
    {
        return None;
    }
    let start = (value as usize).checked_sub(block.as_ptr() as usize)?;
    // Strings report their length in UTF-16 units, binary values in bytes.
    let bytes = if key.starts_with(&"\\StringFileInfo".encode_utf16().collect::<Vec<_>>()) {
        (length as usize).checked_mul(2)?
    } else {
        length as usize
    };
    block.get(start..start.checked_add(bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_stickymd_or_unreadable_product_may_be_stickymd() {
        assert!(Product::Named("StickyMD".into()).may_be_stickymd());
        assert!(Product::Named(" stickymd ".into()).may_be_stickymd());
        assert!(Product::Unreadable("denied".into()).may_be_stickymd());
        assert!(!Product::Named("Clash Verge".into()).may_be_stickymd());
        assert!(!Product::Absent.may_be_stickymd());
    }

    fn utf16(text: &str) -> Vec<u8> {
        text.encode_utf16()
            .chain([0])
            .flat_map(u16::to_le_bytes)
            .collect()
    }

    /// A present resource yields a name only when a translation's ProductName is found; every
    /// other outcome is an error, so it can never pass as "no version resource".
    #[test]
    fn a_present_resource_without_a_found_name_is_unreadable() {
        let english = [0x09, 0x04, 0xb0, 0x04];
        let name = utf16("StickyMD");
        let found =
            |key: &str| (key == "\\StringFileInfo\\040904b0\\ProductName").then_some(&name[..]);
        assert_eq!(
            product_name(Some(&english), found),
            Ok("StickyMD".to_owned())
        );
        // A ProductName exists but the translation table is missing or malformed.
        assert!(product_name(None, found).is_err());
        assert!(product_name(Some(&[]), found).is_err());
        assert!(product_name(Some(&english[..3]), found).is_err());
        // Translations exist but no lookup succeeds.
        assert!(product_name(Some(&english), |_| None).is_err());
        // A later translation can still carry the name.
        let two = [0x00, 0x00, 0xb0, 0x04, 0x09, 0x04, 0xb0, 0x04];
        assert_eq!(product_name(Some(&two), found), Ok("StickyMD".to_owned()));
    }

    /// A system image with a known version resource exercises the FFI end to end.
    #[test]
    fn a_system_image_names_its_product() {
        let root = std::env::var_os("SystemRoot").expect("Windows sets SystemRoot");
        let image = Path::new(&root).join("System32").join("kernel32.dll");
        match image_product(&image) {
            Product::Named(name) => assert!(name.contains("Windows"), "{name}"),
            other => panic!("{other:?}"),
        }
    }

    /// The running test binary is a real image: reading it exercises the FFI end to end.
    #[test]
    fn the_running_image_reads_without_error() {
        let product = image_product(&std::env::current_exe().unwrap());
        assert!(!matches!(product, Product::Unreadable(_)), "{product:?}");
    }
}
