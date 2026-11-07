//! Recovery choice and retry state independent of windows, files, and UI.
//!
//! plan_ref: docs/plan/05_document_persistence.md#recovery

use stickymd_core::{
    DiskFingerprint, ExternalFileFact, ExternalFileState, LoadedDocument, PreserveExpectation,
    RecoveryCandidate,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryOperation {
    Restoring,
    UsingCanonical,
}

#[derive(Debug, Clone)]
pub struct RestorePlan {
    pub temporary: LoadedDocument,
    pub expected: Option<DiskFingerprint>,
    /// The canonical observation to preserve before restoring, if it must be preserved.
    pub preserve_canonical: Option<PreserveExpectation>,
}

#[derive(Debug, Clone)]
pub enum CanonicalRecoveryPlan {
    Present(ExternalFileFact),
    Missing,
    Unusable,
}

#[derive(Debug)]
pub struct RecoveryCoordinator {
    candidate: Option<RecoveryCandidate>,
    /// The unusable canonical observation (invalid UTF-8 or too large) that must be preserved
    /// before a restore may publish over it; `None` when no preservation is required.
    canonical_to_preserve: Option<PreserveExpectation>,
    operation: Option<RecoveryOperation>,
    /// Identity of the pending operation; its I/O receipts are accepted only while it is
    /// current (plan 04 generation rules 4–5: stale results are dropped).
    operation_id: Option<u64>,
    issued_operations: u64,
    restore_guard: Option<Option<DiskFingerprint>>,
}

impl RecoveryCoordinator {
    pub fn new(
        candidate: Option<RecoveryCandidate>,
        canonical_to_preserve: Option<PreserveExpectation>,
    ) -> Self {
        Self {
            candidate,
            canonical_to_preserve,
            operation: None,
            operation_id: None,
            issued_operations: 0,
            restore_guard: None,
        }
    }

    fn start(&mut self, operation: RecoveryOperation) {
        self.issued_operations += 1;
        self.operation = Some(operation);
        self.operation_id = Some(self.issued_operations);
    }

    /// The pending operation's identity, carried by its I/O requests.
    pub fn operation_id(&self) -> Option<u64> {
        self.operation_id
    }

    /// Whether a receipt issued for `operation` belongs to the pending operation. A receipt of
    /// a failed, finished or superseded operation is stale and must change nothing.
    pub fn accepts(&self, operation: u64) -> bool {
        self.operation_id == Some(operation)
    }

    pub fn is_pending(&self) -> bool {
        self.candidate.is_some()
    }

    pub fn operation_pending(&self) -> bool {
        self.operation.is_some()
    }

    pub fn begin_restore(&mut self) -> Option<RestorePlan> {
        if self.operation.is_some() {
            return None;
        }
        let candidate = self.candidate.as_ref()?;
        let expected = candidate
            .canonical
            .as_ref()
            .map(|canonical| canonical.fingerprint);
        let temporary = candidate.temporary.clone();
        self.start(RecoveryOperation::Restoring);
        self.restore_guard = Some(expected);
        Some(RestorePlan {
            temporary,
            expected,
            preserve_canonical: self.canonical_to_preserve,
        })
    }

    pub fn begin_use_canonical(&mut self) -> Option<CanonicalRecoveryPlan> {
        if self.operation.is_some() || self.candidate.is_none() {
            return None;
        }
        if self.canonical_to_preserve.is_some() {
            return Some(CanonicalRecoveryPlan::Unusable);
        }
        self.start(RecoveryOperation::UsingCanonical);
        Some(
            self.candidate
                .as_ref()
                .and_then(|candidate| candidate.canonical.clone())
                .map_or(CanonicalRecoveryPlan::Missing, |canonical| {
                    CanonicalRecoveryPlan::Present(canonical.into_external_fact())
                }),
        )
    }

    pub fn take_restore_guard(&mut self) -> Option<Option<DiskFingerprint>> {
        self.restore_guard.take()
    }

    pub fn operation(&self) -> Option<RecoveryOperation> {
        self.operation
    }

    pub fn fail_operation(&mut self) {
        self.operation = None;
        self.operation_id = None;
        self.restore_guard = None;
    }

    pub fn refresh_canonical_after_conflict(&mut self, observed: ExternalFileState) {
        let Some(candidate) = &mut self.candidate else {
            return;
        };
        // An invalid canonical's observation already fingerprints its full content; a too-large
        // one carries only its size, so it is never moved automatically.
        let to_preserve = match &observed {
            ExternalFileState::InvalidUtf8 { fingerprint } => {
                Some(PreserveExpectation::Content(*fingerprint))
            }
            ExternalFileState::TooLarge { .. } => Some(PreserveExpectation::Unverifiable),
            ExternalFileState::Present(_) | ExternalFileState::Missing => None,
        };
        match observed {
            ExternalFileState::Present(fact) => {
                candidate.canonical = Some(LoadedDocument {
                    durable_len: fact.durable_len,
                    text: fact.text,
                    line_ending: fact.line_ending,
                    fingerprint: fact.fingerprint,
                });
            }
            ExternalFileState::Missing
            | ExternalFileState::InvalidUtf8 { .. }
            | ExternalFileState::TooLarge { .. } => candidate.canonical = None,
        }
        self.canonical_to_preserve = to_preserve;
        self.fail_operation();
    }

    pub fn finish(&mut self) {
        self.candidate = None;
        self.canonical_to_preserve = None;
        self.fail_operation();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stickymd_core::{LineEnding, hash_bytes};

    fn loaded(text: &str) -> LoadedDocument {
        LoadedDocument::from_durable_bytes(text.as_bytes()).unwrap()
    }

    #[test]
    fn restore_uses_canonical_fingerprint_and_blocks_parallel_choices() {
        let canonical = loaded("disk");
        let expected = canonical.fingerprint;
        let mut recovery = RecoveryCoordinator::new(
            Some(RecoveryCandidate {
                canonical: Some(canonical),
                temporary: loaded("temporary"),
                temporary_is_newer: true,
            }),
            None,
        );
        let plan = recovery.begin_restore().unwrap();
        assert_eq!(plan.expected, Some(expected));
        assert_eq!(plan.temporary.text, "temporary");
        assert!(recovery.begin_restore().is_none());
        assert!(recovery.begin_use_canonical().is_none());
    }

    /// A conflict observed during recovery sets what a later restore must preserve: an invalid
    /// canonical by its content fingerprint, a too-large one as unverifiable, a usable one not at all.
    #[test]
    fn a_conflict_sets_the_next_preserve_expectation() {
        let mut recovery = RecoveryCoordinator::new(
            Some(RecoveryCandidate {
                canonical: None,
                temporary: loaded("temporary"),
                temporary_is_newer: true,
            }),
            Some(PreserveExpectation::Content(hash_bytes(b"\xff"))),
        );
        let invalid = hash_bytes(b"\xfe");
        for (observed, expected) in [
            (
                ExternalFileState::InvalidUtf8 {
                    fingerprint: invalid,
                },
                Some(PreserveExpectation::Content(invalid)),
            ),
            (
                ExternalFileState::TooLarge { bytes: 1 << 30 },
                Some(PreserveExpectation::Unverifiable),
            ),
            (ExternalFileState::Missing, None),
        ] {
            recovery.begin_restore().unwrap();
            recovery.refresh_canonical_after_conflict(observed);
            assert_eq!(recovery.operation_id(), None);
            assert_eq!(
                recovery.begin_restore().unwrap().preserve_canonical,
                expected
            );
            recovery.fail_operation();
        }
    }

    #[test]
    fn receipts_are_accepted_only_for_the_pending_operation() {
        let candidate = RecoveryCandidate {
            canonical: Some(loaded("disk")),
            temporary: loaded("temporary"),
            temporary_is_newer: true,
        };
        let mut recovery = RecoveryCoordinator::new(Some(candidate), None);
        assert_eq!(recovery.operation_id(), None);
        recovery.begin_restore().unwrap();
        let first = recovery.operation_id().unwrap();
        assert!(recovery.accepts(first));
        // A failed operation's late receipt is stale, and so is it under the next operation.
        recovery.fail_operation();
        assert!(!recovery.accepts(first));
        assert!(matches!(
            recovery.begin_use_canonical(),
            Some(CanonicalRecoveryPlan::Present(_))
        ));
        let second = recovery.operation_id().unwrap();
        assert_ne!(first, second);
        assert!(!recovery.accepts(first));
        assert!(recovery.accepts(second));
        recovery.finish();
        assert!(!recovery.accepts(second));
    }

    #[test]
    fn conflict_refreshes_the_next_guard_without_losing_temporary_evidence() {
        let mut recovery = RecoveryCoordinator::new(
            Some(RecoveryCandidate {
                canonical: Some(loaded("old")),
                temporary: loaded("temporary"),
                temporary_is_newer: true,
            }),
            None,
        );
        recovery.begin_restore().unwrap();
        let newer = hash_bytes(b"new external");
        recovery.refresh_canonical_after_conflict(ExternalFileState::Present(ExternalFileFact {
            fingerprint: newer,
            text: "new external".into(),
            line_ending: LineEnding::Lf,
            durable_len: 12,
        }));
        let retried = recovery.begin_restore().unwrap();
        assert_eq!(retried.expected, Some(newer));
        assert_eq!(retried.temporary.text, "temporary");
    }
}
