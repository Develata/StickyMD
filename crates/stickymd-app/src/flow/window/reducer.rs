//! Deterministic mutation owner for window, docking, timers, and quit staging.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use super::geometry::{
    MonitorGeometry, PhysicalRect, WindowPlacement, collapsed_frame, expanded_frame,
    placement_from_frame, recover_placement, should_undock, snap_edge,
};
use super::placement::{NO_RECEIPT, PlacementCommit};
use super::state::{
    ANIMATION_DURATION_MS, ANIMATION_FRAME_MS, AUTO_COLLAPSE_DELAY_MS, AnimationState, DockEdge,
    DockState, HOVER_LEAVE_COLLAPSE_DELAY_MS, HOVER_REVEAL_DELAY_MS, LifecycleState,
    StableVisibility, VisibilityState, WindowEffect, WindowGuardSnapshot, WindowIntent,
    WindowShellState,
};
use super::topmost::TopmostState;

pub struct WindowShellCoordinator {
    pub(super) state: WindowShellState,
}

impl WindowShellCoordinator {
    pub fn new(
        placement: WindowPlacement,
        initial_visibility: StableVisibility,
        monitor: MonitorGeometry,
        tray_available: bool,
    ) -> Self {
        let edge = initial_visibility.dock_edge();
        let expanded = expanded_frame(&placement, edge, &monitor);
        let frame = match initial_visibility {
            StableVisibility::DockedCollapsed(edge) => collapsed_frame(expanded, edge, &monitor),
            _ => expanded,
        };
        Self {
            state: WindowShellState {
                visibility: VisibilityState::Presented(initial_visibility),
                dock: DockState {
                    edge,
                    monitor,
                    offset_ratio: placement.dock_offset_ratio,
                    manually_hidden: matches!(
                        initial_visibility,
                        StableVisibility::DockedCollapsed(_)
                    ),
                    hover_revealed: false,
                },
                confirmed_placement: placement.clone(),
                placement,
                frame,
                confirmed_dock_edge: edge,
                guards: WindowGuardSnapshot::default(),
                deadlines: Default::default(),
                lifecycle: LifecycleState::Running,
                tray_available,
                hide_save_pending: false,
                last_window_action: 0,
                focus_pending: None,
                placement_pending: None,
                native_move_active: false,
                topmost: TopmostState {
                    sensor_confirmed: matches!(
                        initial_visibility,
                        StableVisibility::DockedCollapsed(_)
                    ),
                    ..Default::default()
                },
            },
        }
    }

    /// Takes the user's always-on-top value as confirmed: the startup applies it to the window
    /// on its own, with no receipt.
    pub fn with_user_topmost(mut self, on: bool) -> Self {
        self.state.topmost.user_confirmed = on;
        self
    }

    pub fn state(&self) -> &WindowShellState {
        &self.state
    }

    pub fn dispatch(&mut self, intent: WindowIntent) -> Vec<WindowEffect> {
        let mut effects = Vec::new();
        match intent {
            WindowIntent::CloseRequested { now_ms, guards } => {
                self.state.guards = guards;
                self.hide_to_tray(now_ms, &mut effects);
            }
            WindowIntent::ShowRequested { now_ms, .. } => self.show(now_ms, &mut effects),
            WindowIntent::TrayToggleRequested { now_ms, guards } => {
                self.state.guards = guards;
                // The tray still says "show" while a show waits for its receipt; asking again
                // supersedes it, so a receipt that never comes does not hold the wait open.
                if matches!(
                    self.state.visibility,
                    VisibilityState::HiddenToTray { .. } | VisibilityState::ShowPending { .. }
                ) {
                    self.show(now_ms, &mut effects);
                } else {
                    self.hide_to_tray(now_ms, &mut effects);
                }
            }
            WindowIntent::TrayQuitRequested { guards } => {
                self.state.guards = guards;
                self.request_quit(&mut effects);
            }
            WindowIntent::ManualCollapse { now_ms } | WindowIntent::EscapePressed { now_ms } => {
                if !self.state.lifecycle.is_quitting()
                    && self.current_stable().dock_edge().is_some()
                {
                    effects.push(WindowEffect::CancelImePreedit);
                    self.state.dock.manually_hidden = true;
                    self.start_collapse(now_ms, &mut effects);
                }
            }
            WindowIntent::SensorEntered { now_ms } => {
                if matches!(self.current_stable(), StableVisibility::DockedCollapsed(_))
                    && !self.state.lifecycle.is_quitting()
                {
                    let deadline = now_ms.saturating_add(HOVER_REVEAL_DELAY_MS);
                    self.state.deadlines.hover_reveal_ms = Some(deadline);
                    effects.push(WindowEffect::WakeAt(deadline));
                }
            }
            WindowIntent::PointerLeft { now_ms } => {
                self.state.deadlines.hover_reveal_ms = None;
                if self.state.dock.hover_revealed
                    && !self.state.guards.blocks_auto_collapse()
                    && !self.state.lifecycle.is_quitting()
                {
                    let deadline = now_ms.saturating_add(HOVER_LEAVE_COLLAPSE_DELAY_MS);
                    self.state.deadlines.hover_leave_collapse_ms = Some(deadline);
                    effects.push(WindowEffect::WakeAt(deadline));
                }
            }
            WindowIntent::AbandonQuit => self.abandon_quit(&mut effects),
            WindowIntent::GuardsChanged { guards, now_ms } => {
                self.update_guards(guards, now_ms, &mut effects);
            }
            WindowIntent::DragEnded {
                frame,
                monitor,
                now_ms,
            } => self.finish_drag(frame, monitor, now_ms, &mut effects),
            WindowIntent::DisplayTopologyChanged { monitors, now_ms } => {
                self.recover_topology(&monitors, now_ms, &mut effects);
            }
            WindowIntent::MoveResizeStarted => self.start_native_move(),
            WindowIntent::MoveResizeEnded => self.state.native_move_active = false,
            WindowIntent::PlacementObserved {
                id,
                observed,
                now_ms,
            } => self.observe_placement(id, observed, now_ms, &mut effects),
            WindowIntent::UserTopmostRequested { on, now_ms } => {
                self.request_user_topmost(on, now_ms, &mut effects);
            }
            WindowIntent::SplitModeChanged { split } => {
                self.update_split_mode(split, &mut effects);
            }
            WindowIntent::Tick { now_ms } => self.tick(now_ms, &mut effects),
            WindowIntent::PasteSettled { guards, now_ms } => {
                self.state.guards = guards;
                self.paste_settled(now_ms, &mut effects);
            }
            WindowIntent::HideSaveCompleted {
                succeeded,
                guards,
                now_ms,
            } => {
                self.state.guards = guards;
                self.complete_hide_save(succeeded, now_ms, &mut effects);
            }
            WindowIntent::QuitBarrierCompleted {
                barrier,
                succeeded,
                guards,
            } => {
                self.state.guards = guards;
                self.complete_quit_barrier(barrier, succeeded, &mut effects);
            }
            WindowIntent::ActionCompleted {
                id,
                outcome,
                now_ms,
            } => {
                self.complete_window_action(id, outcome, now_ms, &mut effects);
            }
        }
        effects
    }

    fn update_guards(
        &mut self,
        guards: WindowGuardSnapshot,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        // Guards are refreshed for IME, persistence, and drag facts too. Only
        // a real focus acquisition may revoke the sensor's temporary z-order,
        // and not while the window is or is becoming a strip: focus handed back
        // to it (returning to its desktop) must not make the 2-DIP recovery
        // path unreachable. The expansion that follows revokes it (plan 09).
        let gained_focus = !self.state.guards.window_focused && guards.window_focused;
        self.state.guards = guards;
        let staying_collapsed =
            matches!(self.current_stable(), StableVisibility::DockedCollapsed(_));
        if gained_focus && !staying_collapsed {
            self.set_temporary_sensor_topmost(false, now_ms, effects);
        }
        if guards.blocks_auto_collapse() {
            self.state.deadlines.auto_collapse_ms = None;
            self.state.deadlines.hover_leave_collapse_ms = None;
            if !self.state.dock.manually_hidden
                && matches!(
                    self.state.visibility,
                    VisibilityState::Animating(AnimationState {
                        final_visibility: StableVisibility::DockedCollapsed(_),
                        ..
                    })
                )
            {
                self.start_expand(now_ms, false, effects);
            }
        } else {
            self.arm_auto_collapse(now_ms, effects);
        }
    }

    /// Starts the auto-collapse timer of a docked window that nothing holds open. A window coming
    /// back from a failed hide needs this too: its wait could not start the timer.
    pub(super) fn arm_auto_collapse(&mut self, now_ms: u64, effects: &mut Vec<WindowEffect>) {
        if matches!(self.current_stable(), StableVisibility::DockedExpanded(_))
            && self.state.deadlines.auto_collapse_ms.is_none()
            && !self.state.lifecycle.is_quitting()
            && !self.state.guards.blocks_auto_collapse()
            && self.may_animate()
        {
            let deadline = now_ms.saturating_add(AUTO_COLLAPSE_DELAY_MS);
            self.state.deadlines.auto_collapse_ms = Some(deadline);
            effects.push(WindowEffect::WakeAt(deadline));
        }
    }

    fn finish_drag(
        &mut self,
        frame: PhysicalRect,
        monitor: MonitorGeometry,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        // Only a window on screen can land here: a hide or a show in progress keeps its decision.
        if self.state.lifecycle.is_quitting() || !self.may_animate() {
            return;
        }
        self.state.deadlines.clear();
        let previous_edge = self.current_stable().dock_edge();
        let edge = resolve_drag_edge(previous_edge, frame, &monitor);
        let placement = placement_from_frame(frame, &monitor, &self.state.placement, edge);
        self.state.placement = placement;
        self.state.dock.monitor = monitor;
        self.state.dock.edge = edge;
        self.state.dock.offset_ratio = self.state.placement.dock_offset_ratio;
        self.state.dock.manually_hidden = false;
        self.state.dock.hover_revealed = false;
        self.set_temporary_sensor_topmost(false, now_ms, effects);
        self.state.frame =
            effective_expanded_frame(&self.state.placement, edge, &self.state.dock.monitor);
        let stable = edge.map_or(StableVisibility::Floating, StableVisibility::DockedExpanded);
        self.state.visibility = VisibilityState::Presented(stable);
        self.request_placement(
            self.state.frame,
            Some(PlacementCommit::FromObserved {
                monitor: self.state.dock.monitor.clone(),
                dock_edge: edge,
            }),
            now_ms,
            effects,
        );
        if edge.is_some() && !self.state.guards.blocks_auto_collapse() {
            let deadline = now_ms.saturating_add(AUTO_COLLAPSE_DELAY_MS);
            self.state.deadlines.auto_collapse_ms = Some(deadline);
            effects.push(WindowEffect::WakeAt(deadline));
        }
    }

    fn recover_topology(
        &mut self,
        monitors: &[MonitorGeometry],
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        let prior_stable = self.current_stable();
        let edge = prior_stable.dock_edge();
        let Some((monitor, expanded)) = recover_placement(&self.state.placement, edge, monitors)
        else {
            return;
        };
        let hidden_to_tray = matches!(self.state.visibility, VisibilityState::HiddenToTray { .. });
        let stable = if hidden_to_tray {
            prior_stable
        } else {
            // A pending show lands expanded too, so its frame follows the display change here.
            edge.map_or(StableVisibility::Floating, StableVisibility::DockedExpanded)
        };
        self.state.placement.monitor_identity = Some(monitor.identity.clone());
        self.state.dock.monitor = monitor.clone();
        self.state.dock.edge = edge;
        self.state.dock.hover_revealed = false;
        let sensor_topmost = matches!(stable, StableVisibility::DockedCollapsed(_));
        self.set_temporary_sensor_topmost(sensor_topmost, now_ms, effects);
        self.state.frame = match stable {
            StableVisibility::DockedCollapsed(edge) => collapsed_frame(expanded, edge, monitor),
            _ => expanded,
        };
        self.state.visibility = match self.state.visibility {
            VisibilityState::HiddenToTray { .. } => {
                VisibilityState::HiddenToTray { restore: stable }
            }
            VisibilityState::HidePending {
                id,
                sensor_topmost,
                deadline_ms,
                ..
            } => VisibilityState::HidePending {
                id,
                restore: stable,
                sensor_topmost,
                deadline_ms,
            },
            VisibilityState::AwaitingHideSave { .. } => {
                VisibilityState::AwaitingHideSave { restore: stable }
            }
            pending @ VisibilityState::ShowPending { .. } => pending,
            _ => VisibilityState::Presented(stable),
        };
        if matches!(
            self.state.visibility,
            VisibilityState::Presented(_)
                | VisibilityState::Animating(_)
                | VisibilityState::ShowPending { .. }
        ) {
            self.request_placement(
                self.state.frame,
                Some(PlacementCommit::AsRequested { dock_edge: edge }),
                now_ms,
                effects,
            );
        } else {
            // A hidden or hiding window is not moved, so there is no frame to confirm. What was
            // waiting cannot be confirmed any more either, and only the monitor it now belongs to
            // is recorded, on the placement a frame last proved.
            self.abandon_placement();
            self.state.confirmed_placement.monitor_identity = Some(monitor.identity.clone());
            effects.push(WindowEffect::CommitPlacement {
                placement: self.state.confirmed_placement.clone(),
                dock_edge: self.state.confirmed_dock_edge,
            });
        }
    }

    fn update_split_mode(&mut self, _split: bool, _effects: &mut Vec<WindowEffect>) {
        // Phase 10 keeps every ViewMode at the user-selected width, including
        // the 220 DIP minimum. Split is a 50/50 projection, not geometry policy.
    }

    fn tick(&mut self, now_ms: u64, effects: &mut Vec<WindowEffect>) {
        self.tick_placement(now_ms, effects);
        self.expire_window_actions(now_ms, effects);
        if let VisibilityState::Animating(mut animation) = self.state.visibility {
            self.state.frame = animation.frame_at(now_ms);
            if now_ms >= animation.end_ms {
                self.request_placement(self.state.frame, None, now_ms, effects);
                effects.push(WindowEffect::RequestRedraw);
                self.state.visibility = VisibilityState::Presented(animation.final_visibility);
            } else {
                effects.push(WindowEffect::ApplyFrame {
                    id: NO_RECEIPT,
                    target: self.state.frame,
                    last: false,
                });
                effects.push(WindowEffect::RequestRedraw);
                animation.next_frame_ms = now_ms
                    .saturating_add(ANIMATION_FRAME_MS)
                    .min(animation.end_ms);
                self.state.visibility = VisibilityState::Animating(animation);
                effects.push(WindowEffect::WakeAt(animation.next_frame_ms));
                return;
            }
        }

        if self
            .state
            .deadlines
            .auto_collapse_ms
            .is_some_and(|deadline| now_ms >= deadline)
            || self
                .state
                .deadlines
                .hover_leave_collapse_ms
                .is_some_and(|deadline| now_ms >= deadline)
        {
            self.state.deadlines.auto_collapse_ms = None;
            self.state.deadlines.hover_leave_collapse_ms = None;
            if !self.state.guards.blocks_auto_collapse() {
                self.start_collapse(now_ms, effects);
            }
        }
        if self
            .state
            .deadlines
            .hover_reveal_ms
            .is_some_and(|deadline| now_ms >= deadline)
        {
            self.state.deadlines.hover_reveal_ms = None;
            self.start_expand(now_ms, true, effects);
        }
    }

    /// Only a window that stays on screen collapses or expands. A dock animation replaces the
    /// visibility state, which would lose a hide in progress and the record of a hidden window.
    fn may_animate(&self) -> bool {
        matches!(
            self.state.visibility,
            VisibilityState::Presented(_) | VisibilityState::Animating(_)
        )
    }

    fn start_collapse(&mut self, now_ms: u64, effects: &mut Vec<WindowEffect>) {
        if !self.may_animate() {
            return;
        }
        let Some(edge) = self.current_stable().dock_edge() else {
            return;
        };
        if matches!(self.current_stable(), StableVisibility::DockedCollapsed(_)) {
            return;
        }
        self.state.deadlines.clear();
        self.state.dock.edge = Some(edge);
        self.state.dock.hover_revealed = false;
        self.set_temporary_sensor_topmost(true, now_ms, effects);
        let expanded =
            effective_expanded_frame(&self.state.placement, Some(edge), &self.state.dock.monitor);
        let target = collapsed_frame(expanded, edge, &self.state.dock.monitor);
        self.start_animation(
            now_ms,
            target,
            StableVisibility::DockedCollapsed(edge),
            effects,
        );
    }

    pub(super) fn start_expand(
        &mut self,
        now_ms: u64,
        hover_revealed: bool,
        effects: &mut Vec<WindowEffect>,
    ) {
        if !self.may_animate() {
            return;
        }
        let Some(edge) = self.current_stable().dock_edge() else {
            return;
        };
        if matches!(self.current_stable(), StableVisibility::DockedExpanded(_))
            && !matches!(self.state.visibility, VisibilityState::Animating(_))
        {
            return;
        }
        self.state.deadlines.clear();
        self.state.dock.hover_revealed = hover_revealed;
        self.state.dock.manually_hidden = false;
        self.set_temporary_sensor_topmost(
            hover_revealed && !self.state.guards.window_focused,
            now_ms,
            effects,
        );
        let target =
            effective_expanded_frame(&self.state.placement, Some(edge), &self.state.dock.monitor);
        self.start_animation(
            now_ms,
            target,
            StableVisibility::DockedExpanded(edge),
            effects,
        );
    }

    fn start_animation(
        &mut self,
        now_ms: u64,
        target: PhysicalRect,
        final_visibility: StableVisibility,
        effects: &mut Vec<WindowEffect>,
    ) {
        let from = match self.state.visibility {
            VisibilityState::Animating(animation) => animation.frame_at(now_ms),
            _ => self.state.frame,
        };
        // The animation's own last frame is the placement that counts now.
        self.abandon_placement();
        let end_ms = now_ms.saturating_add(ANIMATION_DURATION_MS);
        let next_frame_ms = now_ms.saturating_add(ANIMATION_FRAME_MS).min(end_ms);
        self.state.frame = from;
        self.state.visibility = VisibilityState::Animating(AnimationState {
            from,
            to: target,
            started_ms: now_ms,
            end_ms,
            next_frame_ms,
            final_visibility,
        });
        effects.push(WindowEffect::RequestRedraw);
        effects.push(WindowEffect::WakeAt(next_frame_ms));
    }

    pub(super) fn current_stable(&self) -> StableVisibility {
        match self.state.visibility {
            VisibilityState::Presented(stable)
            | VisibilityState::AwaitingHideSave { restore: stable }
            | VisibilityState::HidePending {
                restore: stable, ..
            }
            | VisibilityState::ShowPending {
                restore: stable, ..
            }
            | VisibilityState::HiddenToTray { restore: stable } => stable,
            VisibilityState::Animating(animation) => animation.final_visibility,
        }
    }
}

fn resolve_drag_edge(
    previous_edge: Option<DockEdge>,
    frame: PhysicalRect,
    monitor: &MonitorGeometry,
) -> Option<DockEdge> {
    let snapped = snap_edge(frame, monitor);
    match (previous_edge, snapped) {
        // A directly reached different edge is one atomic re-dock, not an
        // intermediate detach followed by a second user drag.
        (Some(previous), Some(next)) if next != previous => Some(next),
        (Some(previous), _) if should_undock(previous, frame, monitor) => None,
        (_, snapped) => snapped.or(previous_edge),
    }
}

pub(super) fn effective_expanded_frame(
    placement: &WindowPlacement,
    edge: Option<DockEdge>,
    monitor: &MonitorGeometry,
) -> PhysicalRect {
    expanded_frame(placement, edge, monitor)
}

#[cfg(test)]
mod phase8_window_tests {
    use std::time::Instant;

    use super::super::geometry::MonitorIdentity;
    use super::super::state::{
        DockEdge, HOVER_LEAVE_COLLAPSE_DELAY_MS, HOVER_REVEAL_DELAY_MS, LifecycleState,
        QuitBarrier, QuitStage, ShowReason, TopmostRequest, TopmostSource, WindowSaveReason,
    };
    use super::*;
    use crate::flow::window::placement::tests::observe_at_target;
    use crate::ports::FailureKind;

    fn monitor(identity: &str, x: i32, scale: f64, primary: bool) -> MonitorGeometry {
        MonitorGeometry::new(
            MonitorIdentity::new(identity),
            PhysicalRect::new(x, -200, 1920, 1080),
            scale,
            primary,
        )
    }

    /// Whether `effects` ask the host to take the window's bit to `on`, for the edge sensor.
    fn asks_sensor_topmost(effects: &[WindowEffect], on: bool) -> bool {
        effects.iter().any(|effect| {
            matches!(
                effect,
                WindowEffect::ApplyTopmost(TopmostRequest {
                    desired_effective,
                    source: TopmostSource::Sensor,
                    ..
                }) if *desired_effective == on
            )
        })
    }

    fn coordinator(visibility: StableVisibility) -> WindowShellCoordinator {
        let monitor = monitor("primary", -1920, 1.5, true);
        WindowShellCoordinator::new(
            WindowPlacement::new(520.0, 680.0, None, 0.5, 0.5, 0.25),
            visibility,
            monitor,
            true,
        )
    }

    #[test]
    fn dirty_close_freezes_until_latest_save_succeeds_then_hides_without_gc() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        coordinator.dispatch(WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot {
                note_save_required: true,
                ..Default::default()
            },
            now_ms: 0,
        });
        let effects = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 1,
            guards: WindowGuardSnapshot {
                note_save_required: true,
                ..Default::default()
            },
        });
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::AwaitingHideSave { .. }
        ));
        assert_eq!(coordinator.state().lifecycle(), LifecycleState::Running);
        assert!(effects.contains(&WindowEffect::RequestNoteSave(WindowSaveReason::HideToTray)));
        assert_eq!(hide_request(&effects), None);
        assert!(!effects.contains(&WindowEffect::RequestSafeAssetGc));
        let effects = coordinator.dispatch(WindowIntent::HideSaveCompleted {
            now_ms: 0,
            succeeded: true,
            guards: WindowGuardSnapshot::default(),
        });
        assert!(hide_request(&effects).is_some());
        report_hidden(&mut coordinator, &effects);
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::HiddenToTray { .. }
        ));
    }

    /// The action a host is asked to run to hide the window.
    fn hide_request(effects: &[WindowEffect]) -> Option<u64> {
        effects.iter().find_map(|effect| match effect {
            WindowEffect::HideWindow { id } => Some(*id),
            _ => None,
        })
    }

    /// What a host does after `HideWindow`: reports that the window is hidden.
    fn report_hidden(
        coordinator: &mut WindowShellCoordinator,
        effects: &[WindowEffect],
    ) -> Vec<WindowEffect> {
        coordinator.dispatch(WindowIntent::ActionCompleted {
            now_ms: 0,
            id: hide_request(effects).expect("a hide request"),
            outcome: Ok(()),
        })
    }

    /// Closes a clean window, up to the host's report.
    fn request_clean_hide(coordinator: &mut WindowShellCoordinator) -> u64 {
        let effects = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: WindowGuardSnapshot {
                window_focused: true,
                ..Default::default()
            },
        });
        hide_request(&effects).expect("a clean close hides at once")
    }

    #[test]
    fn a_hide_commits_only_on_its_success_receipt() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let id = request_clean_hide(&mut coordinator);
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::HidePending { .. }
        ));
        let effects = coordinator.dispatch(WindowIntent::ActionCompleted {
            now_ms: 0,
            id,
            outcome: Ok(()),
        });
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::HiddenToTray { .. }
        ));
        assert_eq!(effects, [WindowEffect::ReleaseHiddenCaches]);
    }

    /// Plan 04: a rejected hide neither enters `HiddenToTray` nor releases the hidden caches.
    #[test]
    fn a_rejected_hide_stays_shown_with_its_input_and_caches() {
        for kind in [
            FailureKind::Failed,
            FailureKind::Unavailable,
            FailureKind::Unsupported,
        ] {
            let mut coordinator = coordinator(StableVisibility::Floating);
            let id = request_clean_hide(&mut coordinator);
            let effects = coordinator.dispatch(WindowIntent::ActionCompleted {
                now_ms: 0,
                id,
                outcome: Err(kind),
            });
            assert_eq!(
                coordinator.state().visibility(),
                VisibilityState::Presented(StableVisibility::Floating)
            );
            assert_eq!(
                effects,
                [
                    WindowEffect::SetImeAllowed(true),
                    WindowEffect::SetEditorInputEnabled(true),
                    WindowEffect::HideRejected(kind),
                    WindowEffect::RequestRedraw,
                ]
            );
        }
    }

    #[test]
    fn a_rejected_hide_returns_the_edge_sensor_to_topmost() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        coordinator.state.topmost.sensor_confirmed = true;
        let id = request_clean_hide(&mut coordinator);
        assert!(!coordinator.state().temporary_sensor_topmost());
        let effects = coordinator.dispatch(WindowIntent::ActionCompleted {
            now_ms: 0,
            id,
            outcome: Err(FailureKind::Failed),
        });
        assert!(coordinator.state().temporary_sensor_topmost());
        assert!(asks_sensor_topmost(&effects, true));
    }

    #[test]
    fn a_stale_hide_receipt_changes_nothing() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let id = request_clean_hide(&mut coordinator);
        let before = coordinator.state().visibility();
        for outcome in [Ok(()), Err(FailureKind::Failed)] {
            let effects = coordinator.dispatch(WindowIntent::ActionCompleted {
                now_ms: 0,
                id: id + 1,
                outcome,
            });
            assert!(effects.is_empty());
            assert_eq!(coordinator.state().visibility(), before);
        }
    }

    /// Losing focus starts the auto-collapse timer of a docked window on screen. A window being
    /// hidden, or hidden already, must keep its state: its receipt or the tray still needs it.
    #[test]
    fn a_hiding_or_hidden_docked_window_never_collapses() {
        let unfocused = WindowGuardSnapshot::default();
        let dirty = WindowGuardSnapshot {
            note_save_required: true,
            ..Default::default()
        };
        let docked = StableVisibility::DockedExpanded(DockEdge::Left);
        for hide_receipt in [false, true] {
            let mut coordinator = coordinator(docked);
            let id = request_clean_hide(&mut coordinator);
            if hide_receipt {
                coordinator.dispatch(WindowIntent::ActionCompleted {
                    now_ms: 0,
                    id,
                    outcome: Ok(()),
                });
            }
            let before = coordinator.state().visibility();
            coordinator.dispatch(WindowIntent::GuardsChanged {
                guards: unfocused,
                now_ms: 1,
            });
            coordinator.dispatch(WindowIntent::Tick {
                now_ms: 1 + AUTO_COLLAPSE_DELAY_MS,
            });
            assert_eq!(coordinator.state().visibility(), before);
            if !hide_receipt {
                let effects = coordinator.dispatch(WindowIntent::ActionCompleted {
                    now_ms: 0,
                    id,
                    outcome: Ok(()),
                });
                assert_eq!(effects, [WindowEffect::ReleaseHiddenCaches]);
            }
        }

        let mut coordinator = coordinator(docked);
        coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: dirty,
        });
        coordinator.dispatch(WindowIntent::GuardsChanged {
            guards: dirty,
            now_ms: 1,
        });
        coordinator.dispatch(WindowIntent::Tick {
            now_ms: 1 + AUTO_COLLAPSE_DELAY_MS,
        });
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::AwaitingHideSave { .. }
        ));
    }

    /// A window back on screen after a failed hide save or a rejected hide collapses on its own,
    /// though no timer could start while it waited.
    #[test]
    fn a_window_back_from_a_failed_hide_arms_its_auto_collapse() {
        let unfocused = WindowGuardSnapshot::default();
        let docked = StableVisibility::DockedExpanded(DockEdge::Left);
        let collapsing = |coordinator: &mut WindowShellCoordinator, now_ms: u64| {
            coordinator.dispatch(WindowIntent::Tick {
                now_ms: now_ms + AUTO_COLLAPSE_DELAY_MS,
            });
            matches!(
                coordinator.state().visibility(),
                VisibilityState::Animating(_)
            )
        };

        let mut saving = coordinator(docked);
        saving.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: WindowGuardSnapshot {
                note_save_required: true,
                ..Default::default()
            },
        });
        saving.dispatch(WindowIntent::GuardsChanged {
            guards: unfocused,
            now_ms: 1,
        });
        let effects = saving.dispatch(WindowIntent::HideSaveCompleted {
            now_ms: 10,
            succeeded: false,
            guards: unfocused,
        });
        assert!(effects.contains(&WindowEffect::WakeAt(10 + AUTO_COLLAPSE_DELAY_MS)));
        assert!(collapsing(&mut saving, 10));

        let mut rejected = coordinator(docked);
        let effects = rejected.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: unfocused,
        });
        let effects = rejected.dispatch(WindowIntent::ActionCompleted {
            now_ms: 10,
            id: hide_request(&effects).unwrap(),
            outcome: Err(FailureKind::Failed),
        });
        assert!(effects.contains(&WindowEffect::WakeAt(10 + AUTO_COLLAPSE_DELAY_MS)));
        assert!(collapsing(&mut rejected, 10));
    }

    /// A show overtakes a hide still in flight; the hide's late receipt must not hide the window.
    #[test]
    fn a_hide_receipt_after_a_show_changes_nothing() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let id = request_clean_hide(&mut coordinator);
        coordinator.dispatch(WindowIntent::ShowRequested {
            reason: ShowReason::Tray,
            now_ms: 1,
        });
        let shown = coordinator.state().visibility();
        assert!(matches!(shown, VisibilityState::ShowPending { .. }));
        let effects = coordinator.dispatch(WindowIntent::ActionCompleted {
            now_ms: 0,
            id,
            outcome: Ok(()),
        });
        assert!(effects.is_empty());
        assert_eq!(coordinator.state().visibility(), shown);
    }

    #[test]
    fn dirty_close_save_failure_keeps_window_visible_and_reenables_input() {
        let mut coordinator = coordinator(StableVisibility::DockedExpanded(DockEdge::Left));
        coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 1,
            guards: WindowGuardSnapshot {
                window_focused: true,
                note_save_required: true,
                ..Default::default()
            },
        });
        let effects = coordinator.dispatch(WindowIntent::HideSaveCompleted {
            now_ms: 0,
            succeeded: false,
            guards: WindowGuardSnapshot {
                window_focused: true,
                note_save_required: true,
                ..Default::default()
            },
        });
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedExpanded(DockEdge::Left))
        );
        assert!(effects.contains(&WindowEffect::HideCancelled));
        assert!(effects.contains(&WindowEffect::SetEditorInputEnabled(true)));
        assert_eq!(hide_request(&effects), None);
    }

    #[test]
    fn recovery_or_conflict_blocks_hide_without_freezing_editor() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 1,
            guards: WindowGuardSnapshot {
                conflict_or_recovery: true,
                note_save_required: true,
                ..Default::default()
            },
        });
        assert_eq!(effects, vec![WindowEffect::HideBlocked]);
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::Floating)
        );
    }

    #[test]
    fn tray_unavailable_close_uses_the_safe_quit_barrier() {
        let monitor = monitor("primary", 0, 1.0, true);
        let mut coordinator = WindowShellCoordinator::new(
            WindowPlacement::new(520.0, 680.0, None, 0.5, 0.5, 0.25),
            StableVisibility::Floating,
            monitor,
            false,
        );
        let effects = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 1,
            guards: WindowGuardSnapshot::default(),
        });
        assert!(effects.contains(&WindowEffect::ReportTrayUnavailable));
        assert!(effects.contains(&WindowEffect::RequestSafeAssetGc));
        assert_eq!(
            coordinator.state().lifecycle(),
            LifecycleState::Quitting(QuitStage::AwaitingAssetGc)
        );
        assert_eq!(hide_request(&effects), None);
    }

    #[test]
    fn stale_hide_save_receipt_requests_latest_dirty_generation_before_hiding() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 1,
            guards: WindowGuardSnapshot {
                note_save_required: true,
                ..Default::default()
            },
        });
        let effects = coordinator.dispatch(WindowIntent::HideSaveCompleted {
            now_ms: 0,
            succeeded: true,
            guards: WindowGuardSnapshot {
                note_save_required: true,
                ..Default::default()
            },
        });
        assert_eq!(
            effects,
            vec![WindowEffect::RequestNoteSave(WindowSaveReason::HideToTray)]
        );
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::AwaitingHideSave { .. }
        ));
    }

    #[test]
    fn close_defers_save_until_pending_paste_and_asset_convergence_settle() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        coordinator.dispatch(WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot {
                paste_pending: true,
                asset_transaction_pending: true,
                note_save_required: true,
                ..Default::default()
            },
            now_ms: 0,
        });
        let effects = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 1,
            guards: WindowGuardSnapshot {
                paste_pending: true,
                asset_transaction_pending: true,
                note_save_required: true,
                ..Default::default()
            },
        });
        assert!(
            !effects
                .iter()
                .any(|effect| matches!(effect, WindowEffect::RequestNoteSave(_)))
        );
        coordinator.dispatch(WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot {
                note_save_required: true,
                ..Default::default()
            },
            now_ms: 2,
        });
        let effects = coordinator.dispatch(WindowIntent::PasteSettled {
            guards: WindowGuardSnapshot {
                note_save_required: true,
                ..Default::default()
            },
            now_ms: 3,
        });
        assert_eq!(
            effects,
            vec![WindowEffect::RequestNoteSave(WindowSaveReason::HideToTray)]
        );
    }

    #[test]
    fn tray_quit_obeys_paste_save_gc_config_order() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        coordinator.dispatch(WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot {
                paste_pending: true,
                ..Default::default()
            },
            now_ms: 0,
        });
        assert_eq!(
            coordinator.dispatch(WindowIntent::TrayQuitRequested {
                guards: WindowGuardSnapshot {
                    paste_pending: true,
                    ..Default::default()
                },
            }),
            vec![WindowEffect::SetEditorInputEnabled(false)]
        );
        assert_eq!(
            coordinator.state().lifecycle(),
            LifecycleState::Quitting(QuitStage::AwaitingPaste)
        );

        coordinator.dispatch(WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot {
                note_save_required: true,
                ..Default::default()
            },
            now_ms: 1,
        });
        assert_eq!(
            coordinator.dispatch(WindowIntent::QuitBarrierCompleted {
                barrier: QuitBarrier::Paste,
                succeeded: true,
                guards: WindowGuardSnapshot {
                    note_save_required: true,
                    ..Default::default()
                },
            }),
            vec![WindowEffect::RequestNoteSave(WindowSaveReason::Shutdown)]
        );
        assert_eq!(
            coordinator.state().lifecycle(),
            LifecycleState::Quitting(QuitStage::AwaitingNoteSave)
        );

        coordinator.dispatch(WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot::default(),
            now_ms: 2,
        });
        assert_eq!(
            coordinator.dispatch(WindowIntent::QuitBarrierCompleted {
                barrier: QuitBarrier::NoteSave,
                succeeded: true,
                guards: WindowGuardSnapshot::default(),
            }),
            vec![WindowEffect::RequestSafeAssetGc]
        );
        assert_eq!(
            coordinator.dispatch(WindowIntent::QuitBarrierCompleted {
                barrier: QuitBarrier::AssetGc,
                succeeded: true,
                guards: WindowGuardSnapshot::default(),
            }),
            vec![WindowEffect::RequestConfigFlush]
        );
        assert_eq!(
            coordinator.dispatch(WindowIntent::QuitBarrierCompleted {
                barrier: QuitBarrier::Config,
                succeeded: true,
                guards: WindowGuardSnapshot::default(),
            }),
            vec![WindowEffect::ExitProcess]
        );
        assert_eq!(
            coordinator.state().lifecycle(),
            LifecycleState::Quitting(QuitStage::ReadyToExit)
        );
    }

    /// The note is safe by then. The cleanup that is stuck counts as a pending asset transaction,
    /// so going through the barriers again would wait on it with nothing left to end the wait.
    #[test]
    fn a_quit_abandoned_after_the_note_is_safe_ends_at_once() {
        for (stage, barrier) in [
            (QuitStage::AwaitingAssetGc, QuitBarrier::AssetGc),
            (QuitStage::AwaitingConfig, QuitBarrier::Config),
        ] {
            let mut coordinator = coordinator(StableVisibility::Floating);
            coordinator.state.guards.asset_transaction_pending = true;
            coordinator.state.lifecycle = LifecycleState::Quitting(stage);
            assert_eq!(
                coordinator.dispatch(WindowIntent::AbandonQuit),
                vec![
                    WindowEffect::QuitWarning(barrier),
                    WindowEffect::ExitProcess
                ]
            );
            assert_eq!(
                coordinator.state().lifecycle(),
                LifecycleState::Quitting(QuitStage::ReadyToExit)
            );
        }
    }

    #[test]
    fn a_quit_the_host_stopped_waiting_for_gives_the_editor_back() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        coordinator.dispatch(WindowIntent::TrayQuitRequested {
            guards: WindowGuardSnapshot {
                note_save_required: true,
                ..Default::default()
            },
        });
        let effects = coordinator.dispatch(WindowIntent::AbandonQuit);
        assert!(effects.contains(&WindowEffect::SetEditorInputEnabled(true)));
        assert!(effects.contains(&WindowEffect::QuitTimedOut));
        assert_eq!(coordinator.state().lifecycle(), LifecycleState::Running);

        // The save finishing afterwards must not end a program the user has gone back to.
        let late = coordinator.dispatch(WindowIntent::QuitBarrierCompleted {
            barrier: QuitBarrier::NoteSave,
            succeeded: true,
            guards: WindowGuardSnapshot::default(),
        });
        assert!(late.is_empty());
        assert!(coordinator.dispatch(WindowIntent::AbandonQuit).is_empty());
    }

    #[test]
    fn note_save_failure_cancels_quit_and_reenables_input() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = coordinator.dispatch(WindowIntent::TrayQuitRequested {
            guards: WindowGuardSnapshot {
                note_save_required: true,
                ..Default::default()
            },
        });
        assert_eq!(
            effects,
            vec![
                WindowEffect::SetEditorInputEnabled(false),
                WindowEffect::RequestNoteSave(WindowSaveReason::Shutdown),
            ]
        );
        assert!(
            coordinator
                .dispatch(WindowIntent::QuitBarrierCompleted {
                    barrier: QuitBarrier::Config,
                    succeeded: true,
                    guards: WindowGuardSnapshot::default(),
                })
                .is_empty()
        );
        let effects = coordinator.dispatch(WindowIntent::QuitBarrierCompleted {
            barrier: QuitBarrier::NoteSave,
            succeeded: false,
            guards: WindowGuardSnapshot {
                note_save_required: true,
                ..Default::default()
            },
        });
        assert_eq!(
            effects,
            vec![
                WindowEffect::SetImeAllowed(true),
                WindowEffect::SetEditorInputEnabled(true),
                WindowEffect::QuitCancelled(QuitBarrier::NoteSave),
            ]
        );
        assert_eq!(coordinator.state().lifecycle(), LifecycleState::Running);
    }

    #[test]
    fn phase9_conflict_blocks_tray_quit_before_any_destructive_barrier() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = coordinator.dispatch(WindowIntent::TrayQuitRequested {
            guards: WindowGuardSnapshot {
                conflict_or_recovery: true,
                note_save_required: true,
                ..Default::default()
            },
        });

        assert_eq!(effects, vec![WindowEffect::QuitBlocked]);
        assert_eq!(coordinator.state().lifecycle(), LifecycleState::Running);
        assert!(!effects.contains(&WindowEffect::RequestSafeAssetGc));
        assert!(!effects.contains(&WindowEffect::ExitProcess));
    }

    #[test]
    fn asset_gc_failure_warns_but_continues_to_config_barrier() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        coordinator.dispatch(WindowIntent::TrayQuitRequested {
            guards: WindowGuardSnapshot::default(),
        });
        let effects = coordinator.dispatch(WindowIntent::QuitBarrierCompleted {
            barrier: QuitBarrier::AssetGc,
            succeeded: false,
            guards: WindowGuardSnapshot::default(),
        });
        assert_eq!(
            effects,
            vec![
                WindowEffect::QuitWarning(QuitBarrier::AssetGc),
                WindowEffect::RequestConfigFlush,
            ]
        );
        assert_eq!(
            coordinator.state().lifecycle(),
            LifecycleState::Quitting(QuitStage::AwaitingConfig)
        );
    }

    #[test]
    fn config_failure_warns_but_reaches_ready_to_exit() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        coordinator.dispatch(WindowIntent::TrayQuitRequested {
            guards: WindowGuardSnapshot::default(),
        });
        coordinator.dispatch(WindowIntent::QuitBarrierCompleted {
            barrier: QuitBarrier::AssetGc,
            succeeded: true,
            guards: WindowGuardSnapshot::default(),
        });
        let effects = coordinator.dispatch(WindowIntent::QuitBarrierCompleted {
            barrier: QuitBarrier::Config,
            succeeded: false,
            guards: WindowGuardSnapshot::default(),
        });
        assert_eq!(
            effects,
            vec![
                WindowEffect::QuitWarning(QuitBarrier::Config),
                WindowEffect::ExitProcess,
            ]
        );
        assert_eq!(
            coordinator.state().lifecycle(),
            LifecycleState::Quitting(QuitStage::ReadyToExit)
        );
    }

    #[test]
    fn show_is_ignored_while_quitting() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let effects = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: WindowGuardSnapshot::default(),
        });
        report_hidden(&mut coordinator, &effects);
        coordinator.dispatch(WindowIntent::TrayQuitRequested {
            guards: WindowGuardSnapshot::default(),
        });
        let effects = coordinator.dispatch(WindowIntent::ShowRequested {
            reason: ShowReason::SecondInstance,
            now_ms: 1,
        });
        assert!(effects.is_empty());
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::HiddenToTray { .. }
        ));
    }

    #[test]
    fn every_edge_auto_collapse_uses_exact_700ms_boundary_and_focus_guard() {
        for edge in [DockEdge::Left, DockEdge::Top, DockEdge::Right] {
            let mut coordinator = coordinator(StableVisibility::DockedExpanded(edge));
            let focused_effects = coordinator.dispatch(WindowIntent::GuardsChanged {
                guards: WindowGuardSnapshot {
                    window_focused: true,
                    ..Default::default()
                },
                now_ms: 0,
            });
            assert!(focused_effects.is_empty());
            coordinator.dispatch(WindowIntent::Tick { now_ms: 700 });
            assert_eq!(
                coordinator.state().visibility(),
                VisibilityState::Presented(StableVisibility::DockedExpanded(edge))
            );
            let effects = coordinator.dispatch(WindowIntent::GuardsChanged {
                guards: WindowGuardSnapshot::default(),
                now_ms: 710,
            });
            assert!(effects.contains(&WindowEffect::WakeAt(710 + AUTO_COLLAPSE_DELAY_MS)));
            coordinator.dispatch(WindowIntent::Tick { now_ms: 1_409 });
            assert_eq!(
                coordinator.state().visibility(),
                VisibilityState::Presented(StableVisibility::DockedExpanded(edge))
            );
            coordinator.dispatch(WindowIntent::Tick { now_ms: 1_410 });
            assert!(matches!(
                coordinator.state().visibility(),
                VisibilityState::Animating(AnimationState {
                    final_visibility: StableVisibility::DockedCollapsed(collapsed_edge),
                    ..
                }) if collapsed_edge == edge
            ));

            coordinator.dispatch(WindowIntent::GuardsChanged {
                guards: WindowGuardSnapshot {
                    ime_composing: true,
                    ..Default::default()
                },
                now_ms: 1_411,
            });
            assert!(matches!(
                coordinator.state().visibility(),
                VisibilityState::Animating(AnimationState {
                    final_visibility: StableVisibility::DockedExpanded(expanded_edge),
                    ..
                }) if expanded_edge == edge
            ));
        }
    }

    #[test]
    fn hover_reveal_and_leave_delays_are_exact() {
        let mut coordinator = coordinator(StableVisibility::DockedCollapsed(DockEdge::Right));
        coordinator.dispatch(WindowIntent::SensorEntered { now_ms: 1_000 });
        coordinator.dispatch(WindowIntent::Tick {
            now_ms: 1_000 + HOVER_REVEAL_DELAY_MS - 1,
        });
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedCollapsed(DockEdge::Right))
        );
        coordinator.dispatch(WindowIntent::Tick {
            now_ms: 1_000 + HOVER_REVEAL_DELAY_MS,
        });
        coordinator.dispatch(WindowIntent::Tick {
            now_ms: 1_000 + HOVER_REVEAL_DELAY_MS + ANIMATION_DURATION_MS,
        });
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedExpanded(DockEdge::Right))
        );

        coordinator.dispatch(WindowIntent::PointerLeft { now_ms: 2_000 });
        coordinator.dispatch(WindowIntent::Tick {
            now_ms: 2_000 + HOVER_LEAVE_COLLAPSE_DELAY_MS - 1,
        });
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedExpanded(DockEdge::Right))
        );
        coordinator.dispatch(WindowIntent::Tick {
            now_ms: 2_000 + HOVER_LEAVE_COLLAPSE_DELAY_MS,
        });
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::Animating(_)
        ));
    }

    #[test]
    fn animation_finishes_at_140ms_and_stops_requesting_wakes() {
        let mut coordinator = coordinator(StableVisibility::DockedExpanded(DockEdge::Top));
        coordinator.dispatch(WindowIntent::ManualCollapse { now_ms: 100 });
        coordinator.dispatch(WindowIntent::Tick { now_ms: 239 });
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::Animating(_)
        ));
        let effects = coordinator.dispatch(WindowIntent::Tick { now_ms: 240 });
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedCollapsed(DockEdge::Top))
        );
        observe_at_target(&mut coordinator, &effects);
        coordinator.answer_pending_receipts();
        assert_eq!(coordinator.state().next_deadline_ms(), None);
    }

    /// Plan 09 single-instance wake: a same-directory second instance shows and activates the
    /// first, and a window resting collapsed at its dock edge is restored expanded like a show
    /// from the tray, however it collapsed: after 700 ms without focus, by the collapse button
    /// while focused, or already collapsed at start.
    #[test]
    fn a_second_instance_expands_a_collapsed_dock() {
        #[derive(Debug, Clone, Copy)]
        enum Collapse {
            Blur,
            ByHandFocused,
            AtStart,
        }
        let focused = |window_focused| WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot {
                window_focused,
                ..Default::default()
            },
            now_ms: 0,
        };
        for edge in [DockEdge::Left, DockEdge::Right, DockEdge::Top] {
            for collapse in [Collapse::Blur, Collapse::ByHandFocused, Collapse::AtStart] {
                let label = format!("{edge:?} {collapse:?}");
                let mut coordinator = match collapse {
                    Collapse::AtStart => coordinator(StableVisibility::DockedCollapsed(edge)),
                    Collapse::Blur | Collapse::ByHandFocused => {
                        coordinator(StableVisibility::DockedExpanded(edge))
                    }
                };
                let expanded = effective_expanded_frame(
                    &coordinator.state.placement,
                    Some(edge),
                    &coordinator.state.dock.monitor,
                );
                match collapse {
                    Collapse::Blur => {
                        coordinator.dispatch(focused(false));
                        coordinator.dispatch(WindowIntent::Tick {
                            now_ms: AUTO_COLLAPSE_DELAY_MS,
                        });
                        coordinator.dispatch(WindowIntent::Tick {
                            now_ms: AUTO_COLLAPSE_DELAY_MS + ANIMATION_DURATION_MS,
                        });
                        assert!(!coordinator.state.dock.manually_hidden, "{label}");
                    }
                    Collapse::ByHandFocused => {
                        coordinator.dispatch(focused(true));
                        coordinator.dispatch(WindowIntent::ManualCollapse { now_ms: 10 });
                        coordinator.dispatch(WindowIntent::Tick {
                            now_ms: 10 + ANIMATION_DURATION_MS,
                        });
                    }
                    Collapse::AtStart => {}
                }
                assert_eq!(
                    coordinator.state().visibility(),
                    VisibilityState::Presented(StableVisibility::DockedCollapsed(edge)),
                    "{label}"
                );
                assert_ne!(coordinator.state().frame(), expanded, "{label}");

                let now = 1_000;
                let effects = coordinator.dispatch(WindowIntent::ShowRequested {
                    reason: ShowReason::SecondInstance,
                    now_ms: now,
                });
                assert!(
                    effects
                        .iter()
                        .any(|effect| matches!(effect, WindowEffect::FocusWindow { .. })),
                    "{label}"
                );
                assert!(
                    !coordinator.state().temporary_sensor_topmost(),
                    "{label}: the sensor's temporary topmost ends with the wake"
                );
                // The 140 ms expansion starts at the wake, not before or after it.
                coordinator.dispatch(WindowIntent::Tick {
                    now_ms: now + ANIMATION_DURATION_MS - 1,
                });
                assert!(
                    matches!(
                        coordinator.state().visibility(),
                        VisibilityState::Animating(_)
                    ),
                    "{label}"
                );
                let effects = coordinator.dispatch(WindowIntent::Tick {
                    now_ms: now + ANIMATION_DURATION_MS,
                });
                assert_eq!(
                    coordinator.state().visibility(),
                    VisibilityState::Presented(StableVisibility::DockedExpanded(edge)),
                    "{label}"
                );
                assert_eq!(coordinator.state().frame(), expanded, "{label}");
                observe_at_target(&mut coordinator, &effects);
                coordinator.answer_pending_receipts();
                assert_eq!(coordinator.state().next_deadline_ms(), None, "{label}");
                assert!(coordinator.state().accepts_editor_mutation(), "{label}");
                // The focus the wake requested keeps it expanded.
                coordinator.dispatch(focused(true));
                assert_eq!(coordinator.state().next_deadline_ms(), None, "{label}");
                assert_eq!(
                    coordinator.state().visibility(),
                    VisibilityState::Presented(StableVisibility::DockedExpanded(edge)),
                    "{label}"
                );
            }
        }
    }

    #[test]
    fn escape_collapses_only_a_docked_window_without_delay() {
        let mut docked = coordinator(StableVisibility::DockedExpanded(DockEdge::Left));
        let effects = docked.dispatch(WindowIntent::EscapePressed { now_ms: 50 });
        assert!(effects.contains(&WindowEffect::CancelImePreedit));
        assert!(matches!(
            docked.state().visibility(),
            VisibilityState::Animating(_)
        ));
        docked.dispatch(WindowIntent::Tick {
            now_ms: 50 + ANIMATION_DURATION_MS,
        });
        assert_eq!(
            docked.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedCollapsed(DockEdge::Left))
        );

        let mut floating = coordinator(StableVisibility::Floating);
        let effects = floating.dispatch(WindowIntent::EscapePressed { now_ms: 50 });
        assert!(effects.is_empty());
        assert!(!floating.state().dock().manually_hidden);
        assert_eq!(
            floating.state().visibility(),
            VisibilityState::Presented(StableVisibility::Floating)
        );
        assert_eq!(floating.state().next_deadline_ms(), None);
    }

    #[test]
    fn cancelled_hover_deadline_cannot_reveal_from_a_stale_tick() {
        let mut coordinator = coordinator(StableVisibility::DockedCollapsed(DockEdge::Right));
        coordinator.dispatch(WindowIntent::SensorEntered { now_ms: 1_000 });
        coordinator.dispatch(WindowIntent::PointerLeft { now_ms: 1_050 });
        coordinator.dispatch(WindowIntent::Tick {
            now_ms: 1_000 + HOVER_REVEAL_DELAY_MS,
        });
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedCollapsed(DockEdge::Right))
        );
        assert_eq!(coordinator.state().next_deadline_ms(), None);
    }

    #[test]
    fn temporary_sensor_topmost_survives_hover_reveal_until_focus() {
        let mut coordinator = coordinator(StableVisibility::DockedExpanded(DockEdge::Left));
        let effects = coordinator.dispatch(WindowIntent::ManualCollapse { now_ms: 100 });
        assert!(asks_sensor_topmost(&effects, true));
        coordinator.dispatch(WindowIntent::Tick { now_ms: 240 });
        assert!(coordinator.state().temporary_sensor_topmost());

        coordinator.dispatch(WindowIntent::SensorEntered { now_ms: 300 });
        coordinator.dispatch(WindowIntent::Tick {
            now_ms: 300 + HOVER_REVEAL_DELAY_MS,
        });
        coordinator.dispatch(WindowIntent::Tick {
            now_ms: 300 + HOVER_REVEAL_DELAY_MS + ANIMATION_DURATION_MS,
        });
        assert!(coordinator.state().temporary_sensor_topmost());
        let effects = coordinator.dispatch(WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot {
                window_focused: true,
                ..Default::default()
            },
            now_ms: 600,
        });
        assert!(asks_sensor_topmost(&effects, false));
        assert!(!coordinator.state().temporary_sensor_topmost());
    }

    /// Plan 09: returning to the desktop hands focus back to a window that is a strip by now.
    #[test]
    fn focus_handed_back_to_a_strip_keeps_it_on_top_and_does_not_expand_it() {
        let guards = |window_focused, now_ms| WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot {
                window_focused,
                ..Default::default()
            },
            now_ms,
        };
        let mut coordinator = coordinator(StableVisibility::DockedExpanded(DockEdge::Top));
        coordinator.dispatch(guards(true, 0));
        coordinator.dispatch(WindowIntent::ManualCollapse { now_ms: 100 });
        coordinator.dispatch(WindowIntent::Tick { now_ms: 240 });
        assert!(coordinator.state().temporary_sensor_topmost());
        assert!(matches!(
            coordinator.state().visibility,
            VisibilityState::Presented(StableVisibility::DockedCollapsed(DockEdge::Top))
        ));

        coordinator.dispatch(guards(false, 300));
        let effects = coordinator.dispatch(guards(true, 400));

        assert!(!asks_sensor_topmost(&effects, false));
        assert!(coordinator.state().temporary_sensor_topmost());
        assert!(matches!(
            coordinator.state().visibility,
            VisibilityState::Presented(StableVisibility::DockedCollapsed(DockEdge::Top))
        ));
    }

    #[test]
    fn focus_arriving_during_the_collapse_animation_keeps_the_strip_on_top() {
        let guards = |window_focused, now_ms| WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot {
                window_focused,
                ..Default::default()
            },
            now_ms,
        };
        let mut coordinator = coordinator(StableVisibility::DockedExpanded(DockEdge::Left));
        coordinator.dispatch(WindowIntent::ManualCollapse { now_ms: 100 });
        let effects = coordinator.dispatch(guards(true, 120));
        assert!(!asks_sensor_topmost(&effects, false));
    }

    #[test]
    fn focused_manual_collapse_keeps_sensor_topmost_until_expansion_starts() {
        let mut coordinator = coordinator(StableVisibility::DockedExpanded(DockEdge::Left));
        coordinator.dispatch(WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot {
                window_focused: true,
                ..Default::default()
            },
            now_ms: 0,
        });

        coordinator.dispatch(WindowIntent::ManualCollapse { now_ms: 100 });
        let repeated_guard_effects = coordinator.dispatch(WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot {
                window_focused: true,
                ..Default::default()
            },
            now_ms: 101,
        });
        assert!(!asks_sensor_topmost(&repeated_guard_effects, false));
        coordinator.dispatch(WindowIntent::Tick { now_ms: 240 });
        assert!(coordinator.state().temporary_sensor_topmost());

        coordinator.dispatch(WindowIntent::SensorEntered { now_ms: 300 });
        let expand_effects = coordinator.dispatch(WindowIntent::Tick {
            now_ms: 300 + HOVER_REVEAL_DELAY_MS,
        });
        assert!(asks_sensor_topmost(&expand_effects, false));
        assert!(!coordinator.state().temporary_sensor_topmost());
    }

    #[test]
    fn hidden_to_tray_never_keeps_temporary_sensor_topmost() {
        let mut coordinator = coordinator(StableVisibility::DockedCollapsed(DockEdge::Top));
        assert!(coordinator.state().temporary_sensor_topmost());
        let effects = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: WindowGuardSnapshot::default(),
        });
        assert!(asks_sensor_topmost(&effects, false));
        assert!(!coordinator.state().temporary_sensor_topmost());
    }

    #[test]
    fn drag_snap_and_undock_update_one_durable_placement() {
        let mut coordinator = coordinator(StableVisibility::Floating);
        let monitor = monitor("primary", -1920, 1.5, true);
        let near_left = PhysicalRect::new(monitor.work_area.x + 10, -100, 780, 900);
        let effects = coordinator.dispatch(WindowIntent::DragEnded {
            frame: near_left,
            monitor: monitor.clone(),
            now_ms: 0,
        });
        assert_eq!(coordinator.state().dock().edge, Some(DockEdge::Left));
        let effects = observe_at_target(&mut coordinator, &effects);
        assert_eq!(
            effects
                .iter()
                .filter(|effect| matches!(effect, WindowEffect::CommitPlacement { .. }))
                .count(),
            1
        );

        let inward = PhysicalRect::new(
            monitor.work_area.x + (16.0 * monitor.scale_factor) as i32 + 1,
            -100,
            780,
            900,
        );
        coordinator.dispatch(WindowIntent::DragEnded {
            frame: inward,
            monitor,
            now_ms: 1,
        });
        assert_eq!(coordinator.state().dock().edge, None);
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::Floating)
        );
    }

    #[test]
    fn direct_drag_between_dock_edges_redocks_in_one_transition() {
        let monitor = monitor("direct-redock", -1920, 1.5, true);
        let targets = [
            (
                PhysicalRect::new(monitor.work_area.x, monitor.work_area.y + 100, 780, 900),
                DockEdge::Left,
            ),
            (
                PhysicalRect::new(monitor.work_area.x + 400, monitor.work_area.y, 780, 900),
                DockEdge::Top,
            ),
            (
                PhysicalRect::new(
                    (monitor.work_area.right() - 780) as i32,
                    monitor.work_area.y + 100,
                    780,
                    900,
                ),
                DockEdge::Right,
            ),
        ];

        for previous_edge in [DockEdge::Left, DockEdge::Top, DockEdge::Right] {
            for (frame, expected_edge) in targets {
                if previous_edge == expected_edge {
                    continue;
                }
                let mut coordinator = coordinator(StableVisibility::DockedExpanded(previous_edge));
                let effects = coordinator.dispatch(WindowIntent::DragEnded {
                    frame,
                    monitor: monitor.clone(),
                    now_ms: 10,
                });
                assert_eq!(coordinator.state().dock().edge, Some(expected_edge));
                assert_eq!(
                    coordinator.state().visibility(),
                    VisibilityState::Presented(StableVisibility::DockedExpanded(expected_edge))
                );
                let effects = observe_at_target(&mut coordinator, &effects);
                assert_eq!(
                    coordinator.state().next_deadline_ms(),
                    Some(10 + AUTO_COLLAPSE_DELAY_MS)
                );
                assert!(effects.iter().any(|effect| matches!(
                    effect,
                    WindowEffect::CommitPlacement {
                        dock_edge: Some(edge),
                        ..
                    } if *edge == expected_edge
                )));
            }
        }
    }

    #[test]
    fn topology_recovery_preserves_edge_on_primary_with_negative_coordinates() {
        let mut coordinator = coordinator(StableVisibility::DockedCollapsed(DockEdge::Right));
        let primary = monitor("new-primary", 0, 2.0, true);
        let effects = coordinator.dispatch(WindowIntent::DisplayTopologyChanged {
            monitors: vec![primary.clone()],
            now_ms: 0,
        });
        assert_eq!(
            coordinator.state().dock().monitor.identity,
            primary.identity
        );
        assert_eq!(coordinator.state().dock().edge, Some(DockEdge::Right));
        assert_eq!(
            coordinator.state().frame().right(),
            primary.work_area.right()
        );
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedExpanded(DockEdge::Right))
        );
        let effects = observe_at_target(&mut coordinator, &effects);
        assert!(
            effects
                .iter()
                .any(|effect| matches!(effect, WindowEffect::CommitPlacement { .. }))
        );
    }

    #[test]
    fn phase10_focused_dock_release_stays_expanded_without_collapse_deadline() {
        let monitor = monitor("focused-release", -1920, 1.5, true);
        let mut coordinator = WindowShellCoordinator::new(
            WindowPlacement::new(520.0, 680.0, None, 0.5, 0.5, 0.25),
            StableVisibility::Floating,
            monitor.clone(),
            true,
        );
        coordinator.dispatch(WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot {
                window_focused: true,
                dragging: true,
                ..Default::default()
            },
            now_ms: 0,
        });
        let frame = PhysicalRect::new(monitor.work_area.x, monitor.work_area.y + 100, 780, 900);
        let effects = coordinator.dispatch(WindowIntent::DragEnded {
            frame,
            monitor,
            now_ms: 10,
        });
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedExpanded(DockEdge::Left))
        );
        observe_at_target(&mut coordinator, &effects);
        assert_eq!(coordinator.state().next_deadline_ms(), None);
    }

    #[test]
    fn hidden_monitor_recovery_stays_hidden_then_shows_expanded_on_primary() {
        let mut coordinator = coordinator(StableVisibility::DockedCollapsed(DockEdge::Left));
        let effects = coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: WindowGuardSnapshot::default(),
        });
        report_hidden(&mut coordinator, &effects);
        let primary = monitor("replacement-primary", 0, 1.25, true);
        coordinator.dispatch(WindowIntent::DisplayTopologyChanged {
            monitors: vec![primary.clone()],
            now_ms: 0,
        });
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::HiddenToTray { .. }
        ));
        let effects = coordinator.dispatch(WindowIntent::ShowRequested {
            reason: ShowReason::Tray,
            now_ms: 1,
        });
        let id = effects
            .iter()
            .find_map(|effect| match effect {
                WindowEffect::ShowWindow { id } => Some(*id),
                _ => None,
            })
            .expect("a show request");
        coordinator.dispatch(WindowIntent::ActionCompleted {
            now_ms: 1,
            id,
            outcome: Ok(()),
        });
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedExpanded(DockEdge::Left))
        );
        assert!(primary.work_area.contains(coordinator.state().frame()));
    }

    #[test]
    fn phase10_split_keeps_the_user_selected_compact_width() {
        let monitor = monitor("right", -1920, 1.5, true);
        let placement =
            WindowPlacement::new(220.0, 120.0, Some(monitor.identity.clone()), 0.5, 0.5, 0.25);
        let mut coordinator = WindowShellCoordinator::new(
            placement,
            StableVisibility::DockedExpanded(DockEdge::Right),
            monitor.clone(),
            true,
        );
        let original = coordinator.state().frame();
        assert!(
            coordinator
                .dispatch(WindowIntent::SplitModeChanged { split: true })
                .is_empty()
        );
        assert_eq!(coordinator.state().frame(), original);
        assert!(
            coordinator
                .dispatch(WindowIntent::SplitModeChanged { split: false })
                .is_empty()
        );
        assert_eq!(coordinator.state().frame(), original);
    }

    #[test]
    fn collapsed_and_unfocused_hover_reveal_never_accept_editor_mutation() {
        let mut coordinator = coordinator(StableVisibility::DockedCollapsed(DockEdge::Left));
        assert!(!coordinator.state().accepts_editor_mutation());
        coordinator.state.dock.hover_revealed = true;
        coordinator.state.visibility =
            VisibilityState::Presented(StableVisibility::DockedExpanded(DockEdge::Left));
        assert!(!coordinator.state().accepts_editor_mutation());
        coordinator.state.guards.window_focused = true;
        assert!(coordinator.state().accepts_editor_mutation());
    }

    #[test]
    fn phase11b_pin_is_orthogonal_to_every_auto_hide_reducer_transition() {
        for edge in [DockEdge::Left, DockEdge::Top, DockEdge::Right] {
            let mut unpinned =
                coordinator(StableVisibility::DockedExpanded(edge)).with_user_topmost(false);
            let mut pinned =
                coordinator(StableVisibility::DockedExpanded(edge)).with_user_topmost(true);
            let intents = [
                WindowIntent::GuardsChanged {
                    guards: WindowGuardSnapshot::default(),
                    now_ms: 0,
                },
                WindowIntent::GuardsChanged {
                    guards: WindowGuardSnapshot {
                        ime_composing: true,
                        ..Default::default()
                    },
                    now_ms: 100,
                },
                WindowIntent::GuardsChanged {
                    guards: WindowGuardSnapshot {
                        dragging: true,
                        ..Default::default()
                    },
                    now_ms: 200,
                },
                WindowIntent::GuardsChanged {
                    guards: WindowGuardSnapshot {
                        popup_open: true,
                        ..Default::default()
                    },
                    now_ms: 300,
                },
                WindowIntent::GuardsChanged {
                    guards: WindowGuardSnapshot::default(),
                    now_ms: 400,
                },
                WindowIntent::Tick {
                    now_ms: 400 + AUTO_COLLAPSE_DELAY_MS,
                },
                WindowIntent::Tick {
                    now_ms: 400 + AUTO_COLLAPSE_DELAY_MS + ANIMATION_DURATION_MS,
                },
                WindowIntent::SensorEntered { now_ms: 1_300 },
                WindowIntent::Tick {
                    now_ms: 1_300 + HOVER_REVEAL_DELAY_MS,
                },
                WindowIntent::Tick {
                    now_ms: 1_300 + HOVER_REVEAL_DELAY_MS + ANIMATION_DURATION_MS,
                },
                WindowIntent::PointerLeft { now_ms: 1_600 },
                WindowIntent::Tick {
                    now_ms: 1_600 + HOVER_LEAVE_COLLAPSE_DELAY_MS,
                },
            ];

            // The topmost effects and fields are the one thing Pin may change.
            let without_topmost = |effects: Vec<WindowEffect>| -> Vec<WindowEffect> {
                effects
                    .into_iter()
                    .filter(|effect| !matches!(effect, WindowEffect::ApplyTopmost(_)))
                    .collect()
            };
            assert!(pinned.state.topmost.user_confirmed);
            assert!(!unpinned.state.topmost.user_confirmed);
            for intent in intents {
                let unpinned_effects = unpinned.dispatch(intent.clone());
                let pinned_effects = pinned.dispatch(intent);
                assert_eq!(
                    without_topmost(unpinned_effects),
                    without_topmost(pinned_effects)
                );
                assert_eq!(unpinned.state().visibility(), pinned.state().visibility());
                assert_eq!(unpinned.state().dock(), pinned.state().dock());
                assert_eq!(
                    unpinned.state().next_deadline_ms(),
                    pinned.state().next_deadline_ms()
                );
                assert_eq!(
                    super::super::state::effective_topmost(
                        false,
                        unpinned.state().temporary_sensor_topmost()
                    ),
                    unpinned.state().temporary_sensor_topmost()
                );
                assert!(super::super::state::effective_topmost(
                    true,
                    pinned.state().temporary_sensor_topmost()
                ));
            }
        }

        let mut floating = coordinator(StableVisibility::Floating);
        floating.dispatch(WindowIntent::GuardsChanged {
            guards: WindowGuardSnapshot::default(),
            now_ms: 0,
        });
        assert_eq!(floating.state().next_deadline_ms(), None);
        assert_eq!(
            floating.state().visibility(),
            VisibilityState::Presented(StableVisibility::Floating)
        );
    }

    #[test]
    #[ignore = "Release-only Phase 8 state and geometry performance receipt"]
    fn phase8_performance_window_reducer_and_geometry() {
        let mut reducer_samples = Vec::with_capacity(25);
        let mut geometry_samples = Vec::with_capacity(25);
        for batch in 0..25_u64 {
            let mut coordinator = coordinator(StableVisibility::DockedExpanded(DockEdge::Left));
            let started = Instant::now();
            for index in 0..100_000_u64 {
                let now_ms = batch * 100_000 + index;
                let intent = if index % 2 == 0 {
                    WindowIntent::GuardsChanged {
                        guards: WindowGuardSnapshot {
                            window_focused: index % 4 == 0,
                            ..Default::default()
                        },
                        now_ms,
                    }
                } else {
                    WindowIntent::Tick { now_ms }
                };
                std::hint::black_box(coordinator.dispatch(intent));
            }
            reducer_samples.push(started.elapsed());

            let monitor = monitor(
                "perf",
                -1920,
                [1.0, 1.25, 1.5, 2.0][batch as usize % 4],
                true,
            );
            let started = Instant::now();
            let mut checksum = 0_i64;
            for index in 0..100_000_i32 {
                let frame = PhysicalRect::new(
                    monitor.work_area.x + index % 32,
                    monitor.work_area.y + index % 64,
                    520,
                    680,
                );
                checksum += snap_edge(frame, &monitor).is_some() as i64;
                checksum += should_undock(DockEdge::Left, frame, &monitor) as i64;
            }
            std::hint::black_box(checksum);
            geometry_samples.push(started.elapsed());
        }
        reducer_samples.sort_unstable();
        geometry_samples.sort_unstable();
        eprintln!(
            "phase8 reducer_100k median={:?} p95={:?} max={:?}; geometry_100k median={:?} p95={:?} max={:?}",
            reducer_samples[12],
            reducer_samples[23],
            reducer_samples[24],
            geometry_samples[12],
            geometry_samples[23],
            geometry_samples[24]
        );
    }
}
