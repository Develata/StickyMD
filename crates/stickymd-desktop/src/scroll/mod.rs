//! Pane scrolling: the scrollbars' geometry, paint and gesture state.
//!
//! plan_ref: docs/plan/09_windows_shell.md#vertical-scrollbars

mod scrollbar;

pub use scrollbar::{
    DragUpdate, PAGE_SHARE, PaneScroll, PointerOutcome, PressOutcome, PreviewFacts, ScrollPane,
    Scrollbar, ScrollbarDrag, ScrollbarInteraction, ScrollbarPress, gutter_at, page_pixels,
    pane_bars, preview_offset_at, preview_offset_by, preview_scroll, view_bars,
};
