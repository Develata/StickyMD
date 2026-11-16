//! What each worker receipt means for the session.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use crate::config::{ConfigAck, ConfigRevision};
use crate::execution::IoCompletion;
use crate::flow::notice::Notice;
use crate::flow::window::state::{QuitBarrier, WindowIntent};
use crate::ports::{ExportCompletion, NoteReceipt, StorageFailure};

use super::turns::Turn;
use super::{AppSession, ShellEffect, ShellEffects, ViewFacts};

impl AppSession {
    pub fn handle_io(&mut self, completion: IoCompletion, view: ViewFacts, out: &mut ShellEffects) {
        match completion {
            IoCompletion::Note(receipt) => {
                // Protocol, not a decision: it takes the queued note job and releases the barrier,
                // so it must come before a follow-up save is submitted, or that save is dropped.
                if matches!(receipt, NoteReceipt::Save { .. }) {
                    self.worker.acknowledge_note_completion();
                }
                let now_ms = self.now_ms();
                let effects = self.note_io().on_receipt(receipt, now_ms);
                Self::queue_note(effects, out);
            }
            IoCompletion::Config { revision, result } => {
                self.on_config_receipt(revision, result, view, out);
            }
            IoCompletion::AssetPaste(result) => self.on_asset_paste(result, out),
            IoCompletion::AssetSync {
                request_id, result, ..
            } => self.on_asset_sync(request_id, result, out),
            IoCompletion::Export { result, .. } => self.on_export_receipt(result, out),
            IoCompletion::WorkerStopped => self.on_worker_stopped(view, out),
        }
    }

    pub fn submit_config_if_needed(&mut self) {
        if !self.config_persistence_allowed {
            return;
        }
        let Some(request) = self.config.begin_persist() else {
            return;
        };
        self.worker.submit_config(request);
    }

    fn on_config_receipt(
        &mut self,
        revision: ConfigRevision,
        result: Result<(), StorageFailure>,
        view: ViewFacts,
        out: &mut ShellEffects,
    ) {
        let succeeded = result.is_ok();
        let acknowledgement = self.config.acknowledge(revision, succeeded);
        if let Err(error) = result {
            self.show_notice(Notice::ConfigSaveFailed(error.to_string()), out);
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
                WindowIntent::QuitBarrierCompleted {
                    barrier: QuitBarrier::Config,
                    succeeded,
                    guards: self.guards(view),
                },
                out,
            );
        }
    }

    fn on_export_receipt(
        &mut self,
        result: Result<ExportCompletion, StorageFailure>,
        out: &mut ShellEffects,
    ) {
        self.export_in_flight = false;
        let notice = match result {
            Ok(completion) => Notice::Exported {
                target: completion.target.display().to_string(),
                assets: completion.copied_assets,
            },
            Err(error) => Notice::ExportFailed(error.to_string()),
        };
        self.show_notice(notice, out);
    }

    /// Nothing the worker was doing will ever report back: every barrier waiting on it ends, and
    /// so does every operation in progress on it (plan 04 #notice-state). The export is one of
    /// them too, or "exporting" would show forever.
    fn on_worker_stopped(&mut self, view: ViewFacts, out: &mut ShellEffects) {
        if self
            .persistence
            .worker_stopped("持久化工作线程已停止；不会假装自动保存仍可用。".into())
        {
            self.start_fault_display(out);
        }
        self.recovery.fail_operation();
        self.export_in_flight = false;
        if self.assets.worker_stopped() {
            out.push(ShellEffect::StartWatcher);
        }
        self.dispatch_window_intent(
            WindowIntent::HideSaveCompleted {
                succeeded: false,
                guards: self.guards(view),
                now_ms: self.now_ms(),
            },
            out,
        );
        for barrier in [
            QuitBarrier::Paste,
            QuitBarrier::NoteSave,
            QuitBarrier::AssetGc,
            QuitBarrier::Config,
        ] {
            out.turn(Turn::QuitBarrier {
                barrier,
                succeeded: false,
            });
        }
        out.push(ShellEffect::RedrawNeeded);
    }
}
