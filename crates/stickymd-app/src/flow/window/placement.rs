//! Window placement as request, observation and verdict (plan 04 window-action-receipts). Each
//! `last` placement gets exactly one verdict: its target is confirmed by an observed frame, the
//! host reports it failed, or the confirmation times out. A newer placement replaces an older one
//! without a verdict, and a late observation then commits nothing.
//!
//! Animation frames that are not the last are presentation requests with no receipt.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#window-action-receipts

use super::geometry::{MonitorGeometry, PhysicalRect, placement_from_frame};
use super::reducer::WindowShellCoordinator;
use super::state::{DockEdge, PLACEMENT_CONFIRM_TIMEOUT_MS, PLACEMENT_TOLERANCE_PX, WindowEffect};
use crate::ports::{FailureKind, FrameFact};

/// The id of a frame that gets no receipt; actions are numbered from 1.
pub(super) const NO_RECEIPT: u64 = 0;

/// What a confirmed placement writes.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum PlacementCommit {
    /// The end of a drag: the placement is what was observed on `monitor`.
    FromObserved {
        monitor: MonitorGeometry,
        dock_edge: Option<DockEdge>,
    },
    /// A topology recovery: only the monitor changed. The size and ratios stay the user's, so a
    /// frame clamped to a smaller display is not saved as the size the user chose.
    AsRequested { dock_edge: Option<DockEdge> },
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PendingPlacement {
    id: u64,
    target: PhysicalRect,
    deadline_ms: u64,
    /// The deadline's own observation was asked for, so the next tick past it is the end.
    final_observation_requested: bool,
    commit: Option<PlacementCommit>,
}

impl PendingPlacement {
    pub(super) fn id(&self) -> u64 {
        self.id
    }

    pub(super) fn deadline_ms(&self) -> u64 {
        self.deadline_ms
    }
}

/// Whether `observed` is `target`, edge by edge.
// The comparison is the contract (plan 04); the tolerance only happens to be zero today.
#[allow(clippy::absurd_extreme_comparisons)]
fn within_tolerance(observed: PhysicalRect, target: PhysicalRect) -> bool {
    observed.x.abs_diff(target.x) <= PLACEMENT_TOLERANCE_PX
        && observed.y.abs_diff(target.y) <= PLACEMENT_TOLERANCE_PX
        && observed.width.abs_diff(target.width) <= PLACEMENT_TOLERANCE_PX
        && observed.height.abs_diff(target.height) <= PLACEMENT_TOLERANCE_PX
}

impl WindowShellCoordinator {
    /// Asks for a frame that gets a verdict, replacing any placement still waiting for one. While
    /// a native drag or resize runs the window is not where a request puts it, so the frame is
    /// only presented: no wait, no verdict, nothing written. The drag's own end is what is judged.
    pub(super) fn request_placement(
        &mut self,
        target: PhysicalRect,
        commit: Option<PlacementCommit>,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        if self.state.native_move_active {
            effects.push(WindowEffect::ApplyFrame {
                id: NO_RECEIPT,
                target,
                last: false,
            });
            return;
        }
        let Some(id) = self.next_window_action() else {
            // Nothing can be judged any more, so nothing is sent. What was waiting ends too: its
            // observation would otherwise confirm the newer placement this one asked for.
            self.abandon_placement();
            effects.push(WindowEffect::PlacementFailed(FailureKind::Failed));
            return;
        };
        self.state.placement_pending = Some(PendingPlacement {
            id,
            target,
            deadline_ms: now_ms.saturating_add(PLACEMENT_CONFIRM_TIMEOUT_MS),
            final_observation_requested: false,
            commit,
        });
        effects.push(WindowEffect::ApplyFrame {
            id,
            target,
            last: true,
        });
    }

    /// A newer movement of the window takes over: what was waiting is never judged.
    pub(super) fn abandon_placement(&mut self) {
        self.state.placement_pending = None;
    }

    /// The window is under the user's hand from now, which is no confirmation of what was asked.
    pub(super) fn start_native_move(&mut self) {
        self.state.native_move_active = true;
        self.abandon_placement();
    }

    pub(super) fn observe_placement(
        &mut self,
        id: u64,
        observed: Result<FrameFact, FailureKind>,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        let Some(pending) = self
            .state
            .placement_pending
            .as_ref()
            .filter(|pending| pending.id == id)
        else {
            return;
        };
        let target = pending.target;
        let deadline_ms = pending.deadline_ms;
        match observed {
            Ok(fact) if within_tolerance(fact.outer, target) => {
                self.confirm_placement(fact, effects)
            }
            // This host cannot ever report the frame; waiting would only delay the same answer.
            Err(FailureKind::Unsupported) => {
                self.fail_placement(FailureKind::Unsupported, effects);
            }
            _ if now_ms >= deadline_ms => self.fail_placement(FailureKind::Failed, effects),
            _ => {}
        }
    }

    fn confirm_placement(&mut self, fact: FrameFact, effects: &mut Vec<WindowEffect>) {
        let Some(pending) = self.state.placement_pending.take() else {
            return;
        };
        let Some(commit) = pending.commit else {
            return;
        };
        let (placement, dock_edge) = match commit {
            PlacementCommit::FromObserved { monitor, dock_edge } => (
                placement_from_frame(fact.outer, &monitor, &self.state.placement, dock_edge),
                dock_edge,
            ),
            PlacementCommit::AsRequested { dock_edge } => (self.state.placement.clone(), dock_edge),
        };
        self.state.placement = placement.clone();
        self.state.confirmed_placement = placement.clone();
        self.state.confirmed_dock_edge = dock_edge;
        effects.push(WindowEffect::CommitPlacement {
            placement,
            dock_edge,
        });
    }

    fn fail_placement(&mut self, kind: FailureKind, effects: &mut Vec<WindowEffect>) {
        self.state.placement_pending = None;
        effects.push(WindowEffect::PlacementFailed(kind));
    }

    /// The host could not even start action `id`. Whether `id` was a placement waiting for this.
    pub(super) fn complete_placement(
        &mut self,
        id: u64,
        outcome: Result<(), FailureKind>,
        effects: &mut Vec<WindowEffect>,
    ) -> bool {
        if self
            .state
            .placement_pending
            .as_ref()
            .is_none_or(|pending| pending.id != id)
        {
            return false;
        }
        if let Err(kind) = outcome {
            self.fail_placement(kind, effects);
        }
        true
    }

    /// At the deadline the window is asked once more, since a window already at its target sends
    /// no move event. A tick past that with no answer ends the wait all the same.
    pub(super) fn tick_placement(&mut self, now_ms: u64, effects: &mut Vec<WindowEffect>) {
        let Some(pending) = self
            .state
            .placement_pending
            .as_mut()
            .filter(|pending| now_ms >= pending.deadline_ms)
        else {
            return;
        };
        if pending.final_observation_requested {
            self.fail_placement(FailureKind::Failed, effects);
        } else {
            pending.final_observation_requested = true;
            effects.push(WindowEffect::ObservePlacement { id: pending.id });
        }
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::super::geometry::{MonitorIdentity, WindowPlacement};
    use super::super::state::{
        ANIMATION_DURATION_MS, QuitStage, ShowReason, StableVisibility, VisibilityState,
        WindowGuardSnapshot, WindowIntent,
    };
    use super::*;
    use crate::flow::window::state::LifecycleState;

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

    /// The observation of `frame`, as a host reports it.
    pub(in super::super) fn fact(frame: PhysicalRect) -> FrameFact {
        FrameFact { outer: frame }
    }

    /// The placement a set of effects asks for that gets a verdict.
    pub(in super::super) fn placed(effects: &[WindowEffect]) -> Option<(u64, PhysicalRect)> {
        effects.iter().find_map(|effect| match effect {
            WindowEffect::ApplyFrame {
                id,
                target,
                last: true,
            } => Some((*id, *target)),
            _ => None,
        })
    }

    /// What a host does after `ApplyFrame`: reports that the window is at the target.
    pub(in super::super) fn observe_at_target(
        coordinator: &mut WindowShellCoordinator,
        effects: &[WindowEffect],
    ) -> Vec<WindowEffect> {
        let (id, target) = placed(effects).expect("a placement");
        coordinator.dispatch(WindowIntent::PlacementObserved {
            id,
            observed: Ok(fact(target)),
            now_ms: 1,
        })
    }

    fn observe(
        coordinator: &mut WindowShellCoordinator,
        id: u64,
        observed: Result<FrameFact, FailureKind>,
        now_ms: u64,
    ) -> Vec<WindowEffect> {
        coordinator.dispatch(WindowIntent::PlacementObserved {
            id,
            observed,
            now_ms,
        })
    }

    fn committed(effects: &[WindowEffect]) -> bool {
        effects
            .iter()
            .any(|effect| matches!(effect, WindowEffect::CommitPlacement { .. }))
    }

    fn drag(coordinator: &mut WindowShellCoordinator, frame: PhysicalRect) -> Vec<WindowEffect> {
        coordinator.dispatch(WindowIntent::DragEnded {
            frame,
            monitor: monitor("primary", 0),
            now_ms: 10,
        })
    }

    fn left_edge_drag(coordinator: &mut WindowShellCoordinator) -> Vec<WindowEffect> {
        drag(coordinator, PhysicalRect::new(10, 100, 780, 900))
    }

    fn floating_drag(coordinator: &mut WindowShellCoordinator) -> Vec<WindowEffect> {
        drag(coordinator, PhysicalRect::new(500, 100, 780, 900))
    }

    /// A drag's placement waits for its frame: no commit in the same dispatch.
    #[test]
    fn a_drag_commits_nothing_until_the_frame_is_confirmed() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = left_edge_drag(&mut coordinator);
        assert!(!committed(&effects));
        let (id, target) = placed(&effects).expect("a placement");
        assert_eq!(coordinator.state().pending_placement_id(), Some(id));
        let effects = observe(&mut coordinator, id, Ok(fact(target)), 11);
        assert!(committed(&effects));
        assert_eq!(coordinator.state().pending_placement_id(), None);
    }

    /// No move event is needed: one observation right after the request can settle it, which is
    /// the case for a window already where it was asked to be.
    #[test]
    fn a_placement_confirms_without_any_move_event() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = floating_drag(&mut coordinator);
        let (id, target) = placed(&effects).unwrap();
        assert_eq!(target, coordinator.state().frame());
        assert!(committed(&observe(
            &mut coordinator,
            id,
            Ok(fact(target)),
            10
        )));
    }

    #[test]
    fn a_confirmed_drag_writes_what_was_observed() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = floating_drag(&mut coordinator);
        let (id, target) = placed(&effects).unwrap();
        let effects = observe(&mut coordinator, id, Ok(fact(target)), 11);
        let WindowEffect::CommitPlacement {
            placement,
            dock_edge,
        } = &effects[0]
        else {
            panic!("{effects:?}");
        };
        assert_eq!(*dock_edge, None);
        assert_eq!(
            *placement,
            placement_from_frame(
                target,
                &monitor("primary", 0),
                &coordinator.state.placement,
                None
            )
        );
    }

    /// Any single edge off by one is a mismatch at a tolerance of 0: it waits, then fails.
    #[test]
    fn the_tolerance_is_exact_on_every_edge() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = floating_drag(&mut coordinator);
        let (id, target) = placed(&effects).unwrap();
        let off = [
            PhysicalRect::new(target.x + 1, target.y, target.width, target.height),
            PhysicalRect::new(target.x, target.y - 1, target.width, target.height),
            PhysicalRect::new(target.x, target.y, target.width + 1, target.height),
            PhysicalRect::new(target.x, target.y, target.width, target.height - 1),
        ];
        for frame in off {
            assert!(observe(&mut coordinator, id, Ok(fact(frame)), 11).is_empty());
            assert_eq!(coordinator.state().pending_placement_id(), Some(id));
        }
        assert!(committed(&observe(
            &mut coordinator,
            id,
            Ok(fact(target)),
            11
        )));
    }

    #[test]
    fn an_observation_past_the_deadline_that_does_not_match_fails_without_commit() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = floating_drag(&mut coordinator);
        let (id, target) = placed(&effects).unwrap();
        let wrong = PhysicalRect::new(target.x + 40, target.y, target.width, target.height);
        let effects = observe(
            &mut coordinator,
            id,
            Ok(fact(wrong)),
            10 + PLACEMENT_CONFIRM_TIMEOUT_MS,
        );
        assert_eq!(
            effects,
            [WindowEffect::PlacementFailed(FailureKind::Failed)]
        );
        assert_eq!(coordinator.state().pending_placement_id(), None);
    }

    /// The deadline's own observation is asked for once; a confirmation it brings still counts.
    #[test]
    fn the_deadline_asks_once_more_and_can_still_confirm() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = floating_drag(&mut coordinator);
        let (id, target) = placed(&effects).unwrap();
        let deadline = 10 + PLACEMENT_CONFIRM_TIMEOUT_MS;
        assert_eq!(coordinator.state().next_deadline_ms(), Some(deadline));
        assert!(
            coordinator
                .dispatch(WindowIntent::Tick {
                    now_ms: deadline - 1
                })
                .is_empty()
        );
        assert_eq!(
            coordinator.dispatch(WindowIntent::Tick { now_ms: deadline }),
            [WindowEffect::ObservePlacement { id }]
        );
        assert!(committed(&observe(
            &mut coordinator,
            id,
            Ok(fact(target)),
            deadline
        )));
    }

    /// If the deadline's observation never arrives the wait still ends, at the next tick.
    #[test]
    fn a_placement_never_waits_forever() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = floating_drag(&mut coordinator);
        let (id, _) = placed(&effects).unwrap();
        let deadline = 10 + PLACEMENT_CONFIRM_TIMEOUT_MS;
        coordinator.dispatch(WindowIntent::Tick { now_ms: deadline });
        assert_eq!(coordinator.state().pending_placement_id(), Some(id));
        assert_eq!(
            coordinator.dispatch(WindowIntent::Tick {
                now_ms: deadline + 1
            }),
            [WindowEffect::PlacementFailed(FailureKind::Failed)]
        );
        assert_eq!(coordinator.state().pending_placement_id(), None);
        assert_eq!(coordinator.state().next_deadline_ms(), None);
    }

    /// The host could not start the move: the verdict is that failure, at once.
    #[test]
    fn a_failed_apply_ends_the_wait_without_commit() {
        for kind in [
            FailureKind::Failed,
            FailureKind::Unavailable,
            FailureKind::Unsupported,
        ] {
            let mut coordinator = coordinator(StableVisibility::Floating);
            let effects = floating_drag(&mut coordinator);
            let (id, _) = placed(&effects).unwrap();
            let effects = coordinator.dispatch(WindowIntent::ActionCompleted {
                id,
                outcome: Err(kind),
                now_ms: 11,
            });
            assert_eq!(effects, [WindowEffect::PlacementFailed(kind)]);
            assert_eq!(coordinator.state().pending_placement_id(), None);
        }
    }

    #[test]
    fn a_host_that_cannot_observe_ends_the_wait_at_once() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = floating_drag(&mut coordinator);
        let (id, _) = placed(&effects).unwrap();
        let effects = observe(&mut coordinator, id, Err(FailureKind::Unsupported), 11);
        assert_eq!(
            effects,
            [WindowEffect::PlacementFailed(FailureKind::Unsupported)]
        );
    }

    /// A transient read failure does not decide anything until the deadline does.
    #[test]
    fn an_unavailable_observation_waits_for_the_deadline() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = floating_drag(&mut coordinator);
        let (id, target) = placed(&effects).unwrap();
        assert!(observe(&mut coordinator, id, Err(FailureKind::Unavailable), 11).is_empty());
        assert_eq!(coordinator.state().pending_placement_id(), Some(id));
        assert!(committed(&observe(
            &mut coordinator,
            id,
            Ok(fact(target)),
            12
        )));
    }

    #[test]
    fn a_stale_observation_commits_nothing() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = floating_drag(&mut coordinator);
        let (id, target) = placed(&effects).unwrap();
        assert!(observe(&mut coordinator, id + 100, Ok(fact(target)), 11).is_empty());
        assert_eq!(coordinator.state().pending_placement_id(), Some(id));
    }

    /// A newer placement replaces an older one with no verdict for the old: its late observation
    /// commits nothing, and only the newer one's confirmation writes.
    #[test]
    fn a_newer_placement_supersedes_the_older_receipt() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let first = placed(&left_edge_drag(&mut coordinator)).unwrap();
        let second = placed(&floating_drag(&mut coordinator)).unwrap();
        assert_ne!(first.0, second.0);
        assert!(observe(&mut coordinator, first.0, Ok(fact(first.1)), 11).is_empty());
        assert_eq!(coordinator.state().pending_placement_id(), Some(second.0));
        assert!(committed(&observe(
            &mut coordinator,
            second.0,
            Ok(fact(second.1)),
            11
        )));
    }

    /// A failed verdict ends only that placement: the next one is judged on its own.
    #[test]
    fn a_failed_placement_can_be_retried() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let (id, _) = placed(&floating_drag(&mut coordinator)).unwrap();
        observe(&mut coordinator, id, Err(FailureKind::Unsupported), 11);
        let retry = floating_drag(&mut coordinator);
        let (retried, target) = placed(&retry).unwrap();
        assert_ne!(retried, id);
        assert!(committed(&observe(
            &mut coordinator,
            retried,
            Ok(fact(target)),
            20
        )));
    }

    /// Animation frames before the last have no receipt; the last one gets exactly one verdict.
    #[test]
    fn only_the_last_animation_frame_is_judged() {
        let mut coordinator = coordinator(StableVisibility::DockedExpanded(
            super::super::state::DockEdge::Left,
        ));
        coordinator.dispatch(WindowIntent::ManualCollapse { now_ms: 100 });
        let intermediate = coordinator.dispatch(WindowIntent::Tick { now_ms: 116 });
        assert!(matches!(
            intermediate[0],
            WindowEffect::ApplyFrame { last: false, .. }
        ));
        assert_eq!(coordinator.state().pending_placement_id(), None);
        assert!(
            !intermediate
                .iter()
                .any(|effect| matches!(effect, WindowEffect::ObservePlacement { .. }))
        );
        let last = coordinator.dispatch(WindowIntent::Tick {
            now_ms: 100 + ANIMATION_DURATION_MS,
        });
        let (id, target) = placed(&last).expect("the final frame is judged");
        assert_eq!(target, coordinator.state().frame());
        assert!(
            observe(&mut coordinator, id, Ok(fact(target)), 300).is_empty(),
            "a dock animation writes no placement"
        );
        assert_eq!(coordinator.state().pending_placement_id(), None);
        coordinator.answer_pending_receipts();
        assert_eq!(coordinator.state().next_deadline_ms(), None);
    }

    /// A new animation replaces whatever placement was waiting: reversing or interrupting one
    /// leaves only the newest target to be judged, once.
    #[test]
    fn an_animation_supersedes_a_waiting_placement() {
        let mut coordinator = coordinator(StableVisibility::DockedExpanded(
            super::super::state::DockEdge::Left,
        ));
        coordinator.dispatch(WindowIntent::ManualCollapse { now_ms: 100 });
        let last = coordinator.dispatch(WindowIntent::Tick {
            now_ms: 100 + ANIMATION_DURATION_MS,
        });
        let (id, target) = placed(&last).unwrap();
        coordinator.dispatch(WindowIntent::SensorEntered { now_ms: 300 });
        coordinator.dispatch(WindowIntent::Tick { now_ms: 500 });
        assert_eq!(coordinator.state().pending_placement_id(), None);
        assert!(observe(&mut coordinator, id, Ok(fact(target)), 501).is_empty());
    }

    #[test]
    fn a_failed_final_frame_ends_no_animation_forever() {
        let mut coordinator = coordinator(StableVisibility::DockedExpanded(
            super::super::state::DockEdge::Left,
        ));
        coordinator.dispatch(WindowIntent::ManualCollapse { now_ms: 100 });
        let last = coordinator.dispatch(WindowIntent::Tick {
            now_ms: 100 + ANIMATION_DURATION_MS,
        });
        let (id, _) = placed(&last).unwrap();
        observe(&mut coordinator, id, Err(FailureKind::Unsupported), 300);
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::Presented(_)
        ));
        coordinator.answer_pending_receipts();
        assert_eq!(coordinator.state().next_deadline_ms(), None);
    }

    /// The window under the user's hand is not a confirmation of what was asked before.
    #[test]
    fn a_physical_drag_abandons_a_waiting_placement() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let (id, target) = placed(&floating_drag(&mut coordinator)).unwrap();
        let effects = coordinator.dispatch(WindowIntent::MoveResizeStarted);
        assert!(effects.is_empty());
        assert_eq!(coordinator.state().pending_placement_id(), None);
        assert!(observe(&mut coordinator, id, Ok(fact(target)), 11).is_empty());
        coordinator.dispatch(WindowIntent::MoveResizeEnded);
        let (next, target) = placed(&floating_drag(&mut coordinator)).unwrap();
        assert!(committed(&observe(
            &mut coordinator,
            next,
            Ok(fact(target)),
            12
        )));
    }

    fn failed(effects: &[WindowEffect]) -> bool {
        effects
            .iter()
            .any(|effect| matches!(effect, WindowEffect::PlacementFailed(_)))
    }

    /// A display change under a running drag only presents its frame. Nothing waits, so the
    /// timeout has nothing to report, and the drag's end is the one placement that gets judged.
    #[test]
    fn a_topology_change_during_a_drag_starts_no_wait() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        coordinator.dispatch(WindowIntent::MoveResizeStarted);
        let effects = coordinator.dispatch(WindowIntent::DisplayTopologyChanged {
            monitors: vec![monitor("replacement", 4000)],
            now_ms: 10,
        });
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, WindowEffect::ApplyFrame { last: false, .. })),
            "{effects:?}"
        );
        assert_eq!(placed(&effects), None);
        assert!(!committed(&effects));
        assert_eq!(coordinator.state().pending_placement_id(), None);
        assert_eq!(coordinator.state().next_deadline_ms(), None);

        let timeout = 10 + PLACEMENT_CONFIRM_TIMEOUT_MS;
        for now_ms in [timeout, timeout + 1] {
            let effects = coordinator.dispatch(WindowIntent::Tick { now_ms });
            assert!(!failed(&effects), "{effects:?}");
        }

        coordinator.dispatch(WindowIntent::MoveResizeEnded);
        let (id, target) = placed(&floating_drag(&mut coordinator)).expect("the drag's end");
        assert!(committed(&observe(
            &mut coordinator,
            id,
            Ok(fact(target)),
            timeout + 2
        )));
    }

    /// Whatever arises during the drag, the placements after it are judged as usual, and a
    /// drag that ends with no `DragEnded` lifts the hold all the same.
    #[test]
    fn a_placement_after_a_drag_is_judged_as_usual() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        coordinator.dispatch(WindowIntent::MoveResizeStarted);
        coordinator.dispatch(WindowIntent::MoveResizeEnded);
        let effects = coordinator.dispatch(WindowIntent::DisplayTopologyChanged {
            monitors: vec![monitor("replacement", 4000)],
            now_ms: 10,
        });
        let (id, _) = placed(&effects).expect("an ordinary placement");
        assert_eq!(coordinator.state().pending_placement_id(), Some(id));
        let deadline = 10 + PLACEMENT_CONFIRM_TIMEOUT_MS;
        coordinator.dispatch(WindowIntent::Tick { now_ms: deadline });
        let effects = coordinator.dispatch(WindowIntent::Tick {
            now_ms: deadline + 1,
        });
        assert_eq!(
            effects,
            [WindowEffect::PlacementFailed(FailureKind::Failed)]
        );
    }

    /// A topology recovery writes only the monitor it found: a frame clamped to a smaller display
    /// is not saved as the size the user chose.
    #[test]
    fn a_topology_recovery_commits_its_placement_only_when_confirmed() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let replacement = monitor("replacement", 4000);
        let effects = coordinator.dispatch(WindowIntent::DisplayTopologyChanged {
            monitors: vec![replacement.clone()],
            now_ms: 10,
        });
        assert!(!committed(&effects));
        let (id, target) = placed(&effects).expect("a placement");
        let before = coordinator.state.placement.clone();
        let effects = observe(&mut coordinator, id, Ok(fact(target)), 11);
        let WindowEffect::CommitPlacement {
            placement,
            dock_edge,
        } = &effects[0]
        else {
            panic!("{effects:?}");
        };
        assert_eq!(*placement, before);
        assert_eq!(*dock_edge, None);
    }

    /// A topology change while hidden moves nothing, so what was waiting can never be confirmed:
    /// it dies, and its late observation commits nothing. The commit carries only the monitor,
    /// on the placement a frame last proved: not the size, ratios or dock of the unconfirmed drag.
    #[test]
    fn a_topology_change_while_hidden_keeps_an_unconfirmed_drag_out_of_the_commit() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let confirmed = coordinator.state.placement.clone();
        let effects = left_edge_drag(&mut coordinator);
        let (drag, drag_target) = placed(&effects).unwrap();
        assert_ne!(coordinator.state.placement, confirmed);

        let close = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 20,
            guards: WindowGuardSnapshot::default(),
        });
        let hide = close
            .iter()
            .find_map(|effect| match effect {
                WindowEffect::HideWindow { id } => Some(*id),
                _ => None,
            })
            .unwrap();
        coordinator.dispatch(WindowIntent::ActionCompleted {
            id: hide,
            outcome: Ok(()),
            now_ms: 21,
        });
        assert_eq!(coordinator.state().pending_placement_id(), Some(drag));

        let replacement = monitor("replacement", 4000);
        let effects = coordinator.dispatch(WindowIntent::DisplayTopologyChanged {
            monitors: vec![replacement.clone()],
            now_ms: 30,
        });
        assert_eq!(coordinator.state().pending_placement_id(), None);
        let WindowEffect::CommitPlacement {
            placement,
            dock_edge,
        } = effects
            .iter()
            .find(|effect| matches!(effect, WindowEffect::CommitPlacement { .. }))
            .expect("the monitor is recorded")
        else {
            unreachable!();
        };
        assert_eq!(*dock_edge, None, "the unconfirmed dock stays out");
        assert_eq!(
            *placement,
            WindowPlacement {
                monitor_identity: Some(replacement.identity.clone()),
                ..confirmed
            }
        );
        assert!(observe(&mut coordinator, drag, Ok(fact(drag_target)), 31).is_empty());
    }

    /// The last id leaves a placement waiting; the next request cannot get one. It sends no frame
    /// at all, and the old placement dies with it instead of confirming the newer one's state.
    #[test]
    fn an_exhausted_placement_sends_no_frame_and_ends_the_old_one() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        coordinator.state.last_window_action = u64::MAX - 1;
        let (id, target) = placed(&floating_drag(&mut coordinator)).unwrap();
        assert_eq!(id, u64::MAX);
        let effects = left_edge_drag(&mut coordinator);
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, WindowEffect::ApplyFrame { .. })),
            "{effects:?}"
        );
        assert!(effects.contains(&WindowEffect::PlacementFailed(FailureKind::Failed)));
        assert_eq!(coordinator.state().pending_placement_id(), None);
        assert!(observe(&mut coordinator, id, Ok(fact(target)), 11).is_empty());
    }

    /// Only a window on screen is moved; a hidden one only records where it now belongs.
    #[test]
    fn a_hidden_topology_recovery_commits_at_once() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let close = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: WindowGuardSnapshot::default(),
        });
        let hide = close
            .iter()
            .find_map(|effect| match effect {
                WindowEffect::HideWindow { id } => Some(*id),
                _ => None,
            })
            .unwrap();
        coordinator.dispatch(WindowIntent::ActionCompleted {
            id: hide,
            outcome: Ok(()),
            now_ms: 1,
        });
        let effects = coordinator.dispatch(WindowIntent::DisplayTopologyChanged {
            monitors: vec![monitor("replacement", 4000)],
            now_ms: 10,
        });
        assert!(committed(&effects));
        assert_eq!(placed(&effects), None);
    }

    /// Hiding does not settle or cancel a placement, and a show asks for its own frame, replacing
    /// whatever was waiting.
    #[test]
    fn a_show_supersedes_a_waiting_placement() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let first = placed(&floating_drag(&mut coordinator)).unwrap();
        let close = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 20,
            guards: WindowGuardSnapshot::default(),
        });
        assert_eq!(coordinator.state().pending_placement_id(), Some(first.0));
        let hide = close
            .iter()
            .find_map(|effect| match effect {
                WindowEffect::HideWindow { id } => Some(*id),
                _ => None,
            })
            .unwrap();
        coordinator.dispatch(WindowIntent::ActionCompleted {
            id: hide,
            outcome: Ok(()),
            now_ms: 21,
        });
        assert_eq!(coordinator.state().pending_placement_id(), Some(first.0));
        let show = coordinator.dispatch(WindowIntent::ShowRequested {
            reason: ShowReason::Tray,
            now_ms: 22,
        });
        let (id, target) = placed(&show).expect("a show places the window");
        assert_ne!(id, first.0);
        assert!(observe(&mut coordinator, first.0, Ok(fact(first.1)), 23).is_empty());
        assert!(observe(&mut coordinator, id, Ok(fact(target)), 23).is_empty());
        assert_eq!(coordinator.state().pending_placement_id(), None);
    }

    /// Quit neither waits for a placement nor cancels it.
    #[test]
    fn a_quit_ignores_a_waiting_placement() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let (id, target) = placed(&floating_drag(&mut coordinator)).unwrap();
        coordinator.dispatch(WindowIntent::TrayQuitRequested {
            guards: WindowGuardSnapshot::default(),
        });
        assert_eq!(
            coordinator.state().lifecycle(),
            LifecycleState::Quitting(QuitStage::AwaitingAssetGc)
        );
        assert!(committed(&observe(
            &mut coordinator,
            id,
            Ok(fact(target)),
            11
        )));
    }
}
