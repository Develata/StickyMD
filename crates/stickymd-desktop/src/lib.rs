//! StickyMD Shared Desktop.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates
//!
//! This crate holds the shared part of the Interaction Shell: view geometry, the title projection,
//! the software frame path, the toolbar controls, the scrollbars, the Search overlay, the key
//! decisions, the pointer's place in the view, the `Host` a Platform Host implements and the shell
//! that runs the window over it (`shell::DesktopShell`). It may use
//! winit and softbuffer, but no native platform API, and it contains no `unsafe`; a Platform Host
//! supplies the native facts. Saving, conflicts, asset collection and lifecycle rules stay in
//! `stickymd-app`.
#![forbid(unsafe_code)]

pub mod controls;
pub mod frame;
pub mod geometry;
pub mod host;
pub mod input;
pub mod preview;
pub mod scroll;
pub mod search;
pub mod shell;
#[cfg(any(test, feature = "testing"))]
pub mod testing;
pub mod title;
