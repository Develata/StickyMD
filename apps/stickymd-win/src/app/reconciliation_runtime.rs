//! External-file reconciliation presentation bridge.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use stickymd_core::{ExternalFileState, FileConflict, Selection};

use super::StickyApp;
use crate::flow::{ReconciliationAction, SaveTrigger};
use crate::persistence::{PersistMode, TemporaryCleanup};

impl StickyApp {
    pub(super) fn reconcile_external(&mut self, external: ExternalFileState) {
        if self.recovery.is_pending() {
            return;
        }
        let (dirty, base_disk_hash, generation) = {
            let view = self.coordinator.view();
            (view.dirty, view.base_disk_hash, view.generation)
        };
        let had_conflict = self.persistence.conflict().is_some();
        match self
            .persistence
            .observe_external(dirty, base_disk_hash, generation, external)
        {
            ReconciliationAction::IgnoreKnown => {
                if had_conflict {
                    self.diagnostic = Some("外部文件已恢复为已知版本，冲突已解除。".into());
                    if dirty {
                        self.submit_save(SaveTrigger::Debounce, None);
                    }
                    self.update_window_title();
                    self.request_redraw();
                }
                return;
            }
            ReconciliationAction::RecreateMissing => {
                self.submit_save(
                    SaveTrigger::RecreateMissing,
                    Some(PersistMode::Guarded { expected: None }),
                );
                return;
            }
            ReconciliationAction::ReloadClean(fact) => {
                match self.coordinator.reconcile_external(&fact) {
                    Ok(_) => {
                        self.persistence.confirm_durable_present();
                        self.persistence.clear_conflict();
                        self.full_projection_resync();
                        self.diagnostic = Some("已载入外部修改。".into());
                        self.update_window_title();
                        self.request_redraw();
                    }
                    Err(error) => self.diagnostic = Some(format!("外部内容协调失败：{error}")),
                }
                return;
            }
            ReconciliationAction::ConflictChanged => {}
        }
        self.diagnostic = Some(conflict_diagnostic(self.persistence.conflict()));
        self.update_window_title();
        self.request_redraw();
    }

    pub(super) fn full_projection_resync(&mut self) {
        let snapshot = self.coordinator.snapshot();
        self.session.selection = clamp_selection(&snapshot.text, self.session.selection);
        if let Some(projection) = &mut self.projection {
            let _ = projection.resync(&snapshot);
        }
        self.on_preview_document_changed(snapshot.generation);
        self.after_presentation_change();
    }

    pub(super) fn load_external_conflict(&mut self) {
        let Some(conflict) = self.persistence.conflict() else {
            return;
        };
        let ExternalFileState::Present(fact) = &conflict.external else {
            self.diagnostic = Some("外部文件不可载入；只能 F7 保留本地覆盖。".into());
            return;
        };
        let fact = fact.clone();
        match self.coordinator.reconcile_external(&fact) {
            Ok(_) => {
                self.persistence.clear_conflict();
                self.full_projection_resync();
                self.worker.remove_temporary(
                    self.paths.note_tmp.clone(),
                    TemporaryCleanup::ConflictDiscarded,
                );
                self.diagnostic = Some("已载入最新外部内容；撤销历史已清空。".into());
                self.update_window_title();
            }
            Err(error) => self.diagnostic = Some(format!("载入外部内容失败：{error}")),
        }
        self.request_redraw();
    }

    pub(super) fn keep_local_conflict(&mut self) {
        self.resolving_keep_local = true;
        self.submit_save(SaveTrigger::KeepLocal, Some(PersistMode::ForceOverwrite));
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

fn clamp_selection(text: &str, selection: Selection) -> Selection {
    fn clamp(text: &str, mut byte: usize) -> usize {
        byte = byte.min(text.len());
        while !text.is_char_boundary(byte) {
            byte -= 1;
        }
        byte
    }
    Selection::new(
        clamp(text, selection.anchor.byte),
        clamp(text, selection.active.byte),
    )
}
