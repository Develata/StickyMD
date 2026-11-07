//! The exact effect order of the note decisions, without a host. The Windows shell runs the same
//! paths end to end in `phase15_receipt_tests`.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use stickymd_core::{
    DocumentState, EditKind, ExternalFileFact, ExternalFileState, Generation, LineEnding,
    LoadedDocument, PreserveExpectation, RecoveryCandidate, Selection, hash_bytes,
};

use super::save::keep_local_needs_resubmit;
use super::{NoteEffect, NoteEffects, NoteIo};
use crate::flow::{
    ClipboardError, ClipboardPort, EditorCoordinator, PersistenceCoordinator, RecoveryCoordinator,
    SaveTrigger,
};
use crate::instruction::AppIntent;
use crate::ports::{
    FailureKind, NoteReceipt, PersistMode, PersistResult, PreserveOutcome, StorageFailure,
    TemporaryCleanup,
};

const GUARDED: PersistMode = PersistMode::Guarded { expected: None };

struct NoClipboard;

impl ClipboardPort for NoClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        Ok(None)
    }

    fn write_text(&mut self, _text: &str) -> Result<(), ClipboardError> {
        Ok(())
    }
}

struct Note {
    editor: EditorCoordinator<NoClipboard>,
    persistence: PersistenceCoordinator,
    recovery: RecoveryCoordinator,
}

impl Note {
    fn new(text: &str) -> Self {
        Self {
            editor: EditorCoordinator::new(
                DocumentState::loaded(text, LineEnding::Lf, None),
                NoClipboard,
            ),
            persistence: PersistenceCoordinator::default(),
            recovery: RecoveryCoordinator::new(None, None),
        }
    }

    fn io(&mut self) -> NoteIo<'_, NoClipboard> {
        NoteIo::new(&mut self.editor, &mut self.persistence, &mut self.recovery)
    }

    fn generation(&self) -> Generation {
        self.editor.view().generation
    }

    fn type_letter(&mut self) {
        let expected_generation = self.generation();
        self.editor
            .dispatch(AppIntent::Edit {
                expected_generation,
                selection: Selection::caret(0),
                inserted: "a".into(),
                kind: EditKind::Typing,
                timestamp_ms: 1,
            })
            .unwrap();
    }

    fn offer_recovery(&mut self, preserve: Option<PreserveExpectation>) {
        self.recovery = RecoveryCoordinator::new(
            Some(RecoveryCandidate {
                canonical: Some(loaded("disk")),
                temporary: loaded("temporary"),
                temporary_is_newer: true,
            }),
            preserve,
        );
        self.persistence.set_recovery_pending(true);
    }

    fn enter_conflict(&mut self) {
        let receipt = observed(self, present("outside"));
        let _ = self.io().on_receipt(receipt, 0);
        assert!(self.persistence.conflict().is_some());
    }

    fn saved_receipt(&self, mode: PersistMode) -> NoteReceipt {
        let generation = self.generation();
        NoteReceipt::Save {
            generation,
            mode,
            result: Ok(PersistResult::Saved {
                generation,
                fingerprint: hash_bytes(b"saved"),
                durable_bytes: 5,
            }),
        }
    }
}

fn loaded(text: &str) -> LoadedDocument {
    LoadedDocument::from_durable_bytes(text.as_bytes()).unwrap()
}

fn fact(text: &str) -> ExternalFileFact {
    loaded(text).into_external_fact()
}

fn present(text: &str) -> ExternalFileState {
    ExternalFileState::Present(fact(text))
}

fn observed(note: &Note, state: ExternalFileState) -> NoteReceipt {
    NoteReceipt::External {
        requested_base: note.editor.view().base_disk_hash,
        result: Ok(state),
    }
}

fn failure() -> StorageFailure {
    StorageFailure::new(FailureKind::Unavailable, "unstable")
}

fn names(effects: NoteEffects) -> Vec<&'static str> {
    effects
        .into_iter()
        .map(|effect| match effect {
            NoteEffect::SubmitSave(_) => "submit",
            NoteEffect::InspectExternal { .. } => "inspect",
            NoteEffect::RemoveTemporary(_) => "remove-temporary",
            NoteEffect::PreserveCanonical { .. } => "preserve",
            NoteEffect::ApplyDocument(_) => "apply-document",
            NoteEffect::ResyncProjection => "resync",
            NoteEffect::AssetsAfterRecovery => "assets",
            NoteEffect::NoteBarriers { succeeded: true } => "barriers-ok",
            NoteEffect::NoteBarriers { succeeded: false } => "barriers-failed",
            NoteEffect::Diagnostic(_) => "diagnostic",
            NoteEffect::RefreshTitle => "title",
            NoteEffect::Redraw => "redraw",
        })
        .collect()
}

fn last_diagnostic(effects: NoteEffects) -> Option<String> {
    effects.0.into_iter().rev().find_map(|effect| match effect {
        NoteEffect::Diagnostic(message) => Some(message),
        _ => None,
    })
}

#[test]
fn keep_local_is_satisfied_only_by_a_force_overwrite_receipt() {
    assert!(keep_local_needs_resubmit(true, true, GUARDED));
    assert!(!keep_local_needs_resubmit(
        true,
        true,
        PersistMode::ForceOverwrite
    ));
    assert!(!keep_local_needs_resubmit(false, true, GUARDED));
    assert!(!keep_local_needs_resubmit(true, false, GUARDED));
}

#[test]
fn an_idle_tick_has_no_effects_and_allocates_nothing() {
    let mut note = Note::new("text");
    assert_eq!(note.io().tick(0).0.capacity(), 0);
}

#[test]
fn a_save_is_guarded_by_the_known_fingerprint() {
    let mut note = Note::new("text");
    let base = hash_bytes(b"base");
    let generation = note.generation();
    note.editor.acknowledge_persisted(generation, base).unwrap();
    let effects = note.io().submit_save(SaveTrigger::Manual, None);
    let mut effects = effects.into_iter();
    let Some(NoteEffect::SubmitSave(request)) = effects.next() else {
        panic!("the request comes first");
    };
    assert_eq!(
        request.mode,
        PersistMode::Guarded {
            expected: Some(base)
        }
    );
    assert_eq!(request.text.as_ref(), "text");
    assert!(note.persistence.note_save_in_flight());
}

#[test]
fn recovery_blocks_every_save_except_its_own() {
    let mut note = Note::new("text");
    note.offer_recovery(None);
    assert!(names(note.io().submit_save(SaveTrigger::Manual, None)).is_empty());
    assert!(!note.persistence.note_save_in_flight());
    assert_eq!(
        names(note.io().submit_save(SaveTrigger::KeepLocal, None)),
        ["submit", "diagnostic", "redraw"]
    );
}

#[test]
fn a_saved_receipt_settles_the_barriers_before_the_redraw() {
    let mut note = Note::new("text");
    note.type_letter();
    let receipt = note.saved_receipt(GUARDED);
    assert_eq!(
        names(note.io().on_receipt(receipt, 0)),
        ["diagnostic", "title", "barriers-ok", "redraw"]
    );
}

/// Pinned as it is: an unreached generation reports and stops, with no barrier and no redraw.
#[test]
fn a_receipt_for_an_unreached_generation_only_reports() {
    let mut note = Note::new("text");
    note.persistence.note_save_submitted(SaveTrigger::Manual);
    let future = note.generation().checked_next().unwrap();
    let receipt = NoteReceipt::Save {
        generation: future,
        mode: GUARDED,
        result: Ok(PersistResult::Saved {
            generation: future,
            fingerprint: hash_bytes(b"saved"),
            durable_bytes: 5,
        }),
    };
    assert_eq!(names(note.io().on_receipt(receipt, 0)), ["diagnostic"]);
    assert!(!note.persistence.note_save_in_flight());
}

#[test]
fn a_valid_older_save_receipt_keeps_the_newer_edit_dirty_and_saves_it() {
    let mut note = Note::new("text");
    note.type_letter();
    let receipt = note.saved_receipt(GUARDED);
    note.type_letter();
    assert_eq!(
        names(note.io().on_receipt(receipt, 0)),
        [
            "diagnostic",
            "submit",
            "diagnostic",
            "redraw",
            "title",
            "barriers-ok",
            "redraw"
        ]
    );
    let view = note.editor.view();
    assert!(view.dirty);
    assert_eq!(view.base_disk_hash, Some(hash_bytes(b"saved")));
}

#[test]
fn a_keep_local_handover_holds_the_barriers() {
    let mut note = Note::new("text");
    note.type_letter();
    note.enter_conflict();
    let _ = note.io().resolve(false).unwrap();
    assert!(note.persistence.keep_local_in_progress());
    let generation = note.generation();
    let receipt = NoteReceipt::Save {
        generation,
        mode: GUARDED,
        result: Err(failure()),
    };
    assert_eq!(
        names(note.io().on_receipt(receipt, 0)),
        [
            "diagnostic",
            "submit",
            "diagnostic",
            "redraw",
            "title",
            "redraw"
        ]
    );
    let receipt = note.saved_receipt(PersistMode::ForceOverwrite);
    let effects = note.io().on_receipt(receipt, 0);
    assert_eq!(
        last_diagnostic(effects).as_deref(),
        Some("已保留本地内容并覆盖外部文件。")
    );
    assert!(!note.persistence.keep_local_in_progress());
    assert!(note.persistence.conflict().is_none());
}

#[test]
fn a_stale_identity_changes_nothing() {
    let mut note = Note::new("disk");
    note.offer_recovery(None);
    let _ = note.io().resolve(false).unwrap();
    let operation = note.recovery.operation_id().unwrap();
    let stale = operation + 1;
    let receipts = [
        NoteReceipt::TemporaryRemoved {
            purpose: TemporaryCleanup::RecoveryResolved { operation: stale },
            result: Ok(()),
        },
        NoteReceipt::TemporaryRemoved {
            purpose: TemporaryCleanup::RecoveryResolved { operation: stale },
            result: Err(failure()),
        },
        NoteReceipt::CanonicalPreserved {
            operation: stale,
            result: Ok(PreserveOutcome::Preserved),
        },
        NoteReceipt::CanonicalPreserved {
            operation: stale,
            result: Err(failure()),
        },
    ];
    for receipt in receipts {
        assert!(names(note.io().on_receipt(receipt, 0)).is_empty());
        assert_eq!(note.recovery.operation_id(), Some(operation));
    }
}

#[test]
fn a_superseded_observation_only_schedules_a_recheck() {
    let mut note = Note::new("local");
    let receipt = NoteReceipt::External {
        requested_base: Some(hash_bytes(b"older")),
        result: Ok(present("outside")),
    };
    assert!(names(note.io().on_receipt(receipt, 10)).is_empty());
    assert_eq!(note.editor.view().text, "local");
    assert!(note.persistence.external_deadline().is_some());
}

#[test]
fn a_clean_reload_applies_the_document_before_reporting_it() {
    let mut note = Note::new("start");
    let receipt = observed(&note, present("outside"));
    assert_eq!(
        names(note.io().on_receipt(receipt, 0)),
        ["apply-document", "diagnostic", "title", "redraw"]
    );
    assert_eq!(note.editor.view().text, "outside");
}

#[test]
fn loading_the_external_side_applies_it_before_discarding_the_temporary() {
    let mut note = Note::new("start");
    note.type_letter();
    note.enter_conflict();
    assert_eq!(
        names(note.io().resolve(true).unwrap()),
        [
            "apply-document",
            "remove-temporary",
            "diagnostic",
            "title",
            "redraw"
        ]
    );
    assert!(note.persistence.conflict().is_none());
}

#[test]
fn restoring_resyncs_before_saving_or_preserving() {
    let mut note = Note::new("disk");
    note.offer_recovery(None);
    assert_eq!(
        names(note.io().resolve(true).unwrap()),
        ["resync", "submit", "diagnostic", "redraw"]
    );

    let mut note = Note::new("disk");
    note.offer_recovery(Some(PreserveExpectation::Content(hash_bytes(b"\xff"))));
    assert_eq!(
        names(note.io().resolve(true).unwrap()),
        ["resync", "preserve"]
    );
}

#[test]
fn finishing_recovery_converges_the_assets_first() {
    let mut note = Note::new("disk");
    note.offer_recovery(None);
    let _ = note.io().resolve(false).unwrap();
    let operation = note.recovery.operation_id().unwrap();
    let receipt = NoteReceipt::TemporaryRemoved {
        purpose: TemporaryCleanup::RecoveryResolved { operation },
        result: Ok(()),
    };
    assert_eq!(
        names(note.io().on_receipt(receipt, 0)),
        ["assets", "diagnostic", "title", "redraw"]
    );
    assert!(!note.recovery.is_pending());
}

#[test]
fn the_keys_are_not_consumed_without_a_recovery_or_a_conflict() {
    let mut note = Note::new("text");
    assert!(note.io().resolve(true).is_none());
    assert!(note.io().resolve(false).is_none());
}
