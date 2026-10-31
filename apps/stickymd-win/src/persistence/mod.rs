//! Portable note persistence execution domain.
//!
//! plan_ref: docs/plan/05_document_persistence.md#atomic-save

mod storage;
mod worker;

pub(crate) use storage::{MAX_NOTE_LOAD, inspect_note_state_with_retry};
pub use storage::{
    NoteObservation, NoteStorageError, PersistMode, PersistRequest, PersistResult, PreserveOutcome,
    inspect_note, persist_note, preserve_canonical_if_unchanged, quarantine_temporary,
    remove_temporary,
};
pub(crate) use worker::AssetSyncRequest;
pub use worker::{IoCompletion, PersistenceWorker, TemporaryCleanup};
