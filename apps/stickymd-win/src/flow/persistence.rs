//! Persistence coordination state, independent of filesystem APIs and UI types.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use stickymd_core::{
    DiskFingerprint, ExternalFileFact, ExternalFileState, FileConflict, Generation,
};

use super::{
    AutosaveAction, AutosaveScheduler, ExternalDecision, SaveTrigger, decide_external_change,
};

pub const EXTERNAL_DEBOUNCE_MS: u64 = 150;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconciliationAction {
    IgnoreKnown,
    RecreateMissing,
    ReloadClean(ExternalFileFact),
    ConflictChanged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuitAction {
    BlockedByRecovery,
    BlockedByConflict,
    WaitForInFlightSave,
    RecreateMissing,
    SaveDirty,
    Exit,
}

#[derive(Debug, Default)]
pub struct PersistenceCoordinator {
    autosave: AutosaveScheduler,
    external_deadline_ms: Option<u64>,
    conflict: Option<FileConflict>,
    recovery_pending: bool,
    note_save_in_flight: bool,
    required_write_in_flight: bool,
    durability_required: bool,
}

impl PersistenceCoordinator {
    pub fn on_document_changed(&mut self, now_ms: u64, generation: Generation) {
        if self.conflict.is_none() {
            self.autosave.on_document_changed(now_ms, generation);
        }
    }

    pub fn request_save(
        &mut self,
        generation: Generation,
        trigger: SaveTrigger,
    ) -> Option<AutosaveAction> {
        self.autosave.request_now(generation, trigger)
    }

    pub fn tick_autosave(&mut self, now_ms: u64) -> Option<AutosaveAction> {
        self.autosave.tick(now_ms)
    }

    pub fn autosave_deadline(&self) -> Option<u64> {
        self.autosave.next_deadline_ms()
    }

    pub fn on_watch_hint(&mut self, now_ms: u64) {
        self.external_deadline_ms = now_ms.checked_add(EXTERNAL_DEBOUNCE_MS);
    }

    pub fn take_external_check(&mut self, now_ms: u64) -> bool {
        if self
            .external_deadline_ms
            .is_some_and(|deadline| now_ms >= deadline)
        {
            self.external_deadline_ms = None;
            true
        } else {
            false
        }
    }

    pub fn external_deadline(&self) -> Option<u64> {
        self.external_deadline_ms
    }

    /// Classify one fresh durable observation and advance only the conflict /
    /// durability coordination state. Canonical text replacement remains a
    /// separate typed capability owned by `EditorCoordinator`.
    pub fn observe_external(
        &mut self,
        dirty: bool,
        base_disk_hash: Option<DiskFingerprint>,
        generation: Generation,
        external: ExternalFileState,
    ) -> ReconciliationAction {
        match decide_external_change(dirty, base_disk_hash, external) {
            ExternalDecision::IgnoreKnown => {
                self.confirm_durable_present();
                self.clear_conflict();
                ReconciliationAction::IgnoreKnown
            }
            ExternalDecision::RecreateMissing => {
                self.clear_conflict();
                ReconciliationAction::RecreateMissing
            }
            ExternalDecision::ReloadClean(fact) => ReconciliationAction::ReloadClean(fact),
            ExternalDecision::EnterConflict(external) => {
                let conflict = FileConflict {
                    external,
                    detected_at_generation: generation,
                    previous_disk_fingerprint: base_disk_hash,
                };
                if self.conflict.is_some() {
                    self.update_conflict(conflict);
                } else {
                    self.enter_conflict(conflict);
                }
                ReconciliationAction::ConflictChanged
            }
        }
    }

    pub fn enter_conflict(&mut self, conflict: FileConflict) {
        self.conflict = Some(conflict);
        self.refresh_autosave_pause();
    }

    pub fn update_conflict(&mut self, conflict: FileConflict) {
        debug_assert!(self.conflict.is_some());
        self.conflict = Some(conflict);
    }

    pub fn conflict(&self) -> Option<&FileConflict> {
        self.conflict.as_ref()
    }

    pub fn clear_conflict(&mut self) {
        self.conflict = None;
        self.refresh_autosave_pause();
    }

    pub fn set_recovery_pending(&mut self, pending: bool) {
        self.recovery_pending = pending;
        self.refresh_autosave_pause();
    }

    fn refresh_autosave_pause(&mut self) {
        self.autosave
            .set_paused(self.recovery_pending || self.conflict.is_some());
    }

    /// Record an actual hand-off to the bounded note worker. A recreate request
    /// remains required until a recreate job itself succeeds; a different
    /// in-flight save receipt cannot accidentally satisfy that obligation.
    pub fn note_save_submitted(&mut self, trigger: SaveTrigger) {
        let required = trigger == SaveTrigger::RecreateMissing;
        if required {
            self.durability_required = true;
        }
        if !self.note_save_in_flight {
            self.note_save_in_flight = true;
            self.required_write_in_flight = required;
        }
    }

    pub fn note_save_finished(&mut self, succeeded: bool) -> bool {
        let completed_required = self.required_write_in_flight;
        if succeeded && completed_required {
            self.durability_required = false;
        }
        self.note_save_in_flight = false;
        self.required_write_in_flight = false;
        completed_required
    }

    pub fn durability_required(&self) -> bool {
        self.durability_required
    }

    pub fn has_required_write(&self) -> bool {
        self.note_save_in_flight || self.durability_required
    }

    pub fn decide_quit(&self, recovery_pending: bool, document_dirty: bool) -> QuitAction {
        if recovery_pending {
            QuitAction::BlockedByRecovery
        } else if self.conflict.is_some() {
            QuitAction::BlockedByConflict
        } else if self.note_save_in_flight {
            QuitAction::WaitForInFlightSave
        } else if self.durability_required {
            QuitAction::RecreateMissing
        } else if document_dirty {
            QuitAction::SaveDirty
        } else {
            QuitAction::Exit
        }
    }

    /// A fresh inspection found the expected durable file. This discharges a
    /// conservative recreate obligation left behind by an overlapping save.
    pub fn confirm_durable_present(&mut self) {
        self.durability_required = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stickymd_core::{LineEnding, hash_bytes};

    #[test]
    fn raw_watch_hints_coalesce_to_one_check() {
        let mut flow = PersistenceCoordinator::default();
        for now in 0..100 {
            flow.on_watch_hint(now);
        }
        assert!(!flow.take_external_check(248));
        assert!(flow.take_external_check(249));
        assert!(!flow.take_external_check(1000));
    }

    #[test]
    fn unrelated_save_receipt_cannot_satisfy_required_recreate() {
        let mut flow = PersistenceCoordinator::default();
        flow.note_save_submitted(SaveTrigger::Debounce);
        flow.note_save_submitted(SaveTrigger::RecreateMissing);
        assert!(!flow.note_save_finished(true));

        assert!(flow.durability_required());
        assert!(!flow.note_save_in_flight);

        flow.note_save_submitted(SaveTrigger::RecreateMissing);
        assert!(flow.note_save_finished(true));
        assert!(!flow.has_required_write());
    }

    #[test]
    fn recovery_and_conflict_pause_are_independent() {
        let mut flow = PersistenceCoordinator::default();
        flow.set_recovery_pending(true);
        flow.clear_conflict();
        assert!(
            flow.request_save(Generation::initial(), SaveTrigger::Manual)
                .is_none()
        );
        flow.set_recovery_pending(false);
        assert!(
            flow.request_save(Generation::initial(), SaveTrigger::Manual)
                .is_some()
        );
    }

    #[test]
    fn known_base_and_missing_observations_clear_stale_conflicts() {
        let mut flow = PersistenceCoordinator::default();
        let known = hash_bytes(b"known");
        flow.enter_conflict(FileConflict {
            external: ExternalFileState::InvalidUtf8 {
                fingerprint: hash_bytes(&[0xff]),
            },
            detected_at_generation: Generation::initial(),
            previous_disk_fingerprint: Some(known),
        });
        assert_eq!(
            flow.observe_external(
                true,
                Some(known),
                Generation::initial(),
                ExternalFileState::Present(ExternalFileFact {
                    fingerprint: known,
                    text: "known".into(),
                    line_ending: LineEnding::Lf,
                    durable_len: 5,
                })
            ),
            ReconciliationAction::IgnoreKnown
        );
        assert!(flow.conflict().is_none());

        flow.enter_conflict(FileConflict {
            external: ExternalFileState::Present(ExternalFileFact {
                fingerprint: hash_bytes(b"old external"),
                text: "old external".into(),
                line_ending: LineEnding::Lf,
                durable_len: 12,
            }),
            detected_at_generation: Generation::initial(),
            previous_disk_fingerprint: Some(known),
        });
        assert_eq!(
            flow.observe_external(
                true,
                Some(known),
                Generation::initial(),
                ExternalFileState::Missing
            ),
            ReconciliationAction::RecreateMissing
        );
        assert!(flow.conflict().is_none());
    }

    #[test]
    fn quit_never_bypasses_clean_conflict_or_required_write() {
        let mut flow = PersistenceCoordinator::default();
        let hash = hash_bytes(b"disk");
        flow.enter_conflict(FileConflict {
            external: ExternalFileState::InvalidUtf8 {
                fingerprint: hash_bytes(&[0xff]),
            },
            detected_at_generation: Generation::initial(),
            previous_disk_fingerprint: Some(hash),
        });
        assert_eq!(
            flow.decide_quit(false, false),
            QuitAction::BlockedByConflict
        );
        flow.clear_conflict();
        flow.note_save_submitted(SaveTrigger::RecreateMissing);
        assert_eq!(
            flow.decide_quit(false, false),
            QuitAction::WaitForInFlightSave
        );
        flow.note_save_finished(false);
        assert_eq!(flow.decide_quit(false, false), QuitAction::RecreateMissing);
    }
}
