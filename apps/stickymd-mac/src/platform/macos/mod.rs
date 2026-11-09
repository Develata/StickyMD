//! macOS platform adapters. Native calls and `unsafe` live only below this directory.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

pub mod app;
pub mod atomic_file;
pub mod caret_layer;
pub mod clipboard;
pub mod dialog;
pub mod file;
pub mod file_watch;
pub mod instance;
pub mod monitor;
pub mod pointer_band;
pub mod shell;
pub mod tray;
pub mod window;
