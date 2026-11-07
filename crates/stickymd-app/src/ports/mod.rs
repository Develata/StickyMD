//! Capability Ports: what the application asks a Platform Host to execute and the receipts it
//! gets back. A host implements a port with its native adapter and reports facts; decisions stay
//! in `flow`. A port covers one whole safe operation, so no caller holds a path to act on.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

mod failure;
mod note;

pub use failure::{FailureKind, StorageFailure};
pub use note::{
    NoteReceipt, NoteStorage, PersistMode, PersistRequest, PersistResult, PreserveOutcome,
    TemporaryCleanup,
};
