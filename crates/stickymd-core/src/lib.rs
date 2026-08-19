//! StickyMD platform-independent core.
//!
//! plan_ref: docs/plan/03_system_architecture.md#object-plane
//!
//! This crate holds the platform-independent document model: the canonical text
//! store, text deltas, generation semantics, cursor snapshots, undo/redo and the
//! runtime document authority (`DocumentState`). It must never depend on Windows
//! APIs, windowing, rendering or any UI crate, and it contains no `unsafe`.
//!
//! Layering rule: UI and background tasks only ever see immutable
//! [`DocumentSnapshot`]s; the only mutable authority is [`DocumentState`], reached
//! through typed intents (see `docs/plan/04`).
#![forbid(unsafe_code)]

pub mod cursor;
pub mod document;
pub mod error;
pub mod generation;
pub mod hash;
pub mod line_ending;
pub mod text_delta;
pub mod text_store;
pub mod undo;

pub use cursor::CursorSnapshot;
pub use document::{DocumentSnapshot, DocumentState};
pub use error::{EditError, PersistAckError, PersistError};
pub use generation::Generation;
pub use hash::Hash32;
pub use line_ending::LineEnding;
pub use text_delta::{InputKind, TextDelta};
pub use text_store::{StringTextStore, TextStore};
pub use undo::{UndoEntry, UndoManager};
