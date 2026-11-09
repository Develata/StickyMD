//! Hide-to-tray and ordered quit barriers for the native window lifecycle.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use super::geometry::PhysicalRect;
use super::reducer::WindowShellCoordinator;
use super::state::{
    LifecycleState, PendingFocus, QuitBarrier, QuitStage, StableVisibility, VisibilityState,
    WINDOW_ACTION_RECEIPT_TIMEOUT_MS, WindowEffect, WindowSaveReason,
};
use crate::ports::FailureKind;

impl WindowShellCoordinator {
    pub(super) fn hide_to_tray(&mut self, now_ms: u64, effects: &mut Vec<WindowEffect>) {
        if self.state.lifecycle.is_quitting() {
            return;
        }
        if !self.state.tray_available {
            effects.push(WindowEffect::ReportTrayUnavailable);
            self.request_quit(effects);
            return;
        }
        if self.state.guards.conflict_or_recovery {
            effects.push(WindowEffect::HideBlocked);
            return;
        }
        if matches!(
            self.state.visibility,
            VisibilityState::AwaitingHideSave { .. }
                | VisibilityState::HidePending { .. }
                | VisibilityState::ShowPending { .. }
                | VisibilityState::HiddenToTray { .. }
        ) {
            return;
        }
        let restore = self.current_stable();
        self.state.deadlines.clear();
        self.state.hide_save_pending =
            self.state.guards.paste_pending || self.state.guards.asset_transaction_pending;
        effects.push(WindowEffect::CancelImePreedit);
        effects.push(WindowEffect::SetImeAllowed(false));
        effects.push(WindowEffect::SetEditorInputEnabled(false));
        if self.state.guards.note_save_required || self.state.hide_save_pending {
            self.state.visibility = VisibilityState::AwaitingHideSave { restore };
        } else {
            self.complete_hide(restore, now_ms, effects);
        }
        if self.state.guards.note_save_required && !self.state.hide_save_pending {
            effects.push(WindowEffect::RequestNoteSave(WindowSaveReason::HideToTray));
        }
    }

    pub(super) fn show(&mut self, now_ms: u64, effects: &mut Vec<WindowEffect>) {
        if self.state.lifecycle.is_quitting() {
            return;
        }
        let restore = match self.state.visibility {
            // A newer show replaces one in flight, whose identity dies with it; so does a hide
            // the show overtakes.
            VisibilityState::HiddenToTray { restore }
            | VisibilityState::HidePending { restore, .. }
            | VisibilityState::ShowPending { restore, .. } => restore,
            VisibilityState::AwaitingHideSave { restore } => {
                self.cancel_hide_save(restore, now_ms, effects);
                return;
            }
            _ => {
                // A visible window resting, or settling, collapsed at its dock edge is restored
                // expanded like one shown from the tray (plan 09 single-instance wake).
                if matches!(self.current_stable(), StableVisibility::DockedCollapsed(_)) {
                    self.start_expand(now_ms, false, effects);
                }
                self.request_focus(now_ms, effects);
                return;
            }
        };
        // A show needs its placement's id as well: without it a window hidden while collapsed
        // would be committed expanded while it still sits at its collapsed rectangle.
        let id = self
            .has_action_ids(2)
            .then(|| self.next_window_action())
            .flatten();
        let Some(id) = id else {
            effects.push(WindowEffect::ShowRejected(FailureKind::Failed));
            return;
        };
        self.state.visibility = VisibilityState::ShowPending {
            id,
            restore,
            deadline_ms: now_ms.saturating_add(WINDOW_ACTION_RECEIPT_TIMEOUT_MS),
        };
        // The frame is a request either way (the receipt does not prove it), so it moves now and
        // the window is in place when it appears.
        self.state.frame = self.show_frame(restore);
        self.request_placement(self.state.frame, None, now_ms, effects);
        effects.push(WindowEffect::ShowWindow { id });
    }

    /// Where a window restored from `restore` lands: expanded at the dock edge, or floating.
    fn show_frame(&self, restore: StableVisibility) -> PhysicalRect {
        super::reducer::effective_expanded_frame(
            &self.state.placement,
            restore.dock_edge(),
            &self.state.dock.monitor,
        )
    }

    /// The wake reached a window that never left the screen: nothing native to confirm, so it only
    /// ends the hide's freeze. The save that was waiting then finds no hide to finish.
    fn cancel_hide_save(
        &mut self,
        restore: StableVisibility,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        // Without an id for the frame the window would be committed where it is not.
        if !self.has_action_ids(1) {
            effects.push(WindowEffect::ShowRejected(FailureKind::Failed));
            return;
        }
        self.state.frame = self.show_frame(restore);
        self.request_placement(self.state.frame, None, now_ms, effects);
        self.commit_show(restore, now_ms, effects);
    }

    /// Everything that depends on the window being shown. Quitting has taken the keyboard, so a
    /// show that lands then records the fact and gives no input and no focus back.
    fn commit_show(
        &mut self,
        restore: StableVisibility,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        let restore = match restore {
            StableVisibility::DockedCollapsed(edge) => StableVisibility::DockedExpanded(edge),
            other => other,
        };
        self.state.visibility = VisibilityState::Presented(restore);
        self.state.dock.edge = restore.dock_edge();
        self.state.dock.manually_hidden = false;
        self.state.dock.hover_revealed = false;
        self.set_temporary_sensor_topmost(false, now_ms, effects);
        self.state.deadlines.clear();
        effects.push(WindowEffect::WindowShown);
        if !self.state.lifecycle.is_quitting() {
            self.request_focus(now_ms, effects);
            effects.push(WindowEffect::SetImeAllowed(true));
            effects.push(WindowEffect::SetEditorInputEnabled(true));
        }
        effects.push(WindowEffect::RequestRedraw);
    }

    fn request_focus(&mut self, now_ms: u64, effects: &mut Vec<WindowEffect>) {
        let Some(id) = self.next_window_action() else {
            effects.push(WindowEffect::ShowRejected(FailureKind::Failed));
            return;
        };
        self.state.focus_pending = Some(PendingFocus {
            id,
            deadline_ms: now_ms.saturating_add(WINDOW_ACTION_RECEIPT_TIMEOUT_MS),
        });
        effects.push(WindowEffect::FocusWindow { id });
    }

    /// Whether `count` more action ids exist.
    fn has_action_ids(&self, count: u64) -> bool {
        self.state.last_window_action.checked_add(count).is_some()
    }

    /// The next action id, or `None` once the ids have run out: every caller then fails the way
    /// its own request fails, and no id is ever reused.
    pub(super) fn next_window_action(&mut self) -> Option<u64> {
        let id = self.state.last_window_action.checked_add(1)?;
        self.state.last_window_action = id;
        Some(id)
    }

    /// A request whose receipt never came fails like a rejected one: the id dies and its failure
    /// handling runs. Today's hosts answer in the same dispatch, so this is for hosts that do not.
    pub(super) fn expire_window_actions(&mut self, now_ms: u64, effects: &mut Vec<WindowEffect>) {
        let visibility_action = match self.state.visibility {
            VisibilityState::HidePending {
                id, deadline_ms, ..
            }
            | VisibilityState::ShowPending {
                id, deadline_ms, ..
            } if now_ms >= deadline_ms => Some(id),
            _ => None,
        };
        let focus = self
            .state
            .focus_pending
            .filter(|focus| now_ms >= focus.deadline_ms)
            .map(|focus| focus.id);
        let topmost = self.state.topmost.overdue(now_ms);
        for id in [focus, visibility_action, topmost].into_iter().flatten() {
            self.complete_window_action(id, Err(FailureKind::Failed), now_ms, effects);
        }
    }

    /// Answers the outstanding focus and topmost requests, as a host's same-dispatch receipts do.
    #[cfg(test)]
    pub(super) fn answer_pending_receipts(&mut self) {
        let focus = self.state.focus_pending.map(|focus| focus.id);
        let topmost = self.state.topmost.in_flight_id();
        for id in [focus, topmost].into_iter().flatten() {
            self.dispatch(super::state::WindowIntent::ActionCompleted {
                id,
                outcome: Ok(()),
                now_ms: 0,
            });
        }
    }

    /// Input and the IME follow what the window is now, rather than what it was when the quit
    /// began: a quit may end while it is hidden, or just after a show landed during it.
    fn reproject_input(&self, effects: &mut Vec<WindowEffect>) {
        let takes_input = matches!(
            self.state.visibility,
            VisibilityState::Presented(_) | VisibilityState::Animating(_)
        );
        effects.push(WindowEffect::SetImeAllowed(takes_input));
        effects.push(WindowEffect::SetEditorInputEnabled(takes_input));
    }

    pub(super) fn request_quit(&mut self, effects: &mut Vec<WindowEffect>) {
        if self.state.lifecycle.is_quitting() {
            return;
        }
        if self.state.guards.blocks_quit() {
            effects.push(WindowEffect::QuitBlocked);
            return;
        }
        self.state.deadlines.clear();
        effects.push(WindowEffect::SetEditorInputEnabled(false));
        self.advance_quit(effects);
    }

    fn advance_quit(&mut self, effects: &mut Vec<WindowEffect>) {
        if self.state.guards.blocks_quit() {
            self.state.lifecycle = LifecycleState::Running;
            self.reproject_input(effects);
            effects.push(WindowEffect::QuitBlocked);
        } else if self.state.guards.paste_pending || self.state.guards.asset_transaction_pending {
            self.state.lifecycle = LifecycleState::Quitting(QuitStage::AwaitingPaste);
        } else if self.state.guards.requires_note_barrier() {
            self.state.lifecycle = LifecycleState::Quitting(QuitStage::AwaitingNoteSave);
            if !self.state.guards.note_save_in_flight {
                effects.push(WindowEffect::RequestNoteSave(WindowSaveReason::Shutdown));
            }
        } else {
            self.state.lifecycle = LifecycleState::Quitting(QuitStage::AwaitingAssetGc);
            effects.push(WindowEffect::RequestSafeAssetGc);
        }
    }

    pub(super) fn paste_settled(&mut self, now_ms: u64, effects: &mut Vec<WindowEffect>) {
        if matches!(
            self.state.visibility,
            VisibilityState::AwaitingHideSave { .. }
        ) && !self.state.guards.paste_pending
            && !self.state.guards.asset_transaction_pending
        {
            self.state.hide_save_pending = false;
            if self.state.guards.note_save_required {
                effects.push(WindowEffect::RequestNoteSave(WindowSaveReason::HideToTray));
            } else if let VisibilityState::AwaitingHideSave { restore } = self.state.visibility {
                self.complete_hide(restore, now_ms, effects);
            }
        }
        if self.state.lifecycle == LifecycleState::Quitting(QuitStage::AwaitingPaste) {
            self.advance_quit(effects);
        }
    }

    pub(super) fn complete_hide_save(
        &mut self,
        succeeded: bool,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        let VisibilityState::AwaitingHideSave { restore } = self.state.visibility else {
            return;
        };
        if !succeeded {
            self.state.visibility = VisibilityState::Presented(restore);
            self.state.hide_save_pending = false;
            effects.push(WindowEffect::SetImeAllowed(
                self.state.guards.window_focused,
            ));
            effects.push(WindowEffect::SetEditorInputEnabled(true));
            effects.push(WindowEffect::HideCancelled);
            effects.push(WindowEffect::RequestRedraw);
            self.arm_auto_collapse(now_ms, effects);
            return;
        }
        if self.state.guards.paste_pending || self.state.guards.asset_transaction_pending {
            self.state.hide_save_pending = true;
        } else if self.state.guards.note_save_required {
            effects.push(WindowEffect::RequestNoteSave(WindowSaveReason::HideToTray));
        } else {
            self.complete_hide(restore, now_ms, effects);
        }
    }

    fn complete_hide(
        &mut self,
        restore: StableVisibility,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        let sensor_topmost = self.state.topmost.sensor_confirmed;
        let Some(id) = self.next_window_action() else {
            self.state.hide_save_pending = false;
            let failed = Err(FailureKind::Failed);
            self.complete_hide_action(failed, restore, sensor_topmost, now_ms, effects);
            return;
        };
        self.state.visibility = VisibilityState::HidePending {
            id,
            restore,
            sensor_topmost,
            deadline_ms: now_ms.saturating_add(WINDOW_ACTION_RECEIPT_TIMEOUT_MS),
        };
        self.state.hide_save_pending = false;
        self.set_temporary_sensor_topmost(false, now_ms, effects);
        effects.push(WindowEffect::HideWindow { id });
    }

    /// Applies the receipt of action `id` to the action it belongs to. A receipt no action still
    /// waits for is stale and commits nothing.
    pub(super) fn complete_window_action(
        &mut self,
        id: u64,
        outcome: Result<(), FailureKind>,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        if self.complete_topmost(id, outcome, now_ms, effects)
            || self.complete_placement(id, outcome, effects)
        {
            return;
        }
        if self.state.focus_pending.is_some_and(|focus| focus.id == id) {
            self.state.focus_pending = None;
            if let Err(kind) = outcome {
                effects.push(WindowEffect::ShowRejected(kind));
            }
            return;
        }
        match self.state.visibility {
            VisibilityState::HidePending {
                id: pending,
                restore,
                sensor_topmost,
                ..
            } if pending == id => {
                self.complete_hide_action(outcome, restore, sensor_topmost, now_ms, effects);
            }
            VisibilityState::ShowPending {
                id: pending,
                restore,
                ..
            } if pending == id => match outcome {
                Ok(()) => self.commit_show(restore, now_ms, effects),
                Err(kind) => {
                    self.state.visibility = VisibilityState::HiddenToTray { restore };
                    effects.push(WindowEffect::ShowRejected(kind));
                }
            },
            _ => {}
        }
    }

    /// A hide commits only on its own success receipt; a rejected one leaves the window shown,
    /// with its input back and every hidden-state cache still in place.
    fn complete_hide_action(
        &mut self,
        outcome: Result<(), FailureKind>,
        restore: StableVisibility,
        sensor_topmost: bool,
        now_ms: u64,
        effects: &mut Vec<WindowEffect>,
    ) {
        match outcome {
            Ok(()) => {
                self.state.visibility = VisibilityState::HiddenToTray { restore };
                effects.push(WindowEffect::ReleaseHiddenCaches);
            }
            Err(kind) => {
                self.state.visibility = VisibilityState::Presented(restore);
                self.set_temporary_sensor_topmost(sensor_topmost, now_ms, effects);
                effects.push(WindowEffect::SetImeAllowed(
                    self.state.guards.window_focused,
                ));
                effects.push(WindowEffect::SetEditorInputEnabled(true));
                effects.push(WindowEffect::HideRejected(kind));
                effects.push(WindowEffect::RequestRedraw);
                self.arm_auto_collapse(now_ms, effects);
            }
        }
    }

    /// The editor goes back to the user, and a barrier that completes later finds nothing waiting
    /// for it. Once ready to exit there is nothing left to take back.
    pub(super) fn abandon_quit(&mut self, effects: &mut Vec<WindowEffect>) {
        let LifecycleState::Quitting(stage) = self.state.lifecycle else {
            return;
        };
        let barrier = match stage {
            QuitStage::AwaitingPaste => QuitBarrier::Paste,
            QuitStage::AwaitingNoteSave => QuitBarrier::NoteSave,
            QuitStage::AwaitingAssetGc => QuitBarrier::AssetGc,
            QuitStage::AwaitingConfig => QuitBarrier::Config,
            QuitStage::ReadyToExit => return,
        };
        self.state.lifecycle = LifecycleState::Running;
        self.reproject_input(effects);
        effects.push(WindowEffect::QuitCancelled(barrier));
    }

    pub(super) fn complete_quit_barrier(
        &mut self,
        barrier: QuitBarrier,
        succeeded: bool,
        effects: &mut Vec<WindowEffect>,
    ) {
        let expected = match self.state.lifecycle {
            LifecycleState::Quitting(QuitStage::AwaitingPaste) => QuitBarrier::Paste,
            LifecycleState::Quitting(QuitStage::AwaitingNoteSave) => QuitBarrier::NoteSave,
            LifecycleState::Quitting(QuitStage::AwaitingAssetGc) => QuitBarrier::AssetGc,
            LifecycleState::Quitting(QuitStage::AwaitingConfig) => QuitBarrier::Config,
            LifecycleState::Running | LifecycleState::Quitting(QuitStage::ReadyToExit) => return,
        };
        if barrier != expected {
            return;
        }
        match barrier {
            QuitBarrier::Paste | QuitBarrier::NoteSave => {
                if succeeded {
                    self.advance_quit(effects);
                } else {
                    self.state.lifecycle = LifecycleState::Running;
                    self.reproject_input(effects);
                    effects.push(WindowEffect::QuitCancelled(barrier));
                }
            }
            QuitBarrier::AssetGc => {
                if !succeeded {
                    effects.push(WindowEffect::QuitWarning(QuitBarrier::AssetGc));
                }
                if self.state.guards.paste_pending
                    || self.state.guards.asset_transaction_pending
                    || self.state.guards.requires_note_barrier()
                {
                    self.advance_quit(effects);
                } else {
                    self.state.lifecycle = LifecycleState::Quitting(QuitStage::AwaitingConfig);
                    effects.push(WindowEffect::RequestConfigFlush);
                }
            }
            QuitBarrier::Config => {
                if !succeeded {
                    effects.push(WindowEffect::QuitWarning(QuitBarrier::Config));
                }
                self.state.lifecycle = LifecycleState::Quitting(QuitStage::ReadyToExit);
                effects.push(WindowEffect::ExitProcess);
            }
        }
    }
}
