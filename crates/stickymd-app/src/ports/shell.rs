//! What the shell asks of the desktop around the window. These run on the main thread, so none of
//! them is `Send` (plan 03 rule 6).
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use std::path::PathBuf;

use stickymd_render::preview::LinkKind;

use super::HostFailure;

/// What the tray shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TrayView {
    pub window_visible: bool,
    pub always_on_top: bool,
}

/// What the tray asks for: exactly its three menu items.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayCommand {
    ToggleVisibility,
    ToggleAlwaysOnTop,
    Quit,
}

/// Tells the user why StickyMD stops or cannot be reached; works with or without a window.
pub trait FatalNotice {
    fn show_error(&self, title: &str, body: &str);
}

/// Opens a Preview link with the desktop's handler for it.
pub trait ShellLauncher {
    /// `kind` is what the Preview classified. The host checks it against the destination itself
    /// and refuses a mismatch; a relative destination is resolved against the note directory.
    fn open_target(&mut self, target: &str, kind: LinkKind) -> Result<(), HostFailure>;
}

/// The save dialog of an export: modal, it blocks the calling thread until the user answers.
pub trait ExportTargetPicker {
    /// `Ok(None)` is a cancelled dialog.
    fn choose_markdown_export(&mut self) -> Result<Option<PathBuf>, HostFailure>;
}
