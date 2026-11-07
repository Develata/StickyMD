//! Persistence intents and worker receipts of the native shell.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use winit::event_loop::ActiveEventLoop;

use super::StickyApp;
use crate::config::ConfigAck;
use stickymd_app::execution::IoCompletion;
use stickymd_app::flow::SaveTrigger;
use stickymd_app::flow::window::state::{QuitBarrier, WindowIntent};
use stickymd_app::instruction::{PersistenceIntent, SaveReason};
use stickymd_app::ports::NoteReceipt;

impl StickyApp {
    pub(super) fn submit_config_if_needed(&mut self) {
        if !self.config_persistence_allowed {
            return;
        }
        let Some(request) = self.config.begin_persist() else {
            return;
        };
        self.worker.submit_config(request);
    }

    /// The end of a complete main-thread event pass: after this pass's autosave, external check,
    /// preview and window ticks have scheduled their work. The quit-cancelled notice lasts only
    /// while the quit barrier still needs the note (plan 09); the title then shows the settled
    /// state.
    pub(super) fn finish_event_pass(&mut self) {
        let note_barrier_required = self.window_guards().requires_note_barrier();
        self.quit_notice.settle(note_barrier_required);
        self.update_window_title();
    }

    /// The only writer of the window title (plan 09 title priority).
    pub(super) fn update_window_title(&mut self) {
        let Some(window) = &self.window else { return };
        let title = super::title::window_title(super::title::TitleFacts {
            recovery_pending: self.recovery.is_pending(),
            quit_cancelled: self.quit_notice.active(),
            conflict: self.persistence.conflict().is_some(),
            dirty: self.coordinator.view().dirty,
            formula_error: self.formula_error_under_pointer(),
        });
        if title != self.shown_title {
            window.set_title(&title);
            self.shown_title = title.into_owned();
        }
    }

    pub(super) fn dispatch_persistence_intent(
        &mut self,
        event_loop: Option<&ActiveEventLoop>,
        intent: PersistenceIntent,
    ) -> bool {
        match intent {
            PersistenceIntent::SaveNow(reason) => {
                let trigger = match reason {
                    SaveReason::Manual => SaveTrigger::Manual,
                    SaveReason::FocusLoss => SaveTrigger::FocusLoss,
                };
                self.request_immediate_save(event_loop, trigger);
                true
            }
            PersistenceIntent::Export => {
                self.request_export();
                true
            }
            PersistenceIntent::ResolvePrimary => self.handle_resolution_key(event_loop, true),
            PersistenceIntent::ResolveSecondary => self.handle_resolution_key(event_loop, false),
        }
    }

    pub(super) fn handle_io_completion(
        &mut self,
        event_loop: Option<&ActiveEventLoop>,
        completion: IoCompletion,
    ) {
        match completion {
            IoCompletion::Note(receipt) => {
                // Protocol, not a decision: it takes the queued note job and releases the barrier,
                // so it must come before a follow-up save is submitted, or that save is dropped.
                if matches!(receipt, NoteReceipt::Save { .. }) {
                    self.worker.acknowledge_note_completion();
                }
                let now_ms = self.timestamp_ms();
                let effects = self.note_io().on_receipt(receipt, now_ms);
                self.run_note(event_loop, effects);
            }
            IoCompletion::Config { revision, result } => {
                let succeeded = result.is_ok();
                let acknowledgement = self.config.acknowledge(revision, succeeded);
                if let Err(error) = result {
                    self.diagnostic = Some(format!("配置保存失败；笔记保存不受影响：{error}"));
                    self.request_redraw();
                } else if matches!(
                    acknowledgement,
                    ConfigAck::Applied {
                        needs_follow_up: true
                    }
                ) {
                    self.submit_config_if_needed();
                }
                if self.config.settles_quit(acknowledgement, succeeded) {
                    self.dispatch_window_intent(
                        event_loop,
                        WindowIntent::QuitBarrierCompleted {
                            barrier: QuitBarrier::Config,
                            succeeded,
                            guards: self.window_guards(),
                        },
                    );
                }
            }
            IoCompletion::AssetPaste(result) => {
                self.handle_asset_paste_completion(event_loop, result)
            }
            IoCompletion::AssetSync {
                request_id, result, ..
            } => self.handle_asset_sync_completion(event_loop, request_id, result),
            IoCompletion::Export { result, .. } => self.handle_export_completion(result),
            IoCompletion::WorkerStopped => {
                self.diagnostic = Some("持久化工作线程已停止；不会假装自动保存仍可用。".into());
                self.assets.worker_stopped();
                self.dispatch_window_intent(
                    event_loop,
                    WindowIntent::HideSaveCompleted {
                        succeeded: false,
                        guards: self.window_guards(),
                    },
                );
                for barrier in [
                    QuitBarrier::Paste,
                    QuitBarrier::NoteSave,
                    QuitBarrier::AssetGc,
                    QuitBarrier::Config,
                ] {
                    self.dispatch_window_intent(
                        event_loop,
                        WindowIntent::QuitBarrierCompleted {
                            barrier,
                            succeeded: false,
                            guards: self.window_guards(),
                        },
                    );
                }
                self.request_redraw();
            }
        }
    }
}
