//! Carries out the managed-image decisions of `AssetIo` in the native shell.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#state-changes

use stickymd_core::{AssetEffect, Generation};
use winit::event_loop::ActiveEventLoop;

use super::StickyApp;
use crate::platform::windows::ArboardClipboard;
use stickymd_app::flow::window::state::{QuitBarrier, WindowIntent};
use stickymd_app::flow::{AssetIo, AssetStep, PendingAssetPaste, SaveTrigger};
use stickymd_app::ports::{
    AssetPasteCompletion, AssetPasteFailure, AssetPasteRequest, AssetReconcileMode,
    AssetReconcileReport, PersistMode, StorageFailure,
};

impl StickyApp {
    fn asset_io(&mut self) -> AssetIo<'_, ArboardClipboard> {
        AssetIo::new(&mut self.coordinator, &mut self.assets)
    }

    pub(super) fn submit_asset_paste(&mut self, pending: PendingAssetPaste) {
        if self.assets.paste_pending {
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
        if self.worker.submit_asset_paste(request) {
            self.assets.paste_pending = true;
            self.diagnostic = Some("正在准备图片…".into());
        } else {
            self.diagnostic = Some("图片工作队列暂不可用；未修改文档。".into());
        }
        self.request_redraw();
    }

    pub(super) fn handle_asset_paste_completion(
        &mut self,
        event_loop: Option<&ActiveEventLoop>,
        result: Result<AssetPasteCompletion, AssetPasteFailure>,
    ) {
        let steps = self.asset_io().on_paste(result);
        self.run_assets(event_loop, steps);
    }

    pub(super) fn submit_asset_sync(
        &mut self,
        generation: Generation,
        effects: Vec<AssetEffect>,
        reconcile: Option<AssetReconcileMode>,
        for_quit: bool,
    ) {
        let steps = self
            .asset_io()
            .submit_sync(generation, effects, reconcile, for_quit);
        self.run_assets(None, steps);
    }

    pub(super) fn sync_assets_after_recovery(&mut self) {
        let steps = self.asset_io().sync_after_recovery();
        self.run_assets(None, steps);
    }

    pub(super) fn handle_asset_sync_completion(
        &mut self,
        event_loop: Option<&ActiveEventLoop>,
        request_id: u64,
        result: Result<AssetReconcileReport, StorageFailure>,
    ) {
        let steps = self.asset_io().on_sync(request_id, result);
        self.run_assets(event_loop, steps);
    }

    fn run_assets(&mut self, event_loop: Option<&ActiveEventLoop>, steps: Vec<AssetStep>) {
        for step in steps {
            match step {
                AssetStep::SubmitSync(request) => self.worker.submit_asset_sync(request),
                AssetStep::ApplyDocument(effect) => self.apply_effect(effect),
                AssetStep::Diagnostic(message) => self.diagnostic = Some(message),
                AssetStep::Redraw => self.request_redraw(),
                AssetStep::StartWatcher => self.start_watcher(),
                AssetStep::RecreateNote => {
                    let effects = self.note_io().submit_save(
                        SaveTrigger::RecreateMissing,
                        Some(PersistMode::Guarded { expected: None }),
                    );
                    self.run_note(event_loop, effects);
                }
                AssetStep::PasteSettled => self.dispatch_window_intent(
                    event_loop,
                    WindowIntent::PasteSettled {
                        guards: self.window_guards(),
                    },
                ),
                AssetStep::QuitBarrier { succeeded } => self.dispatch_window_intent(
                    event_loop,
                    WindowIntent::QuitBarrierCompleted {
                        barrier: QuitBarrier::AssetGc,
                        succeeded,
                        guards: self.window_guards(),
                    },
                ),
            }
        }
    }
}
