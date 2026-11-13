//! macOS platform adapters. Native calls and `unsafe` live only below this directory and in
//! `stickymd-posix`, whose POSIX modules are re-exported here under their old names.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

pub mod app;
pub mod caret_layer;
pub mod clipboard;
pub mod dialog;
pub mod file_watch;
pub mod input_latency;
pub mod monitor;
pub mod pointer_band;
pub mod shell;
pub mod sigterm;
pub mod tray;
pub mod window;

pub use stickymd_posix::{atomic_file, file, instance};
