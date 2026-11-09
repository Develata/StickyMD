//! Allowlisted preview-link activation through the Windows shell.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use std::os::windows::ffi::OsStrExt;
use std::path::Path;

use stickymd_render::preview::{LinkKind, classify_link};
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
    if classify_link(destination) != kind {
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
        assert_eq!(classify_link("C:\\note\\local.md"), LinkKind::Relative);
        assert_eq!(classify_link("HTTPS://example.com"), LinkKind::Https);
    }
}
