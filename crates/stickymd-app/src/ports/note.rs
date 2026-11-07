//! The note storage port and its receipts.
//!
//! plan_ref: docs/plan/05_document_persistence.md#atomic-save

use std::sync::Arc;

use stickymd_core::{
    DiskFingerprint, ExternalFileState, Generation, LineEnding, PreserveExpectation,
};

use super::StorageFailure;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PersistMode {
    Guarded { expected: Option<DiskFingerprint> },
    ForceOverwrite,
}

#[derive(Debug, Clone)]
pub struct PersistRequest {
    pub generation: Generation,
    pub text: Arc<str>,
    pub line_ending: LineEnding,
    pub mode: PersistMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistResult {
    Saved {
        generation: Generation,
        fingerprint: DiskFingerprint,
        durable_bytes: usize,
    },
    Conflict {
        generation: Generation,
        observed: ExternalFileState,
    },
}

/// Result of preserving the canonical note before a recovery restore.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreserveOutcome {
    Preserved,
    /// The canonical note is no longer the content that required preserving it; it was left in
    /// place.
    Changed(ExternalFileState),
    /// No content fingerprint exists for it (a too-large canonical), so it is never moved.
    Unverifiable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TemporaryCleanup {
    /// Ends the recovery operation `operation` (`RecoveryCoordinator::operation_id`).
    RecoveryResolved { operation: u64 },
    /// Follows a loaded external conflict. Its receipt commits no state: success changes
    /// nothing and a failure reports a durable fact, so it needs no identity.
    ConflictDiscarded,
}

/// One note operation's Execution Receipt, with the identity that decides whether it is stale
/// (plan 04 generation rule 6).
#[derive(Debug)]
pub enum NoteReceipt {
    Save {
        generation: Generation,
        mode: PersistMode,
        result: Result<PersistResult, StorageFailure>,
    },
    /// One durable observation, requested while `requested_base` was the known Durable
    /// Fingerprint; `PersistenceCoordinator::external_observation_current` decides whether it
    /// still applies.
    External {
        requested_base: Option<DiskFingerprint>,
        result: Result<ExternalFileState, StorageFailure>,
    },
    TemporaryRemoved {
        purpose: TemporaryCleanup,
        result: Result<(), StorageFailure>,
    },
    /// Preserving the canonical note for the recovery operation `operation`.
    CanonicalPreserved {
        operation: u64,
        result: Result<PreserveOutcome, StorageFailure>,
    },
}

/// Durable storage of the one canonical note. The note's location is bound when the adapter is
/// built, and each call is one complete transaction: the single I/O worker owns the storage, so a
/// call needs `&mut self` and `Send` but never `Sync`.
pub trait NoteStorage: Send + 'static {
    /// Publishes `request` atomically; a `Guarded` request that no longer matches the disk
    /// returns `PersistResult::Conflict` and writes nothing (see `guarded_publish_verdict`).
    fn persist(&mut self, request: &PersistRequest) -> Result<PersistResult, StorageFailure>;

    /// One stable observation of the canonical note.
    fn observe(&mut self) -> Result<ExternalFileState, StorageFailure>;

    /// Removes the fixed transaction temporary; an absent one is success.
    fn remove_temporary(&mut self) -> Result<(), StorageFailure>;

    /// Moves the canonical note aside only while it is still `expected`, under a name the
    /// adapter chooses.
    fn preserve_canonical(
        &mut self,
        expected: PreserveExpectation,
    ) -> Result<PreserveOutcome, StorageFailure>;
}
