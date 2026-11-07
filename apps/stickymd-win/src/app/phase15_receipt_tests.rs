//! One row per note receipt arm, through the real completion handler and, where a request leaves
//! the shell, the real worker and disk. Phase 15 B moves these decisions out of the shell; this
//! table runs after every batch so a moved branch cannot change what it does.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use std::fs;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use stickymd_app::flow::window::state::{
    DockEdge, LifecycleState, QuitStage, ShowReason, StableVisibility, VisibilityState,
    WindowIntent,
};
use stickymd_core::{
    ExternalFileState, Generation, LineEnding, LoadedDocument, PreserveExpectation, hash_bytes,
};

use crate::app::headless_tests::{HeadlessApp, fixture, receipt_fixture, window_flow};
use crate::app::receipt_tests::{external, offer_recovery, recovering, saves, type_letter};
use stickymd_app::execution::IoCompletion;
use stickymd_app::ports::{
    FailureKind, NoteReceipt, PersistMode, PersistResult, PreserveOutcome, StorageFailure,
    TemporaryCleanup,
};

const GUARDED: PersistMode = PersistMode::Guarded { expected: None };
const FORCED: PersistMode = PersistMode::ForceOverwrite;
const UNSTABLE: &str = "note changed while it was being observed";
const CONFLICT: &str = "文件已在外部修改  |  [F6 载入外部]  [F7 保留本地]";
const RECOVERY_DONE: &str = "恢复选择已完成；正在核对受管图片后启用编辑。";
const RECEIPT_WAIT: Duration = Duration::from_secs(10);

/// What a row pins: the status line, the saves this step handed to the worker, and the facts the
/// title and the window guards read.
#[derive(Debug, PartialEq, Eq)]
struct Outcome {
    diagnostic: Option<String>,
    saves: u64,
    recovery_pending: bool,
    operation_pending: bool,
    conflict: bool,
    dirty: bool,
    keep_local: bool,
}

fn settled(diagnostic: impl Into<String>) -> Outcome {
    Outcome {
        diagnostic: Some(diagnostic.into()),
        saves: 0,
        recovery_pending: false,
        operation_pending: false,
        conflict: false,
        dirty: false,
        keep_local: false,
    }
}

fn unstable() -> StorageFailure {
    StorageFailure::new(FailureKind::Unavailable, UNSTABLE)
}

fn saving(app: &HeadlessApp) -> String {
    format!(
        "正在保存 generation {}…",
        app.coordinator.view().generation.value()
    )
}

fn step(app: &mut HeadlessApp, action: impl FnOnce(&mut HeadlessApp)) -> Outcome {
    let before = saves(app);
    action(app);
    Outcome {
        diagnostic: app.diagnostic.clone(),
        saves: saves(app) - before,
        recovery_pending: app.recovery.is_pending(),
        operation_pending: app.recovery.operation_pending(),
        conflict: app.persistence.conflict().is_some(),
        dirty: app.coordinator.view().dirty,
        keep_local: app.persistence.keep_local_in_progress(),
    }
}

fn deliver(app: &mut HeadlessApp, receipt: IoCompletion) -> Outcome {
    step(app, |app| app.handle_io_completion(None, receipt))
}

fn press(app: &mut HeadlessApp, primary: bool) -> Outcome {
    step(app, |app| assert!(app.handle_resolution_key(None, primary)))
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
    let bytes = LoadedDocument::encode_runtime(&app.coordinator.snapshot().text, LineEnding::Lf);
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

fn saved(app: &HeadlessApp, mode: PersistMode) -> IoCompletion {
    saved_as(app, app.coordinator.view().generation, mode)
}

fn failed(app: &HeadlessApp, mode: PersistMode) -> IoCompletion {
    note(app.coordinator.view().generation, mode, Err(unstable()))
}

fn conflicted(app: &HeadlessApp, mode: PersistMode) -> IoCompletion {
    let generation = app.coordinator.view().generation;
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
    let submitted = app.coordinator.view().generation;
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
    let future = app.coordinator.view().generation.checked_next().unwrap();
    saved_as(app, future, GUARDED)
}

fn observed(app: &HeadlessApp, state: ExternalFileState) -> IoCompletion {
    IoCompletion::Note(NoteReceipt::External {
        requested_base: app.coordinator.view().base_disk_hash,
        result: Ok(state),
    })
}

fn edited() -> HeadlessApp {
    let mut app = fixture("start");
    type_letter(&mut app);
    app
}

fn enter_conflict(app: &mut HeadlessApp) {
    let receipt = observed(app, ExternalFileState::Present(external("outside")));
    app.handle_io_completion(None, receipt);
    assert!(app.persistence.conflict().is_some());
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
    assert!(app.persistence.keep_local_in_progress());
    app
}

fn restoring(preserve: Option<PreserveExpectation>) -> HeadlessApp {
    let mut app = recovering(preserve);
    app.recovery.begin_restore().unwrap();
    app
}

/// The next note receipt; asset convergence runs beside these flows and is not pinned here.
fn next(receipts: &Receiver<IoCompletion>) -> IoCompletion {
    loop {
        match receipts
            .recv_timeout(RECEIPT_WAIT)
            .expect("a worker receipt")
        {
            IoCompletion::AssetSync { .. } => {}
            receipt => return receipt,
        }
    }
}

fn saved_mode(receipt: &IoCompletion) -> PersistMode {
    match receipt {
        IoCompletion::Note(NoteReceipt::Save { mode, .. }) => *mode,
        other => panic!("expected a save receipt, got {other:?}"),
    }
}

fn is_recovery_cleanup(receipt: &IoCompletion, expected: u64) -> bool {
    matches!(
        receipt,
        IoCompletion::Note(NoteReceipt::TemporaryRemoved {
            purpose: TemporaryCleanup::RecoveryResolved { operation },
            result: Ok(()),
        }) if *operation == expected
    )
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
    let submitted = app.coordinator.view().generation.value();
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
    let generation = app.coordinator.view().generation.value();
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
    let generation = app.coordinator.view().generation.value();
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

/// Pinned as it is: this path reports the receipt and leaves the barriers unsettled (see
/// `the_note_barriers_settle_by_receipt`). A fix is a separate, behaviour-changing commit.
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
    app.persistence.note_save_submitted(SaveTrigger::Debounce);
    app.persistence
        .note_save_submitted(SaveTrigger::RecreateMissing);
    let receipt = saved(&app, GUARDED);
    app.handle_io_completion(None, receipt);
    assert!(app.persistence.durability_required());
    let IoCompletion::Note(NoteReceipt::External { requested_base, .. }) = next(&receipts) else {
        panic!("expected a durable inspection");
    };
    assert_eq!(requested_base, app.coordinator.view().base_disk_hash);
}

#[test]
fn a_failed_observation_changes_nothing_and_says_so() {
    let mut app = edited();
    let requested_base = app.coordinator.view().base_disk_hash;
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
    assert_eq!(app.coordinator.view().text, "outside");
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
    assert!(app.persistence.durability_required());
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
    let known = app.coordinator.view().text.to_string();
    let receipt = saved(&app, GUARDED);
    app.handle_io_completion(None, receipt);
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
    assert_eq!(app.coordinator.view().text, "disk");
}

#[test]
fn a_failed_recovery_cleanup_keeps_the_recovery_open() {
    let mut app = recovering(None);
    app.recovery.begin_use_canonical().unwrap();
    let operation = app.recovery.operation_id().unwrap();
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
    let operation = app.recovery.operation_id().unwrap();
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
    app.recovery.begin_restore().unwrap();
    let operation = app.recovery.operation_id().unwrap();
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
    assert!(!app.handle_resolution_key(None, true));
    assert!(!app.handle_resolution_key(None, false));
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

#[test]
fn restoring_publishes_the_temporary_and_then_removes_the_evidence() {
    let (mut app, receipts) = receipt_fixture("disk");
    fs::write(&app.paths.note_file, "disk").unwrap();
    fs::write(&app.paths.note_tmp, "temporary").unwrap();
    offer_recovery(&mut app, Some("disk"), None);
    let outcome = press(&mut app, true);
    assert_eq!(app.coordinator.view().text, "temporary");
    assert_eq!(outcome.saves, 1);
    assert!(outcome.operation_pending);
    let operation = app.recovery.operation_id().unwrap();
    let save = next(&receipts);
    assert_eq!(
        saved_mode(&save),
        PersistMode::Guarded {
            expected: Some(hash_bytes(b"disk"))
        }
    );
    let diagnostic = app.diagnostic.clone();
    let outcome = deliver(&mut app, save);
    assert_eq!(outcome.diagnostic, diagnostic);
    assert!(outcome.recovery_pending);
    let cleanup = next(&receipts);
    assert!(is_recovery_cleanup(&cleanup, operation), "{cleanup:?}");
    assert_eq!(deliver(&mut app, cleanup), settled(RECOVERY_DONE));
    assert_eq!(fs::read(&app.paths.note_file).unwrap(), b"temporary");
    assert!(!app.paths.note_tmp.exists());
}

#[test]
fn restoring_over_an_invalid_canonical_moves_it_aside_first() {
    let (mut app, receipts) = receipt_fixture("");
    fs::write(&app.paths.note_file, b"\xff").unwrap();
    fs::write(&app.paths.note_tmp, "temporary").unwrap();
    let expected = PreserveExpectation::Content(hash_bytes(b"\xff"));
    offer_recovery(&mut app, None, Some(expected));
    let outcome = press(&mut app, true);
    assert_eq!(outcome.saves, 0);
    let operation = app.recovery.operation_id().unwrap();
    let preserved = next(&receipts);
    assert!(
        matches!(
            &preserved,
            IoCompletion::Note(NoteReceipt::CanonicalPreserved {
                operation: seen,
                result: Ok(PreserveOutcome::Preserved),
            }) if *seen == operation
        ),
        "{preserved:?}"
    );
    assert_eq!(deliver(&mut app, preserved).saves, 1);
    let save = next(&receipts);
    assert_eq!(saved_mode(&save), GUARDED);
    app.handle_io_completion(None, save);
    let cleanup = next(&receipts);
    assert!(is_recovery_cleanup(&cleanup, operation), "{cleanup:?}");
    assert_eq!(deliver(&mut app, cleanup), settled(RECOVERY_DONE));
    assert_eq!(fs::read(&app.paths.note_file).unwrap(), b"temporary");
    let moved: Vec<_> = fs::read_dir(&app.paths.note_dir)
        .unwrap()
        .map(|entry| entry.unwrap())
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with("note.invalid-")
        })
        .collect();
    assert_eq!(moved.len(), 1);
    assert_eq!(fs::read(moved[0].path()).unwrap(), b"\xff");
}

#[test]
fn using_the_canonical_removes_the_temporary_without_a_save() {
    let (mut app, receipts) = receipt_fixture("local");
    fs::write(&app.paths.note_file, "disk").unwrap();
    fs::write(&app.paths.note_tmp, "temporary").unwrap();
    offer_recovery(&mut app, Some("disk"), None);
    assert_eq!(
        press(&mut app, false),
        Outcome {
            recovery_pending: true,
            operation_pending: true,
            ..settled("正在确认使用当前 note.md…")
        }
    );
    assert_eq!(app.coordinator.view().text, "disk");
    let operation = app.recovery.operation_id().unwrap();
    let cleanup = next(&receipts);
    assert!(is_recovery_cleanup(&cleanup, operation), "{cleanup:?}");
    assert_eq!(deliver(&mut app, cleanup), settled(RECOVERY_DONE));
    assert_eq!(fs::read(&app.paths.note_file).unwrap(), b"disk");
    assert!(!app.paths.note_tmp.exists());
}

/// The temporary is the only durable evidence until an empty canonical note exists.
#[test]
fn using_a_missing_canonical_publishes_it_before_removing_the_temporary() {
    let (mut app, receipts) = receipt_fixture("local");
    fs::write(&app.paths.note_tmp, "temporary").unwrap();
    offer_recovery(&mut app, None, None);
    assert_eq!(press(&mut app, false).saves, 1);
    assert_eq!(app.coordinator.view().text, "");
    let operation = app.recovery.operation_id().unwrap();
    let save = next(&receipts);
    assert_eq!(saved_mode(&save), GUARDED);
    app.handle_io_completion(None, save);
    let cleanup = next(&receipts);
    assert!(is_recovery_cleanup(&cleanup, operation), "{cleanup:?}");
    assert_eq!(deliver(&mut app, cleanup), settled(RECOVERY_DONE));
    assert_eq!(fs::read(&app.paths.note_file).unwrap(), b"");
    assert!(!app.paths.note_tmp.exists());
}

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
    assert_eq!(app.coordinator.view().text, "disk");
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
    assert_eq!(app.coordinator.view().text, "outside");
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
    app.handle_io_completion(None, receipt);
    assert_eq!(
        press(&mut app, true),
        Outcome {
            conflict: true,
            dirty: true,
            ..settled("外部文件不可载入；只能 F7 保留本地覆盖。")
        }
    );
}

#[test]
fn keeping_local_forces_the_save_and_refuses_a_second_choice() {
    let (mut app, receipts) = receipt_fixture("start");
    fs::write(&app.paths.note_file, "outside").unwrap();
    type_letter(&mut app);
    enter_conflict(&mut app);
    let expected = saving(&app);
    assert_eq!(
        press(&mut app, false),
        Outcome {
            saves: 1,
            conflict: true,
            dirty: true,
            keep_local: true,
            ..settled(expected)
        }
    );
    let save = next(&receipts);
    assert_eq!(saved_mode(&save), FORCED);
    assert_eq!(
        press(&mut app, true),
        Outcome {
            conflict: true,
            dirty: true,
            keep_local: true,
            ..settled("正在保存本地内容，请稍候。")
        }
    );
    assert_eq!(
        deliver(&mut app, save),
        settled("已保留本地内容并覆盖外部文件。")
    );
    assert_eq!(
        fs::read(&app.paths.note_file).unwrap(),
        app.coordinator.view().text.as_bytes()
    );
    assert!(receipts.recv_timeout(Duration::from_millis(200)).is_err());
}

#[derive(Debug, PartialEq, Eq)]
enum Hide {
    Waiting,
    Cancelled,
    Hidden,
}

fn hide_state(app: &HeadlessApp) -> Hide {
    match app.window_flow.as_ref().unwrap().state().visibility() {
        VisibilityState::AwaitingHideSave { .. } => Hide::Waiting,
        VisibilityState::Presented(_) => Hide::Cancelled,
        VisibilityState::HiddenToTray { .. } => Hide::Hidden,
        other => panic!("unexpected visibility {other:?}"),
    }
}

fn request_hide(app: &mut HeadlessApp) {
    let guards = app.window_guards();
    app.dispatch_window_intent(None, WindowIntent::CloseRequested { now_ms: 1, guards });
    assert_eq!(hide_state(app), Hide::Waiting);
}

fn lifecycle(app: &HeadlessApp) -> LifecycleState {
    app.window_flow.as_ref().unwrap().state().lifecycle()
}

fn request_quit(app: &mut HeadlessApp) {
    let guards = app.window_guards();
    app.dispatch_window_intent(None, WindowIntent::TrayQuitRequested { guards });
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
        (
            from_the_future,
            Hide::Waiting,
            Quitting(QuitStage::AwaitingNoteSave),
        ),
    ];
    for (receipt, hide, quit) in rows {
        let mut app = edited();
        request_hide(&mut app);
        let completion = receipt(&app);
        app.handle_io_completion(None, completion);
        assert_eq!(hide_state(&app), hide);

        let mut app = edited();
        request_quit(&mut app);
        let completion = receipt(&app);
        app.handle_io_completion(None, completion);
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
    app.handle_io_completion(None, receipt);
    assert_eq!(hide_state(&app), Hide::Waiting);
    let receipt = saved(&app, FORCED);
    app.handle_io_completion(None, receipt);
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
    app.dispatch_window_intent(None, WindowIntent::CloseRequested { now_ms: 1, guards });
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
    app.dispatch_window_intent(
        None,
        WindowIntent::ShowRequested {
            reason: ShowReason::Tray,
            now_ms: 2,
        },
    );
    assert_eq!(
        applied_effects(&mut app),
        [
            "ApplyFrame",
            "ShowWindow",
            "FocusWindow",
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
        (Some(Err(FailureKind::Failed)), "Failed"),
        (None, "Unavailable"),
    ];
    for (outcome, kind) in rejected {
        let mut app = fixture("start");
        app.headless_hide = outcome;
        let guards = app.window_guards();
        app.dispatch_window_intent(None, WindowIntent::CloseRequested { now_ms: 1, guards });
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
    app.window_flow = Some(window_flow(edge));
}

fn auto_collapse_armed(app: &HeadlessApp) -> bool {
    let flow = app.window_flow.as_ref().unwrap();
    flow.state().next_deadline_ms().is_some()
}

/// A quit cancelled by the save that also failed a hide: the timer the quit held back starts.
#[test]
fn a_quit_cancelled_with_a_failed_hide_restarts_the_auto_collapse() {
    let mut app = edited();
    docked(&mut app);
    request_hide(&mut app);
    let guards = app.window_guards();
    app.dispatch_window_intent(None, WindowIntent::TrayQuitRequested { guards });
    assert!(lifecycle(&app).is_quitting());
    let receipt = failed(&app, GUARDED);
    app.handle_io_completion(None, receipt);
    assert_eq!(lifecycle(&app), LifecycleState::Running);
    assert_eq!(hide_state(&app), Hide::Cancelled);
    assert!(auto_collapse_armed(&app));

    let mut app = edited();
    docked(&mut app);
    request_hide(&mut app);
    let guards = app.window_guards();
    app.dispatch_window_intent(None, WindowIntent::TrayQuitRequested { guards });
    app.handle_io_completion(None, IoCompletion::WorkerStopped);
    assert_eq!(lifecycle(&app), LifecycleState::Running);
    assert!(auto_collapse_armed(&app));
}

/// Hiding closes the opacity popup. A rejected hide must not keep waiting on the popup it closed.
#[test]
fn a_rejected_hide_does_not_stay_blocked_by_the_popup_it_closed() {
    let mut app = fixture("start");
    docked(&mut app);
    app.controls.opacity_popup_open = true;
    app.headless_hide = Some(Err(FailureKind::Failed));
    let guards = app.window_guards();
    assert!(guards.popup_open);
    app.dispatch_window_intent(None, WindowIntent::CloseRequested { now_ms: 1, guards });
    assert_eq!(hide_state(&app), Hide::Cancelled);
    assert!(!app.controls.opacity_popup_open);
    assert!(auto_collapse_armed(&app));
}
