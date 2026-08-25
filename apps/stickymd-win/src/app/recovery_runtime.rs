//! Recovery-choice coordination bridge.
//!
//! plan_ref: docs/plan/05_document_persistence.md#recovery

use stickymd_core::ExternalFileState;

use super::StickyApp;
use crate::flow::{CanonicalRecoveryPlan, SaveTrigger};
use crate::persistence::{PersistMode, TemporaryCleanup};

impl StickyApp {
    pub(super) fn handle_resolution_key(&mut self, primary: bool) -> bool {
        if self.recovery.is_pending() {
            if self.recovery.operation_pending() {
                self.diagnostic = Some("恢复操作正在完成，请稍候。".into());
                self.request_redraw();
                return true;
            }
            if primary {
                self.restore_temporary();
            } else {
                self.use_canonical();
            }
            return true;
        }
        if self.persistence.conflict().is_some() {
            if self.resolving_keep_local {
                self.diagnostic = Some("正在保存本地内容，请稍候。".into());
                self.request_redraw();
                return true;
            }
            if primary {
                self.load_external_conflict();
            } else {
                self.keep_local_conflict();
            }
            return true;
        }
        false
    }

    fn restore_temporary(&mut self) {
        let Some(plan) = self.recovery.begin_restore() else {
            return;
        };
        if let Err(error) = self
            .coordinator
            .load_unpersisted_recovery(&plan.temporary.text, plan.temporary.line_ending)
        {
            self.recovery.fail_operation();
            self.diagnostic = Some(format!("无法载入临时内容：{error}"));
            return;
        }
        self.full_projection_resync();
        if plan.preserve_canonical_first {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |duration| duration.as_secs());
            self.worker.preserve_canonical(
                self.paths.note_file.clone(),
                self.paths.note_dir.join(format!("note.invalid-{stamp}.md")),
            );
        } else {
            let _ = self.recovery.take_restore_guard();
            self.submit_save(
                SaveTrigger::KeepLocal,
                Some(PersistMode::Guarded {
                    expected: plan.expected,
                }),
            );
        }
    }

    fn use_canonical(&mut self) {
        let Some(plan) = self.recovery.begin_use_canonical() else {
            return;
        };
        let canonical_was_missing = matches!(&plan, CanonicalRecoveryPlan::Missing);
        let restore_result = match &plan {
            CanonicalRecoveryPlan::Present(canonical) => {
                self.coordinator.reconcile_external(canonical)
            }
            CanonicalRecoveryPlan::Missing => self.coordinator.reset_to_missing_document(),
            CanonicalRecoveryPlan::Unusable => {
                self.diagnostic =
                    Some("当前 note.md 不是有效 UTF-8；只能 F6 恢复有效临时内容。".into());
                self.request_redraw();
                return;
            }
        };
        if let Err(error) = restore_result {
            self.recovery.fail_operation();
            self.diagnostic = Some(format!("无法恢复当前 note.md 状态：{error}"));
            self.request_redraw();
            return;
        }
        self.full_projection_resync();
        if canonical_was_missing {
            // The temporary file is the only durable evidence until an empty
            // canonical note has been published. Keep recovery pending and
            // let the successful save receipt perform the cleanup.
            self.submit_save(
                SaveTrigger::KeepLocal,
                Some(PersistMode::Guarded { expected: None }),
            );
            return;
        }
        self.worker.remove_temporary(
            self.paths.note_tmp.clone(),
            TemporaryCleanup::RecoveryResolved,
        );
        self.diagnostic = Some("正在确认使用当前 note.md…".into());
        self.request_redraw();
    }

    pub(super) fn finish_recovery_cleanup(&mut self) {
        self.recovery.finish();
        self.persistence.set_recovery_pending(false);
        self.sync_assets_after_recovery();
        self.diagnostic = Some("恢复选择已完成；正在核对受管图片后启用编辑。".into());
        self.update_window_title();
        self.request_redraw();
    }

    pub(super) fn handle_recovery_save_conflict(&mut self, observed: ExternalFileState) {
        self.recovery.refresh_canonical_after_conflict(observed);
        self.diagnostic =
            Some("恢复等待期间 note.md 已变化，未覆盖外部内容；请重新选择 F6/F7。".into());
        self.update_window_title();
        self.request_redraw();
    }
}
