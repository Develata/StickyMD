//! Carries out the managed-image decisions of `AssetIo`.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#state-changes

use crate::flow::window::state::{QuitBarrier, WindowIntent};
use crate::flow::{AssetIo, AssetStep, PendingAssetPaste, SaveTrigger};
use crate::ports::{
    AssetPasteCompletion, AssetPasteFailure, AssetPasteRequest, AssetReconcileMode,
    AssetReconcileReport, PersistMode, StorageFailure,
};
use stickymd_core::{AssetEffect, Generation};

use super::turns::Turn;
use super::{AppSession, SessionClipboard, ShellEffect, ShellEffects, ViewFacts};

impl AppSession {
    fn asset_io(&mut self) -> AssetIo<'_, SessionClipboard> {
        AssetIo::new(&mut self.coordinator, &mut self.assets)
    }

    pub fn submit_asset_paste(&mut self, pending: PendingAssetPaste, out: &mut ShellEffects) {
        if self.assets.paste_pending {
            out.push(ShellEffect::Diagnostic(
                "上一批图片仍在处理中；本次粘贴未开始。".into(),
            ));
            out.push(ShellEffect::RedrawNeeded);
            return;
        }
        let request = AssetPasteRequest {
            expected_generation: pending.expected_generation,
            selection: pending.selection,
            timestamp_ms: pending.timestamp_ms,
            payload: pending.payload,
        };
        if self.worker.submit_asset_paste(request) {
            self.assets.paste_pending = true;
            out.push(ShellEffect::Diagnostic("正在准备图片…".into()));
        } else {
            out.push(ShellEffect::Diagnostic(
                "图片工作队列暂不可用；未修改文档。".into(),
            ));
        }
        out.push(ShellEffect::RedrawNeeded);
    }

    pub fn submit_asset_sync(
        &mut self,
        generation: Generation,
        effects: Vec<AssetEffect>,
        reconcile: Option<AssetReconcileMode>,
        for_quit: bool,
        out: &mut ShellEffects,
    ) {
        let steps = self
            .asset_io()
            .submit_sync(generation, effects, reconcile, for_quit);
        Self::queue_assets(steps, out);
    }

    pub fn sync_assets_after_recovery(&mut self, out: &mut ShellEffects) {
        let steps = self.asset_io().sync_after_recovery();
        Self::queue_assets(steps, out);
    }

    pub(super) fn on_asset_paste(
        &mut self,
        result: Result<AssetPasteCompletion, AssetPasteFailure>,
        out: &mut ShellEffects,
    ) {
        let steps = self.asset_io().on_paste(result);
        Self::queue_assets(steps, out);
    }

    pub(super) fn on_asset_sync(
        &mut self,
        request_id: u64,
        result: Result<AssetReconcileReport, StorageFailure>,
        out: &mut ShellEffects,
    ) {
        let steps = self.asset_io().on_sync(request_id, result);
        Self::queue_assets(steps, out);
    }

    fn queue_assets(steps: Vec<AssetStep>, out: &mut ShellEffects) {
        for step in steps {
            out.turn(Turn::Asset(step));
        }
    }

    pub(super) fn apply_asset_step(
        &mut self,
        step: AssetStep,
        view: ViewFacts,
        out: &mut ShellEffects,
    ) {
        match step {
            AssetStep::SubmitSync(request) => self.worker.submit_asset_sync(request),
            AssetStep::ApplyDocument(effect) => self.apply_app_effect(effect, out),
            AssetStep::Diagnostic(message) => out.push(ShellEffect::Diagnostic(message)),
            AssetStep::Redraw => out.push(ShellEffect::RedrawNeeded),
            AssetStep::StartWatcher => out.push(ShellEffect::StartWatcher),
            AssetStep::RecreateNote => {
                let effects = self.note_io().submit_save(
                    SaveTrigger::RecreateMissing,
                    Some(PersistMode::Guarded { expected: None }),
                );
                Self::queue_note(effects, out);
            }
            AssetStep::PasteSettled => self.dispatch_window_intent(
                WindowIntent::PasteSettled {
                    guards: self.guards(view),
                    now_ms: self.now_ms(),
                },
                out,
            ),
            AssetStep::QuitBarrier { succeeded } => self.dispatch_window_intent(
                WindowIntent::QuitBarrierCompleted {
                    barrier: QuitBarrier::AssetGc,
                    succeeded,
                    guards: self.guards(view),
                },
                out,
            ),
        }
    }
}
