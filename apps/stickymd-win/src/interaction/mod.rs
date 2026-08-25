//! Interaction-shell session state and event translation.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#ime-semantics

mod navigation;
mod search;
mod session;

pub use search::{SearchField, SearchSession};
pub use session::{EditorSession, ImeSignal};
