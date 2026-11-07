//! External-file reconciliation and the two ways out of a conflict.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use stickymd_core::{ExternalFileState, FileConflict};

use super::{NoteEffect, NoteEffects, NoteIo};
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
                    effects.diagnostic("外部文件已恢复为已知版本，冲突已解除。");
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
                        effects.diagnostic("已载入外部修改。");
                        effects.push(NoteEffect::RefreshTitle);
                        effects.push(NoteEffect::Redraw);
                    }
                    Err(error) => effects.diagnostic(format!("外部内容协调失败：{error}")),
                }
                return;
            }
            ReconciliationAction::ConflictChanged => {}
        }
        effects.diagnostic(conflict_diagnostic(self.persistence.conflict()));
        effects.push(NoteEffect::RefreshTitle);
        effects.push(NoteEffect::Redraw);
    }

    pub(super) fn load_external_conflict(&mut self, effects: &mut NoteEffects) {
        let Some(conflict) = self.persistence.conflict() else {
            return;
        };
        let ExternalFileState::Present(fact) = &conflict.external else {
            effects.diagnostic("外部文件不可载入；只能 F7 保留本地覆盖。");
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
                effects.diagnostic("已载入最新外部内容；撤销历史已清空。");
                effects.push(NoteEffect::RefreshTitle);
            }
            Err(error) => effects.diagnostic(format!("载入外部内容失败：{error}")),
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

fn conflict_diagnostic(conflict: Option<&FileConflict>) -> String {
    match conflict.map(|value| &value.external) {
        Some(ExternalFileState::Present(_)) => {
            "文件已在外部修改  |  [F6 载入外部]  [F7 保留本地]".into()
        }
        Some(ExternalFileState::InvalidUtf8 { .. }) => {
            "外部文件不是有效 UTF-8；F7 保留本地覆盖。".into()
        }
        Some(ExternalFileState::TooLarge { bytes }) => {
            format!("外部 note.md 过大（{bytes} bytes）；F7 保留本地覆盖。")
        }
        Some(ExternalFileState::Missing) | None => "外部文件状态冲突；F7 保留本地。".into(),
    }
}
