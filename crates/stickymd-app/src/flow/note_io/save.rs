//! Save requests and the follow-up a save receipt calls for.
//!
//! plan_ref: docs/plan/05_document_persistence.md#autosave-and-save-queue

use stickymd_core::{DiskFingerprint, Generation};

use super::{NoteEffect, NoteEffects, NoteIo};
use crate::flow::{ClipboardPort, RecoveryOperation, SaveTrigger};
use crate::ports::{PersistMode, PersistRequest, TemporaryCleanup};

impl<C: ClipboardPort> NoteIo<'_, C> {
    pub fn request_save(&mut self, trigger: SaveTrigger) -> NoteEffects {
        let mut effects = NoteEffects::default();
        if let Some(action) = self
            .persistence
            .request_save(self.editor.view().generation, trigger)
        {
            self.submit(action.trigger, None, &mut effects);
        }
        effects
    }

    pub fn submit_save(&mut self, trigger: SaveTrigger, mode: Option<PersistMode>) -> NoteEffects {
        let mut effects = NoteEffects::default();
        self.submit(trigger, mode, &mut effects);
        effects
    }

    /// The autosave and external-check deadlines reached by `now_ms`.
    pub fn tick(&mut self, now_ms: u64) -> NoteEffects {
        let mut effects = NoteEffects::default();
        if let Some(action) = self.persistence.tick_autosave(now_ms) {
            self.submit(action.trigger, None, &mut effects);
        }
        if self.persistence.take_external_check(now_ms) {
            self.inspect(&mut effects);
        }
        effects
    }

    /// Queues one durable observation, stamped with the Durable Fingerprint known now so its
    /// receipt can be checked for staleness.
    pub(super) fn inspect(&self, effects: &mut NoteEffects) {
        effects.push(NoteEffect::InspectExternal {
            requested_base: self.editor.view().base_disk_hash,
        });
    }

    pub(super) fn submit(
        &mut self,
        trigger: SaveTrigger,
        override_mode: Option<PersistMode>,
        effects: &mut NoteEffects,
    ) {
        if self.recovery.is_pending() && trigger != SaveTrigger::KeepLocal {
            return;
        }
        let snapshot = self.editor.snapshot();
        let mode = override_mode.unwrap_or(PersistMode::Guarded {
            expected: self.editor.view().base_disk_hash,
        });
        self.persistence.note_save_submitted(trigger);
        let generation = snapshot.generation;
        effects.push(NoteEffect::SubmitSave(PersistRequest {
            generation,
            text: snapshot.text,
            line_ending: snapshot.line_ending,
            mode,
        }));
        effects.diagnostic(format!("正在保存 generation {}…", generation.value()));
        effects.push(NoteEffect::Redraw);
    }

    pub(super) fn on_saved(
        &mut self,
        generation: Generation,
        fingerprint: DiskFingerprint,
        mode: PersistMode,
        effects: &mut NoteEffects,
    ) {
        let completed_required = self.persistence.note_save_finished(true);
        if let Err(error) = self.editor.acknowledge_persisted(generation, fingerprint) {
            effects.diagnostic(format!("保存回执无效：{error}"));
            return;
        }
        if let (
            Some(RecoveryOperation::Restoring | RecoveryOperation::UsingCanonical),
            Some(operation),
        ) = (self.recovery.operation(), self.recovery.operation_id())
        {
            effects.push(NoteEffect::RemoveTemporary(
                TemporaryCleanup::RecoveryResolved { operation },
            ));
        } else if self.persistence.keep_local_in_progress() && mode == PersistMode::ForceOverwrite {
            self.persistence.end_keep_local();
            self.persistence.clear_conflict();
            effects.diagnostic("已保留本地内容并覆盖外部文件。");
        } else {
            effects.diagnostic("已保存");
        }
        let keep_local_resubmitted = self.resubmit_keep_local(mode, effects);
        if keep_local_resubmitted {
            // The explicit conflict resolution owns the one latest-pending
            // slot. A generic stale-generation follow-up must not replace it.
        } else if self.editor.view().dirty {
            self.submit(SaveTrigger::Debounce, None, effects);
        } else if self.persistence.durability_required() && !completed_required {
            // A recreate hint overlapped a different save. Inspect the durable
            // path before deciding whether a second write is still needed.
            self.inspect(effects);
        }
        effects.push(NoteEffect::RefreshTitle);
        if !keep_local_resubmitted {
            effects.push(NoteEffect::NoteBarriers { succeeded: true });
        }
        effects.push(NoteEffect::Redraw);
    }

    pub(super) fn resubmit_keep_local(
        &mut self,
        completed_mode: PersistMode,
        effects: &mut NoteEffects,
    ) -> bool {
        if keep_local_needs_resubmit(
            self.persistence.keep_local_in_progress(),
            self.persistence.conflict().is_some(),
            completed_mode,
        ) {
            effects.diagnostic("旧保存任务已完成；正在用最新内存文本执行“保留本地”…");
            self.submit(
                SaveTrigger::KeepLocal,
                Some(PersistMode::ForceOverwrite),
                effects,
            );
            true
        } else {
            false
        }
    }
}

pub(super) fn keep_local_needs_resubmit(
    keep_local_in_progress: bool,
    conflict_present: bool,
    completed_mode: PersistMode,
) -> bool {
    keep_local_in_progress && conflict_present && completed_mode != PersistMode::ForceOverwrite
}
