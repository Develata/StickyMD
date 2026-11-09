//! The session's order without a shell: the loop the shell runs, recording what reaches it. The
//! shell runs the same paths end to end in `phase15_receipt_tests` (`stickymd-desktop`).
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use stickymd_core::{
    DocumentState, EditKind, ExternalFileState, Generation, LineEnding, LoadedDocument,
    PreserveExpectation, Selection, hash_bytes,
};

use super::*;
use crate::config::RuntimeConfig;
use crate::execution::{IoCompletion, IoPorts};
use crate::flow::window::geometry::{
    MonitorGeometry, MonitorIdentity, PhysicalRect, WindowPlacement,
};
use crate::flow::window::state::{
    LifecycleState, QuitStage, StableVisibility, WindowEffect, WindowIntent,
};
use crate::flow::{ClipboardError, ClipboardPort};
use crate::instruction::AppIntent;
use crate::ports::{
    AssetPasteCompletion, AssetPasteFailure, AssetPasteRequest, AssetReconcileReport,
    AssetSyncRequest, ConfigStorage, ExportCompletion, ExportRequest, ExportSink, FailureKind,
    ManagedAssetStore, NoteReceipt, NoteStorage, PersistMode, PersistRequest, PersistResult,
    PreserveOutcome, StorageFailure,
};

struct NoClipboard;

impl ClipboardPort for NoClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        Ok(None)
    }

    fn write_text(&mut self, _text: &str) -> Result<(), ClipboardError> {
        Ok(())
    }
}

/// Ports that take every job and report nothing useful: the tests deliver the receipts.
struct NoPort;

fn refused() -> StorageFailure {
    StorageFailure::new(FailureKind::Unsupported, "no port in this test")
}

impl NoteStorage for NoPort {
    fn persist(&mut self, _request: &PersistRequest) -> Result<PersistResult, StorageFailure> {
        Err(refused())
    }

    fn observe(&mut self) -> Result<ExternalFileState, StorageFailure> {
        Err(refused())
    }

    fn remove_temporary(&mut self) -> Result<(), StorageFailure> {
        Err(refused())
    }

    fn preserve_canonical(
        &mut self,
        _expected: PreserveExpectation,
    ) -> Result<PreserveOutcome, StorageFailure> {
        Err(refused())
    }
}

impl ManagedAssetStore for NoPort {
    fn paste(
        &mut self,
        _request: AssetPasteRequest,
    ) -> Result<AssetPasteCompletion, AssetPasteFailure> {
        unreachable!("no test here pastes")
    }

    fn sync(
        &mut self,
        _request: &AssetSyncRequest,
    ) -> Result<AssetReconcileReport, StorageFailure> {
        Err(refused())
    }
}

impl ExportSink for NoPort {
    fn export(&mut self, _request: ExportRequest) -> Result<ExportCompletion, StorageFailure> {
        Err(refused())
    }
}

impl ConfigStorage for NoPort {
    fn save(&mut self, _config: &RuntimeConfig) -> Result<(), StorageFailure> {
        Err(refused())
    }
}

struct FixedClock;

impl MonotonicClock for FixedClock {
    fn now_ms(&self) -> u64 {
        1_000
    }
}

pub(super) fn session(text: &str) -> AppSession {
    let ports = IoPorts {
        note: NoPort,
        assets: NoPort,
        export: NoPort,
        config: NoPort,
    };
    let mut session = AppSession::new(SessionParts {
        document: DocumentState::loaded(text, LineEnding::Lf, None),
        clipboard: Box::new(NoClipboard),
        config: RuntimeConfig::default(),
        config_persistence_allowed: false,
        recovery: None,
        recovery_canonical_to_preserve: None,
        worker: crate::execution::PersistenceWorker::start(ports, |_| {}).unwrap(),
        clock: Box::new(FixedClock),
    });
    session.window_flow = Some(WindowShellCoordinator::new(
        WindowPlacement::new(900.0, 600.0, None, 0.0, 0.5, 0.5),
        StableVisibility::Floating,
        MonitorGeometry::new(
            MonitorIdentity::new("test"),
            PhysicalRect::new(0, 0, 1920, 1080),
            1.0,
            true,
        ),
        true,
    ));
    session
}

fn label(effect: &ShellEffect) -> String {
    match effect {
        ShellEffect::Window(effect) => format!("{effect:?}"),
        ShellEffect::Resume(_) => "Resume".to_owned(),
        ShellEffect::Projection(ProjectionChange::Delta { .. }) => "Delta".to_owned(),
        ShellEffect::Projection(ProjectionChange::FullResync(_)) => "FullResync".to_owned(),
        ShellEffect::PreviewDocumentChanged(_) => "PreviewDocumentChanged".to_owned(),
        ShellEffect::SubmitPreview(_) => "SubmitPreview".to_owned(),
        ShellEffect::PresentationChanged => "PresentationChanged".to_owned(),
        ShellEffect::ViewModeChanged(mode) => format!("ViewModeChanged({mode:?})"),
        ShellEffect::KeyboardOwnerChanged => "KeyboardOwnerChanged".to_owned(),
        ShellEffect::SourceFocused => "SourceFocused".to_owned(),
        ShellEffect::ImeAreaChanged => "ImeAreaChanged".to_owned(),
        ShellEffect::Preference(effect) => format!("{effect:?}"),
        ShellEffect::TitleDirty => "TitleDirty".to_owned(),
        ShellEffect::RedrawNeeded => "RedrawNeeded".to_owned(),
        ShellEffect::Diagnostic(message) => message.clone(),
        ShellEffect::StartWatcher => "StartWatcher".to_owned(),
        ShellEffect::FocusLost => "FocusLost".to_owned(),
        ShellEffect::Exit => "Exit".to_owned(),
    }
}

/// One frame, as the shell applies it: a turn runs at its place and finishes first.
fn drain(session: &mut AppSession, out: &mut ShellEffects, frame: Frame, seen: &mut Vec<String>) {
    out.seal(frame);
    while let Some(effect) = out.next(frame) {
        if let ShellEffect::Resume(turn) = effect {
            let frame = out.open();
            session.resume(turn, ViewFacts::default(), out);
            drain(session, out, frame, seen);
        } else {
            seen.push(label(&effect));
        }
    }
}

/// What a step asked of the shell, its turns taken.
pub(super) fn run(
    session: &mut AppSession,
    step: impl FnOnce(&mut AppSession, &mut ShellEffects),
) -> Vec<String> {
    let mut out = ShellEffects::default();
    let mut seen = Vec::new();
    let frame = out.open();
    step(session, &mut out);
    drain(session, &mut out, frame, &mut seen);
    seen
}

fn edit(session: &AppSession) -> AppIntent {
    AppIntent::Edit {
        expected_generation: session.coordinator.view().generation,
        selection: Selection::caret(0),
        inserted: "a".into(),
        kind: EditKind::Typing,
        timestamp_ms: 1,
    }
}

fn stage(session: &AppSession) -> QuitStage {
    match session.window_flow.as_ref().unwrap().state().lifecycle() {
        LifecycleState::Quitting(stage) => stage,
        LifecycleState::Running => panic!("the quit ended"),
    }
}

/// A step that an effect runs comes before what is left of the step that asked for it.
#[test]
fn a_nested_frame_runs_before_the_rest_of_its_parent() {
    let mut out = ShellEffects::default();
    let outer = out.open();
    out.push(ShellEffect::TitleDirty);
    out.push(ShellEffect::RedrawNeeded);
    out.seal(outer);
    assert!(matches!(out.next(outer), Some(ShellEffect::TitleDirty)));

    let inner = out.open();
    out.push(ShellEffect::FocusLost);
    out.push(ShellEffect::StartWatcher);
    out.seal(inner);
    assert!(matches!(out.next(inner), Some(ShellEffect::FocusLost)));
    assert!(matches!(out.next(inner), Some(ShellEffect::StartWatcher)));
    assert!(out.next(inner).is_none());

    assert!(matches!(out.next(outer), Some(ShellEffect::RedrawNeeded)));
    assert!(out.next(outer).is_none());
}

/// The order of the document fan-out, turns included.
#[test]
fn a_document_change_asks_in_the_order_of_the_fan_out() {
    let mut session = session("start");
    let mut out = ShellEffects::default();
    let frame = out.open();
    let intent = edit(&session);
    session.dispatch(intent, &mut out);
    out.seal(frame);
    let mut asked = Vec::new();
    while let Some(effect) = out.next(frame) {
        asked.push(label(&effect));
    }
    assert_eq!(
        asked,
        [
            "PreviewDocumentChanged",
            "Resume",
            "Delta",
            "PresentationChanged",
            "TitleDirty",
        ]
    );
}

/// Keystrokes reuse the buffer: once it has grown it allocates nothing.
#[test]
fn typing_does_not_grow_the_buffer() {
    let mut session = session("start");
    let mut out = ShellEffects::default();
    let mut seen = Vec::new();
    let mut key = |session: &mut AppSession, out: &mut ShellEffects| {
        let frame = out.open();
        let intent = edit(session);
        session.dispatch(intent, out);
        drain(session, out, frame, &mut seen);
    };
    key(&mut session, &mut out);
    let capacity = out.capacity();
    assert!(capacity > 0);
    for _ in 0..200 {
        key(&mut session, &mut out);
    }
    assert_eq!(out.capacity(), capacity);
}

#[test]
fn focus_loss_ends_the_shells_part_first_and_saves_a_dirty_note() {
    let mut session = session("start");
    let intent = edit(&session);
    run(&mut session, |session, out| session.dispatch(intent, out));

    let seen = run(&mut session, |session, out| {
        session.on_focus_changed(false, out);
    });
    assert_eq!(seen[0], "FocusLost");
    assert!(seen[1].starts_with("正在保存 generation"), "{seen:?}");
    assert_eq!(session.worker.metrics().note_submitted, 1);
    assert!(!session.editor.focused);

    // Gaining focus, or losing it clean, saves nothing.
    let mut clean = super::tests::session("start");
    let seen = run(&mut clean, |session, out| {
        session.on_focus_changed(false, out);
    });
    assert_eq!(seen, ["FocusLost"]);
    assert_eq!(clean.worker.metrics().note_submitted, 0);
    assert!(
        run(&mut clean, |session, out| session
            .on_focus_changed(true, out))
        .is_empty()
    );
}

/// What the session keeps of the window's effects never reaches the shell.
#[test]
fn the_shell_sees_no_save_gc_flush_or_placement() {
    let placement = WindowPlacement::new(300.0, 200.0, None, 0.0, 0.5, 0.5);
    let mut session = session("start");
    for effect in [
        WindowEffect::RequestSafeAssetGc,
        WindowEffect::RequestConfigFlush,
        WindowEffect::CommitPlacement {
            placement,
            dock_edge: None,
        },
    ] {
        let seen = run(&mut session, |_, out| {
            AppSession::route_window_effect(effect, out);
        });
        assert!(seen.is_empty(), "{seen:?}");
    }
    assert_eq!(session.config.current().window.width_dip, 300);
    assert_eq!(session.config.current().window.height_dip, 200);
    assert!(session.assets.sync_in_flight);
}

/// Each barrier of a quit holds `Exit` back until its own receipt arrives.
#[test]
fn exit_appears_only_after_the_last_barrier() {
    let mut session = session("start");
    let intent = edit(&session);
    run(&mut session, |session, out| session.dispatch(intent, out));
    session.config_persistence_allowed = true;
    session.config.update(|config| config.opacity = 50).unwrap();

    let guards = session.guards(ViewFacts::default());
    let seen = run(&mut session, |session, out| {
        session.dispatch_window_intent(WindowIntent::TrayQuitRequested { guards }, out);
    });
    assert_eq!(stage(&session), QuitStage::AwaitingNoteSave);
    assert!(!seen.iter().any(|effect| effect == "Exit"), "{seen:?}");

    let generation = session.coordinator.view().generation;
    let bytes = LoadedDocument::encode_runtime(session.coordinator.view().text, LineEnding::Lf);
    let saved = IoCompletion::Note(NoteReceipt::Save {
        generation,
        mode: PersistMode::Guarded { expected: None },
        result: Ok(PersistResult::Saved {
            generation,
            fingerprint: hash_bytes(&bytes),
            durable_bytes: bytes.len(),
        }),
    });
    let seen = run(&mut session, |session, out| {
        session.handle_io(saved, ViewFacts::default(), out);
    });
    assert_eq!(stage(&session), QuitStage::AwaitingAssetGc);
    assert!(!seen.iter().any(|effect| effect == "Exit"), "{seen:?}");

    let request_id = session.assets.quit_sync_request_id.unwrap();
    let synced = IoCompletion::AssetSync {
        request_id,
        generation: Generation::initial(),
        result: Ok(AssetReconcileReport::default()),
    };
    let seen = run(&mut session, |session, out| {
        session.handle_io(synced, ViewFacts::default(), out);
    });
    assert_eq!(stage(&session), QuitStage::AwaitingConfig);
    assert!(!seen.iter().any(|effect| effect == "Exit"), "{seen:?}");

    let revision = session.config.revision();
    let written = IoCompletion::Config {
        revision,
        result: Ok(()),
    };
    let seen = run(&mut session, |session, out| {
        session.handle_io(written, ViewFacts::default(), out);
    });
    assert_eq!(stage(&session), QuitStage::ReadyToExit);
    assert_eq!(seen, ["Exit"]);
}

/// Hiding reads the guards the shell reports, not a copy from before the step.
#[test]
fn a_barrier_turn_reads_the_view_at_its_turn() {
    let mut session = session("start");
    let dragging = ViewFacts {
        scrollbar_dragging: true,
        ..ViewFacts::default()
    };
    assert!(session.guards(dragging).dragging);
    assert!(!session.guards(ViewFacts::default()).dragging);
    session.window_flow = None;
    assert!(!session.mutation_allowed(false, true));
}

/// The next wake is the earliest of everything that keeps a deadline.
#[test]
fn the_next_deadline_is_the_earliest_of_all() {
    let mut session = session("start");
    assert_eq!(session.next_deadline(), None);
    session.zoom_config_deadline = Some(9_000);
    assert_eq!(session.next_deadline(), Some(9_000));
    let intent = edit(&session);
    run(&mut session, |session, out| session.dispatch(intent, out));
    let autosave = session.persistence.autosave_deadline().unwrap();
    assert_eq!(session.next_deadline(), Some(autosave.min(9_000)));
}
