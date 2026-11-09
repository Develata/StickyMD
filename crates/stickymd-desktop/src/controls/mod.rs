//! The fixed toolbar: its layout, the opacity popup's state and their painting.
//!
//! plan_ref: docs/plan/09_windows_shell.md#window-geometry

mod layout;
mod state;
mod toolbar_paint;

pub use layout::{
    ControlHit, ControlId, ControlLayout, ControlRect, PopupHit, resize_direction, split_sync_rect,
};
pub use state::{ControlState, OpacityField};
pub use toolbar_paint::{ToolbarVisual, paint_toolbar};

#[cfg(test)]
mod tests;
