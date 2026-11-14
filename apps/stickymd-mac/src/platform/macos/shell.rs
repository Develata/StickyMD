//! Allowlisted preview-link activation through the system.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use std::path::Path;

use objc2::runtime::AnyObject;
use objc2::{class, msg_send};
use objc2_foundation::NSString;
use stickymd_render::preview::{LinkKind, classify_link};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ShellOpenError {
    #[error("preview destination uses a blocked URI scheme")]
    BlockedScheme,
    #[error("the destination is not a valid URL")]
    InvalidUrl,
    #[error("the system refused to open the destination")]
    Rejected,
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
    let url: *mut AnyObject = match kind {
        LinkKind::Blocked => return Err(ShellOpenError::BlockedScheme),
        LinkKind::Relative => {
            let path = note_dir.join(destination);
            let path = NSString::from_str(&path.to_string_lossy());
            // SAFETY: a class method taking a live string, on the main thread.
            unsafe { msg_send![class!(NSURL), fileURLWithPath: &*path] }
        }
        LinkKind::Http | LinkKind::Https | LinkKind::Mailto | LinkKind::File => {
            let text = NSString::from_str(destination);
            // SAFETY: a class method taking a live string; it returns nil for an invalid URL.
            unsafe { msg_send![class!(NSURL), URLWithString: &*text] }
        }
    };
    if url.is_null() {
        return Err(ShellOpenError::InvalidUrl);
    }
    // SAFETY: AppKit calls with a live URL, on the main thread.
    let opened: bool = unsafe {
        let workspace: *mut AnyObject = msg_send![class!(NSWorkspace), sharedWorkspace];
        msg_send![workspace, openURL: url]
    };
    if opened {
        Ok(())
    } else {
        Err(ShellOpenError::Rejected)
    }
}
