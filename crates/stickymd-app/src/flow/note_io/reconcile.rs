//! External-file reconciliation and the two ways out of a conflict.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use stickymd_core::ExternalFileState;

use super::{NoteEffect, NoteEffects, NoteIo};
use crate::flow::notice::Notice;
use crate::flow::{ClipboardPort, ReconciliationAction, SaveTrigger};
use crate::ports::{PersistMode, TemporaryCleanup};

impl<C: ClipboardPort> NoteIo<'_, C> {
    pub(super) fn reconcile(&mut self, external: ExternalFileState, effects: &mut NoteEffects) {
        if self.recovery.is_pending() {
            return;
        }
        let (dirty, base_disk_hash, generation) = {
            let view = self.editor.view();
            (view.dirty, view.base_disk_hash, view.generation)
        };
        let had_conflict = self.persistence.conflict().is_some();
        match self
            .persistence
            .observe_external(dirty, base_disk_hash, generation, external)
        {
            ReconciliationAction::IgnoreKnown => {
                if had_conflict {
                    effects.notice(Notice::ConflictClearedByRestore);
                    if dirty {
                        self.submit(SaveTrigger::Debounce, None, effects);
                    }
                    effects.push(NoteEffect::RefreshTitle);
                    effects.push(NoteEffect::Redraw);
                }
                return;
            }
            ReconciliationAction::RecreateMissing => {
                self.submit(
                    SaveTrigger::RecreateMissing,
                    Some(PersistMode::Guarded { expected: None }),
                    effects,
                );
                return;
            }
            ReconciliationAction::ReloadClean(fact) => {
                match self.editor.reconcile_external_for_runtime(&fact) {
                    Ok(effect) => {
                        self.persistence.accept_external();
                        effects.push(NoteEffect::ApplyDocument(effect));
                        effects.notice(Notice::ExternalLoaded);
                        effects.push(NoteEffect::RefreshTitle);
                        effects.push(NoteEffect::Redraw);
                    }
                    Err(error) => {
                        effects.notice(Notice::ExternalReconcileFailed(error.to_string()));
                        effects.push(NoteEffect::Redraw);
                    }
                }
                return;
            }
            ReconciliationAction::ConflictChanged => {}
        }
        // The conflict itself shows in the derived layer, with a button per available choice.
        effects.push(NoteEffect::RefreshTitle);
        effects.push(NoteEffect::Redraw);
    }

    pub(super) fn load_external_conflict(&mut self, effects: &mut NoteEffects) {
        let Some(conflict) = self.persistence.conflict() else {
            return;
        };
        let ExternalFileState::Present(fact) = &conflict.external else {
            effects.notice(Notice::ExternalNotLoadable);
            effects.push(NoteEffect::Redraw);
            return;
        };
        let fact = fact.clone();
        match self.editor.reconcile_external_for_runtime(&fact) {
            Ok(effect) => {
                self.persistence.accept_external();
                effects.push(NoteEffect::ApplyDocument(effect));
                effects.push(NoteEffect::RemoveTemporary(
                    TemporaryCleanup::ConflictDiscarded,
                ));
                effects.notice(Notice::ExternalReloadedHistoryCleared);
                effects.push(NoteEffect::RefreshTitle);
            }
            Err(error) => effects.notice(Notice::ExternalLoadFailed(error.to_string())),
        }
        effects.push(NoteEffect::Redraw);
    }

    pub(super) fn keep_local_conflict(&mut self, effects: &mut NoteEffects) {
        self.persistence.begin_keep_local();
        self.submit(
            SaveTrigger::KeepLocal,
            Some(PersistMode::ForceOverwrite),
            effects,
        );
    }
}
