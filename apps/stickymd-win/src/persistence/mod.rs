//! Portable note persistence execution domain.
//!
//! plan_ref: docs/plan/05_document_persistence.md#atomic-save

mod storage;
mod worker;

#[cfg(test)]
pub use storage::MAX_NOTE_LOAD;
pub(crate) use storage::inspect_note_state_with_retry;
pub use storage::{
    NoteObservation, NoteStorageError, PersistMode, PersistRequest, PersistResult, inspect_note,
    persist_note, preserve_canonical, quarantine_temporary, remove_temporary,
};
pub use worker::{IoCompletion, PersistenceWorker, TemporaryCleanup};
