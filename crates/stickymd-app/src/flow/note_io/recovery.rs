//! The recovery choice: restore the temporary or use the canonical note.
//!
//! plan_ref: docs/plan/05_document_persistence.md#recovery

use stickymd_core::ExternalFileState;

use super::{NoteEffect, NoteEffects, NoteIo};
use crate::flow::notice::Notice;
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
            effects.notice(Notice::TemporaryLoadFailed(error.to_string()));
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
                effects.notice(Notice::CanonicalNotLoadable);
                effects.push(NoteEffect::Redraw);
                return;
            }
        };
        if let Err(error) = restore_result {
            self.recovery.fail_operation();
            effects.notice(Notice::CanonicalRestoreFailed(error.to_string()));
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
        effects.push(NoteEffect::Redraw);
    }

    pub(super) fn finish_recovery(&mut self, effects: &mut NoteEffects) {
        self.recovery.finish();
        self.persistence.set_recovery_pending(false);
        effects.push(NoteEffect::AssetsAfterRecovery);
        effects.push(NoteEffect::RefreshTitle);
        effects.push(NoteEffect::Redraw);
    }

    pub(super) fn on_recovery_conflict(
        &mut self,
        observed: ExternalFileState,
        effects: &mut NoteEffects,
    ) {
        self.recovery.refresh_canonical_after_conflict(observed);
        effects.notice(Notice::RecoveryCanonicalChanged);
        effects.push(NoteEffect::RefreshTitle);
        effects.push(NoteEffect::Redraw);
    }
}
