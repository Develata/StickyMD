//! Native source-editor projection and painting.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

mod fonts;
mod geometry;
mod paint;
mod projection;
mod rendering;

pub use fonts::{FontSelection, ScriptClass, ScriptRun, segment_script_runs};
pub use projection::{
    EditorRect, PreeditVisual, SourceInitializationMilestone, SourceProjection,
    SourceProjectionError,
};
pub use rendering::SourceTheme;
