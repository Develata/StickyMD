//! Allowlisted preview-link activation through the Windows shell.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use stickymd_render::preview::LinkKind;
use thiserror::Error;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
use windows::core::{PCWSTR, w};

#[derive(Debug, Error)]
pub enum ShellOpenError {
    #[error("preview destination uses a blocked URI scheme")]
    BlockedScheme,
    #[error("Windows Shell refused the preview destination (code {0})")]
    ShellRejected(isize),
}

pub fn open_target(
    destination: &str,
    kind: LinkKind,
    note_dir: &Path,
) -> Result<(), ShellOpenError> {
    let destination = destination.trim();
    if classify_target(destination) != kind {
        return Err(ShellOpenError::BlockedScheme);
    }
    let mut destination = match kind {
        LinkKind::Blocked => return Err(ShellOpenError::BlockedScheme),
        LinkKind::Relative => note_dir
            .join(destination)
            .as_os_str()
            .encode_wide()
            .collect::<Vec<_>>(),
        LinkKind::Http | LinkKind::Https | LinkKind::Mailto | LinkKind::File => {
            destination.encode_utf16().collect::<Vec<_>>()
        }
    };
    destination.push(0);
    // SAFETY: `destination` and the static `open` verb are valid, NUL-terminated
    // UTF-16 strings for the duration of the call. No handle ownership is
    // transferred; ShellExecuteW returns an integer-like status value.
    let status = unsafe {
        ShellExecuteW(
            None,
            w!("open"),
            PCWSTR(destination.as_ptr()),
            None,
            None,
            SW_SHOWNORMAL,
        )
    };
    let code = status.0 as isize;
    if code <= 32 {
        Err(ShellOpenError::ShellRejected(code))
    } else {
        Ok(())
    }
}

fn classify_target(destination: &str) -> LinkKind {
    if windows_absolute_path(destination) {
        return LinkKind::Relative;
    }
    let Some(colon) = destination.find(':') else {
        return LinkKind::Relative;
    };
    let scheme = &destination[..colon];
    if scheme.is_empty()
        || !scheme.as_bytes()[0].is_ascii_alphabetic()
        || !scheme
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    {
        return LinkKind::Relative;
    }
    if scheme.eq_ignore_ascii_case("http") {
        LinkKind::Http
    } else if scheme.eq_ignore_ascii_case("https") {
        LinkKind::Https
    } else if scheme.eq_ignore_ascii_case("mailto") {
        LinkKind::Mailto
    } else if scheme.eq_ignore_ascii_case("file") {
        LinkKind::File
    } else {
        LinkKind::Blocked
    }
}

fn windows_absolute_path(destination: &str) -> bool {
    let bytes = destination.as_bytes();
    bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocked_scheme_fails_before_calling_the_shell() {
        assert!(matches!(
            open_target(
                "javascript:alert(1)",
                LinkKind::Blocked,
                Path::new("C:\\note")
            ),
            Err(ShellOpenError::BlockedScheme)
        ));
    }

    #[test]
    fn forged_allowed_kind_cannot_bypass_adapter_reclassification() {
        assert!(matches!(
            open_target(
                "javascript:alert(1)",
                LinkKind::Https,
                Path::new("C:\\note")
            ),
            Err(ShellOpenError::BlockedScheme)
        ));
        assert_eq!(classify_target("C:\\note\\local.md"), LinkKind::Relative);
        assert_eq!(classify_target("HTTPS://example.com"), LinkKind::Https);
    }
}
