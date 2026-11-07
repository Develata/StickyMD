//! I/O receipts through the real completion handler of the windowless shell: the Phase 15 B
//! entry evidence that a receipt with a stale identity changes nothing and that a valid older
//! save receipt is still accepted.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#generation

use stickymd_app::flow::SaveTrigger;
use stickymd_core::{
    ExternalFileFact, ExternalFileState, LineEnding, LoadedDocument, PreserveExpectation,
    RecoveryCandidate, hash_bytes,
};
use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::app::headless_tests::{HeadlessApp, fixture};
use crate::app::input::ShellKey;
use crate::persistence::{
    IoCompletion, NoteCompletion, PersistMode, PersistResult, PreserveOutcome, TemporaryCleanup,
};

fn loaded(text: &str) -> LoadedDocument {
    LoadedDocument::from_durable_bytes(text.as_bytes()).unwrap()
}

fn external(text: &str) -> ExternalFileFact {
    loaded(text).into_external_fact()
}

/// A shell waiting for a recovery choice between a canonical note and a temp.
fn recovering(preserve: Option<PreserveExpectation>) -> HeadlessApp {
    let mut app = fixture("disk");
    app.recovery = stickymd_app::flow::RecoveryCoordinator::new(
        Some(RecoveryCandidate {
            canonical: Some(loaded("disk")),
            temporary: loaded("temporary"),
            temporary_is_newer: true,
        }),
        preserve,
    );
    app.persistence.set_recovery_pending(true);
    app
}

fn saves(app: &HeadlessApp) -> u64 {
    app.worker.metrics().note_submitted
}

fn type_letter(app: &mut HeadlessApp) {
    app.apply_key(
        ShellKey {
            physical_key: PhysicalKey::Code(KeyCode::KeyA),
            state: ElementState::Pressed,
            repeat: false,
            ime_owned: false,
            text: Some("a"),
        },
        false,
    );
}

#[test]
fn a_recovery_cleanup_receipt_of_another_operation_changes_nothing() {
    let mut app = recovering(None);
    app.recovery.begin_use_canonical().unwrap();
    let operation = app.recovery.operation_id().unwrap();
    app.handle_io_completion(
        None,
        IoCompletion::TemporaryRemoved {
            purpose: TemporaryCleanup::RecoveryResolved {
                operation: operation + 1,
            },
            result: Ok(()),
        },
    );
    assert!(app.recovery.is_pending());
    assert_eq!(app.recovery.operation_id(), Some(operation));
    app.handle_io_completion(
        None,
        IoCompletion::TemporaryRemoved {
            purpose: TemporaryCleanup::RecoveryResolved { operation },
            result: Ok(()),
        },
    );
    assert!(!app.recovery.is_pending());
}

#[test]
fn a_preserve_receipt_of_another_operation_starts_no_save() {
    let expected = PreserveExpectation::Content(hash_bytes(b"\xff"));
    let mut app = recovering(Some(expected));
    app.recovery.begin_restore().unwrap();
    let operation = app.recovery.operation_id().unwrap();
    let before = saves(&app);
    app.handle_io_completion(
        None,
        IoCompletion::CanonicalPreserved {
            operation: operation + 1,
            result: Ok(PreserveOutcome::Preserved),
        },
    );
    assert_eq!(saves(&app), before);
    assert_eq!(app.recovery.operation_id(), Some(operation));
    app.handle_io_completion(
        None,
        IoCompletion::CanonicalPreserved {
            operation,
            result: Ok(PreserveOutcome::Preserved),
        },
    );
    assert_eq!(saves(&app), before + 1);
}

/// A canonical changed while the choice waited ends the operation, keeps the candidate and sets
/// what the next restore must find; the old receipt can no longer act.
#[test]
fn a_changed_canonical_becomes_a_conflict_and_the_retry_uses_the_new_expectation() {
    let mut app = recovering(Some(PreserveExpectation::Content(hash_bytes(b"\xff"))));
    app.recovery.begin_restore().unwrap();
    let first = app.recovery.operation_id().unwrap();
    let changed = hash_bytes(b"\xfe");
    app.handle_io_completion(
        None,
        IoCompletion::CanonicalPreserved {
            operation: first,
            result: Ok(PreserveOutcome::Changed(ExternalFileState::InvalidUtf8 {
                fingerprint: changed,
            })),
        },
    );
    assert!(app.recovery.is_pending());
    assert_eq!(app.recovery.operation_id(), None);
    assert!(app.diagnostic.as_deref().unwrap().contains("已变化"));
    let plan = app.recovery.begin_restore().unwrap();
    assert_eq!(
        plan.preserve_canonical,
        Some(PreserveExpectation::Content(changed))
    );
    assert_ne!(app.recovery.operation_id(), Some(first));
    assert!(!app.recovery.accepts(first));
}

#[test]
fn an_unverifiable_canonical_ends_the_operation_without_a_save() {
    let mut app = recovering(Some(PreserveExpectation::Unverifiable));
    app.recovery.begin_restore().unwrap();
    let operation = app.recovery.operation_id().unwrap();
    let before = saves(&app);
    app.handle_io_completion(
        None,
        IoCompletion::CanonicalPreserved {
            operation,
            result: Ok(PreserveOutcome::Unverifiable),
        },
    );
    assert_eq!(saves(&app), before);
    assert!(app.recovery.is_pending());
    assert_eq!(app.recovery.operation_id(), None);
    assert!(app.diagnostic.as_deref().unwrap().contains("过大"));
}

/// An observation requested before the known fingerprint changed is not applied, and a fresh
/// check is scheduled; one requested against the current fingerprint is applied.
#[test]
fn a_superseded_external_observation_is_rechecked_and_a_current_one_applied() {
    let mut app = fixture("local");
    let current = app.coordinator.view().base_disk_hash;
    let stale = Some(hash_bytes(b"an older durable state"));
    assert_ne!(stale, current);
    app.handle_io_completion(
        None,
        IoCompletion::External {
            requested_base: stale,
            result: Ok(ExternalFileState::Present(external("outside"))),
        },
    );
    assert_eq!(app.coordinator.view().text, "local");
    assert!(app.persistence.external_deadline().is_some());
    app.handle_io_completion(
        None,
        IoCompletion::External {
            requested_base: current,
            result: Ok(ExternalFileState::Present(external("outside"))),
        },
    );
    assert_eq!(app.coordinator.view().text, "outside");
}

/// A save receipt for an older generation is valid: it records what reached the disk, the newer
/// edit stays dirty and a follow-up save is submitted for it.
#[test]
fn a_valid_older_save_receipt_is_accepted_and_the_newer_edit_stays_dirty() {
    let mut app = fixture("start");
    type_letter(&mut app);
    let saved = app.coordinator.view().generation;
    let snapshot = app.coordinator.snapshot();
    let bytes = LoadedDocument::encode_runtime(&snapshot.text, LineEnding::Lf);
    let fingerprint = hash_bytes(&bytes);
    app.persistence.note_save_submitted(SaveTrigger::Debounce);
    type_letter(&mut app);
    assert_ne!(app.coordinator.view().generation, saved);
    let before = saves(&app);
    app.handle_io_completion(
        None,
        IoCompletion::Note(NoteCompletion {
            generation: saved,
            mode: PersistMode::Guarded { expected: None },
            result: Ok(PersistResult::Saved {
                generation: saved,
                fingerprint,
                durable_bytes: bytes.len(),
            }),
        }),
    );
    let view = app.coordinator.view();
    assert_eq!(view.base_disk_hash, Some(fingerprint));
    assert!(view.dirty);
    assert_eq!(saves(&app), before + 1);
}
