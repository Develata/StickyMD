use stickymd_core::Generation;

use super::*;
use crate::flow::RecoveryOperation;
use crate::flow::notice::derived::{Choice, ConflictSide, InProgress, RecoveryFacts};

fn conflict(side: ConflictSide) -> DerivedFacts {
    DerivedFacts {
        conflict: Some(side),
        ..DerivedFacts::default()
    }
}

fn recovery(operation: Option<RecoveryOperation>, canonical_usable: bool) -> DerivedFacts {
    DerivedFacts {
        recovery: Some(RecoveryFacts {
            operation,
            canonical_usable,
        }),
        ..DerivedFacts::default()
    }
}

fn shown<'a>(view: &'a Option<NoticeView<'a>>) -> Option<(&'a str, NoticeTone)> {
    view.as_ref().map(|view| (view.text.as_ref(), view.tone))
}

fn buttons(view: Option<NoticeView<'_>>) -> Vec<Choice> {
    view.and_then(|view| view.choices)
        .map(|choices| choices.iter().collect())
        .unwrap_or_default()
}

/// P17-N01: when the timed notice ends, the derived layer shows what holds then, a waiting
/// decision before an operation in progress.
#[test]
fn the_derived_layer_shows_through_when_the_timed_notice_ends() {
    let mut state = NoticeState::default();
    let mut facts = conflict(ConflictSide::Loadable);
    facts.export_in_flight = true;
    let id = state
        .notify(MessageClass::Reminder, "refused".into(), 0)
        .unwrap();
    assert_eq!(
        shown(&state.view(&facts, None)),
        Some(("refused", NoticeTone::Reminder))
    );
    assert!(state.expire(id, 5_000));
    assert_eq!(
        shown(&state.view(&facts, None)),
        Some(("文件已在外部修改", NoticeTone::Pending))
    );
    facts.conflict = None;
    assert_eq!(
        shown(&state.view(&facts, None)),
        Some(("正在导出当前快照…", NoticeTone::InProgress))
    );
    facts.export_in_flight = false;
    assert_eq!(state.view(&facts, None), None);
}

/// P17-N01: a decision is waiting only while no choice for it is being carried out; the choice
/// in flight shows as in progress instead.
#[test]
fn a_decision_waits_only_while_no_choice_for_it_is_in_flight() {
    let state = NoticeState::default();
    let mut facts = conflict(ConflictSide::Loadable);
    facts.keep_local_in_flight = true;
    assert_eq!(
        shown(&state.view(&facts, None)),
        Some(("正在用内存文本执行“保留本地”…", NoticeTone::InProgress))
    );
    let facts = recovery(Some(RecoveryOperation::Restoring), true);
    assert_eq!(
        shown(&state.view(&facts, None)),
        Some(("正在恢复临时内容…", NoticeTone::InProgress))
    );
    assert_eq!(
        buttons(state.view(&facts, None)),
        [],
        "no buttons while the choice runs"
    );
    let facts = recovery(None, true);
    assert_eq!(
        shown(&state.view(&facts, None)),
        Some(("发现未完成保存的内容", NoticeTone::Pending))
    );
}

/// The operations in progress show in plan 04's order: recovery, keep local, manual save,
/// images, export.
#[test]
fn operations_in_progress_show_in_the_planned_order() {
    let generation = Generation::initial();
    let all = DerivedFacts {
        conflict: None,
        keep_local_in_flight: true,
        recovery: Some(RecoveryFacts {
            operation: Some(RecoveryOperation::UsingCanonical),
            canonical_usable: true,
        }),
        manual_save: Some(generation),
        preparing_images: true,
        checking_images: true,
        export_in_flight: true,
    };
    let mut facts = all;
    let mut order = Vec::new();
    while let Some(operation) = facts.in_progress() {
        order.push(operation);
        match operation {
            InProgress::Recovery(_) => facts.recovery = None,
            InProgress::KeepLocal => facts.keep_local_in_flight = false,
            InProgress::ManualSave(_) => facts.manual_save = None,
            InProgress::PreparingImages => facts.preparing_images = false,
            InProgress::CheckingImages => facts.checking_images = false,
            InProgress::Export => facts.export_in_flight = false,
        }
    }
    assert_eq!(
        order,
        [
            InProgress::Recovery(RecoveryOperation::UsingCanonical),
            InProgress::KeepLocal,
            InProgress::ManualSave(generation),
            InProgress::PreparingImages,
            InProgress::CheckingImages,
            InProgress::Export,
        ]
    );
}

/// P17-N07: a waiting decision offers one button per available option, and one button when
/// there is only one.
#[test]
fn a_waiting_decision_offers_a_button_per_available_option() {
    let state = NoticeState::default();
    assert_eq!(
        buttons(state.view(&conflict(ConflictSide::Loadable), None)),
        [Choice::LoadExternal, Choice::KeepLocal]
    );
    for side in [
        ConflictSide::InvalidUtf8,
        ConflictSide::TooLarge { bytes: 9 },
        ConflictSide::Other,
    ] {
        assert_eq!(
            buttons(state.view(&conflict(side), None)),
            [Choice::KeepLocal],
            "{side:?}"
        );
    }
    assert_eq!(
        buttons(state.view(&recovery(None, true), None)),
        [Choice::RestoreTemporary, Choice::UseCanonical]
    );
    assert_eq!(
        buttons(state.view(&recovery(None, false), None)),
        [Choice::RestoreTemporary]
    );
    for choice in [Choice::LoadExternal, Choice::RestoreTemporary] {
        assert!(choice.is_primary(), "{choice:?} is F6");
    }
    for choice in [Choice::KeepLocal, Choice::UseCanonical] {
        assert!(!choice.is_primary(), "{choice:?} is F7");
    }
}

/// P17-N16 and P17-N13: whatever timed notice covers a waiting conflict, its buttons stay; so a
/// failed "keep local" shows the failure and the conflict's choices together.
#[test]
fn the_buttons_of_a_waiting_decision_stay_under_any_timed_notice() {
    let mut state = NoticeState::default();
    let facts = conflict(ConflictSide::Loadable);
    let both = [Choice::LoadExternal, Choice::KeepLocal];
    state.notify(MessageClass::Success, "copied".into(), 0);
    assert_eq!(buttons(state.view(&facts, None)), both);
    state.notify(MessageClass::Reminder, "refused".into(), 0);
    assert_eq!(buttons(state.view(&facts, None)), both);
    state.notify(MessageClass::Important, "export failed".into(), 0);
    assert_eq!(buttons(state.view(&facts, None)), both);
    state.start_fault_display(0);
    let view = state.view(&facts, Some("保存失败：磁盘已满"));
    assert_eq!(
        shown(&view),
        Some(("保存失败：磁盘已满", NoticeTone::Fault))
    );
    assert_eq!(buttons(view), both);
}

/// A fault display shows the save failure's current text, and once the failure is resolved the
/// layer below shows through.
#[test]
fn a_fault_display_shows_the_current_failure_and_goes_with_it() {
    let mut state = NoticeState::default();
    state.notify(MessageClass::Important, "export failed".into(), 0);
    state.start_fault_display(10);
    let facts = DerivedFacts::default();
    assert_eq!(
        shown(&state.view(&facts, Some("first"))),
        Some(("first", NoticeTone::Fault))
    );
    assert_eq!(
        shown(&state.view(&facts, Some("second"))),
        Some(("second", NoticeTone::Fault))
    );
    state.clear_fault_displays();
    assert_eq!(
        shown(&state.view(&facts, None)),
        Some(("export failed", NoticeTone::Important))
    );
}
