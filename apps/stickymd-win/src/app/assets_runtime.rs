//! Managed-image paste and lifecycle result coordination.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#state-changes

use stickymd_core::{AssetEffect, Generation};
use winit::event_loop::ActiveEventLoop;

use super::StickyApp;
use crate::assets::{AssetPasteCompletion, AssetPasteRequest, AssetReconcileMode};
use crate::flow::{ExitGcAction, PendingAssetPaste, SaveTrigger};

impl StickyApp {
    pub(super) fn submit_asset_paste(&mut self, pending: PendingAssetPaste) {
        if self.asset_paste_pending {
            self.diagnostic = Some("上一批图片仍在处理中；本次粘贴未开始。".into());
            self.request_redraw();
            return;
        }
        let request = AssetPasteRequest {
            expected_generation: pending.expected_generation,
            selection: pending.selection,
            timestamp_ms: pending.timestamp_ms,
            payload: pending.payload,
        };
        if self.worker.submit_asset_paste(
            self.paths.images_dir.clone(),
            self.paths.trash_dir.clone(),
            request,
        ) {
            self.asset_paste_pending = true;
            self.diagnostic = Some("正在准备图片…".into());
        } else {
            self.diagnostic = Some("图片工作队列暂不可用；未修改文档。".into());
        }
        self.request_redraw();
    }

    pub(super) fn handle_asset_paste_completion(
        &mut self,
        event_loop: &ActiveEventLoop,
        result: Result<AssetPasteCompletion, crate::assets::AssetPasteFailure>,
    ) {
        self.asset_paste_pending = false;
        let completion = match result {
            Ok(completion) => completion,
            Err(failure) => {
                let effects = failure
                    .rollback
                    .convergence_effects(&self.coordinator.managed_ref_counts());
                if !effects.is_empty() {
                    self.submit_asset_sync(
                        self.coordinator.view().generation,
                        effects,
                        Some(AssetReconcileMode::Runtime),
                        false,
                    );
                }
                self.diagnostic = Some(format!(
                    "图片粘贴失败；未插入引用，资产正按最新引用状态收敛：{}",
                    failure.error
                ));
                self.request_redraw();
                self.resume_pending_quit(event_loop);
                return;
            }
        };
        let AssetPasteCompletion {
            expected_generation,
            selection,
            timestamp_ms,
            markdown,
            rollback,
        } = completion;
        if self.coordinator.view().generation != expected_generation {
            let effects = rollback.convergence_effects(&self.coordinator.managed_ref_counts());
            self.submit_asset_sync(
                self.coordinator.view().generation,
                effects,
                Some(AssetReconcileMode::Runtime),
                false,
            );
            self.diagnostic =
                Some("图片准备期间文档已变化；未插入引用，资产正按最新引用状态收敛。".into());
            self.request_redraw();
            self.resume_pending_quit(event_loop);
            return;
        }
        match self.coordinator.commit_prepared_paste(
            expected_generation,
            selection,
            markdown,
            timestamp_ms,
        ) {
            Ok(effect) => self.apply_effect(effect),
            Err(error) => {
                let effects = rollback.convergence_effects(&self.coordinator.managed_ref_counts());
                self.submit_asset_sync(
                    self.coordinator.view().generation,
                    effects,
                    Some(AssetReconcileMode::Runtime),
                    false,
                );
                self.diagnostic = Some(format!(
                    "图片已准备但文档事务未提交；资产正按最新引用状态收敛：{error}"
                ));
                self.request_redraw();
            }
        }
        self.resume_pending_quit(event_loop);
    }

    pub(super) fn submit_asset_sync(
        &mut self,
        generation: Generation,
        effects: Vec<AssetEffect>,
        reconcile: Option<AssetReconcileMode>,
        for_exit: bool,
    ) {
        let Some(request_id) = self.asset_sync_sequence.checked_add(1) else {
            self.diagnostic = Some("图片事务序号已耗尽；已停止自动整理。".into());
            self.request_redraw();
            return;
        };
        self.asset_sync_sequence = request_id;
        self.asset_sync_in_flight = true;
        self.asset_sync_request_id = Some(request_id);
        self.exit_gc_pending |= for_exit;
        self.worker
            .submit_asset_sync(crate::persistence::AssetSyncRequest {
                images: self.paths.images_dir.clone(),
                trash: self.paths.trash_dir.clone(),
                request_id,
                generation,
                effects,
                references: self.coordinator.managed_ref_counts(),
                reconcile,
                safe_note: (reconcile == Some(AssetReconcileMode::SafeBoundary)).then(|| {
                    (
                        self.paths.note_file.clone(),
                        self.coordinator.view().base_disk_hash,
                    )
                }),
            });
    }

    pub(super) fn sync_assets(&mut self, for_exit: bool) {
        self.submit_asset_sync(
            self.coordinator.view().generation,
            Vec::new(),
            Some(if for_exit {
                AssetReconcileMode::SafeBoundary
            } else {
                AssetReconcileMode::Runtime
            }),
            for_exit,
        );
    }

    pub(super) fn sync_assets_after_recovery(&mut self) {
        self.asset_reconcile_pending = true;
        self.submit_asset_sync(
            self.coordinator.view().generation,
            Vec::new(),
            Some(AssetReconcileMode::SafeBoundary),
            false,
        );
    }

    pub(super) fn handle_asset_sync_completion(
        &mut self,
        event_loop: &ActiveEventLoop,
        request_id: u64,
        _generation: Generation,
        result: Result<crate::assets::AssetReconcileReport, crate::assets::AssetStorageError>,
    ) {
        if self.asset_sync_request_id == Some(request_id) {
            self.asset_sync_in_flight = false;
            self.asset_sync_request_id = None;
        }
        let succeeded = result.is_ok();
        match result {
            Ok(report) => {
                if report.physical_delete_deferred {
                    self.diagnostic =
                        Some("便签磁盘状态正在变化；已保留回收站证据并延后物理清理。".into());
                }
                if !report.missing_references.is_empty() {
                    self.diagnostic = Some(format!(
                        "有 {} 个受管图片引用缺少文件；Markdown 未修改。",
                        report.missing_references.len()
                    ));
                }
            }
            Err(error) => {
                self.diagnostic = Some(format!("图片整理未完成；用户文件与文档均未删除：{error}"));
            }
        }
        if self.asset_reconcile_pending && !self.asset_sync_in_flight {
            self.asset_reconcile_pending = false;
            self.start_watcher();
            if self.coordinator.view().base_disk_hash.is_none() {
                self.submit_save(
                    SaveTrigger::RecreateMissing,
                    Some(crate::persistence::PersistMode::Guarded { expected: None }),
                );
            }
        }
        if self.exit_gc_pending && !self.asset_sync_in_flight {
            self.exit_gc_pending = false;
            match self.persistence.decide_exit_gc_completion(
                succeeded,
                self.asset_paste_pending,
                self.coordinator.view().dirty,
            ) {
                ExitGcAction::CancelQuit => self.quit_pending = false,
                ExitGcAction::ResumeQuit => self.resume_pending_quit(event_loop),
                ExitGcAction::Exit => {
                    event_loop.exit();
                    return;
                }
            }
        }
        self.request_redraw();
    }

    fn resume_pending_quit(&mut self, event_loop: &ActiveEventLoop) {
        if self.quit_pending && !self.asset_paste_pending {
            self.request_quit(event_loop);
        }
    }
}
