//! What each note receipt means. A receipt whose identity no longer matches changes nothing.
//!
//! plan_ref: docs/plan/04_runtime_state_model.md#generation

use stickymd_core::{DiskFingerprint, ExternalFileState, Generation};

use super::{NoteEffect, NoteEffects, NoteIo};
use crate::flow::{ClipboardPort, SaveTrigger};
use crate::ports::{
    NoteReceipt, PersistMode, PersistResult, PreserveOutcome, StorageFailure, TemporaryCleanup,
};

impl<C: ClipboardPort> NoteIo<'_, C> {
    /// `now_ms` schedules the recheck of a superseded observation.
    pub fn on_receipt(&mut self, receipt: NoteReceipt, now_ms: u64) -> NoteEffects {
        let mut effects = NoteEffects::default();
        match receipt {
            NoteReceipt::Save {
                generation,
                mode,
                result,
            } => self.on_save(generation, mode, result, &mut effects),
            NoteReceipt::External {
                requested_base,
                result,
            } => self.on_observed(requested_base, result, now_ms, &mut effects),
            NoteReceipt::TemporaryRemoved { purpose, result } => {
                self.on_temporary_removed(purpose, result, &mut effects);
            }
            NoteReceipt::CanonicalPreserved { operation, result } => {
                if self.recovery.accepts(operation) {
                    self.on_canonical_preserved(result, &mut effects);
                }
            }
        }
        effects
    }

    fn on_save(
        &mut self,
        submitted: Generation,
        mode: PersistMode,
        result: Result<PersistResult, StorageFailure>,
        effects: &mut NoteEffects,
    ) {
        match result {
            Ok(PersistResult::Saved {
                generation,
                fingerprint,
                ..
            }) if generation == submitted => self.on_saved(generation, fingerprint, mode, effects),
            Ok(PersistResult::Saved { generation, .. }) => {
                self.persistence.note_save_finished(false);
                self.recovery.fail_operation();
                self.persistence.end_keep_local();
                effects.diagnostic(format!(
                    "保存回执 generation 不一致：submitted={} result={}",
                    submitted.value(),
                    generation.value()
                ));
                effects.push(NoteEffect::NoteBarriers { succeeded: false });
                effects.push(NoteEffect::Redraw);
            }
            Ok(PersistResult::Conflict { observed, .. }) => {
                self.persistence.note_save_finished(false);
                if self.recovery.is_pending() {
                    self.on_recovery_conflict(observed, effects);
                } else {
                    self.reconcile(observed, effects);
                }
                if !self.resubmit_keep_local(mode, effects) {
                    effects.push(NoteEffect::NoteBarriers { succeeded: false });
                }
            }
            Err(error) => {
                self.persistence.note_save_finished(false);
                self.recovery.fail_operation();
                let resubmitted = self.resubmit_keep_local(mode, effects);
                if !resubmitted {
                    self.persistence.end_keep_local();
                    effects.diagnostic(format!(
                        "保存 generation {} 失败；内存文本仍保留。Ctrl+S 重试：{error}",
                        submitted.value()
                    ));
                }
                effects.push(NoteEffect::RefreshTitle);
                if !resubmitted {
                    effects.push(NoteEffect::NoteBarriers { succeeded: false });
                }
                effects.push(NoteEffect::Redraw);
            }
        }
    }

    fn on_observed(
        &mut self,
        requested_base: Option<DiskFingerprint>,
        result: Result<ExternalFileState, StorageFailure>,
        now_ms: u64,
        effects: &mut NoteEffects,
    ) {
        let current_base = self.editor.view().base_disk_hash;
        if !self
            .persistence
            .external_observation_current(requested_base, current_base, now_ms)
        {
            return;
        }
        match result {
            Ok(external) => self.reconcile(external, effects),
            Err(error) => {
                effects.diagnostic(format!("外部文件读取失败，未改变当前文本：{error}"));
                effects.push(NoteEffect::Redraw);
            }
        }
    }

    fn on_temporary_removed(
        &mut self,
        purpose: TemporaryCleanup,
        result: Result<(), StorageFailure>,
        effects: &mut NoteEffects,
    ) {
        match (purpose, result) {
            (TemporaryCleanup::RecoveryResolved { operation }, _)
                if !self.recovery.accepts(operation) => {}
            (TemporaryCleanup::RecoveryResolved { .. }, Ok(())) => self.finish_recovery(effects),
            (TemporaryCleanup::ConflictDiscarded, Ok(())) => {}
            (TemporaryCleanup::RecoveryResolved { .. }, Err(error)) => {
                self.recovery.fail_operation();
                effects.diagnostic(format!("临时恢复证据无法清理，仍保持恢复状态：{error}"));
                effects.push(NoteEffect::Redraw);
            }
            (TemporaryCleanup::ConflictDiscarded, Err(error)) => {
                effects.diagnostic(format!(
                    "外部内容已载入，但本地临时副本无法清理；下次启动可能再次提示恢复：{error}"
                ));
                effects.push(NoteEffect::Redraw);
            }
        }
    }

    fn on_canonical_preserved(
        &mut self,
        result: Result<PreserveOutcome, StorageFailure>,
        effects: &mut NoteEffects,
    ) {
        match result {
            Ok(PreserveOutcome::Changed(observed)) => self.on_recovery_conflict(observed, effects),
            Ok(PreserveOutcome::Preserved) => {
                let expected = self.recovery.take_restore_guard().unwrap_or(None);
                self.submit(
                    SaveTrigger::KeepLocal,
                    Some(PersistMode::Guarded { expected }),
                    effects,
                );
            }
            Ok(PreserveOutcome::Unverifiable) => {
                effects.diagnostic(
                    "note.md 过大，未恢复：请结束进程，移走 note.md（保留 .tmp）后重启按 F6。",
                );
                self.recovery.fail_operation();
                effects.push(NoteEffect::Redraw);
            }
            Err(error) => {
                effects.diagnostic(format!(
                    "无效的 canonical note.md 无法先行保留；未执行恢复覆盖：{error}"
                ));
                self.recovery.fail_operation();
                effects.push(NoteEffect::Redraw);
            }
        }
    }
}
