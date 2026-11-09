//! The show action as request, receipt and commit, and everything that can arrive while its
//! receipt is outstanding (plan 04 window-action-receipts). Only the visible-and-not-minimized bit
//! is at stake here: focus, occlusion and position are not what a show receipt proves.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#window-action-receipts

use super::geometry::{MonitorGeometry, MonitorIdentity, PhysicalRect, WindowPlacement};
use super::reducer::WindowShellCoordinator;
use super::state::WINDOW_ACTION_RECEIPT_TIMEOUT_MS;
use super::state::{
    AUTO_COLLAPSE_DELAY_MS, DockEdge, LifecycleState, PLACEMENT_CONFIRM_TIMEOUT_MS, QuitBarrier,
    QuitStage, ShowReason, StableVisibility, TopmostSource, VisibilityState, WindowEffect,
    WindowGuardSnapshot, WindowIntent,
};
use crate::ports::FailureKind;

const REJECTIONS: [FailureKind; 3] = [
    FailureKind::Failed,
    FailureKind::Unavailable,
    FailureKind::Unsupported,
];

fn monitor(identity: &str, x: i32) -> MonitorGeometry {
    MonitorGeometry::new(
        MonitorIdentity::new(identity),
        PhysicalRect::new(x, 0, 1920, 1080),
        1.0,
        true,
    )
}

fn coordinator(visibility: StableVisibility) -> WindowShellCoordinator {
    WindowShellCoordinator::new(
        WindowPlacement::new(520.0, 680.0, None, 0.5, 0.5, 0.25),
        visibility,
        monitor("primary", 0),
        true,
    )
}

fn action(effects: &[WindowEffect], kind: fn(&WindowEffect) -> Option<u64>) -> Option<u64> {
    effects.iter().find_map(kind)
}

fn show_id(effects: &[WindowEffect]) -> Option<u64> {
    action(effects, |effect| match effect {
        WindowEffect::ShowWindow { id } => Some(*id),
        _ => None,
    })
}

fn focus_id(effects: &[WindowEffect]) -> Option<u64> {
    action(effects, |effect| match effect {
        WindowEffect::FocusWindow { id } => Some(*id),
        _ => None,
    })
}

fn hide_id(effects: &[WindowEffect]) -> Option<u64> {
    action(effects, |effect| match effect {
        WindowEffect::HideWindow { id } => Some(*id),
        _ => None,
    })
}

fn receipt(
    coordinator: &mut WindowShellCoordinator,
    id: u64,
    outcome: Result<(), FailureKind>,
) -> Vec<WindowEffect> {
    coordinator.dispatch(WindowIntent::ActionCompleted {
        id,
        outcome,
        now_ms: 50,
    })
}

fn show(coordinator: &mut WindowShellCoordinator) -> Vec<WindowEffect> {
    coordinator.dispatch(WindowIntent::ShowRequested {
        reason: ShowReason::Tray,
        now_ms: 10,
    })
}

/// A window hidden to the tray, the hide's receipt in.
fn hidden(visibility: StableVisibility) -> WindowShellCoordinator {
    let mut coordinator = coordinator(visibility);
    let effects = coordinator.dispatch(WindowIntent::CloseRequested {
        now_ms: 0,
        guards: WindowGuardSnapshot::default(),
    });
    receipt(&mut coordinator, hide_id(&effects).unwrap(), Ok(()));
    assert!(matches!(
        coordinator.state().visibility(),
        VisibilityState::HiddenToTray { .. }
    ));
    coordinator
}

/// A hidden window whose show waits for its receipt.
fn showing(visibility: StableVisibility) -> (WindowShellCoordinator, u64) {
    let mut coordinator = hidden(visibility);
    let effects = show(&mut coordinator);
    let id = show_id(&effects).unwrap();
    (coordinator, id)
}

fn input_effects(effects: &[WindowEffect]) -> bool {
    effects.iter().any(|effect| {
        matches!(
            effect,
            WindowEffect::SetImeAllowed(true) | WindowEffect::SetEditorInputEnabled(true)
        )
    })
}

/// The request places the window and asks to show it, and nothing depends on it yet.
#[test]
fn a_show_only_asks_until_its_receipt() {
    let mut coordinator = hidden(StableVisibility::Floating);
    let effects = show(&mut coordinator);
    let id = show_id(&effects).unwrap();
    assert!(matches!(
        effects[0],
        WindowEffect::ApplyFrame { last: true, .. }
    ));
    assert_eq!(effects[1..], [WindowEffect::ShowWindow { id }]);
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::ShowPending {
            id,
            restore: StableVisibility::Floating,
            deadline_ms: 10 + WINDOW_ACTION_RECEIPT_TIMEOUT_MS,
        }
    );
    assert!(!coordinator.state().accepts_editor_mutation());
}

/// Only the success receipt shows the window to the rest of the program, in the order the shell
/// has always applied a show: restored first, then focus, then input.
#[test]
fn a_success_receipt_commits_the_show_and_what_depends_on_it() {
    let (mut coordinator, id) = showing(StableVisibility::Floating);
    let effects = receipt(&mut coordinator, id, Ok(()));
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::Presented(StableVisibility::Floating)
    );
    let focus = focus_id(&effects).expect("a focus request");
    assert_ne!(focus, id);
    assert_eq!(
        effects,
        [
            WindowEffect::WindowShown,
            WindowEffect::FocusWindow { id: focus },
            WindowEffect::SetImeAllowed(true),
            WindowEffect::SetEditorInputEnabled(true),
            WindowEffect::RequestRedraw,
        ]
    );
    assert!(coordinator.state().accepts_editor_mutation());
}

#[test]
fn a_rejected_show_keeps_the_window_hidden_and_input_off() {
    for kind in REJECTIONS {
        let (mut coordinator, id) = showing(StableVisibility::Floating);
        let effects = receipt(&mut coordinator, id, Err(kind));
        assert_eq!(effects, [WindowEffect::ShowRejected(kind)]);
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::HiddenToTray {
                restore: StableVisibility::Floating
            }
        );
        assert!(!coordinator.state().accepts_editor_mutation());
    }
}

/// A rejection leaves exactly what was there: a window hidden while collapsed still says so.
#[test]
fn a_rejected_show_restores_the_prior_record_exactly() {
    let collapsed = StableVisibility::DockedCollapsed(DockEdge::Right);
    let mut coordinator = hidden(collapsed);
    let prior = coordinator.state().visibility();
    let id = show_id(&show(&mut coordinator)).unwrap();
    receipt(&mut coordinator, id, Err(FailureKind::Unsupported));
    assert_eq!(coordinator.state().visibility(), prior);
}

/// Waking a window hidden while collapsed presents it expanded.
#[test]
fn a_show_from_collapsed_commits_expanded() {
    let (mut coordinator, id) = showing(StableVisibility::DockedCollapsed(DockEdge::Top));
    receipt(&mut coordinator, id, Ok(()));
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::Presented(StableVisibility::DockedExpanded(DockEdge::Top))
    );
    assert!(!coordinator.state().temporary_sensor_topmost());
}

#[test]
fn a_stale_show_receipt_changes_nothing() {
    let (mut coordinator, id) = showing(StableVisibility::Floating);
    let before = coordinator.state().visibility();
    for outcome in [Ok(()), Err(FailureKind::Failed)] {
        assert!(receipt(&mut coordinator, id + 100, outcome).is_empty());
        assert_eq!(coordinator.state().visibility(), before);
    }
}

/// The wait always ends: a rejected show can be asked for again, and that one can succeed.
#[test]
fn a_rejected_show_can_be_retried() {
    let (mut coordinator, first) = showing(StableVisibility::Floating);
    receipt(&mut coordinator, first, Err(FailureKind::Failed));
    let effects = show(&mut coordinator);
    let second = show_id(&effects).expect("the same target asked for again");
    assert_ne!(second, first);
    let effects = receipt(&mut coordinator, second, Ok(()));
    assert!(input_effects(&effects));
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::Presented(StableVisibility::Floating)
    );
}

/// A second show (or a tray click on the still-unconfirmed window) replaces the first: the old
/// receipt no longer commits, so a receipt that never came cannot hold the wait open.
#[test]
fn a_newer_show_supersedes_the_older_receipt() {
    let tray = |coordinator: &mut WindowShellCoordinator| {
        coordinator.dispatch(WindowIntent::TrayToggleRequested {
            now_ms: 20,
            guards: WindowGuardSnapshot::default(),
        })
    };
    for again in [
        show as fn(&mut WindowShellCoordinator) -> Vec<WindowEffect>,
        tray,
    ] {
        let (mut coordinator, first) = showing(StableVisibility::Floating);
        let second = show_id(&again(&mut coordinator)).expect("a new request");
        assert_ne!(second, first);
        assert!(receipt(&mut coordinator, first, Ok(())).is_empty());
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::ShowPending { id, .. } if id == second
        ));
        let effects = receipt(&mut coordinator, second, Ok(()));
        assert!(input_effects(&effects));
    }
}

/// The window is about to appear; closing it now would race the receipt, so the receipt decides.
#[test]
fn a_hide_request_waits_out_a_pending_show() {
    let (mut coordinator, id) = showing(StableVisibility::Floating);
    let before = coordinator.state().visibility();
    let effects = coordinator.dispatch(WindowIntent::CloseRequested {
        now_ms: 20,
        guards: WindowGuardSnapshot::default(),
    });
    assert!(effects.is_empty());
    assert_eq!(coordinator.state().visibility(), before);
    assert!(input_effects(&receipt(&mut coordinator, id, Ok(()))));
}

/// A quit stops all input. A receipt still says what the window is, for a quit that gets
/// cancelled, but it gives the keyboard to nothing and asks for no focus.
#[test]
fn a_quit_overrides_the_input_a_pending_show_would_restore() {
    let (mut coordinator, id) = showing(StableVisibility::Floating);
    let effects = coordinator.dispatch(WindowIntent::TrayQuitRequested {
        guards: WindowGuardSnapshot::default(),
    });
    assert_eq!(
        effects,
        [
            WindowEffect::SetEditorInputEnabled(false),
            WindowEffect::RequestSafeAssetGc
        ]
    );
    let effects = receipt(&mut coordinator, id, Ok(()));
    assert_eq!(
        effects,
        [WindowEffect::WindowShown, WindowEffect::RequestRedraw]
    );
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::Presented(StableVisibility::Floating)
    );
    assert_eq!(
        coordinator.state().lifecycle(),
        LifecycleState::Quitting(QuitStage::AwaitingAssetGc)
    );

    let (mut coordinator, id) = showing(StableVisibility::Floating);
    coordinator.dispatch(WindowIntent::TrayQuitRequested {
        guards: WindowGuardSnapshot::default(),
    });
    let effects = receipt(&mut coordinator, id, Err(FailureKind::Failed));
    assert_eq!(effects, [WindowEffect::ShowRejected(FailureKind::Failed)]);
    assert!(matches!(
        coordinator.state().visibility(),
        VisibilityState::HiddenToTray { .. }
    ));
}

/// Nothing arms an auto-collapse while the show is unconfirmed, whatever the guards say, and the
/// commit starts no timer of its own: as ever the focus the show asked for decides that.
#[test]
fn a_pending_show_holds_no_collapse_deadline() {
    let docked = StableVisibility::DockedExpanded(DockEdge::Left);
    let (mut coordinator, id) = showing(docked);
    let effects = coordinator.dispatch(WindowIntent::GuardsChanged {
        guards: WindowGuardSnapshot::default(),
        now_ms: 20,
    });
    assert!(effects.is_empty());
    assert_eq!(
        coordinator.state().next_deadline_ms(),
        Some(10 + PLACEMENT_CONFIRM_TIMEOUT_MS),
        "only the show's placement waits"
    );
    coordinator.dispatch(WindowIntent::Tick {
        now_ms: 20 + AUTO_COLLAPSE_DELAY_MS,
    });
    assert!(matches!(
        coordinator.state().visibility(),
        VisibilityState::ShowPending { .. }
    ));

    coordinator.dispatch(WindowIntent::Tick {
        now_ms: 21 + AUTO_COLLAPSE_DELAY_MS,
    });
    receipt(&mut coordinator, id, Ok(()));
    coordinator.answer_pending_receipts();
    assert_eq!(coordinator.state().next_deadline_ms(), None);
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::Presented(docked)
    );
}

/// The guards the wait took in still count at the commit, and a rejected show does not start any
/// timer for a window that is not on screen.
#[test]
fn a_rejected_show_holds_no_deadline_and_keeps_the_guards() {
    let docked = StableVisibility::DockedExpanded(DockEdge::Left);
    let (mut coordinator, id) = showing(docked);
    let focused = WindowGuardSnapshot {
        window_focused: true,
        ..Default::default()
    };
    coordinator.dispatch(WindowIntent::GuardsChanged {
        guards: focused,
        now_ms: 20,
    });
    receipt(&mut coordinator, id, Err(FailureKind::Failed));
    assert_eq!(
        coordinator.state().next_deadline_ms(),
        Some(10 + PLACEMENT_CONFIRM_TIMEOUT_MS),
        "only the show's placement waits"
    );
    assert_eq!(coordinator.state().guards(), focused);
}

#[test]
fn a_drag_end_cannot_replace_a_pending_show() {
    let (mut coordinator, _) = showing(StableVisibility::Floating);
    let before = coordinator.state().visibility();
    let effects = coordinator.dispatch(WindowIntent::DragEnded {
        frame: PhysicalRect::new(5, 5, 520, 680),
        monitor: monitor("primary", 0),
        now_ms: 20,
    });
    assert!(effects.is_empty());
    assert_eq!(coordinator.state().visibility(), before);
}

/// A display change while the show waits moves the frame it asked for, not the wait.
#[test]
fn a_topology_change_keeps_the_pending_show_and_moves_its_frame() {
    let (mut coordinator, id) = showing(StableVisibility::DockedCollapsed(DockEdge::Left));
    let replacement = monitor("replacement", 4000);
    let effects = coordinator.dispatch(WindowIntent::DisplayTopologyChanged {
        monitors: vec![replacement.clone()],
        now_ms: 20,
    });
    assert!(matches!(
        coordinator.state().visibility(),
        VisibilityState::ShowPending { id: pending, .. } if pending == id
    ));
    assert!(effects.iter().any(|effect| matches!(
        effect,
        WindowEffect::ApplyFrame { target, last: true, .. } if target.x >= replacement.work_area.x
    )));
    receipt(&mut coordinator, id, Ok(()));
    assert!(replacement.work_area.contains(coordinator.state().frame()));
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::Presented(StableVisibility::DockedExpanded(DockEdge::Left))
    );
}

/// A window that never left the screen has nothing to show: the wake only ends the hide's freeze,
/// and the focus request reports back like any other. The hide's save then finds nothing to do.
#[test]
fn a_show_during_a_hide_save_cancels_the_freeze_without_a_show_action() {
    let mut coordinator = coordinator(StableVisibility::Floating);
    let dirty = WindowGuardSnapshot {
        note_save_required: true,
        ..Default::default()
    };
    coordinator.dispatch(WindowIntent::CloseRequested {
        now_ms: 0,
        guards: dirty,
    });
    assert!(matches!(
        coordinator.state().visibility(),
        VisibilityState::AwaitingHideSave { .. }
    ));
    let effects = show(&mut coordinator);
    assert_eq!(show_id(&effects), None);
    let focus = focus_id(&effects).expect("a focus request");
    assert!(matches!(
        effects[0],
        WindowEffect::ApplyFrame { last: true, .. }
    ));
    assert_eq!(
        effects[1..],
        [
            WindowEffect::WindowShown,
            WindowEffect::FocusWindow { id: focus },
            WindowEffect::SetImeAllowed(true),
            WindowEffect::SetEditorInputEnabled(true),
            WindowEffect::RequestRedraw,
        ]
    );
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::Presented(StableVisibility::Floating)
    );
    assert!(
        coordinator
            .dispatch(WindowIntent::HideSaveCompleted {
                succeeded: true,
                guards: WindowGuardSnapshot::default(),
                now_ms: 30,
            })
            .is_empty()
    );
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::Presented(StableVisibility::Floating)
    );
}

/// A show that overtakes a hide in flight kills the hide's identity, so a rejection cannot return
/// to waiting on it: that wait could never end.
#[test]
fn a_rejected_show_after_an_overtaken_hide_rests_hidden() {
    let mut coordinator = coordinator(StableVisibility::Floating);
    let effects = coordinator.dispatch(WindowIntent::CloseRequested {
        now_ms: 0,
        guards: WindowGuardSnapshot::default(),
    });
    let hide = hide_id(&effects).unwrap();
    let show_effects = show(&mut coordinator);
    let id = show_id(&show_effects).unwrap();
    assert!(receipt(&mut coordinator, hide, Ok(())).is_empty());
    let effects = receipt(&mut coordinator, id, Err(FailureKind::Failed));
    assert_eq!(effects, [WindowEffect::ShowRejected(FailureKind::Failed)]);
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::HiddenToTray {
            restore: StableVisibility::Floating
        }
    );
}

/// A visible window has no pending state: a good receipt commits nothing, a bad one is reported
/// and changes nothing, and a stale one is ignored.
#[test]
fn a_focus_request_commits_nothing_and_reports_a_rejection() {
    let mut coordinator = coordinator(StableVisibility::Floating);
    let effects = show(&mut coordinator);
    let id = focus_id(&effects).expect("a focus request");
    assert_eq!(effects, [WindowEffect::FocusWindow { id }]);
    assert!(receipt(&mut coordinator, id, Ok(())).is_empty());

    for kind in REJECTIONS {
        let effects = show(&mut coordinator);
        let id = focus_id(&effects).unwrap();
        assert!(receipt(&mut coordinator, id + 100, Err(kind)).is_empty());
        assert_eq!(
            receipt(&mut coordinator, id, Err(kind)),
            [WindowEffect::ShowRejected(kind)]
        );
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::Floating)
        );
    }

    let first = focus_id(&show(&mut coordinator)).unwrap();
    let second = focus_id(&show(&mut coordinator)).unwrap();
    assert!(
        receipt(&mut coordinator, first, Err(FailureKind::Failed)).is_empty(),
        "a newer focus request replaced this one"
    );
    assert!(receipt(&mut coordinator, second, Ok(())).is_empty());
}

/// A show's success does not claim focus: only a real focus event sets the guard.
#[test]
fn a_committed_show_claims_no_focus() {
    let (mut coordinator, id) = showing(StableVisibility::Floating);
    let effects = receipt(&mut coordinator, id, Ok(()));
    let focus = focus_id(&effects).unwrap();
    receipt(&mut coordinator, focus, Ok(()));
    assert!(!coordinator.state().guards().window_focused);
}

fn tick(coordinator: &mut WindowShellCoordinator, now_ms: u64) -> Vec<WindowEffect> {
    coordinator.dispatch(WindowIntent::Tick { now_ms })
}

fn overdue(start_ms: u64) -> u64 {
    start_ms + WINDOW_ACTION_RECEIPT_TIMEOUT_MS
}

/// A show whose receipt never comes ends at its deadline: the id dies, input stays off, the same
/// target can be asked for again, and the late receipt commits nothing.
#[test]
fn a_show_with_no_receipt_ends_hidden_with_input_off() {
    let (mut coordinator, id) = showing(StableVisibility::Floating);
    assert!(
        tick(&mut coordinator, overdue(10) - 1)
            .iter()
            .all(|effect| matches!(
                effect,
                WindowEffect::ObservePlacement { .. } | WindowEffect::PlacementFailed(_)
            ))
    );
    let effects = tick(&mut coordinator, overdue(10));
    assert!(effects.contains(&WindowEffect::ShowRejected(FailureKind::Failed)));
    assert!(!input_effects(&effects));
    assert!(matches!(
        coordinator.state().visibility(),
        VisibilityState::HiddenToTray { .. }
    ));
    assert!(receipt(&mut coordinator, id, Ok(())).is_empty());
    assert!(matches!(
        coordinator.state().visibility(),
        VisibilityState::HiddenToTray { .. }
    ));
    let second = show_id(&show(&mut coordinator)).expect("asked for again");
    assert!(input_effects(&receipt(&mut coordinator, second, Ok(()))));
}

/// A hide whose receipt never comes ends as a rejected hide: shown, with input back.
#[test]
fn a_hide_with_no_receipt_is_rejected_at_its_deadline() {
    let mut coordinator = coordinator(StableVisibility::Floating);
    let close = coordinator.dispatch(WindowIntent::CloseRequested {
        now_ms: 100,
        guards: WindowGuardSnapshot::default(),
    });
    let id = hide_id(&close).unwrap();
    assert_eq!(coordinator.state().next_deadline_ms(), Some(overdue(100)));
    let effects = tick(&mut coordinator, overdue(100));
    assert!(effects.contains(&WindowEffect::HideRejected(FailureKind::Failed)));
    assert!(effects.contains(&WindowEffect::SetEditorInputEnabled(true)));
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::Presented(StableVisibility::Floating)
    );
    assert!(receipt(&mut coordinator, id, Ok(())).is_empty());
}

/// A focus request with no receipt ends with the same report, and nothing else moves.
#[test]
fn a_focus_with_no_receipt_is_reported_at_its_deadline() {
    let mut coordinator = coordinator(StableVisibility::Floating);
    let id = focus_id(&show(&mut coordinator)).unwrap();
    assert_eq!(coordinator.state().next_deadline_ms(), Some(overdue(10)));
    assert_eq!(
        tick(&mut coordinator, overdue(10)),
        [WindowEffect::ShowRejected(FailureKind::Failed)]
    );
    assert_eq!(coordinator.state().next_deadline_ms(), None);
    assert!(receipt(&mut coordinator, id, Err(FailureKind::Failed)).is_empty());
}

fn dirty() -> WindowGuardSnapshot {
    WindowGuardSnapshot {
        note_save_required: true,
        ..Default::default()
    }
}

fn quit_and_fail_the_save(coordinator: &mut WindowShellCoordinator) -> Vec<WindowEffect> {
    coordinator.dispatch(WindowIntent::QuitBarrierCompleted {
        barrier: QuitBarrier::NoteSave,
        succeeded: false,
        guards: dirty(),
    })
}

/// A quit cancelled while the window is hidden gives it no input and no IME.
#[test]
fn a_cancelled_quit_leaves_a_hidden_window_without_input() {
    let (mut coordinator, id) = showing(StableVisibility::Floating);
    coordinator.dispatch(WindowIntent::TrayQuitRequested { guards: dirty() });
    receipt(&mut coordinator, id, Err(FailureKind::Failed));
    let effects = quit_and_fail_the_save(&mut coordinator);
    assert_eq!(
        effects,
        [
            WindowEffect::SetImeAllowed(false),
            WindowEffect::SetEditorInputEnabled(false),
            WindowEffect::QuitCancelled(QuitBarrier::NoteSave),
        ]
    );
}

/// A show that landed during the quit took no input then; cancelling the quit restores it.
#[test]
fn a_cancelled_quit_restores_input_for_a_show_that_landed_during_it() {
    let (mut coordinator, id) = showing(StableVisibility::Floating);
    coordinator.dispatch(WindowIntent::TrayQuitRequested { guards: dirty() });
    let effects = receipt(&mut coordinator, id, Ok(()));
    assert!(!input_effects(&effects));
    let effects = quit_and_fail_the_save(&mut coordinator);
    assert_eq!(
        effects,
        [
            WindowEffect::SetImeAllowed(true),
            WindowEffect::SetEditorInputEnabled(true),
            WindowEffect::QuitCancelled(QuitBarrier::NoteSave),
        ]
    );
}

/// A drag that ended while the window is on its way to the tray does not bring it back.
#[test]
fn a_drag_end_cannot_overwrite_a_hide_in_progress() {
    let mut coordinator = coordinator(StableVisibility::Floating);
    coordinator.dispatch(WindowIntent::MoveResizeStarted);
    let close = coordinator.dispatch(WindowIntent::CloseRequested {
        now_ms: 0,
        guards: WindowGuardSnapshot::default(),
    });
    let id = hide_id(&close).unwrap();
    coordinator.dispatch(WindowIntent::MoveResizeEnded);
    let effects = coordinator.dispatch(WindowIntent::DragEnded {
        frame: PhysicalRect::new(5, 5, 520, 680),
        monitor: monitor("primary", 0),
        now_ms: 20,
    });
    assert!(effects.is_empty());
    assert!(matches!(
        coordinator.state().visibility(),
        VisibilityState::HidePending { .. }
    ));
    receipt(&mut coordinator, id, Ok(()));
    assert!(matches!(
        coordinator.state().visibility(),
        VisibilityState::HiddenToTray { .. }
    ));

    // The same drag end after the hide landed, and while a save still holds the hide.
    let effects = coordinator.dispatch(WindowIntent::DragEnded {
        frame: PhysicalRect::new(5, 5, 520, 680),
        monitor: monitor("primary", 0),
        now_ms: 30,
    });
    assert!(effects.is_empty());
    assert!(matches!(
        coordinator.state().visibility(),
        VisibilityState::HiddenToTray { .. }
    ));
    let mut waiting = self::coordinator(StableVisibility::Floating);
    waiting.dispatch(WindowIntent::CloseRequested {
        now_ms: 0,
        guards: dirty(),
    });
    let effects = waiting.dispatch(WindowIntent::DragEnded {
        frame: PhysicalRect::new(5, 5, 520, 680),
        monitor: monitor("primary", 0),
        now_ms: 40,
    });
    assert!(effects.is_empty());
    assert!(matches!(
        waiting.state().visibility(),
        VisibilityState::AwaitingHideSave { .. }
    ));
}

/// Once the ids run out every request fails the way it would have failed anyway, and no id is
/// ever used again.
#[test]
fn exhausted_action_ids_fail_every_request_without_reuse() {
    let exhausted = |visibility| {
        let mut coordinator = self::coordinator(visibility);
        coordinator.state.last_window_action = u64::MAX;
        coordinator
    };

    let mut hidden_window = hidden(StableVisibility::Floating);
    hidden_window.state.last_window_action = u64::MAX;
    assert_eq!(
        show(&mut hidden_window),
        [WindowEffect::ShowRejected(FailureKind::Failed)]
    );
    assert!(matches!(
        hidden_window.state().visibility(),
        VisibilityState::HiddenToTray { .. }
    ));

    let mut visible = exhausted(StableVisibility::Floating);
    assert_eq!(
        show(&mut visible),
        [WindowEffect::ShowRejected(FailureKind::Failed)]
    );

    let mut closing = exhausted(StableVisibility::Floating);
    let effects = closing.dispatch(WindowIntent::CloseRequested {
        now_ms: 0,
        guards: WindowGuardSnapshot::default(),
    });
    assert_eq!(hide_id(&effects), None);
    assert!(effects.contains(&WindowEffect::HideRejected(FailureKind::Failed)));
    assert!(effects.contains(&WindowEffect::SetEditorInputEnabled(true)));

    let mut topmost = exhausted(StableVisibility::Floating);
    let effects = topmost.dispatch(WindowIntent::UserTopmostRequested {
        on: true,
        now_ms: 0,
    });
    assert_eq!(
        effects,
        [WindowEffect::TopmostRejected {
            source: TopmostSource::User,
            kind: FailureKind::Failed,
        }]
    );

    let mut dragged = exhausted(StableVisibility::Floating);
    let effects = dragged.dispatch(WindowIntent::DragEnded {
        frame: PhysicalRect::new(500, 100, 780, 900),
        monitor: monitor("primary", 0),
        now_ms: 10,
    });
    assert_eq!(dragged.state().pending_placement_id(), None);
    assert!(effects.contains(&WindowEffect::PlacementFailed(FailureKind::Failed)));
    assert!(effects.iter().all(|effect| !matches!(
        effect,
        WindowEffect::ApplyFrame { .. } | WindowEffect::CommitPlacement { .. }
    )));
    assert_eq!(dragged.state.last_window_action, u64::MAX);
}

/// The last id goes to a request that is still out; the next request of each kind fails on its
/// own, sends nothing native, reuses nothing, and the request in flight keeps its receipt.
#[test]
fn a_request_after_the_last_id_fails_alone() {
    let last = u64::MAX;
    let at_last_id = |coordinator: &mut WindowShellCoordinator| {
        coordinator.state.last_window_action = last - 1;
    };

    // A second show while the first waits: no second request, and the first can still land.
    let mut coordinator = hidden(StableVisibility::Floating);
    coordinator.state.last_window_action = last - 2;
    let first = show(&mut coordinator);
    assert_eq!(show_id(&first), Some(last - 1));
    let effects = show(&mut coordinator);
    assert_eq!(effects, [WindowEffect::ShowRejected(FailureKind::Failed)]);
    assert!(input_effects(&receipt(&mut coordinator, last - 1, Ok(()))));

    // A second focus: no new id, and the old receipt still only reports.
    let mut coordinator = self::coordinator(StableVisibility::Floating);
    at_last_id(&mut coordinator);
    assert_eq!(focus_id(&show(&mut coordinator)), Some(last));
    assert_eq!(
        show(&mut coordinator),
        [WindowEffect::ShowRejected(FailureKind::Failed)]
    );
    assert!(receipt(&mut coordinator, last, Ok(())).is_empty());

    // A hide after the last id went to a topmost request: the window stays, its input back.
    let mut coordinator = self::coordinator(StableVisibility::Floating);
    at_last_id(&mut coordinator);
    coordinator.dispatch(WindowIntent::UserTopmostRequested {
        on: true,
        now_ms: 0,
    });
    let effects = coordinator.dispatch(WindowIntent::CloseRequested {
        now_ms: 0,
        guards: WindowGuardSnapshot::default(),
    });
    assert_eq!(hide_id(&effects), None);
    assert!(effects.contains(&WindowEffect::HideRejected(FailureKind::Failed)));

    // A topmost request while the last id is out: rejected, the one in flight stands.
    let effects = coordinator.dispatch(WindowIntent::UserTopmostRequested {
        on: false,
        now_ms: 0,
    });
    assert_eq!(
        effects,
        [WindowEffect::TopmostRejected {
            source: TopmostSource::User,
            kind: FailureKind::Failed,
        }]
    );
    assert_eq!(
        receipt(&mut coordinator, last, Ok(())),
        [WindowEffect::CommitAlwaysOnTop(true)]
    );
}

/// A host that answers after the expiry tick answers too late: the id is dead and the window gets
/// a new request instead.
#[test]
fn a_success_after_the_expiry_tick_commits_nothing_and_a_retry_works() {
    let (mut coordinator, id) = showing(StableVisibility::Floating);
    tick(&mut coordinator, overdue(10));
    assert!(receipt(&mut coordinator, id, Ok(())).is_empty());
    assert!(matches!(
        coordinator.state().visibility(),
        VisibilityState::HiddenToTray { .. }
    ));
    let retry = show_id(&show(&mut coordinator)).unwrap();
    assert!(input_effects(&receipt(&mut coordinator, retry, Ok(()))));

    let mut coordinator = self::coordinator(StableVisibility::Floating);
    let close = coordinator.dispatch(WindowIntent::CloseRequested {
        now_ms: 100,
        guards: WindowGuardSnapshot::default(),
    });
    let id = hide_id(&close).unwrap();
    tick(&mut coordinator, overdue(100));
    assert!(receipt(&mut coordinator, id, Ok(())).is_empty());
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::Presented(StableVisibility::Floating)
    );
    let retry = coordinator.dispatch(WindowIntent::CloseRequested {
        now_ms: 3_000,
        guards: WindowGuardSnapshot::default(),
    });
    let id = hide_id(&retry).expect("the same target asked for again");
    assert_eq!(
        receipt(&mut coordinator, id, Ok(())),
        [WindowEffect::ReleaseHiddenCaches]
    );
}

/// The deadline takes effect when a tick processes it. A synchronous show that took longer than
/// the deadline still counts: judging it failed would leave a visible window recorded as hidden.
#[test]
fn a_success_past_the_deadline_but_before_the_expiry_tick_commits() {
    let late = overdue(10) + 1;
    let (mut coordinator, id) = showing(StableVisibility::Floating);
    let effects = coordinator.dispatch(WindowIntent::ActionCompleted {
        id,
        outcome: Ok(()),
        now_ms: late,
    });
    assert!(input_effects(&effects));
    assert_eq!(
        coordinator.state().visibility(),
        VisibilityState::Presented(StableVisibility::Floating)
    );

    let mut coordinator = self::coordinator(StableVisibility::Floating);
    let close = coordinator.dispatch(WindowIntent::CloseRequested {
        now_ms: 0,
        guards: WindowGuardSnapshot::default(),
    });
    let id = hide_id(&close).unwrap();
    let effects = coordinator.dispatch(WindowIntent::ActionCompleted {
        id,
        outcome: Ok(()),
        now_ms: overdue(0) + 1,
    });
    assert_eq!(effects, [WindowEffect::ReleaseHiddenCaches]);
    assert!(matches!(
        coordinator.state().visibility(),
        VisibilityState::HiddenToTray { .. }
    ));
}

/// A show that could not also get its placement's id keeps the prior state and sends nothing, so a
/// window hidden while collapsed is not committed expanded at its collapsed rectangle.
#[test]
fn a_show_needs_an_id_for_its_placement_too() {
    let collapsed = StableVisibility::DockedCollapsed(DockEdge::Left);
    for counter in [u64::MAX - 1, u64::MAX] {
        let mut coordinator = hidden(collapsed);
        coordinator.state.last_window_action = counter;
        let prior = coordinator.state().visibility();
        let effects = show(&mut coordinator);
        assert_eq!(effects, [WindowEffect::ShowRejected(FailureKind::Failed)]);
        assert_eq!(coordinator.state().visibility(), prior);
        assert_eq!(coordinator.state.last_window_action, counter);
    }

    let mut coordinator = hidden(collapsed);
    coordinator.state.last_window_action = u64::MAX - 2;
    let effects = show(&mut coordinator);
    assert_eq!(show_id(&effects), Some(u64::MAX - 1));
    assert!(matches!(
        effects[0],
        WindowEffect::ApplyFrame { last: true, .. }
    ));
    assert_eq!(coordinator.state().pending_placement_id(), Some(u64::MAX));
}

/// A wake during a hide save does not cancel the freeze when its frame cannot be placed either.
#[test]
fn a_cancelled_hide_save_needs_an_id_for_its_placement() {
    let mut coordinator = coordinator(StableVisibility::DockedCollapsed(DockEdge::Left));
    coordinator.dispatch(WindowIntent::CloseRequested {
        now_ms: 0,
        guards: dirty(),
    });
    coordinator.state.last_window_action = u64::MAX;
    let prior = coordinator.state().visibility();
    let effects = show(&mut coordinator);
    assert_eq!(effects, [WindowEffect::ShowRejected(FailureKind::Failed)]);
    assert_eq!(coordinator.state().visibility(), prior);
}
