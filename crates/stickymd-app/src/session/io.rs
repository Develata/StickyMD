//! What each worker receipt means for the session.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use crate::config::{ConfigAck, ConfigRevision};
use crate::execution::IoCompletion;
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
            out.push(ShellEffect::Diagnostic(format!(
                "配置保存失败；笔记保存不受影响：{error}"
            )));
            out.push(ShellEffect::RedrawNeeded);
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
        out.push(ShellEffect::Diagnostic(match result {
            Ok(completion) => format!(
                "导出完成：{}（{} 个本地资源）",
                completion.target.display(),
                completion.copied_assets
            ),
            Err(error) => format!("导出失败；工作文档未改变：{error}"),
        }));
        out.push(ShellEffect::RedrawNeeded);
    }

    /// Nothing the worker was doing will ever report back: every barrier waiting on it ends.
    fn on_worker_stopped(&mut self, view: ViewFacts, out: &mut ShellEffects) {
        out.push(ShellEffect::Diagnostic(
            "持久化工作线程已停止；不会假装自动保存仍可用。".into(),
        ));
        self.assets.worker_stopped();
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
