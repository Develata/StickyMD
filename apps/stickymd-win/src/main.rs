//! StickyMD Windows application skeleton.
//!
//! plan_ref: docs/plan/09_windows_shell.md#purpose
//!
//! Phase 1 status: placeholder binary only. The real Interaction Shell and
//! the thin Windows adapters will live here in later phases:
//!
//! - `editor/` future source editor backends (cosmic-text first)
//! - `ui/` future shell controls (translation + presentation only)
//! - `workers/` future preview and I/O workers
//! - `platform/windows/` the ONLY allowed location for Win32 calls
//!
//! Business authority never lives in this crate: DocumentState (core) is
//! the runtime document authority; this shell only translates and presents.
#![deny(unsafe_op_in_unsafe_fn)]

fn main() {
    println!("StickyMD production shell skeleton (Phase 1 placeholder).");
    println!("No runtime feature is implemented yet. See docs/plan/.");
}
