//! Windows note storage and the adapters injected into the shared I/O worker.
//!
//! plan_ref: docs/plan/05_document_persistence.md#atomic-save

mod adapter;
#[cfg(test)]
mod phase15_storage_conformance;
mod storage;
#[cfg(test)]
mod wiring_tests;

pub use adapter::{WinAssetStore, win_io_ports};
#[cfg(test)]
pub use adapter::{WinConfigStorage, WinExportSink, WinNoteStorage};
pub(crate) use storage::{MAX_NOTE_LOAD, inspect_note_state_with_retry};

/// The line ending of a note this host creates; a note read without line breaks, or with as many
/// of each, takes it too (plan 05).
pub const NEW_NOTE_LINE_ENDING: stickymd_core::LineEnding = stickymd_core::LineEnding::Crlf;
pub use storage::{
    NoteStorageError, inspect_note, persist_note, preserve_canonical_if_unchanged,
    quarantine_temporary, remove_temporary,
};
