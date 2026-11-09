//! Note persistence and the storage ports over it.
//!
//! plan_ref: docs/plan/05_document_persistence.md#macos-storage

mod adapter;
mod storage;
#[cfg(test)]
mod storage_conformance;

pub use adapter::{MacAssetStore, mac_io_ports};
pub use storage::{NoteFiles, NoteStorageError};
