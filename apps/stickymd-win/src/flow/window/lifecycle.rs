//! Hide-to-tray and ordered quit barriers for the native window lifecycle.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use super::reducer::WindowShellCoordinator;
use super::state::{
    LifecycleState, QuitBarrier, QuitStage, StableVisibility, VisibilityState, WindowEffect,
    WindowSaveReason,
};

impl WindowShellCoordinator {
    pub(super) fn hide_to_tray(&mut self, effects: &mut Vec<WindowEffect>) {
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
            VisibilityState::AwaitingHideSave { .. } | VisibilityState::HiddenToTray { .. }
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
            self.complete_hide(restore, effects);
        }
        if self.state.guards.note_save_required && !self.state.hide_save_pending {
            effects.push(WindowEffect::RequestNoteSave(WindowSaveReason::HideToTray));
        }
    }

    pub(super) fn show(&mut self, effects: &mut Vec<WindowEffect>) {
        if self.state.lifecycle.is_quitting() {
            return;
        }
        let restore = match self.state.visibility {
            VisibilityState::HiddenToTray { restore }
            | VisibilityState::AwaitingHideSave { restore } => restore,
            _ => {
                effects.push(WindowEffect::FocusWindow);
                return;
            }
        };
        let restore = match restore {
            StableVisibility::DockedCollapsed(edge) => StableVisibility::DockedExpanded(edge),
            other => other,
        };
        self.state.visibility = VisibilityState::Presented(restore);
        self.state.dock.edge = restore.dock_edge();
        self.state.dock.manually_hidden = false;
        self.state.dock.hover_revealed = false;
        self.set_temporary_sensor_topmost(false, effects);
        self.state.deadlines.clear();
        self.state.frame = super::reducer::effective_expanded_frame(
            &self.state.placement,
            restore.dock_edge(),
            &self.state.dock.monitor,
            self.state.pre_split_width_dip.is_some(),
        );
        effects.push(WindowEffect::ApplyFrame(self.state.frame));
        effects.push(WindowEffect::SetVisible(true));
        effects.push(WindowEffect::FocusWindow);
        effects.push(WindowEffect::SetImeAllowed(true));
        effects.push(WindowEffect::SetEditorInputEnabled(true));
        effects.push(WindowEffect::RequestRedraw);
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

    pub(super) fn paste_settled(&mut self, effects: &mut Vec<WindowEffect>) {
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
                self.complete_hide(restore, effects);
            }
        }
        if self.state.lifecycle == LifecycleState::Quitting(QuitStage::AwaitingPaste) {
            self.advance_quit(effects);
        }
    }

    pub(super) fn complete_hide_save(&mut self, succeeded: bool, effects: &mut Vec<WindowEffect>) {
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
            return;
        }
        if self.state.guards.paste_pending || self.state.guards.asset_transaction_pending {
            self.state.hide_save_pending = true;
        } else if self.state.guards.note_save_required {
            effects.push(WindowEffect::RequestNoteSave(WindowSaveReason::HideToTray));
        } else {
            self.complete_hide(restore, effects);
        }
    }

    fn complete_hide(&mut self, restore: StableVisibility, effects: &mut Vec<WindowEffect>) {
        self.state.visibility = VisibilityState::HiddenToTray { restore };
        self.state.hide_save_pending = false;
        self.set_temporary_sensor_topmost(false, effects);
        effects.push(WindowEffect::SetVisible(false));
        effects.push(WindowEffect::ReleaseHiddenCaches);
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
                    effects.push(WindowEffect::SetEditorInputEnabled(true));
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
