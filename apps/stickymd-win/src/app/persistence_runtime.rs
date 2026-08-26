//! Persistence result submission and reconciliation for the native shell.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use winit::event_loop::ActiveEventLoop;

use super::StickyApp;
use crate::config::ConfigAck;
use crate::flow::window::state::{QuitBarrier, WindowIntent};
use crate::flow::{RecoveryOperation, SaveTrigger};
use crate::instruction::{PersistenceIntent, SaveReason};
use crate::persistence::{
    IoCompletion, PersistMode, PersistRequest, PersistResult, TemporaryCleanup,
};

impl StickyApp {
    pub(super) fn submit_config_if_needed(&mut self) {
        if !self.config_persistence_allowed {
            return;
        }
        let Some(request) = self.config.begin_persist() else {
            return;
        };
        self.worker.submit_config(
            self.paths.config_file.clone(),
            self.paths.config_tmp.clone(),
            request,
        );
    }

    pub(super) fn update_window_title(&self) {
        let Some(window) = &self.window else { return };
        let view = self.coordinator.view();
        let title = if self.recovery.is_pending() {
            "StickyMD — 恢复选择未完成"
        } else if self.persistence.conflict().is_some() {
            "StickyMD * — 外部修改冲突"
        } else if view.dirty {
            "StickyMD *"
        } else {
            "StickyMD"
        };
        window.set_title(title);
    }

    pub(super) fn submit_save(&mut self, trigger: SaveTrigger, override_mode: Option<PersistMode>) {
        if self.recovery.is_pending() && trigger != SaveTrigger::KeepLocal {
            return;
        }
        let snapshot = self.coordinator.snapshot();
        let mode = override_mode.unwrap_or(PersistMode::Guarded {
            expected: self.coordinator.view().base_disk_hash,
        });
        #[cfg(debug_assertions)]
        eprintln!(
            "save requested generation={} trigger={trigger:?}",
            snapshot.generation.value()
        );
        self.persistence.note_save_submitted(trigger);
        self.worker.submit_note(
            self.paths.note_file.clone(),
            self.paths.note_tmp.clone(),
            PersistRequest {
                generation: snapshot.generation,
                text: snapshot.text,
                line_ending: snapshot.line_ending,
                mode,
            },
        );
        self.diagnostic = Some(format!(
            "正在保存 generation {}…",
            snapshot.generation.value()
        ));
        self.request_redraw();
    }

    pub(super) fn request_immediate_save(&mut self, trigger: SaveTrigger) {
        if let Some(action) = self
            .persistence
            .request_save(self.coordinator.view().generation, trigger)
        {
            self.submit_save(action.trigger, None);
        }
    }

    pub(super) fn dispatch_persistence_intent(
        &mut self,
        _event_loop: Option<&ActiveEventLoop>,
        intent: PersistenceIntent,
    ) -> bool {
        match intent {
            PersistenceIntent::SaveNow(reason) => {
                let trigger = match reason {
                    SaveReason::Manual => SaveTrigger::Manual,
                    SaveReason::FocusLoss => SaveTrigger::FocusLoss,
                };
                self.request_immediate_save(trigger);
                true
            }
            PersistenceIntent::Export => {
                self.request_export();
                true
            }
            PersistenceIntent::ResolvePrimary => self.handle_resolution_key(true),
            PersistenceIntent::ResolveSecondary => self.handle_resolution_key(false),
        }
    }

    pub(super) fn handle_io_completion(
        &mut self,
        event_loop: &ActiveEventLoop,
        completion: IoCompletion,
    ) {
        match completion {
            IoCompletion::Note(completion) => {
                self.worker.acknowledge_note_completion();
                let completion_generation = completion.generation;
                let completion_mode = completion.mode;
                match completion.result {
                    Ok(PersistResult::Saved {
                        generation,
                        fingerprint,
                        ..
                    }) if generation == completion_generation => {
                        self.handle_note_saved(event_loop, generation, fingerprint, completion_mode)
                    }
                    Ok(PersistResult::Saved { generation, .. }) => {
                        self.persistence.note_save_finished(false);
                        self.recovery.fail_operation();
                        self.resolving_keep_local = false;
                        self.diagnostic = Some(format!(
                            "保存回执 generation 不一致：submitted={} result={}",
                            completion_generation.value(),
                            generation.value()
                        ));
                        self.complete_window_note_barriers(event_loop, false);
                        self.request_redraw();
                    }
                    Ok(PersistResult::Conflict { observed, .. }) => {
                        self.persistence.note_save_finished(false);
                        if self.recovery.is_pending() {
                            self.handle_recovery_save_conflict(observed);
                        } else {
                            self.reconcile_external(observed);
                        }
                        if !self.resubmit_keep_local_after_superseded_save(completion_mode) {
                            self.complete_window_note_barriers(event_loop, false);
                        }
                    }
                    Err(error) => {
                        self.persistence.note_save_finished(false);
                        self.recovery.fail_operation();
                        let resubmitted =
                            self.resubmit_keep_local_after_superseded_save(completion_mode);
                        if !resubmitted {
                            self.resolving_keep_local = false;
                            self.diagnostic = Some(format!(
                                "保存 generation {} 失败；内存文本仍保留。Ctrl+S 重试：{error}",
                                completion_generation.value()
                            ));
                        }
                        self.update_window_title();
                        if !resubmitted {
                            self.complete_window_note_barriers(event_loop, false);
                        }
                        self.request_redraw();
                    }
                }
            }
            IoCompletion::External(Ok(external)) => {
                self.reconcile_external(external);
            }
            IoCompletion::External(Err(error)) => {
                self.diagnostic = Some(format!("外部文件读取失败，未改变当前文本：{error}"));
                self.request_redraw();
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
                if matches!(acknowledgement, ConfigAck::Applied { .. })
                    && !self.config.is_saving()
                    && (!self.config.is_dirty() || !succeeded)
                {
                    self.dispatch_window_intent(
                        Some(event_loop),
                        WindowIntent::QuitBarrierCompleted {
                            barrier: QuitBarrier::Config,
                            succeeded,
                            guards: self.window_guards(),
                        },
                    );
                }
            }
            IoCompletion::TemporaryRemoved { purpose, result } => match (purpose, result) {
                (TemporaryCleanup::RecoveryResolved, Ok(())) => self.finish_recovery_cleanup(),
                (TemporaryCleanup::ConflictDiscarded, Ok(())) => {}
                (TemporaryCleanup::RecoveryResolved, Err(error)) => {
                    self.recovery.fail_operation();
                    self.diagnostic =
                        Some(format!("临时恢复证据无法清理，仍保持恢复状态：{error}"));
                    self.request_redraw();
                }
                (TemporaryCleanup::ConflictDiscarded, Err(error)) => {
                    self.diagnostic = Some(format!(
                        "外部内容已载入，但本地临时副本无法清理；下次启动可能再次提示恢复：{error}"
                    ));
                    self.request_redraw();
                }
            },
            IoCompletion::CanonicalPreserved(Ok(())) => {
                let expected = self.recovery.take_restore_guard().unwrap_or(None);
                self.submit_save(
                    SaveTrigger::KeepLocal,
                    Some(PersistMode::Guarded { expected }),
                );
            }
            IoCompletion::CanonicalPreserved(Err(error)) => {
                self.diagnostic = Some(format!(
                    "无效的 canonical note.md 无法先行保留；未执行恢复覆盖：{error}"
                ));
                self.recovery.fail_operation();
                self.request_redraw();
            }
            IoCompletion::AssetPaste(result) => {
                self.handle_asset_paste_completion(event_loop, result)
            }
            IoCompletion::AssetSync {
                request_id,
                generation,
                result,
            } => self.handle_asset_sync_completion(event_loop, request_id, generation, result),
            IoCompletion::Export { generation, result } => {
                self.handle_export_completion(generation, result)
            }
            IoCompletion::WorkerStopped => {
                self.diagnostic = Some("持久化工作线程已停止；不会假装自动保存仍可用。".into());
                self.asset_paste_pending = false;
                self.asset_sync_in_flight = false;
                self.asset_sync_request_id = None;
                self.quit_asset_sync_request_id = None;
                self.dispatch_window_intent(
                    Some(event_loop),
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
                        Some(event_loop),
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

    fn handle_note_saved(
        &mut self,
        _event_loop: &ActiveEventLoop,
        generation: stickymd_core::Generation,
        fingerprint: stickymd_core::Hash32,
        mode: PersistMode,
    ) {
        let completed_required = self.persistence.note_save_finished(true);
        if let Err(error) = self
            .coordinator
            .acknowledge_persisted(generation, fingerprint)
        {
            self.diagnostic = Some(format!("保存回执无效：{error}"));
            return;
        }
        #[cfg(debug_assertions)]
        {
            let metrics = self.worker.metrics();
            eprintln!(
                "save succeeded generation={} hash={} submitted={} started={} completed={} coalesced={}",
                generation.value(),
                &fingerprint.to_hex()[..8],
                metrics.note_submitted,
                metrics.note_started,
                metrics.note_completed,
                metrics.note_coalesced
            );
        }
        if matches!(
            self.recovery.operation(),
            Some(RecoveryOperation::Restoring | RecoveryOperation::UsingCanonical)
        ) {
            self.worker.remove_temporary(
                self.paths.note_tmp.clone(),
                TemporaryCleanup::RecoveryResolved,
            );
        } else if self.resolving_keep_local && mode == PersistMode::ForceOverwrite {
            self.resolving_keep_local = false;
            self.persistence.clear_conflict();
            self.diagnostic = Some("已保留本地内容并覆盖外部文件。".into());
        } else {
            self.diagnostic = Some("已保存".into());
        }
        let keep_local_resubmitted = self.resubmit_keep_local_after_superseded_save(mode);
        if keep_local_resubmitted {
            // The explicit conflict resolution owns the one latest-pending
            // slot. A generic stale-generation follow-up must not replace it.
        } else if self.coordinator.view().dirty {
            self.submit_save(SaveTrigger::Debounce, None);
        } else if self.persistence.durability_required() && !completed_required {
            // A recreate hint overlapped a different save. Inspect the durable
            // path before deciding whether a second write is still needed.
            self.worker.inspect_external(self.paths.note_file.clone());
        }
        self.update_window_title();
        if !keep_local_resubmitted {
            self.complete_window_note_barriers(_event_loop, true);
        }
        self.request_redraw();
    }

    fn resubmit_keep_local_after_superseded_save(&mut self, completed_mode: PersistMode) -> bool {
        if keep_local_needs_resubmit(
            self.resolving_keep_local,
            self.persistence.conflict().is_some(),
            completed_mode,
        ) {
            self.diagnostic = Some("旧保存任务已完成；正在用最新内存文本执行“保留本地”…".into());
            self.submit_save(SaveTrigger::KeepLocal, Some(PersistMode::ForceOverwrite));
            true
        } else {
            false
        }
    }

    fn complete_window_note_barriers(&mut self, event_loop: &ActiveEventLoop, succeeded: bool) {
        let guards = self.window_guards();
        self.dispatch_window_intent(
            Some(event_loop),
            WindowIntent::HideSaveCompleted { succeeded, guards },
        );
        self.dispatch_window_intent(
            Some(event_loop),
            WindowIntent::QuitBarrierCompleted {
                barrier: QuitBarrier::NoteSave,
                succeeded,
                guards: self.window_guards(),
            },
        );
    }
}

fn keep_local_needs_resubmit(
    resolving_keep_local: bool,
    conflict_present: bool,
    completed_mode: PersistMode,
) -> bool {
    resolving_keep_local && conflict_present && completed_mode != PersistMode::ForceOverwrite
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keep_local_is_satisfied_only_by_a_force_overwrite_receipt() {
        assert!(keep_local_needs_resubmit(
            true,
            true,
            PersistMode::Guarded { expected: None }
        ));
        assert!(!keep_local_needs_resubmit(
            true,
            true,
            PersistMode::ForceOverwrite
        ));
        assert!(!keep_local_needs_resubmit(
            false,
            true,
            PersistMode::Guarded { expected: None }
        ));
        assert!(!keep_local_needs_resubmit(
            true,
            false,
            PersistMode::Guarded { expected: None }
        ));
    }
}
