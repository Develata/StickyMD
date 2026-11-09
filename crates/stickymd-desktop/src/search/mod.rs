//! The Search overlay: its layout, hit testing and painting.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-find-replace

mod layout_paint;

pub use layout_paint::{
    SearchHit, SearchLayout, SearchRect, paint_search_overlay, search_field_display,
};
