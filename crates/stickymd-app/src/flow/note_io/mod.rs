//! Note I/O decisions: when to save, what a receipt means, how a conflict or a recovery choice
//! resolves. `NoteIo` borrows the coordinators for one step and returns what the host must do, in
//! order; it performs no I/O and reads no clock.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

mod receipts;
mod reconcile;
mod recovery;
mod save;
#[cfg(test)]
mod tests;

use stickymd_core::{DiskFingerprint, PreserveExpectation};

use super::{
    AppEffect, ClipboardPort, EditorCoordinator, PersistenceCoordinator, RecoveryCoordinator,
};
use crate::ports::{PersistRequest, TemporaryCleanup};

/// One step of a note decision for the host to carry out. All session state is already updated
/// when the host receives the list, so a host reading it (title, guards) sees the final state.
#[derive(Debug)]
pub enum NoteEffect {
    SubmitSave(PersistRequest),
    InspectExternal {
        requested_base: Option<DiskFingerprint>,
    },
    RemoveTemporary(TemporaryCleanup),
    PreserveCanonical {
        operation: u64,
        expected: PreserveExpectation,
    },
    /// The document changed: update everything projected from it.
    ApplyDocument(AppEffect),
    /// The document was replaced outside the editor: rebuild everything projected from it.
    ResyncProjection,
    AssetsAfterRecovery,
    /// Settle the hide and quit barriers waiting on the note.
    NoteBarriers {
        succeeded: bool,
    },
    /// Replaces the status line; the last one in a list is the one that stays.
    Diagnostic(String),
    RefreshTitle,
    Redraw,
}

/// The ordered effects of one step. An idle step allocates nothing.
#[must_use]
#[derive(Debug, Default)]
pub struct NoteEffects(Vec<NoteEffect>);

impl NoteEffects {
    fn push(&mut self, effect: NoteEffect) {
        self.0.push(effect);
    }

    fn diagnostic(&mut self, message: impl Into<String>) {
        self.push(NoteEffect::Diagnostic(message.into()));
    }
}

impl IntoIterator for NoteEffects {
    type Item = NoteEffect;
    type IntoIter = std::vec::IntoIter<NoteEffect>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

pub struct NoteIo<'a, C: ClipboardPort> {
    editor: &'a mut EditorCoordinator<C>,
    persistence: &'a mut PersistenceCoordinator,
    recovery: &'a mut RecoveryCoordinator,
}

impl<'a, C: ClipboardPort> NoteIo<'a, C> {
    pub fn new(
        editor: &'a mut EditorCoordinator<C>,
        persistence: &'a mut PersistenceCoordinator,
        recovery: &'a mut RecoveryCoordinator,
    ) -> Self {
        Self {
            editor,
            persistence,
            recovery,
        }
    }

    /// Handles a conflict or recovery key. `None` means neither is waiting and the key is not
    /// consumed.
    pub fn resolve(&mut self, primary: bool) -> Option<NoteEffects> {
        let mut effects = NoteEffects::default();
        if self.recovery.is_pending() {
            if self.recovery.operation_pending() {
                effects.diagnostic("恢复操作正在完成，请稍候。");
                effects.push(NoteEffect::Redraw);
            } else if primary {
                self.restore_temporary(&mut effects);
            } else {
                self.use_canonical(&mut effects);
            }
            return Some(effects);
        }
        if self.persistence.conflict().is_some() {
            if self.persistence.keep_local_in_progress() {
                effects.diagnostic("正在保存本地内容，请稍候。");
                effects.push(NoteEffect::Redraw);
            } else if primary {
                self.load_external_conflict(&mut effects);
            } else {
                self.keep_local_conflict(&mut effects);
            }
            return Some(effects);
        }
        None
    }
}
