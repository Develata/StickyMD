//! Thin Windows adapters used by the native shell and persistence execution domain.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

pub(crate) mod atomic_file;
pub(crate) mod caret_overlay;
mod clipboard;
pub(crate) mod diagnostic_event;
pub(crate) mod export_dialog;
pub(crate) mod file_identity;
pub(crate) mod file_watch;
pub(crate) mod managed_file;
pub(crate) mod message_box;
pub(crate) mod monitor;
pub(crate) mod native_message;
pub(crate) mod program_dir;
pub(crate) mod shell;
pub(crate) mod single_instance;
pub(crate) mod tray;
pub(crate) mod window_opacity;
pub(crate) mod window_topmost;

pub use clipboard::ArboardClipboard;
