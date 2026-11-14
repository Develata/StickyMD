//! Key decisions that need no window: the Esc order and the key table.
//!
//! plan_ref: docs/plan/09_windows_shell.md#dock-auto-hide
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

mod escape;
mod keymap;

pub use escape::{EscapeAction, EscapeContext, escape_action, is_escape};
pub use keymap::{ClipboardAlias, KeyCommand, Keymap, WindowKey, ZoomCommand};
