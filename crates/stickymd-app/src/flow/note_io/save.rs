//! Save requests and the follow-up a save receipt calls for.
//!
//! plan_ref: docs/plan/05_document_persistence.md#autosave-and-save-queue

use stickymd_core::{DiskFingerprint, Generation};

use super::{NoteEffect, NoteEffects, NoteIo};
use crate::flow::notice::Notice;
use crate::flow::{ClipboardPort, RecoveryOperation, SaveFaultKind, SaveTrigger};
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
        if !self.persistence.worker_available() {
            // Nothing reaches a stopped worker. A save the user asked for is refused aloud (the
            // fault display already tells why), and a hide or quit waiting on this save ends now
            // instead of never.
            if trigger == SaveTrigger::Manual {
                effects.notice(Notice::PersistenceUnavailable);
                effects.push(NoteEffect::Redraw);
            }
            if matches!(trigger, SaveTrigger::HideToTray | SaveTrigger::Shutdown) {
                effects.push(NoteEffect::NoteBarriers { succeeded: false });
            }
            return;
        }
        let snapshot = self.editor.snapshot();
        let mode = override_mode.unwrap_or(PersistMode::Guarded {
            expected: self.editor.view().base_disk_hash,
        });
        self.persistence.note_save_submitted(trigger);
        let generation = snapshot.generation;
        if trigger == SaveTrigger::Manual {
            self.persistence.begin_manual_save(generation);
        }
        effects.push(NoteEffect::SubmitSave(PersistRequest {
            generation,
            text: snapshot.text,
            line_ending: snapshot.line_ending,
            mode,
        }));
        // Routine autosaves show no progress (plan 09 #notices); a manual save and the
        // persistence choices show theirs in the derived layer.
        effects.push(NoteEffect::Redraw);
    }

    pub(super) fn on_saved(
        &mut self,
        generation: Generation,
        fingerprint: DiskFingerprint,
        mode: PersistMode,
        effects: &mut NoteEffects,
    ) {
        if let Err(error) = self.editor.acknowledge_persisted(generation, fingerprint) {
            // Only a broken protocol reaches this: a receipt for a generation the document never
            // had. It proves nothing about the disk, so it discharges nothing and ends what waits.
            self.persistence.note_save_finished(false);
            self.recovery.fail_operation();
            self.persistence.end_keep_local();
            self.save_failed(
                SaveFaultKind::Failed,
                format!("保存回执无效：{error}"),
                effects,
            );
            effects.push(NoteEffect::NoteBarriers { succeeded: false });
            effects.push(NoteEffect::Redraw);
            return;
        }
        let completed_required = self.persistence.note_save_finished(true);
        // The failure is resolved before this save's own notice (plan 04 #notice-state).
        if self.persistence.resolve_save_failure() {
            effects.push(NoteEffect::FaultResolved);
        }
        let manual_completed = self.persistence.complete_manual_save(generation);
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
            effects.notice(Notice::KeptLocal);
        } else if manual_completed {
            effects.notice(Notice::Saved);
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
