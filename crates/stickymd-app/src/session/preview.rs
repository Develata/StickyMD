//! The Preview's schedule as the session keeps it: what a document change, a relayout, a shown
//! window or a hidden one asks of it, and the worker that builds it. The shell supplies the
//! viewport and submits what falls due, since the viewport is a view fact and the worker is
//! started with the shell's event sink and the host's image source.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling

use stickymd_core::Generation;
use stickymd_render::preview::{PreviewFrame, PreviewSelection};

use crate::execution::PreviewWorker;
use crate::flow::{
    PreviewAction, PreviewEffect, PreviewFacts, PreviewFlowError, PreviewFollowUp, PreviewIo,
    PreviewVisibility,
};
use crate::instruction::PreviewIntent;
use crate::ports::{PreviewCompletion, PreviewViewport};

use super::AppSession;

impl AppSession {
    pub fn dispatch_preview_intent(
        &self,
        intent: PreviewIntent,
    ) -> Result<PreviewEffect, PreviewFlowError> {
        self.preview_flow.dispatch(intent)
    }

    /// The document moved to `generation`: what the Preview's schedule asks for now, if anything.
    pub fn preview_document_changed(&mut self, generation: Generation) -> Option<PreviewAction> {
        self.preview_flow
            .on_document_changed(self.now_ms(), generation, self.preview_visibility())
    }

    /// The Preview's viewport changed: a relayout of the current document, unless no Preview is
    /// shown.
    pub fn preview_relayout_action(&self) -> Option<PreviewAction> {
        if self.preview_visibility() == PreviewVisibility::Hidden {
            return None;
        }
        Some(
            self.preview_flow
                .viewport_action(self.coordinator.view().generation),
        )
    }

    /// The Preview becomes visible again (the window is shown, or the view gained a Preview): the
    /// build or relayout it needs for the current document, if any.
    pub fn show_preview_action(&mut self) -> Option<PreviewAction> {
        let generation = self.coordinator.view().generation;
        self.preview_flow
            .show(generation, self.preview_visibility())
    }

    /// The window is hidden to the tray: the Preview's document projection is dropped, in the
    /// schedule and in the worker.
    pub fn release_hidden_preview(&mut self) {
        self.preview_flow.release_projection();
        if let Some(worker) = &self.preview_worker {
            worker.release_document_projection();
        }
    }

    /// A worker result, judged by `PreviewIo::on_result` against the session's schedule and the
    /// Preview view state the shell owns and lends for this step. The shell carries out the
    /// follow-up.
    pub fn on_preview_result(
        &mut self,
        completion: PreviewCompletion,
        facts: PreviewFacts,
        frame: &mut Option<PreviewFrame>,
        frame_viewport: &mut Option<PreviewViewport>,
        scroll_y: &mut f32,
        selection: &mut PreviewSelection,
    ) -> PreviewFollowUp {
        PreviewIo {
            flow: &mut self.preview_flow,
            frame,
            frame_viewport,
            scroll_y,
            selection,
        }
        .on_result(completion, facts)
    }

    pub fn preview_worker(&self) -> Option<&PreviewWorker> {
        self.preview_worker.as_ref()
    }

    /// The worker the shell started once `preview_worker` found none.
    pub fn install_preview_worker(&mut self, worker: PreviewWorker) {
        self.preview_worker = Some(worker);
    }
}
