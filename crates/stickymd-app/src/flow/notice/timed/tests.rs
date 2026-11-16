use super::*;

fn message(stack: &mut TimedNotices, class: MessageClass, text: &str, now_ms: u64) -> NoticeId {
    stack
        .push_message(class, text.to_owned(), now_ms)
        .expect("the notice is shown")
}

fn shown(stack: &TimedNotices) -> Option<(TimedClass, Option<&str>)> {
    stack.top().map(|top| (top.class(), top.text()))
}

/// P17-N02: each class keeps its own duration, counted from the push on the injected clock.
#[test]
fn each_class_lasts_its_own_duration_from_the_push() {
    for (class, duration) in [
        (MessageClass::Success, 3_000),
        (MessageClass::Reminder, 5_000),
        (MessageClass::Important, 30_000),
    ] {
        let mut stack = TimedNotices::default();
        let id = message(&mut stack, class, "x", 1_000);
        assert_eq!(stack.deadline_ms(), Some(1_000 + duration));
        assert_eq!(stack.due(1_000 + duration - 1), None);
        assert!(
            !stack.expire(id, 1_000 + duration - 1),
            "{class:?} ended early"
        );
        assert_eq!(stack.due(1_000 + duration), Some(id));
        assert!(stack.expire(id, 1_000 + duration));
        assert!(stack.is_empty());
    }
    let mut stack = TimedNotices::default();
    stack.push_fault_display(0);
    assert_eq!(stack.deadline_ms(), Some(30_000));
    assert_eq!(shown(&stack), Some((TimedClass::FaultDisplay, None)));
}

/// P17-N02: time runs while a notice is covered; it is not paused.
#[test]
fn a_covered_notice_keeps_counting_and_is_dropped_once_its_time_has_run_out() {
    let mut stack = TimedNotices::default();
    message(&mut stack, MessageClass::Important, "important", 0);
    let reminder = message(&mut stack, MessageClass::Reminder, "reminder", 1_000);
    assert!(stack.expire(reminder, 6_000));
    assert_eq!(
        shown(&stack),
        Some((TimedClass::Important, Some("important")))
    );
    assert_eq!(stack.deadline_ms(), Some(30_000));

    let mut stack = TimedNotices::default();
    message(&mut stack, MessageClass::Important, "important", 0);
    let reminder = message(&mut stack, MessageClass::Reminder, "reminder", 28_000);
    // The reminder ends after the important notice's own deadline: it is not shown again.
    assert!(stack.expire(reminder, 33_000));
    assert!(stack.is_empty());
}

/// P17-N03: only the shown notice's receipt pops it; a replaced or covered notice's receipt
/// changes nothing.
#[test]
fn only_the_receipt_of_the_shown_notice_pops_it() {
    let mut stack = TimedNotices::default();
    let first = message(&mut stack, MessageClass::Success, "copied", 0);
    let second = message(&mut stack, MessageClass::Success, "saved", 1_000);
    assert_ne!(first, second);
    assert_eq!(stack.len(), 1, "a success replaces the success on top");
    // The replaced notice's timer fires at its old deadline: ignored.
    assert!(!stack.expire(first, 3_000));
    assert_eq!(shown(&stack), Some((TimedClass::Success, Some("saved"))));
    assert_eq!(stack.deadline_ms(), Some(4_000));

    let covered = message(&mut stack, MessageClass::Important, "important", 2_000);
    let top = message(&mut stack, MessageClass::Reminder, "reminder", 2_500);
    assert!(
        !stack.expire(covered, 40_000),
        "a covered notice never pops"
    );
    assert_eq!(stack.top().map(TimedNotice::id), Some(top));
    assert!(!stack.expire(NoticeId(999), 40_000));
    assert!(stack.expire(top, 7_500));
}

/// P17-N04: failures and refusals always land on top; a success only on an empty stack or over
/// a success, and may cover nothing else.
#[test]
fn a_success_never_covers_a_failure_while_failures_always_land_on_top() {
    let mut stack = TimedNotices::default();
    message(&mut stack, MessageClass::Reminder, "refused", 0);
    assert_eq!(
        stack.push_message(MessageClass::Success, "copied".into(), 10),
        None
    );
    assert_eq!(shown(&stack), Some((TimedClass::Reminder, Some("refused"))));

    let mut stack = TimedNotices::default();
    stack.push_fault_display(0);
    assert_eq!(
        stack.push_message(MessageClass::Success, "copied".into(), 10),
        None
    );
    message(&mut stack, MessageClass::Important, "export failed", 20);
    message(&mut stack, MessageClass::Reminder, "refused", 30);
    assert_eq!(stack.len(), 3);
    assert_eq!(shown(&stack), Some((TimedClass::Reminder, Some("refused"))));

    let mut stack = TimedNotices::default();
    message(&mut stack, MessageClass::Success, "copied", 0);
    message(&mut stack, MessageClass::Important, "export failed", 10);
    assert_eq!(stack.len(), 2, "a failure lands on top of a success");
}

/// P17-N18: a reminder over an important notice shows at once, and the important notice shows
/// again after it if its own time is not up.
#[test]
fn an_important_notice_shows_again_after_a_reminder_that_covered_it() {
    let mut stack = TimedNotices::default();
    message(&mut stack, MessageClass::Important, "export failed", 0);
    let copy_failed = message(&mut stack, MessageClass::Reminder, "copy failed", 10_000);
    assert_eq!(
        shown(&stack),
        Some((TimedClass::Reminder, Some("copy failed")))
    );
    assert_eq!(stack.due(15_000), Some(copy_failed));
    assert!(stack.expire(copy_failed, 15_000));
    assert_eq!(
        shown(&stack),
        Some((TimedClass::Important, Some("export failed")))
    );
}

/// The stack keeps the eight newest notices and pops past expired ones in one step.
#[test]
fn the_stack_is_bounded_and_drops_the_oldest() {
    let mut stack = TimedNotices::default();
    let ids: Vec<_> = (0..10)
        .map(|index| {
            message(
                &mut stack,
                MessageClass::Important,
                &index.to_string(),
                index,
            )
        })
        .collect();
    assert_eq!(stack.len(), MAX_TIMED_NOTICES);
    assert_eq!(stack.top().map(TimedNotice::id), ids.last().copied());
    while let Some(id) = stack.due(40_000) {
        assert!(stack.expire(id, 40_000));
    }
    assert!(stack.is_empty());
}

/// Resolving the save failure removes every fault display and keeps the rest in order.
#[test]
fn removing_the_fault_displays_keeps_every_other_notice() {
    let mut stack = TimedNotices::default();
    message(&mut stack, MessageClass::Important, "export failed", 0);
    stack.push_fault_display(10);
    message(&mut stack, MessageClass::Reminder, "refused", 20);
    stack.push_fault_display(30);
    stack.remove_fault_displays();
    assert_eq!(stack.len(), 2);
    assert_eq!(shown(&stack), Some((TimedClass::Reminder, Some("refused"))));
}
