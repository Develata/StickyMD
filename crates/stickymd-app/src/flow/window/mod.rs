//! Platform-independent window, docking, timing, and lifecycle coordination.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

pub mod geometry;
mod lifecycle;
mod placement;
pub mod reducer;
#[cfg(test)]
mod show_receipt_tests;
pub mod state;
mod topmost;
