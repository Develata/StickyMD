//! The session's part of an event: focus changes, one event pass and an export request.
//!
//! plan_ref: docs/plan/03_system_architecture.md#interaction-shell
//! plan_ref: docs/plan/05_document_persistence.md#autosave-and-save-queue

use std::path::PathBuf;

use crate::flow::SaveTrigger;
use crate::flow::window::state::WindowIntent;
use crate::instruction::SaveReason;
use crate::ports::ExportRequest;

use super::turns::Turn;
use super::{AppSession, ShellEffect, ShellEffects};

impl AppSession {
    /// The shell ends what it owns first, and with it an opacity edit that commits on focus loss.
    pub fn on_focus_changed(&mut self, focused: bool, out: &mut ShellEffects) {
        self.editor.focused = focused;
        if !focused {
            out.push(ShellEffect::FocusLost);
            out.turn(Turn::FocusLoss);
        }
    }

    pub(super) fn end_focus(&mut self, out: &mut ShellEffects) {
        self.editor.cancel_preedit();
        // Focus loss ends either keyboard owner's provisional composition.
        // Do not depend on a later native Ime::Disabled event to release the guard.
        self.search.set_preedit(String::new(), None);
        if self.coordinator.view().dirty {
            self.save_now(SaveReason::FocusLoss, out);
        }
    }

    pub fn save_now(&mut self, reason: SaveReason, out: &mut ShellEffects) {
        self.request_save(
            match reason {
                SaveReason::Manual => SaveTrigger::Manual,
                SaveReason::FocusLoss => SaveTrigger::FocusLoss,
            },
            out,
        );
    }

    /// Everything due by `now_ms`: the note, then Preview, then the debounced zoom and the window.
    pub fn settle(&mut self, now_ms: u64, out: &mut ShellEffects) {
        let effects = self.note_io().tick(now_ms);
        Self::queue_note(effects, out);
        out.turn(Turn::PreviewTick { now_ms });
        out.turn(Turn::WindowTick { now_ms });
    }

    pub(super) fn tick_preview(&mut self, now_ms: u64, out: &mut ShellEffects) {
        if let Some(action) = self.preview_flow.tick(now_ms) {
            out.push(ShellEffect::SubmitPreview(action));
        }
    }

    pub(super) fn tick_window(&mut self, now_ms: u64, out: &mut ShellEffects) {
        if self
            .zoom_config_deadline
            .is_some_and(|deadline| now_ms >= deadline)
        {
            self.zoom_config_deadline = None;
            self.submit_config_if_needed();
        }
        self.dispatch_window_intent(WindowIntent::Tick { now_ms }, out);
    }

    /// Whether an export may ask where to go; only one runs at a time.
    pub fn begin_export(&mut self, out: &mut ShellEffects) -> bool {
        if self.export_in_flight {
            out.push(ShellEffect::Diagnostic("已有导出任务正在进行。".into()));
            out.push(ShellEffect::RedrawNeeded);
            return false;
        }
        true
    }

    pub fn submit_export(&mut self, target: PathBuf, out: &mut ShellEffects) {
        let request = ExportRequest {
            snapshot: self.coordinator.snapshot(),
            target,
        };
        if self.worker.submit_export(request) {
            self.export_in_flight = true;
            out.push(ShellEffect::Diagnostic("正在导出当前快照…".into()));
        } else {
            out.push(ShellEffect::Diagnostic("导出队列暂时不可用。".into()));
        }
        out.push(ShellEffect::RedrawNeeded);
    }
}
