//! StickyMD platform-independent core.
//!
//! plan_ref: docs/plan/03_system_architecture.md#object-plane
//!
//! This crate holds the platform-independent document model: the canonical text
//! store, text deltas, generation semantics, cursor snapshots, undo/redo and the
//! runtime document authority (`DocumentState`). It must never depend on Windows
//! APIs, windowing, rendering or any UI crate, and it contains no `unsafe`.
//!
//! Layering rule: the UI may receive a short-lived immutable borrow through its
//! coordinator, while workers and full projection resyncs receive immutable
//! [`DocumentSnapshot`]s. The only mutable authority is [`DocumentState`], reached
//! through typed intents (see `docs/plan/04`).
#![forbid(unsafe_code)]

mod assets;
mod document;
mod edit;
mod error;
mod generation;
mod hash;
mod line_ending;
mod persistence;
mod selection;
mod snapshot;
mod text_store;
mod undo;

pub use assets::{
    AssetEffect, MAX_MANAGED_ASSET_NAME_BYTES, ManagedAssetExtension, ManagedAssetLocation,
    ManagedAssetName, scan_managed_asset_references,
};
pub use document::DocumentState;
pub use edit::{EditKind, EditMeta, EditOutcome, EditRequest, RedoOutcome, TextDelta, UndoOutcome};
pub use error::DocumentError;
pub use generation::Generation;
pub use hash::Hash32;
pub use line_ending::LineEnding;
pub use persistence::{
    DiskFingerprint, ExternalFileFact, ExternalFileState, FileConflict, LoadedDocument,
    NoteDecodeError, RecoveryCandidate, RecoveryDisposition, RecoveryInspection, hash_bytes,
    inspect_recovery,
};
pub use selection::{CursorSnapshot, Selection, TextPosition};
pub use snapshot::DocumentSnapshot;
