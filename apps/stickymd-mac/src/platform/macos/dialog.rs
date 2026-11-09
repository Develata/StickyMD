//! Modal system dialogs: the export destination and fatal notices.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use std::path::PathBuf;

use objc2::rc::Retained;
use objc2::runtime::{AnyObject, Bool};
use objc2::{class, msg_send};
use objc2_foundation::{NSArray, NSString};

use super::app::shared_application;

/// `NSModalResponseOK`.
const MODAL_RESPONSE_OK: isize = 1;

// Linking the framework is what makes `UTType` available to `class!`.
#[link(name = "UniformTypeIdentifiers", kind = "framework")]
unsafe extern "C" {}

/// A menu-less background app is not in front when it opens a dialog; the dialog must be.
fn activate_for_dialog() {
    let app = shared_application();
    // SAFETY: the shared application on the main thread.
    let _: () = unsafe { msg_send![app, activateIgnoringOtherApps: Bool::YES] };
}

/// `None` is a cancelled dialog. It blocks the main thread until the user answers.
pub fn choose_markdown_export() -> Option<PathBuf> {
    activate_for_dialog();
    // SAFETY: AppKit calls on the main thread, with objects that outlive each call.
    unsafe {
        let panel: Retained<AnyObject> = msg_send![class!(NSSavePanel), savePanel];
        let name = NSString::from_str("note.md");
        let _: () = msg_send![&*panel, setNameFieldStringValue: &*name];
        let extension = NSString::from_str("md");
        let markdown: *mut AnyObject =
            msg_send![class!(UTType), typeWithFilenameExtension: &*extension];
        if !markdown.is_null() {
            let types: Retained<NSArray<AnyObject>> =
                msg_send![class!(NSArray), arrayWithObject: markdown];
            let _: () = msg_send![&*panel, setAllowedContentTypes: &*types];
        }
        let response: isize = msg_send![&*panel, runModal];
        if response != MODAL_RESPONSE_OK {
            return None;
        }
        let url: *mut AnyObject = msg_send![&*panel, URL];
        if url.is_null() {
            return None;
        }
        let path: *mut AnyObject = msg_send![url, path];
        (!path.is_null()).then(|| PathBuf::from((*path.cast::<NSString>()).to_string()))
    }
}

pub fn show_error(title: &str, body: &str) {
    activate_for_dialog();
    // SAFETY: AppKit calls on the main thread, with objects that outlive each call.
    unsafe {
        let alert: Retained<AnyObject> = msg_send![class!(NSAlert), new];
        let title = NSString::from_str(title);
        let body = NSString::from_str(body);
        let _: () = msg_send![&*alert, setMessageText: &*title];
        let _: () = msg_send![&*alert, setInformativeText: &*body];
        let _: isize = msg_send![&*alert, runModal];
    }
}
