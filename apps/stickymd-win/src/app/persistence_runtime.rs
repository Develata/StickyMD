//! Persistence result submission and reconciliation for the native shell.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use winit::event_loop::ActiveEventLoop;

use super::StickyApp;
use crate::flow::{QuitAction, RecoveryOperation, SaveTrigger};
use crate::instruction::{PersistenceIntent, SaveReason};
use crate::persistence::{
    IoCompletion, PersistMode, PersistRequest, PersistResult, TemporaryCleanup,
};

impl StickyApp {
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
        event_loop: Option<&ActiveEventLoop>,
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
            PersistenceIntent::RequestQuit => {
                let Some(event_loop) = event_loop else {
                    return false;
                };
                self.request_quit(event_loop);
                true
            }
        }
    }

    pub(super) fn request_quit(&mut self, _event_loop: &ActiveEventLoop) {
        if self.asset_paste_pending {
            self.quit_pending = true;
            self.diagnostic = Some("图片粘贴事务正在完成；完成并保存后再退出。".into());
            self.request_redraw();
            return;
        }
        if self.config_persistence_allowed {
            self.worker.submit_config(
                self.paths.config_file.clone(),
                self.paths.config_tmp.clone(),
                self.config.clone(),
            );
        }
        match self
            .persistence
            .decide_quit(self.recovery.is_pending(), self.coordinator.view().dirty)
        {
            QuitAction::BlockedByRecovery => {
                self.quit_pending = false;
                self.diagnostic =
                    Some("退出前必须完成恢复选择  |  [F6 恢复临时内容]  [F7 使用当前文件]".into());
                self.request_redraw();
            }
            QuitAction::BlockedByConflict => {
                self.quit_pending = false;
                self.diagnostic =
                    Some("退出前必须解决外部修改冲突  |  [F6 载入外部]  [F7 保留本地]".into());
                self.request_redraw();
            }
            QuitAction::WaitForInFlightSave => self.quit_pending = true,
            QuitAction::RecreateMissing => {
                self.quit_pending = true;
                self.submit_save(
                    SaveTrigger::RecreateMissing,
                    Some(PersistMode::Guarded { expected: None }),
                );
            }
            QuitAction::SaveDirty => {
                self.quit_pending = true;
                self.request_immediate_save(SaveTrigger::Shutdown);
            }
            QuitAction::Exit => {
                self.quit_pending = true;
                self.sync_assets(true);
            }
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
                match completion.result {
                    Ok(PersistResult::Saved {
                        generation,
                        fingerprint,
                        ..
                    }) if generation == completion_generation => {
                        self.handle_note_saved(event_loop, generation, fingerprint)
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
                        self.request_redraw();
                    }
                    Ok(PersistResult::Conflict { observed, .. }) => {
                        self.persistence.note_save_finished(false);
                        self.quit_pending = false;
                        if self.recovery.is_pending() {
                            self.handle_recovery_save_conflict(observed);
                        } else {
                            self.reconcile_external(observed);
                        }
                    }
                    Err(error) => {
                        self.persistence.note_save_finished(false);
                        self.quit_pending = false;
                        self.recovery.fail_operation();
                        self.resolving_keep_local = false;
                        self.diagnostic = Some(format!(
                            "保存 generation {} 失败；内存文本仍保留。Ctrl+S 重试：{error}",
                            completion_generation.value()
                        ));
                        self.update_window_title();
                        self.request_redraw();
                    }
                }
            }
            IoCompletion::External(Ok(external)) => {
                self.reconcile_external(external);
                if self.quit_pending
                    && !self.persistence.has_required_write()
                    && self.persistence.conflict().is_none()
                    && !self.coordinator.view().dirty
                {
                    self.sync_assets(true);
                }
            }
            IoCompletion::External(Err(error)) => {
                self.diagnostic = Some(format!("外部文件读取失败，未改变当前文本：{error}"));
                self.request_redraw();
            }
            IoCompletion::Config(Err(error)) => {
                self.diagnostic = Some(format!("配置保存失败；笔记保存不受影响：{error}"));
                self.request_redraw();
            }
            IoCompletion::Config(Ok(())) => {}
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
                self.request_redraw();
            }
        }
    }

    fn handle_note_saved(
        &mut self,
        _event_loop: &ActiveEventLoop,
        generation: stickymd_core::Generation,
        fingerprint: stickymd_core::Hash32,
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
        if self.recovery.operation() == Some(RecoveryOperation::Restoring) {
            self.worker.remove_temporary(
                self.paths.note_tmp.clone(),
                TemporaryCleanup::RecoveryResolved,
            );
        } else if self.resolving_keep_local {
            self.resolving_keep_local = false;
            self.persistence.clear_conflict();
            self.diagnostic = Some("已保留本地内容并覆盖外部文件。".into());
        } else {
            self.diagnostic = Some("已保存".into());
        }
        if self.coordinator.view().dirty {
            self.submit_save(SaveTrigger::Debounce, None);
        } else if self.persistence.durability_required() && !completed_required {
            // A recreate hint overlapped a different save. Inspect the durable
            // path before deciding whether a second write is still needed.
            self.worker.inspect_external(self.paths.note_file.clone());
        } else if self.quit_pending && !self.persistence.has_required_write() {
            self.sync_assets(true);
        }
        self.update_window_title();
        self.request_redraw();
    }
}
