//! Note persistence and the storage ports over it.
//!
//! plan_ref: docs/plan/05_document_persistence.md#macos-storage

mod adapter;
mod storage;
#[cfg(test)]
mod storage_conformance;

pub use adapter::{MacAssetStore, mac_io_ports};
pub use storage::{NoteFiles, NoteStorageError};

/// The line ending of a note this host creates; a note read without line breaks, or with as many
/// of each, takes it too (plan 05).
pub const NEW_NOTE_LINE_ENDING: stickymd_core::LineEnding = stickymd_core::LineEnding::Lf;
