//! The notices end to end in the session, on an injected clock: what shows, when it ends, the
//! fault display and its marker, and what a stopped worker leaves (plan 04 #notice-state).
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#notice-state

use std::cell::Cell;
use std::path::PathBuf;
use std::rc::Rc;

use stickymd_core::{
    ExternalFileFact, ExternalFileState, FileConflict, Generation, LineEnding, hash_bytes,
};

use super::tests::{run, session};
use super::{AppSession, MonotonicClock, ViewFacts};
use crate::execution::IoCompletion;
use crate::flow::notice::{Choice, Notice, NoticeTone};
use crate::flow::{ClipboardError, ClipboardPort, EditorCoordinator};
use crate::instruction::LiteralSearchOptions;
use crate::instruction::{AppIntent, SaveReason};
use crate::ports::{
    AssetPasteFailure, AssetPasteRollback, ExportCompletion, FailureKind, NoteReceipt, PersistMode,
    PersistResult, StorageFailure,
};
use stickymd_core::{DocumentState, Selection};

struct ManualClock(Rc<Cell<u64>>);

impl MonotonicClock for ManualClock {
    fn now_ms(&self) -> u64 {
        self.0.get()
    }
}

/// A session whose clock reads `now`, which the test moves.
fn clocked(text: &str) -> (AppSession, Rc<Cell<u64>>) {
    let mut session = session(text);
    let now = Rc::new(Cell::new(0));
    session.clock = Box::new(ManualClock(Rc::clone(&now)));
    (session, now)
}

fn shown(session: &AppSession) -> Option<(String, NoticeTone)> {
    session
        .notice_view()
        .map(|view| (view.text.into_owned(), view.tone))
}

fn buttons(session: &AppSession) -> Vec<Choice> {
    session
        .notice_view()
        .and_then(|view| view.choices)
        .map(|choices| choices.iter().collect())
        .unwrap_or_default()
}

fn settle(session: &mut AppSession, now: &Cell<u64>, at: u64) -> Vec<String> {
    now.set(at);
    run(session, |session, out| session.settle(at, out))
}

fn generation(session: &AppSession) -> Generation {
    session.coordinator.view().generation
}

fn save_receipt(session: &AppSession, result: Result<(), FailureKind>) -> IoCompletion {
    let generation = generation(session);
    IoCompletion::Note(NoteReceipt::Save {
        generation,
        mode: PersistMode::Guarded { expected: None },
        result: match result {
            Ok(()) => Ok(PersistResult::Saved {
                generation,
                fingerprint: hash_bytes(b"saved"),
                durable_bytes: 5,
            }),
            Err(kind) => Err(StorageFailure::new(kind, "disk full")),
        },
    })
}

fn deliver(session: &mut AppSession, completion: IoCompletion) -> Vec<String> {
    run(session, |session, out| {
        session.handle_io(completion, ViewFacts::default(), out);
    })
}

fn save_manually(session: &mut AppSession) {
    run(session, |session, out| {
        session.save_now(SaveReason::Manual, out)
    });
}

fn fail_a_save(session: &mut AppSession, kind: FailureKind) {
    save_manually(session);
    let receipt = save_receipt(session, Err(kind));
    deliver(session, receipt);
}

/// P17-N01, P17-N02 and P17-N03: the session wakes for the shown notice's deadline, and the
/// expiry it hands back on that wake removes it; the layer below then shows.
#[test]
fn a_timed_notice_ends_on_the_wake_for_its_deadline() {
    let (mut session, now) = clocked("text");
    session.notify(Notice::ExportBusy);
    assert_eq!(session.notice_deadline(), Some(5_000));
    assert!(session.next_deadline().is_some_and(|wake| wake <= 5_000));
    assert!(settle(&mut session, &now, 4_999).is_empty());
    assert_eq!(
        shown(&session),
        Some(("已有导出任务正在进行。".into(), NoticeTone::Reminder))
    );
    assert_eq!(
        settle(&mut session, &now, 5_000),
        ["RedrawNeeded".to_owned()]
    );
    assert_eq!(shown(&session), None);
    assert_eq!(session.notice_deadline(), None);
}

/// P17-N05 and P17-N08: a manual save shows its progress until its own receipt settles it with
/// "saved" for 3 s.
#[test]
fn a_manual_save_shows_its_progress_and_then_saved() {
    let (mut session, now) = clocked("text");
    save_manually(&mut session);
    let text = format!("正在保存 generation {}…", generation(&session).value());
    assert_eq!(shown(&session), Some((text, NoticeTone::InProgress)));
    let receipt = save_receipt(&session, Ok(()));
    deliver(&mut session, receipt);
    assert_eq!(
        shown(&session),
        Some(("已保存".into(), NoticeTone::Success))
    );
    settle(&mut session, &now, 3_000);
    assert_eq!(shown(&session), None);
}

/// P17-N08: an export shows its progress until its receipt replaces it with the result.
#[test]
fn an_export_in_progress_is_settled_by_its_own_receipt() {
    let (mut session, _) = clocked("text");
    run(&mut session, |session, out| {
        session.submit_export(PathBuf::from("out.md"), out);
    });
    assert_eq!(
        shown(&session),
        Some(("正在导出当前快照…".into(), NoticeTone::InProgress))
    );
    let completion = IoCompletion::Export {
        generation: generation(&session),
        result: Ok(ExportCompletion {
            generation: generation(&session),
            target: PathBuf::from("out.md"),
            copied_assets: 2,
        }),
    };
    deliver(&mut session, completion);
    assert_eq!(
        shown(&session),
        Some((
            "导出完成：out.md（2 个本地资源）".into(),
            NoticeTone::Success
        ))
    );
}

/// P17-N11: the marker shows while the failure is unresolved, and a click starts another 30 s of
/// its text; without a failure the click does nothing.
#[test]
fn the_marker_shows_until_a_later_save_and_a_click_shows_the_failure_again() {
    let (mut session, now) = clocked("text");
    assert!(!session.show_save_fault());
    fail_a_save(&mut session, FailureKind::Failed);
    assert!(session.save_fault_marked());
    let (text, tone) = shown(&session).expect("the fault display");
    assert_eq!(tone, NoticeTone::Fault);
    assert!(text.contains("disk full"), "{text}");
    settle(&mut session, &now, 30_000);
    assert_eq!(shown(&session), None);
    assert!(session.save_fault_marked(), "the marker outlasts the text");
    now.set(40_000);
    assert!(session.show_save_fault());
    assert_eq!(
        shown(&session).map(|(_, tone)| tone),
        Some(NoticeTone::Fault)
    );
    assert_eq!(session.notice_deadline(), Some(70_000));
}

/// P17-N17: a second failure of the same kind does not start the display again, during or after
/// the first one; the marker stays.
#[test]
fn a_failure_of_the_same_kind_does_not_start_another_display() {
    let (mut session, now) = clocked("text");
    fail_a_save(&mut session, FailureKind::Failed);
    fail_a_save(&mut session, FailureKind::Failed);
    assert_eq!(session.timed_notice_texts().len(), 1);
    settle(&mut session, &now, 30_000);
    fail_a_save(&mut session, FailureKind::Failed);
    assert_eq!(shown(&session), None);
    assert!(session.save_fault_marked());
}

/// P17-N14: a failure that changes kind (a failed write, then the worker stopping) starts a new
/// segment with a new id; the old segment's expiry changes nothing.
#[test]
fn a_failure_that_changes_kind_starts_a_new_segment() {
    let (mut session, now) = clocked("text");
    fail_a_save(&mut session, FailureKind::Failed);
    let first = session.notices.due(30_000).expect("the first segment");
    now.set(10_000);
    deliver(&mut session, IoCompletion::WorkerStopped);
    assert_eq!(session.timed_notice_texts().len(), 2);
    let (text, tone) = shown(&session).expect("the second segment");
    assert_eq!(tone, NoticeTone::Fault);
    assert!(text.contains("持久化工作线程已停止"), "{text}");
    assert!(!session.notices.expire(first, 30_000), "a stale expiry");
    assert_eq!(session.notice_deadline(), Some(40_000));
}

/// P17-N19: a save that succeeds while the failure shows removes every fault display and the
/// marker; a manual one then shows "saved", a routine one shows nothing.
#[test]
fn a_successful_save_resolves_the_failure_and_only_a_manual_one_says_so() {
    let (mut session, _) = clocked("text");
    fail_a_save(&mut session, FailureKind::Unavailable);
    save_manually(&mut session);
    let receipt = save_receipt(&session, Ok(()));
    deliver(&mut session, receipt);
    assert!(!session.save_fault_marked());
    assert_eq!(session.timed_notice_texts(), ["已保存"]);

    let (mut session, _) = clocked("text");
    fail_a_save(&mut session, FailureKind::Unavailable);
    run(&mut session, |session, out| {
        session.request_save(crate::flow::SaveTrigger::Debounce, out);
    });
    let receipt = save_receipt(&session, Ok(()));
    deliver(&mut session, receipt);
    assert!(!session.save_fault_marked());
    assert!(session.timed_notice_texts().is_empty());
    assert_eq!(shown(&session), None);
}

/// P17-N18: an important notice covered by a reminder shows again when the reminder ends, while
/// its own 30 s last.
#[test]
fn an_important_notice_shows_again_after_the_reminder_over_it() {
    let (mut session, now) = clocked("text");
    session.notify(Notice::ExportFailed("disk".into()));
    now.set(10_000);
    session.notify(Notice::ClipboardFailed("busy".into()));
    assert_eq!(
        shown(&session).map(|(_, tone)| tone),
        Some(NoticeTone::Reminder)
    );
    settle(&mut session, &now, 15_000);
    assert_eq!(
        shown(&session),
        Some((
            "导出失败；工作文档未改变：disk".into(),
            NoticeTone::Important
        ))
    );
}

/// P17-N20 and P17-N13: the worker stopping during "keep local" ends that write; the conflict
/// waits again with both buttons under the fault display, and choosing to keep local is refused
/// at once.
#[test]
fn a_stopped_worker_returns_the_conflict_to_its_choice() {
    let (mut session, _) = clocked("local");
    session.persistence.enter_conflict(FileConflict {
        external: ExternalFileState::Present(ExternalFileFact {
            fingerprint: hash_bytes(b"outside"),
            text: "outside".into(),
            line_ending: LineEnding::Lf,
            durable_len: 7,
        }),
        detected_at_generation: generation(&session),
        previous_disk_fingerprint: None,
    });
    assert_eq!(buttons(&session), [Choice::LoadExternal, Choice::KeepLocal]);
    run(&mut session, |session, out| {
        assert!(session.resolve(false, out));
    });
    assert!(session.persistence.keep_local_in_progress());
    assert_eq!(
        shown(&session).map(|(_, tone)| tone),
        Some(NoticeTone::InProgress)
    );
    assert!(buttons(&session).is_empty());

    deliver(&mut session, IoCompletion::WorkerStopped);
    assert!(!session.persistence.keep_local_in_progress());
    assert_eq!(
        shown(&session).map(|(_, tone)| tone),
        Some(NoticeTone::Fault)
    );
    assert_eq!(buttons(&session), [Choice::LoadExternal, Choice::KeepLocal]);

    run(&mut session, |session, out| {
        assert!(session.resolve(false, out));
    });
    let (text, tone) = shown(&session).expect("the refusal");
    assert_eq!(tone, NoticeTone::Important);
    assert!(text.starts_with("持久化不可用"), "{text}");
    assert_eq!(buttons(&session), [Choice::LoadExternal, Choice::KeepLocal]);
}

/// P17-C11: a copy shows "copied" for 3 s, but not over a failure that is showing.
#[test]
fn a_copy_says_copied_unless_a_failure_shows() {
    let (mut session, _) = clocked("text");
    let before = (generation(&session), session.editor.selection);
    run(&mut session, |session, out| {
        session.dispatch(
            AppIntent::WriteClipboard {
                text: "text".into(),
            },
            out,
        );
    });
    assert_eq!(
        shown(&session),
        Some(("已复制".into(), NoticeTone::Success))
    );
    assert_eq!(
        (generation(&session), session.editor.selection),
        before,
        "a copy changes neither the document nor the selection"
    );

    let (mut session, _) = clocked("text");
    session.notify(Notice::ExportFailed("disk".into()));
    run(&mut session, |session, out| {
        session.dispatch(
            AppIntent::WriteClipboard {
                text: "text".into(),
            },
            out,
        );
    });
    assert_eq!(
        shown(&session).map(|(_, tone)| tone),
        Some(NoticeTone::Important)
    );
}

/// P17-N13: the first write of "keep local" fails: the failure shows for 30 s with the
/// conflict's buttons kept, and then the conflict waits again.
#[test]
fn a_failed_keep_local_shows_its_reason_with_the_choices_kept() {
    let (mut session, now) = clocked("local");
    session.persistence.enter_conflict(FileConflict {
        external: ExternalFileState::Present(ExternalFileFact {
            fingerprint: hash_bytes(b"outside"),
            text: "outside".into(),
            line_ending: LineEnding::Lf,
            durable_len: 7,
        }),
        detected_at_generation: generation(&session),
        previous_disk_fingerprint: None,
    });
    run(&mut session, |session, out| {
        assert!(session.resolve(false, out));
    });
    let failed = IoCompletion::Note(NoteReceipt::Save {
        generation: generation(&session),
        mode: PersistMode::ForceOverwrite,
        result: Err(StorageFailure::new(FailureKind::Failed, "disk full")),
    });
    deliver(&mut session, failed);
    let (text, tone) = shown(&session).expect("the failure");
    assert_eq!(tone, NoticeTone::Fault);
    assert!(text.contains("disk full"), "{text}");
    assert_eq!(buttons(&session), [Choice::LoadExternal, Choice::KeepLocal]);
    settle(&mut session, &now, 30_000);
    assert_eq!(
        shown(&session),
        Some(("文件已在外部修改".into(), NoticeTone::Pending))
    );
    assert_eq!(buttons(&session), [Choice::LoadExternal, Choice::KeepLocal]);
}

fn present(text: &str) -> ExternalFileState {
    ExternalFileState::Present(ExternalFileFact {
        fingerprint: hash_bytes(text.as_bytes()),
        text: text.into(),
        line_ending: LineEnding::Lf,
        durable_len: text.len(),
    })
}

/// A Replace All and a delimiter conversion say how many places they changed (plan 09 #notices:
/// the replacement and conversion counts are success notices).
#[test]
fn a_replace_all_and_a_conversion_say_how_many_places_changed() {
    let (mut session, _) = clocked("a a a");
    let expected_generation = generation(&session);
    run(&mut session, |session, out| {
        session.dispatch(
            AppIntent::ReplaceAllLiteral {
                expected_generation,
                query: "a".into(),
                replacement: "b".into(),
                options: LiteralSearchOptions::default(),
                timestamp_ms: 0,
            },
            out,
        );
    });
    assert_eq!(session.coordinator.view().text, "b b b");
    assert_eq!(
        shown(&session),
        Some(("已替换 3 处".into(), NoticeTone::Success))
    );

    let (mut session, _) = clocked("\\(x\\) and \\(y\\)");
    let expected_generation = generation(&session);
    run(&mut session, |session, out| {
        session.dispatch(
            AppIntent::ConvertLatexMathDelimiters {
                expected_generation,
                selection: Selection::new(0, 0),
                scope_to_selection: false,
                timestamp_ms: 0,
            },
            out,
        );
    });
    assert_eq!(session.coordinator.view().text, "$x$ and $y$");
    assert_eq!(
        shown(&session),
        Some(("已转换 2 处".into(), NoticeTone::Success))
    );
}

/// P17-N08: a manual save that a conflict answers no longer shows as saving; once the conflict is
/// resolved and its notice ends, nothing is in progress.
#[test]
fn a_manual_save_answered_by_a_conflict_is_no_longer_in_progress() {
    let (mut session, now) = clocked("local");
    // An edit makes the note dirty, so the conflict waits for a choice.
    let intent = super::tests::edit(&session);
    run(&mut session, |session, out| session.dispatch(intent, out));
    save_manually(&mut session);
    let receipt = IoCompletion::Note(NoteReceipt::Save {
        generation: generation(&session),
        mode: PersistMode::Guarded { expected: None },
        result: Ok(PersistResult::Conflict {
            generation: generation(&session),
            observed: present("outside"),
        }),
    });
    deliver(&mut session, receipt);
    assert_eq!(session.persistence.manual_save(), None);
    assert_eq!(buttons(&session), [Choice::LoadExternal, Choice::KeepLocal]);
    run(&mut session, |session, out| {
        assert!(session.resolve(true, out));
    });
    settle(&mut session, &now, 60_000);
    assert_eq!(shown(&session), None);
}

/// P17-N08 and P17-N20: a stopped worker ends the image check after a recovery, which gave the
/// note back to editing; an image sync asked of it later fails at once instead of waiting.
#[test]
fn a_stopped_worker_ends_the_image_check_and_fails_a_later_sync() {
    let (mut session, _) = clocked("text");
    session.assets.reconcile_pending = true;
    session.assets.sync_in_flight = true;
    assert_eq!(
        shown(&session).map(|(_, tone)| tone),
        Some(NoticeTone::InProgress)
    );
    let seen = deliver(&mut session, IoCompletion::WorkerStopped);
    assert!(!session.assets.reconcile_pending);
    assert!(
        seen.iter().any(|effect| effect == "StartWatcher"),
        "{seen:?}"
    );
    assert_eq!(
        shown(&session).map(|(_, tone)| tone),
        Some(NoticeTone::Fault)
    );

    let generation = generation(&session);
    run(&mut session, |session, out| {
        session.submit_asset_sync(generation, Vec::new(), None, false, out);
    });
    assert!(!session.assets.sync_in_flight);
    assert!(
        session
            .timed_notice_texts()
            .first()
            .is_some_and(|text| text.starts_with("图片整理未完成")),
        "{:?}",
        session.timed_notice_texts()
    );
}

/// P17-N08: preparing images shows until the paste's own receipt settles it.
#[test]
fn preparing_images_is_settled_by_the_pastes_receipt() {
    let (mut session, _) = clocked("text");
    session.assets.paste_pending = true;
    assert_eq!(
        shown(&session),
        Some(("正在准备图片…".into(), NoticeTone::InProgress))
    );
    let failure = AssetPasteFailure {
        expected_generation: generation(&session),
        error: StorageFailure::new(FailureKind::Failed, "disk full"),
        rollback: AssetPasteRollback::default(),
    };
    deliver(&mut session, IoCompletion::AssetPaste(Err(failure)));
    assert!(!session.assets.paste_pending);
    assert_eq!(
        shown(&session).map(|(_, tone)| tone),
        Some(NoticeTone::Important)
    );
}

/// P17-N17: failures of one kind for different generations and texts start no new segment.
#[test]
fn failures_of_one_kind_for_other_generations_start_no_new_segment() {
    let (mut session, now) = clocked("text");
    let first = generation(&session);
    let second = first.checked_next().unwrap();
    let failed = |generation| {
        IoCompletion::Note(NoteReceipt::Save {
            generation,
            mode: PersistMode::Guarded { expected: None },
            result: Err(StorageFailure::new(FailureKind::Failed, "mkdir failed")),
        })
    };
    deliver(&mut session, failed(first));
    let text = session.timed_notice_texts();
    deliver(&mut session, failed(second));
    assert_eq!(session.timed_notice_texts().len(), 1);
    assert_ne!(
        session.timed_notice_texts(),
        text,
        "the text follows the latest failure"
    );
    settle(&mut session, &now, 30_000);
    deliver(&mut session, failed(first));
    assert!(session.timed_notice_texts().is_empty());
    assert!(session.save_fault_marked());
}

struct FailingClipboard;

impl ClipboardPort for FailingClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        Err(ClipboardError::Unavailable("busy".into()))
    }

    fn write_text(&mut self, _text: &str) -> Result<(), ClipboardError> {
        Err(ClipboardError::Unavailable("busy".into()))
    }
}

/// P17-C11: a failed copy always shows, as a reminder, and changes neither the document nor the
/// selection.
#[test]
fn a_failed_copy_shows_and_changes_nothing() {
    let (mut session, _) = clocked("text");
    session.coordinator = EditorCoordinator::new(
        DocumentState::loaded("text", LineEnding::Lf, None),
        Box::new(FailingClipboard),
    );
    session.notify(Notice::ExportFailed("disk".into()));
    let before = (generation(&session), session.editor.selection);
    run(&mut session, |session, out| {
        session.dispatch(
            AppIntent::WriteClipboard {
                text: "text".into(),
            },
            out,
        );
    });
    let (text, tone) = shown(&session).expect("the failure");
    assert_eq!(tone, NoticeTone::Reminder);
    assert!(text.contains("busy"), "{text}");
    assert_eq!((generation(&session), session.editor.selection), before);
}
