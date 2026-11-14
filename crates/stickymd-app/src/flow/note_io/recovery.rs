//! The recovery choice: restore the temporary or use the canonical note.
//!
//! plan_ref: docs/plan/05_document_persistence.md#recovery

use stickymd_core::ExternalFileState;

use super::{NoteEffect, NoteEffects, NoteIo};
use crate::flow::{CanonicalRecoveryPlan, ClipboardPort, SaveTrigger};
use crate::ports::{PersistMode, TemporaryCleanup};

impl<C: ClipboardPort> NoteIo<'_, C> {
    pub(super) fn restore_temporary(&mut self, effects: &mut NoteEffects) {
        let Some(plan) = self.recovery.begin_restore() else {
            return;
        };
        if let Err(error) = self
            .editor
            .load_unpersisted_recovery(&plan.temporary.text, plan.temporary.line_ending)
        {
            self.recovery.fail_operation();
            effects.diagnostic(format!("无法载入临时内容：{error}"));
            return;
        }
        effects.push(NoteEffect::ResyncProjection);
        if let Some(expected) = plan.preserve_canonical {
            let Some(operation) = self.recovery.operation_id() else {
                return;
            };
            effects.push(NoteEffect::PreserveCanonical {
                operation,
                expected,
            });
        } else {
            let _ = self.recovery.take_restore_guard();
            self.submit(
                SaveTrigger::KeepLocal,
                Some(PersistMode::Guarded {
                    expected: plan.expected,
                }),
                effects,
            );
        }
    }

    pub(super) fn use_canonical(&mut self, effects: &mut NoteEffects) {
        let Some(plan) = self.recovery.begin_use_canonical() else {
            return;
        };
        let canonical_was_missing = matches!(&plan, CanonicalRecoveryPlan::Missing(_));
        let restore_result = match &plan {
            CanonicalRecoveryPlan::Present(canonical) => self.editor.reconcile_external(canonical),
            CanonicalRecoveryPlan::Missing(new_note) => {
                self.editor.reset_to_missing_document(*new_note)
            }
            CanonicalRecoveryPlan::Unusable => {
                effects.diagnostic(
                    "当前 note.md 无法载入（不是有效 UTF-8 或过大）；只能 F6 恢复有效临时内容。",
                );
                effects.push(NoteEffect::Redraw);
                return;
            }
        };
        if let Err(error) = restore_result {
            self.recovery.fail_operation();
            effects.diagnostic(format!("无法恢复当前 note.md 状态：{error}"));
            effects.push(NoteEffect::Redraw);
            return;
        }
        effects.push(NoteEffect::ResyncProjection);
        if canonical_was_missing {
            // The temporary file is the only durable evidence until an empty
            // canonical note has been published. Keep recovery pending and
            // let the successful save receipt perform the cleanup.
            self.submit(
                SaveTrigger::KeepLocal,
                Some(PersistMode::Guarded { expected: None }),
                effects,
            );
            return;
        }
        let Some(operation) = self.recovery.operation_id() else {
            return;
        };
        effects.push(NoteEffect::RemoveTemporary(
            TemporaryCleanup::RecoveryResolved { operation },
        ));
        effects.diagnostic("正在确认使用当前 note.md…");
        effects.push(NoteEffect::Redraw);
    }

    pub(super) fn finish_recovery(&mut self, effects: &mut NoteEffects) {
        self.recovery.finish();
        self.persistence.set_recovery_pending(false);
        effects.push(NoteEffect::AssetsAfterRecovery);
        effects.diagnostic("恢复选择已完成；正在核对受管图片后启用编辑。");
        effects.push(NoteEffect::RefreshTitle);
        effects.push(NoteEffect::Redraw);
    }

    pub(super) fn on_recovery_conflict(
        &mut self,
        observed: ExternalFileState,
        effects: &mut NoteEffects,
    ) {
        self.recovery.refresh_canonical_after_conflict(observed);
        effects.diagnostic("恢复等待期间 note.md 已变化，未覆盖外部内容；请重新选择 F6/F7。");
        effects.push(NoteEffect::RefreshTitle);
        effects.push(NoteEffect::Redraw);
    }
}
