//! Managed-image paste and lifecycle decisions.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#state-changes

use stickymd_core::{AssetEffect, DocumentState, Generation};

use super::{AppEffect, ClipboardPort, EditorCoordinator};
use crate::ports::{
    AssetPasteCompletion, AssetPasteFailure, AssetReconcileMode, AssetReconcileReport,
    AssetSyncRequest, ManagedAssetStore, StorageFailure,
};

/// A normal startup is a quiescent destructive-GC boundary: no editor or worker exists yet, so the
/// reference snapshot cannot become stale while reconciliation deletes proven unreferenced trash.
/// A startup that offers recovery skips it; the recovery choice runs the same boundary before
/// input returns (`AssetIo::sync_after_recovery`). Returns the warning for the status line.
pub fn reconcile_at_startup(
    store: &mut impl ManagedAssetStore,
    document: &DocumentState,
    recovery_pending: bool,
) -> Option<String> {
    if recovery_pending {
        return None;
    }
    // Outside the worker there is no sync sequence; the store does not read the id.
    let request = AssetSyncRequest {
        request_id: 0,
        generation: document.generation(),
        effects: Vec::new(),
        references: document.managed_ref_counts().clone(),
        reconcile: Some(AssetReconcileMode::SafeBoundary),
        safe_expected: Some(document.base_disk_hash()),
    };
    match store.sync(&request) {
        Ok(report) if !report.missing_references.is_empty() => Some(format!(
            "有 {} 个受管图片引用缺少文件；Markdown 未修改。",
            report.missing_references.len()
        )),
        Ok(_) => None,
        Err(error) => Some(format!(
            "启动图片整理未完成；用户文件与文档均未删除：{error}"
        )),
    }
}

/// What is in flight on the asset side of the worker.
#[derive(Debug, Default)]
pub struct AssetFlow {
    pub paste_pending: bool,
    pub sync_in_flight: bool,
    pub sync_sequence: u64,
    pub sync_request_id: Option<u64>,
    pub quit_sync_request_id: Option<u64>,
    /// The sync after a recovery choice has not finished; editing and watching wait for it.
    pub reconcile_pending: bool,
}

impl AssetFlow {
    /// The worker is gone: nothing it was doing will ever report back.
    pub fn worker_stopped(&mut self) {
        self.paste_pending = false;
        self.sync_in_flight = false;
        self.sync_request_id = None;
        self.quit_sync_request_id = None;
    }
}

/// One step of an asset decision for the host to carry out, in order.
#[derive(Debug)]
pub enum AssetStep {
    SubmitSync(AssetSyncRequest),
    ApplyDocument(AppEffect),
    /// Replaces the status line; the last one in a list is the one that stays.
    Diagnostic(String),
    Redraw,
    StartWatcher,
    /// Publish the note the recovery left without a durable file.
    RecreateNote,
    PasteSettled,
    /// Settle the quit's asset barrier.
    QuitBarrier {
        succeeded: bool,
    },
}

pub struct AssetIo<'a, C: ClipboardPort> {
    editor: &'a mut EditorCoordinator<C>,
    assets: &'a mut AssetFlow,
}

impl<'a, C: ClipboardPort> AssetIo<'a, C> {
    pub fn new(editor: &'a mut EditorCoordinator<C>, assets: &'a mut AssetFlow) -> Self {
        Self { editor, assets }
    }

    pub fn submit_sync(
        &mut self,
        generation: Generation,
        effects: Vec<AssetEffect>,
        reconcile: Option<AssetReconcileMode>,
        for_quit: bool,
    ) -> Vec<AssetStep> {
        let mut steps = Vec::new();
        self.sync(generation, effects, reconcile, for_quit, &mut steps);
        steps
    }

    pub fn sync_after_recovery(&mut self) -> Vec<AssetStep> {
        self.assets.reconcile_pending = true;
        self.submit_sync(
            self.editor.view().generation,
            Vec::new(),
            Some(AssetReconcileMode::SafeBoundary),
            false,
        )
    }

    fn sync(
        &mut self,
        generation: Generation,
        effects: Vec<AssetEffect>,
        reconcile: Option<AssetReconcileMode>,
        for_quit: bool,
        steps: &mut Vec<AssetStep>,
    ) {
        let Some(request_id) = self.assets.sync_sequence.checked_add(1) else {
            steps.push(AssetStep::Diagnostic(
                "图片事务序号已耗尽；已停止自动整理。".into(),
            ));
            steps.push(AssetStep::Redraw);
            return;
        };
        self.assets.sync_sequence = request_id;
        self.assets.sync_in_flight = true;
        self.assets.sync_request_id = Some(request_id);
        if for_quit {
            self.assets.quit_sync_request_id = Some(request_id);
        }
        steps.push(AssetStep::SubmitSync(AssetSyncRequest {
            request_id,
            generation,
            effects,
            references: self.editor.managed_ref_counts(),
            reconcile,
            safe_expected: (reconcile == Some(AssetReconcileMode::SafeBoundary))
                .then(|| self.editor.view().base_disk_hash),
        }));
    }

    fn converge(&mut self, effects: Vec<AssetEffect>, steps: &mut Vec<AssetStep>) {
        self.sync(
            self.editor.view().generation,
            effects,
            Some(AssetReconcileMode::Runtime),
            false,
            steps,
        );
    }

    pub fn on_paste(
        &mut self,
        result: Result<AssetPasteCompletion, AssetPasteFailure>,
    ) -> Vec<AssetStep> {
        let mut steps = Vec::new();
        self.assets.paste_pending = false;
        match result {
            Err(failure) => {
                let effects = failure
                    .rollback
                    .convergence_effects(&self.editor.managed_ref_counts());
                if !effects.is_empty() {
                    self.converge(effects, &mut steps);
                }
                steps.push(AssetStep::Diagnostic(format!(
                    "图片粘贴失败；未插入引用，资产正按最新引用状态收敛：{}",
                    failure.error
                )));
                steps.push(AssetStep::Redraw);
            }
            Ok(completion) => self.commit_paste(completion, &mut steps),
        }
        steps.push(AssetStep::PasteSettled);
        steps
    }

    fn commit_paste(&mut self, completion: AssetPasteCompletion, steps: &mut Vec<AssetStep>) {
        let AssetPasteCompletion {
            expected_generation,
            selection,
            timestamp_ms,
            markdown,
            rollback,
        } = completion;
        if self.editor.view().generation != expected_generation {
            let effects = rollback.convergence_effects(&self.editor.managed_ref_counts());
            self.converge(effects, steps);
            steps.push(AssetStep::Diagnostic(
                "图片准备期间文档已变化；未插入引用，资产正按最新引用状态收敛。".into(),
            ));
            steps.push(AssetStep::Redraw);
            return;
        }
        match self.editor.commit_prepared_paste(
            expected_generation,
            selection,
            markdown,
            timestamp_ms,
        ) {
            Ok(effect) => steps.push(AssetStep::ApplyDocument(effect)),
            Err(error) => {
                let effects = rollback.convergence_effects(&self.editor.managed_ref_counts());
                self.converge(effects, steps);
                steps.push(AssetStep::Diagnostic(format!(
                    "图片已准备但文档事务未提交；资产正按最新引用状态收敛：{error}"
                )));
                steps.push(AssetStep::Redraw);
            }
        }
    }

    pub fn on_sync(
        &mut self,
        request_id: u64,
        result: Result<AssetReconcileReport, StorageFailure>,
    ) -> Vec<AssetStep> {
        let mut steps = Vec::new();
        let matched_request = self.assets.sync_request_id == Some(request_id);
        let quit_request = self.assets.quit_sync_request_id == Some(request_id);
        if matched_request {
            self.assets.sync_in_flight = false;
            self.assets.sync_request_id = None;
        }
        if quit_request {
            self.assets.quit_sync_request_id = None;
        }
        let succeeded = result.is_ok();
        match result {
            Ok(report) => {
                if report.physical_delete_deferred {
                    steps.push(AssetStep::Diagnostic(
                        "便签磁盘状态正在变化；已保留回收站证据并延后物理清理。".into(),
                    ));
                }
                if !report.missing_references.is_empty() {
                    steps.push(AssetStep::Diagnostic(format!(
                        "有 {} 个受管图片引用缺少文件；Markdown 未修改。",
                        report.missing_references.len()
                    )));
                }
            }
            Err(error) => steps.push(AssetStep::Diagnostic(format!(
                "图片整理未完成；用户文件与文档均未删除：{error}"
            ))),
        }
        if self.assets.reconcile_pending && !self.assets.sync_in_flight {
            self.assets.reconcile_pending = false;
            steps.push(AssetStep::StartWatcher);
            if self.editor.view().base_disk_hash.is_none() {
                steps.push(AssetStep::RecreateNote);
            }
        }
        if quit_request {
            steps.push(AssetStep::QuitBarrier { succeeded });
        } else if matched_request {
            steps.push(AssetStep::PasteSettled);
        }
        steps.push(AssetStep::Redraw);
        steps
    }
}

#[cfg(test)]
mod startup_tests;
