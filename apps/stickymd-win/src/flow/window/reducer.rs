//! Deterministic mutation owner for window, docking, timers, and quit staging.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use super::geometry::{
    MonitorGeometry, PhysicalRect, WindowPlacement, collapsed_frame, expanded_frame,
    placement_from_frame, recover_placement, should_undock, snap_edge,
};
use super::state::{
    ANIMATION_DURATION_MS, ANIMATION_FRAME_MS, AUTO_COLLAPSE_DELAY_MS, AnimationState, DockEdge,
    DockState, HOVER_LEAVE_COLLAPSE_DELAY_MS, HOVER_REVEAL_DELAY_MS, LifecycleState,
    StableVisibility, VisibilityState, WindowEffect, WindowGuardSnapshot, WindowIntent,
    WindowShellState,
};

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
                placement,
                frame,
                guards: WindowGuardSnapshot::default(),
                deadlines: Default::default(),
                lifecycle: LifecycleState::Running,
                tray_available,
                hide_save_pending: false,
                temporary_sensor_topmost: matches!(
                    initial_visibility,
                    StableVisibility::DockedCollapsed(_)
                ),
            },
        }
    }

    pub fn state(&self) -> &WindowShellState {
        &self.state
    }

    pub fn dispatch(&mut self, intent: WindowIntent) -> Vec<WindowEffect> {
        let mut effects = Vec::new();
        match intent {
            WindowIntent::CloseRequested { guards, .. } => {
                self.state.guards = guards;
                self.hide_to_tray(&mut effects);
            }
            WindowIntent::ShowRequested { .. } => self.show(&mut effects),
            WindowIntent::TrayToggleRequested { now_ms: _, guards } => {
                self.state.guards = guards;
                if matches!(self.state.visibility, VisibilityState::HiddenToTray { .. }) {
                    self.show(&mut effects);
                } else {
                    self.hide_to_tray(&mut effects);
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
            WindowIntent::GuardsChanged { guards, now_ms } => {
                self.update_guards(guards, now_ms, &mut effects);
            }
            WindowIntent::DragEnded {
                frame,
                monitor,
                now_ms,
            } => self.finish_drag(frame, monitor, now_ms, &mut effects),
            WindowIntent::DisplayTopologyChanged { monitors } => {
                self.recover_topology(&monitors, &mut effects);
            }
            WindowIntent::SplitModeChanged { split } => {
                self.update_split_mode(split, &mut effects);
            }
            WindowIntent::Tick { now_ms } => self.tick(now_ms, &mut effects),
            WindowIntent::PasteSettled { guards } => {
                self.state.guards = guards;
                self.paste_settled(&mut effects);
            }
            WindowIntent::HideSaveCompleted { succeeded, guards } => {
                self.state.guards = guards;
                self.complete_hide_save(succeeded, &mut effects);
            }
            WindowIntent::QuitBarrierCompleted {
                barrier,
                succeeded,
                guards,
            } => {
                self.state.guards = guards;
                self.complete_quit_barrier(barrier, succeeded, &mut effects);
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
        // a real focus acquisition may revoke the collapsed sensor's temporary
        // z-order; replaying an unchanged focused snapshot after manual hide
        // must not make the 3-DIP recovery path unreachable.
        let gained_focus = !self.state.guards.window_focused && guards.window_focused;
        self.state.guards = guards;
        if gained_focus {
            self.set_temporary_sensor_topmost(false, effects);
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
        } else if matches!(self.current_stable(), StableVisibility::DockedExpanded(_))
            && self.state.deadlines.auto_collapse_ms.is_none()
            && !self.state.lifecycle.is_quitting()
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
        if self.state.lifecycle.is_quitting() {
            return;
        }
        self.state.deadlines.clear();
        let previous_edge = self.current_stable().dock_edge();
        let edge = if previous_edge.is_some_and(|edge| should_undock(edge, frame, &monitor)) {
            None
        } else {
            snap_edge(frame, &monitor).or(previous_edge)
        };
        let placement = placement_from_frame(frame, &monitor, &self.state.placement, edge);
        self.state.placement = placement;
        self.state.dock.monitor = monitor;
        self.state.dock.edge = edge;
        self.state.dock.offset_ratio = self.state.placement.dock_offset_ratio;
        self.state.dock.manually_hidden = false;
        self.state.dock.hover_revealed = false;
        self.set_temporary_sensor_topmost(false, effects);
        self.state.frame =
            effective_expanded_frame(&self.state.placement, edge, &self.state.dock.monitor);
        let stable = edge.map_or(StableVisibility::Floating, StableVisibility::DockedExpanded);
        self.state.visibility = VisibilityState::Presented(stable);
        effects.push(WindowEffect::ApplyFrame(self.state.frame));
        effects.push(WindowEffect::CommitPlacement {
            placement: self.state.placement.clone(),
            dock_edge: edge,
        });
        if edge.is_some() && !self.state.guards.blocks_auto_collapse() {
            let deadline = now_ms.saturating_add(AUTO_COLLAPSE_DELAY_MS);
            self.state.deadlines.auto_collapse_ms = Some(deadline);
            effects.push(WindowEffect::WakeAt(deadline));
        }
    }

    fn recover_topology(&mut self, monitors: &[MonitorGeometry], effects: &mut Vec<WindowEffect>) {
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
            edge.map_or(StableVisibility::Floating, StableVisibility::DockedExpanded)
        };
        self.state.placement.monitor_identity = Some(monitor.identity.clone());
        self.state.dock.monitor = monitor.clone();
        self.state.dock.edge = edge;
        self.state.dock.hover_revealed = false;
        let sensor_topmost = matches!(stable, StableVisibility::DockedCollapsed(_));
        self.set_temporary_sensor_topmost(sensor_topmost, effects);
        self.state.frame = match stable {
            StableVisibility::DockedCollapsed(edge) => collapsed_frame(expanded, edge, monitor),
            _ => expanded,
        };
        self.state.visibility = match self.state.visibility {
            VisibilityState::HiddenToTray { .. } => {
                VisibilityState::HiddenToTray { restore: stable }
            }
            VisibilityState::AwaitingHideSave { .. } => {
                VisibilityState::AwaitingHideSave { restore: stable }
            }
            _ => VisibilityState::Presented(stable),
        };
        if matches!(
            self.state.visibility,
            VisibilityState::Presented(_) | VisibilityState::Animating(_)
        ) {
            effects.push(WindowEffect::ApplyFrame(self.state.frame));
        }
        effects.push(WindowEffect::CommitPlacement {
            placement: self.state.placement.clone(),
            dock_edge: edge,
        });
    }

    fn update_split_mode(&mut self, _split: bool, _effects: &mut Vec<WindowEffect>) {
        // Phase 10 keeps every ViewMode at the user-selected width, including
        // the 220 DIP minimum. Split is a 50/50 projection, not geometry policy.
    }

    fn tick(&mut self, now_ms: u64, effects: &mut Vec<WindowEffect>) {
        if let VisibilityState::Animating(mut animation) = self.state.visibility {
            self.state.frame = animation.frame_at(now_ms);
            effects.push(WindowEffect::ApplyFrame(self.state.frame));
            effects.push(WindowEffect::RequestRedraw);
            if now_ms >= animation.end_ms {
                self.state.visibility = VisibilityState::Presented(animation.final_visibility);
            } else {
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

    fn start_collapse(&mut self, now_ms: u64, effects: &mut Vec<WindowEffect>) {
        let Some(edge) = self.current_stable().dock_edge() else {
            return;
        };
        if matches!(self.current_stable(), StableVisibility::DockedCollapsed(_)) {
            return;
        }
        self.state.deadlines.clear();
        self.state.dock.edge = Some(edge);
        self.state.dock.hover_revealed = false;
        self.set_temporary_sensor_topmost(true, effects);
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

    fn start_expand(&mut self, now_ms: u64, hover_revealed: bool, effects: &mut Vec<WindowEffect>) {
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

    pub(super) fn set_temporary_sensor_topmost(
        &mut self,
        enabled: bool,
        effects: &mut Vec<WindowEffect>,
    ) {
        if self.state.temporary_sensor_topmost != enabled {
            self.state.temporary_sensor_topmost = enabled;
            effects.push(WindowEffect::SetTemporarySensorTopmost(enabled));
        }
    }

    pub(super) fn current_stable(&self) -> StableVisibility {
        match self.state.visibility {
            VisibilityState::Presented(stable)
            | VisibilityState::AwaitingHideSave { restore: stable }
            | VisibilityState::HiddenToTray { restore: stable } => stable,
            VisibilityState::Animating(animation) => animation.final_visibility,
        }
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
        QuitBarrier, QuitStage, ShowReason, WindowSaveReason,
    };
    use super::*;

    fn monitor(identity: &str, x: i32, scale: f64, primary: bool) -> MonitorGeometry {
        MonitorGeometry::new(
            MonitorIdentity::new(identity),
            PhysicalRect::new(x, -200, 1920, 1080),
            scale,
            primary,
        )
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
        assert!(!effects.contains(&WindowEffect::SetVisible(false)));
        assert!(!effects.contains(&WindowEffect::RequestSafeAssetGc));
        let effects = coordinator.dispatch(WindowIntent::HideSaveCompleted {
            succeeded: true,
            guards: WindowGuardSnapshot::default(),
        });
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::HiddenToTray { .. }
        ));
        assert!(effects.contains(&WindowEffect::SetVisible(false)));
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
        assert!(!effects.contains(&WindowEffect::SetVisible(false)));
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
        assert!(!effects.contains(&WindowEffect::SetVisible(false)));
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
        coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: WindowGuardSnapshot::default(),
        });
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
        coordinator.dispatch(WindowIntent::Tick { now_ms: 240 });
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedCollapsed(DockEdge::Top))
        );
        assert_eq!(coordinator.state().next_deadline_ms(), None);
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
        assert!(effects.contains(&WindowEffect::SetTemporarySensorTopmost(true)));
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
        assert!(effects.contains(&WindowEffect::SetTemporarySensorTopmost(false)));
        assert!(!coordinator.state().temporary_sensor_topmost());
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
        assert!(!repeated_guard_effects.contains(&WindowEffect::SetTemporarySensorTopmost(false)));
        coordinator.dispatch(WindowIntent::Tick { now_ms: 240 });
        assert!(coordinator.state().temporary_sensor_topmost());

        coordinator.dispatch(WindowIntent::SensorEntered { now_ms: 300 });
        let expand_effects = coordinator.dispatch(WindowIntent::Tick {
            now_ms: 300 + HOVER_REVEAL_DELAY_MS,
        });
        assert!(expand_effects.contains(&WindowEffect::SetTemporarySensorTopmost(false)));
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
        assert!(effects.contains(&WindowEffect::SetTemporarySensorTopmost(false)));
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
    fn topology_recovery_preserves_edge_on_primary_with_negative_coordinates() {
        let mut coordinator = coordinator(StableVisibility::DockedCollapsed(DockEdge::Right));
        let primary = monitor("new-primary", 0, 2.0, true);
        let effects = coordinator.dispatch(WindowIntent::DisplayTopologyChanged {
            monitors: vec![primary.clone()],
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
        coordinator.dispatch(WindowIntent::DragEnded {
            frame,
            monitor,
            now_ms: 10,
        });
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedExpanded(DockEdge::Left))
        );
        assert_eq!(coordinator.state().next_deadline_ms(), None);
    }

    #[test]
    fn hidden_monitor_recovery_stays_hidden_then_shows_expanded_on_primary() {
        let mut coordinator = coordinator(StableVisibility::DockedCollapsed(DockEdge::Left));
        coordinator.dispatch(WindowIntent::CloseRequested {
            now_ms: 0,
            guards: WindowGuardSnapshot::default(),
        });
        let primary = monitor("replacement-primary", 0, 1.25, true);
        coordinator.dispatch(WindowIntent::DisplayTopologyChanged {
            monitors: vec![primary.clone()],
        });
        assert!(matches!(
            coordinator.state().visibility(),
            VisibilityState::HiddenToTray { .. }
        ));
        let effects = coordinator.dispatch(WindowIntent::ShowRequested {
            reason: ShowReason::Tray,
            now_ms: 1,
        });
        assert_eq!(
            coordinator.state().visibility(),
            VisibilityState::Presented(StableVisibility::DockedExpanded(DockEdge::Left))
        );
        assert!(primary.work_area.contains(coordinator.state().frame()));
        assert!(effects.contains(&WindowEffect::SetVisible(true)));
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
            let mut unpinned = coordinator(StableVisibility::DockedExpanded(edge));
            let mut pinned = coordinator(StableVisibility::DockedExpanded(edge));
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

            for intent in intents {
                let unpinned_effects = unpinned.dispatch(intent.clone());
                let pinned_effects = pinned.dispatch(intent);
                assert_eq!(unpinned_effects, pinned_effects);
                assert_eq!(unpinned.state(), pinned.state());
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
