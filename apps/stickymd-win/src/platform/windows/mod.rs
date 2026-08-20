//! Thin Windows adapters used by the native shell and persistence execution domain.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

pub(crate) mod atomic_file;
mod clipboard;
pub(crate) mod file_identity;
pub(crate) mod file_watch;
pub(crate) mod message_box;
pub(crate) mod program_dir;
pub(crate) mod shell;
pub(crate) mod single_instance;

pub use clipboard::ArboardClipboard;
