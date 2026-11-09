//! One row per note receipt arm, through the real completion handler and, where a request leaves
//! the shell, the real worker and disk. Phase 15 B moves these decisions out of the shell; this
//! table runs after every batch so a moved branch cannot change what it does.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use stickymd_app::flow::window::state::{
    DockEdge, LifecycleState, QuitStage, ShowReason, StableVisibility, VisibilityState,
    WindowIntent,
};
use stickymd_core::{
    ExternalFileState, Generation, LineEnding, LoadedDocument, PreserveExpectation, hash_bytes,
};

use crate::shell::headless_tests::{
    HeadlessApp, external, fixture, offer_recovery, portable_storage, receipt_fixture, type_letter,
    window_flow,
};
use crate::shell::receipt_tests::recovering;
use crate::shell::storage_conformance::{
    FORCED, GUARDED, Outcome, deliver, enter_conflict, next, observed, press, saved_mode, saving,
    settled,
};
use stickymd_app::execution::IoCompletion;
use stickymd_app::ports::{
    FailureKind, NoteReceipt, PersistMode, PersistResult, PreserveOutcome, StorageFailure,
    TemporaryCleanup,
};

const UNSTABLE: &str = "note changed while it was being observed";
const CONFLICT: &str = "文件已在外部修改  |  [F6 载入外部]  [F7 保留本地]";

fn unstable() -> StorageFailure {
    StorageFailure::new(FailureKind::Unavailable, UNSTABLE)
}

fn note(
    generation: Generation,
    mode: PersistMode,
    result: Result<PersistResult, StorageFailure>,
) -> IoCompletion {
    IoCompletion::Note(NoteReceipt::Save {
        generation,
        mode,
        result,
    })
}

/// The receipt of a save of `generation` that wrote the current text.
fn saved_as(app: &HeadlessApp, generation: Generation, mode: PersistMode) -> IoCompletion {
    let bytes =
        LoadedDocument::encode_runtime(&app.app.coordinator.snapshot().text, LineEnding::Lf);
    note(
        generation,
        mode,
        Ok(PersistResult::Saved {
            generation,
            fingerprint: hash_bytes(&bytes),
            durable_bytes: bytes.len(),
        }),
    )
}

pub(super) fn saved(app: &HeadlessApp, mode: PersistMode) -> IoCompletion {
    saved_as(app, app.app.coordinator.view().generation, mode)
}

fn failed(app: &HeadlessApp, mode: PersistMode) -> IoCompletion {
    note(app.app.coordinator.view().generation, mode, Err(unstable()))
}

fn conflicted(app: &HeadlessApp, mode: PersistMode) -> IoCompletion {
    let generation = app.app.coordinator.view().generation;
    note(
        generation,
        mode,
        Ok(PersistResult::Conflict {
            generation,
            observed: ExternalFileState::Present(external("outside")),
        }),
    )
}

/// A result claiming another generation than the one submitted.
fn mismatched(app: &HeadlessApp) -> IoCompletion {
    let submitted = app.app.coordinator.view().generation;
    note(
        submitted,
        GUARDED,
        Ok(PersistResult::Saved {
            generation: submitted.checked_next().unwrap(),
            fingerprint: hash_bytes(b"other"),
            durable_bytes: 5,
        }),
    )
}

/// A consistent receipt for a generation the document never reached.
fn from_the_future(app: &HeadlessApp) -> IoCompletion {
    let future = app
        .app
        .coordinator
        .view()
        .generation
        .checked_next()
        .unwrap();
    saved_as(app, future, GUARDED)
}

fn edited() -> HeadlessApp {
    let mut app = fixture("start");
    type_letter(&mut app);
    app
}

fn in_conflict() -> HeadlessApp {
    let mut app = edited();
    enter_conflict(&mut app);
    app
}

/// A conflict whose "keep local" save is in flight behind an older one.
fn keeping_local() -> HeadlessApp {
    let mut app = in_conflict();
    press(&mut app, false);
    assert!(app.app.persistence.keep_local_in_progress());
    app
}

fn restoring(preserve: Option<PreserveExpectation>) -> HeadlessApp {
    let mut app = recovering(preserve);
    app.app.recovery.begin_restore().unwrap();
    app
}

#[test]
fn saved_clears_the_edit() {
    let mut app = edited();
    let receipt = saved(&app, GUARDED);
    assert_eq!(deliver(&mut app, receipt), settled("已保存"));
}

#[test]
fn a_mismatched_result_generation_saves_nothing_and_ends_the_operations() {
    let mut app = edited();
    let submitted = app.app.coordinator.view().generation.value();
    let result = submitted + 1;
    let receipt = mismatched(&app);
    assert_eq!(
        deliver(&mut app, receipt),
        Outcome {
            dirty: true,
            ..settled(format!(
                "保存回执 generation 不一致：submitted={submitted} result={result}"
            ))
        }
    );

    let mut app = restoring(None);
    let receipt = mismatched(&app);
    assert!(!deliver(&mut app, receipt).operation_pending);

    let mut app = keeping_local();
    let receipt = mismatched(&app);
    let outcome = deliver(&mut app, receipt);
    assert!(!outcome.keep_local);
    assert!(outcome.conflict);
}

#[test]
fn a_save_conflict_enters_the_conflict() {
    let mut app = edited();
    let receipt = conflicted(&app, GUARDED);
    assert_eq!(
        deliver(&mut app, receipt),
        Outcome {
            conflict: true,
            dirty: true,
            ..settled(CONFLICT)
        }
    );
}

#[test]
fn a_save_conflict_during_recovery_asks_for_a_new_choice() {
    let mut app = restoring(None);
    let receipt = conflicted(&app, GUARDED);
    assert_eq!(
        deliver(&mut app, receipt),
        Outcome {
            recovery_pending: true,
            ..settled("恢复等待期间 note.md 已变化，未覆盖外部内容；请重新选择 F6/F7。")
        }
    );
}

#[test]
fn a_failed_save_keeps_the_text_and_says_so() {
    let mut app = edited();
    let generation = app.app.coordinator.view().generation.value();
    let receipt = failed(&app, GUARDED);
    assert_eq!(
        deliver(&mut app, receipt),
        Outcome {
            dirty: true,
            ..settled(format!(
                "保存 generation {generation} 失败；内存文本仍保留。Ctrl+S 重试：{UNSTABLE}"
            ))
        }
    );
}

#[test]
fn a_failed_save_ends_a_recovery_operation_but_not_the_candidate() {
    let mut app = restoring(None);
    let receipt = failed(&app, GUARDED);
    let outcome = deliver(&mut app, receipt);
    assert!(outcome.recovery_pending);
    assert!(!outcome.operation_pending);
}

/// Keep local must be satisfied by a forced write, so any older receipt submits one and the
/// status line ends on that save.
#[test]
fn an_older_receipt_during_keep_local_submits_the_forced_save() {
    type Receipt = fn(&HeadlessApp, PersistMode) -> IoCompletion;
    let rows: [(Receipt, bool); 3] = [(saved, false), (conflicted, true), (failed, true)];
    for (receipt, dirty) in rows {
        let mut app = keeping_local();
        let completion = receipt(&app, GUARDED);
        let expected = saving(&app);
        assert_eq!(
            deliver(&mut app, completion),
            Outcome {
                saves: 1,
                conflict: true,
                dirty,
                keep_local: true,
                ..settled(expected)
            }
        );
    }
}

#[test]
fn a_forced_receipt_ends_keep_local() {
    let mut app = keeping_local();
    let receipt = saved(&app, FORCED);
    assert_eq!(
        deliver(&mut app, receipt),
        settled("已保留本地内容并覆盖外部文件。")
    );

    let mut app = keeping_local();
    let generation = app.app.coordinator.view().generation.value();
    let receipt = failed(&app, FORCED);
    assert_eq!(
        deliver(&mut app, receipt),
        Outcome {
            conflict: true,
            dirty: true,
            ..settled(format!(
                "保存 generation {generation} 失败；内存文本仍保留。Ctrl+S 重试：{UNSTABLE}"
            ))
        }
    );
}

/// Only a broken protocol reaches this; it counts as a failed save (see
/// `the_note_barriers_settle_by_receipt`).
#[test]
fn a_receipt_for_an_unreached_generation_is_reported() {
    let mut app = edited();
    let receipt = from_the_future(&app);
    assert_eq!(
        deliver(&mut app, receipt),
        Outcome {
            dirty: true,
            ..settled("保存回执无效：persisted generation is ahead of the current document")
        }
    );
}

/// A recreate requested during another save stays owed after that save: the durable path is
/// inspected before deciding whether a second write is needed.
#[test]
fn a_saved_receipt_inspects_the_disk_while_a_recreate_is_still_owed() {
    use stickymd_app::flow::SaveTrigger;
    let (mut app, receipts) = receipt_fixture("start");
    type_letter(&mut app);
    app.app
        .persistence
        .note_save_submitted(SaveTrigger::Debounce);
    app.app
        .persistence
        .note_save_submitted(SaveTrigger::RecreateMissing);
    let receipt = saved(&app, GUARDED);
    app.handle_io_completion(receipt);
    assert!(app.app.persistence.durability_required());
    let IoCompletion::Note(NoteReceipt::External { requested_base, .. }) = next(&receipts) else {
        panic!("expected a durable inspection");
    };
    assert_eq!(requested_base, app.app.coordinator.view().base_disk_hash);
}

#[test]
fn a_failed_observation_changes_nothing_and_says_so() {
    let mut app = edited();
    let requested_base = app.app.coordinator.view().base_disk_hash;
    let outcome = deliver(
        &mut app,
        IoCompletion::Note(NoteReceipt::External {
            requested_base,
            result: Err(unstable()),
        }),
    );
    assert_eq!(
        outcome,
        Outcome {
            dirty: true,
            ..settled(format!("外部文件读取失败，未改变当前文本：{UNSTABLE}"))
        }
    );
}

#[test]
fn an_external_edit_reloads_a_clean_document() {
    let mut app = fixture("start");
    let receipt = observed(&app, ExternalFileState::Present(external("outside")));
    assert_eq!(deliver(&mut app, receipt), settled("已载入外部修改。"));
    assert_eq!(app.app.coordinator.view().text, "outside");
}

#[test]
fn a_missing_note_is_recreated_guarded_on_absence() {
    let (mut app, receipts) = receipt_fixture("start");
    let receipt = observed(&app, ExternalFileState::Missing);
    let expected = saving(&app);
    assert_eq!(
        deliver(&mut app, receipt),
        Outcome {
            saves: 1,
            ..settled(expected)
        }
    );
    assert_eq!(saved_mode(&next(&receipts)), GUARDED);
    assert!(app.app.persistence.durability_required());
}

#[test]
fn every_conflicting_external_state_names_its_way_out() {
    let rows = [
        (
            ExternalFileState::Present(external("outside")),
            CONFLICT.into(),
        ),
        (
            ExternalFileState::InvalidUtf8 {
                fingerprint: hash_bytes(b"\xff"),
            },
            "外部文件不是有效 UTF-8；F7 保留本地覆盖。".to_string(),
        ),
        (
            ExternalFileState::TooLarge { bytes: 7 },
            "外部 note.md 过大（7 bytes）；F7 保留本地覆盖。".into(),
        ),
    ];
    for (state, diagnostic) in rows {
        let mut app = edited();
        let receipt = observed(&app, state);
        assert_eq!(
            deliver(&mut app, receipt),
            Outcome {
                conflict: true,
                dirty: true,
                ..settled(diagnostic)
            }
        );
    }
}

/// The known file coming back ends the conflict; the edit it held back is then saved, and the
/// status line ends on that save.
#[test]
fn the_known_file_returning_ends_the_conflict_and_saves_the_edit() {
    let mut app = edited();
    let known = app.app.coordinator.view().text.to_string();
    let receipt = saved(&app, GUARDED);
    app.handle_io_completion(receipt);
    type_letter(&mut app);
    enter_conflict(&mut app);
    let receipt = observed(&app, ExternalFileState::Present(external(&known)));
    let expected = saving(&app);
    assert_eq!(
        deliver(&mut app, receipt),
        Outcome {
            saves: 1,
            dirty: true,
            ..settled(expected)
        }
    );
}

#[test]
fn an_observation_during_recovery_changes_nothing() {
    let mut app = recovering(None);
    let receipt = observed(&app, ExternalFileState::Present(external("outside")));
    let outcome = deliver(&mut app, receipt);
    assert_eq!(outcome.diagnostic, None);
    assert!(!outcome.conflict);
    assert_eq!(app.app.coordinator.view().text, "disk");
}

#[test]
fn a_failed_recovery_cleanup_keeps_the_recovery_open() {
    let mut app = recovering(None);
    app.app.recovery.begin_use_canonical().unwrap();
    let operation = app.app.recovery.operation_id().unwrap();
    let outcome = deliver(
        &mut app,
        IoCompletion::Note(NoteReceipt::TemporaryRemoved {
            purpose: TemporaryCleanup::RecoveryResolved { operation },
            result: Err(unstable()),
        }),
    );
    assert_eq!(
        outcome,
        Outcome {
            recovery_pending: true,
            ..settled(format!("临时恢复证据无法清理，仍保持恢复状态：{UNSTABLE}"))
        }
    );
}

#[test]
fn a_conflict_discard_receipt_reports_only_a_failure() {
    let mut app = fixture("start");
    let outcome = deliver(
        &mut app,
        IoCompletion::Note(NoteReceipt::TemporaryRemoved {
            purpose: TemporaryCleanup::ConflictDiscarded,
            result: Ok(()),
        }),
    );
    assert_eq!(outcome.diagnostic, None);
    let outcome = deliver(
        &mut app,
        IoCompletion::Note(NoteReceipt::TemporaryRemoved {
            purpose: TemporaryCleanup::ConflictDiscarded,
            result: Err(unstable()),
        }),
    );
    assert_eq!(
        outcome,
        settled(format!(
            "外部内容已载入，但本地临时副本无法清理；下次启动可能再次提示恢复：{UNSTABLE}"
        ))
    );
}

#[test]
fn a_failed_preservation_stops_the_restore() {
    let mut app = restoring(Some(PreserveExpectation::Content(hash_bytes(b"\xff"))));
    let operation = app.app.recovery.operation_id().unwrap();
    let outcome = deliver(
        &mut app,
        IoCompletion::Note(NoteReceipt::CanonicalPreserved {
            operation,
            result: Err(unstable()),
        }),
    );
    assert_eq!(
        outcome,
        Outcome {
            recovery_pending: true,
            ..settled(format!(
                "无效的 canonical note.md 无法先行保留；未执行恢复覆盖：{UNSTABLE}"
            ))
        }
    );
}

#[test]
fn a_preserved_canonical_starts_the_save_guarded_by_the_restore_plan() {
    let (mut app, receipts) = receipt_fixture("disk");
    offer_recovery(
        &mut app,
        Some("disk"),
        Some(PreserveExpectation::Content(hash_bytes(b"\xff"))),
    );
    app.app.recovery.begin_restore().unwrap();
    let operation = app.app.recovery.operation_id().unwrap();
    let expected = saving(&app);
    let outcome = deliver(
        &mut app,
        IoCompletion::Note(NoteReceipt::CanonicalPreserved {
            operation,
            result: Ok(PreserveOutcome::Preserved),
        }),
    );
    assert_eq!(
        outcome,
        Outcome {
            saves: 1,
            recovery_pending: true,
            operation_pending: true,
            ..settled(expected)
        }
    );
    assert_eq!(
        saved_mode(&next(&receipts)),
        PersistMode::Guarded {
            expected: Some(hash_bytes(b"disk"))
        }
    );
}

#[test]
fn the_keys_do_nothing_without_a_recovery_or_a_conflict() {
    let mut app = fixture("start");
    assert!(!app.handle_resolution_key(true));
    assert!(!app.handle_resolution_key(false));
    assert_eq!(app.diagnostic, None);
}

#[test]
fn a_recovery_operation_in_progress_refuses_the_keys() {
    let mut app = restoring(None);
    assert_eq!(
        press(&mut app, true),
        Outcome {
            recovery_pending: true,
            operation_pending: true,
            ..settled("恢复操作正在完成，请稍候。")
        }
    );
}

// The rows that read or write the note's files run on every storage (`storage_conformance.rs`).
crate::testing::storage_conformance!(portable_storage);

#[test]
fn an_unusable_canonical_cannot_be_used() {
    let mut app = recovering(Some(PreserveExpectation::Unverifiable));
    assert_eq!(
        press(&mut app, false),
        Outcome {
            recovery_pending: true,
            ..settled("当前 note.md 无法载入（不是有效 UTF-8 或过大）；只能 F6 恢复有效临时内容。")
        }
    );
    assert_eq!(app.app.coordinator.view().text, "disk");
}

#[test]
fn loading_the_external_side_discards_the_temporary() {
    let (mut app, receipts) = receipt_fixture("start");
    type_letter(&mut app);
    enter_conflict(&mut app);
    assert_eq!(
        press(&mut app, true),
        settled("已载入最新外部内容；撤销历史已清空。")
    );
    assert_eq!(app.app.coordinator.view().text, "outside");
    let cleanup = next(&receipts);
    assert!(
        matches!(
            cleanup,
            IoCompletion::Note(NoteReceipt::TemporaryRemoved {
                purpose: TemporaryCleanup::ConflictDiscarded,
                result: Ok(()),
            })
        ),
        "{cleanup:?}"
    );
}

#[test]
fn an_unloadable_external_side_leaves_only_keep_local() {
    let mut app = edited();
    let receipt = observed(
        &app,
        ExternalFileState::InvalidUtf8 {
            fingerprint: hash_bytes(b"\xff"),
        },
    );
    app.handle_io_completion(receipt);
    assert_eq!(
        press(&mut app, true),
        Outcome {
            conflict: true,
            dirty: true,
            ..settled("外部文件不可载入；只能 F7 保留本地覆盖。")
        }
    );
}

#[derive(Debug, PartialEq, Eq)]
enum Hide {
    Waiting,
    Cancelled,
    Hidden,
}

fn hide_state(app: &HeadlessApp) -> Hide {
    match app.app.window_flow.as_ref().unwrap().state().visibility() {
        VisibilityState::AwaitingHideSave { .. } => Hide::Waiting,
        VisibilityState::Presented(_) => Hide::Cancelled,
        VisibilityState::HiddenToTray { .. } => Hide::Hidden,
        other => panic!("unexpected visibility {other:?}"),
    }
}

fn request_hide(app: &mut HeadlessApp) {
    let guards = app.window_guards();
    app.dispatch_window_intent(WindowIntent::CloseRequested { now_ms: 1, guards });
    assert_eq!(hide_state(app), Hide::Waiting);
}

fn lifecycle(app: &HeadlessApp) -> LifecycleState {
    app.app.window_flow.as_ref().unwrap().state().lifecycle()
}

fn request_quit(app: &mut HeadlessApp) {
    let guards = app.window_guards();
    app.dispatch_window_intent(WindowIntent::TrayQuitRequested { guards });
    assert_eq!(
        lifecycle(app),
        LifecycleState::Quitting(QuitStage::AwaitingNoteSave)
    );
}

/// Which receipt settles the hide and quit barriers, and how.
#[test]
fn the_note_barriers_settle_by_receipt() {
    use LifecycleState::{Quitting, Running};
    type Receipt = fn(&HeadlessApp) -> IoCompletion;
    let rows: [(Receipt, Hide, LifecycleState); 5] = [
        (
            |app| saved(app, GUARDED),
            Hide::Hidden,
            Quitting(QuitStage::AwaitingAssetGc),
        ),
        (|app| failed(app, GUARDED), Hide::Cancelled, Running),
        (|app| conflicted(app, GUARDED), Hide::Cancelled, Running),
        (mismatched, Hide::Cancelled, Running),
        (from_the_future, Hide::Cancelled, Running),
    ];
    for (receipt, hide, quit) in rows {
        let mut app = edited();
        request_hide(&mut app);
        let completion = receipt(&app);
        app.handle_io_completion(completion);
        assert_eq!(hide_state(&app), hide);

        let mut app = edited();
        request_quit(&mut app);
        let completion = receipt(&app);
        app.handle_io_completion(completion);
        assert_eq!(lifecycle(&app), quit);
        assert_eq!(app.quit_notice.active(), quit == Running);
    }
}

/// An older receipt that hands over to the forced save is not the end of that save.
#[test]
fn a_superseded_keep_local_save_holds_the_barrier_until_the_forced_receipt() {
    let mut app = edited();
    request_hide(&mut app);
    enter_conflict(&mut app);
    press(&mut app, false);
    let receipt = saved(&app, GUARDED);
    app.handle_io_completion(receipt);
    assert_eq!(hide_state(&app), Hide::Waiting);
    let receipt = saved(&app, FORCED);
    app.handle_io_completion(receipt);
    assert_eq!(hide_state(&app), Hide::Hidden);
}

fn applied_effects(app: &mut HeadlessApp) -> Vec<String> {
    let effects = std::mem::take(&mut app.applied_window_effects);
    effects
        .into_iter()
        .map(|effect| {
            if effect.starts_with("ApplyFrame") {
                "ApplyFrame".into()
            } else {
                effect
            }
        })
        .collect()
}

/// The order the native window sees: input goes first, the window hides, and only then are the
/// caches released; showing places the window before it appears.
#[test]
fn closing_and_showing_reach_the_window_in_order() {
    let mut app = fixture("start");
    let guards = app.window_guards();
    app.dispatch_window_intent(WindowIntent::CloseRequested { now_ms: 1, guards });
    assert_eq!(hide_state(&app), Hide::Hidden);
    assert_eq!(
        applied_effects(&mut app),
        [
            "CancelImePreedit",
            "SetImeAllowed(false)",
            "SetEditorInputEnabled(false)",
            "HideWindow { id: 1 }",
            "ReleaseHiddenCaches",
        ]
    );
    app.dispatch_window_intent(WindowIntent::ShowRequested {
        reason: ShowReason::Tray,
        now_ms: 2,
    });
    assert_eq!(
        applied_effects(&mut app),
        [
            "ApplyFrame",
            "ShowWindow { id: 2 }",
            "WindowShown",
            "FocusWindow { id: 4 }",
            "SetImeAllowed(true)",
            "SetEditorInputEnabled(true)",
            "RequestRedraw",
        ]
    );
}

/// A rejected hide reaches the user, and nothing that depends on a hidden window happens.
#[test]
fn a_rejected_hide_keeps_the_window_shown_through_the_shell() {
    let rejected = [
        (FailureKind::Failed, "Failed"),
        (FailureKind::Unavailable, "Unavailable"),
    ];
    for (outcome, kind) in rejected {
        let mut app = fixture("start");
        app.host.hide_result = Err(outcome);
        let guards = app.window_guards();
        app.dispatch_window_intent(WindowIntent::CloseRequested { now_ms: 1, guards });
        assert_eq!(hide_state(&app), Hide::Cancelled);
        assert!(app.shell_input_enabled);
        let effects = applied_effects(&mut app);
        assert!(!effects.iter().any(|effect| effect == "ReleaseHiddenCaches"));
        assert_eq!(
            app.diagnostic.as_deref(),
            Some(format!("窗口未能隐藏，保持显示（{kind}）。").as_str())
        );
    }
}

fn docked(app: &mut HeadlessApp) {
    let edge = StableVisibility::DockedExpanded(DockEdge::Left);
    app.app.window_flow = Some(window_flow(edge));
}

fn auto_collapse_armed(app: &HeadlessApp) -> bool {
    let flow = app.app.window_flow.as_ref().unwrap();
    flow.state().next_deadline_ms().is_some()
}

/// A quit cancelled by the save that also failed a hide: the timer the quit held back starts.
#[test]
fn a_quit_cancelled_with_a_failed_hide_restarts_the_auto_collapse() {
    let mut app = edited();
    docked(&mut app);
    request_hide(&mut app);
    let guards = app.window_guards();
    app.dispatch_window_intent(WindowIntent::TrayQuitRequested { guards });
    assert!(lifecycle(&app).is_quitting());
    let receipt = failed(&app, GUARDED);
    app.handle_io_completion(receipt);
    assert_eq!(lifecycle(&app), LifecycleState::Running);
    assert_eq!(hide_state(&app), Hide::Cancelled);
    assert!(auto_collapse_armed(&app));

    let mut app = edited();
    docked(&mut app);
    request_hide(&mut app);
    let guards = app.window_guards();
    app.dispatch_window_intent(WindowIntent::TrayQuitRequested { guards });
    app.handle_io_completion(IoCompletion::WorkerStopped);
    assert_eq!(lifecycle(&app), LifecycleState::Running);
    assert!(auto_collapse_armed(&app));
}

/// Hiding closes the opacity popup. A rejected hide must not keep waiting on the popup it closed.
#[test]
fn a_rejected_hide_does_not_stay_blocked_by_the_popup_it_closed() {
    let mut app = fixture("start");
    docked(&mut app);
    app.controls.opacity_popup_open = true;
    app.host.hide_result = Err(FailureKind::Failed);
    let guards = app.window_guards();
    assert!(guards.popup_open);
    app.dispatch_window_intent(WindowIntent::CloseRequested { now_ms: 1, guards });
    assert_eq!(hide_state(&app), Hide::Cancelled);
    assert!(!app.controls.opacity_popup_open);
    assert!(auto_collapse_armed(&app));
}
