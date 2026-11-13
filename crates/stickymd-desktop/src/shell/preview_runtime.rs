//! Preview-mode layout, scheduling, and worker-result coordination.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use stickymd_core::Generation;
use stickymd_render::preview::{PreviewSelection, PreviewTheme};
use tiny_skia::Pixmap;
use winit::dpi::PhysicalSize;

use super::DesktopShell;
use crate::geometry::{ViewGeometry, geometry};
use crate::host::{DesktopEvent, Host};
use stickymd_app::config::ViewMode;
use stickymd_app::execution::PreviewWorker;
use stickymd_app::flow::{
    PreviewAction, PreviewEffect, PreviewFacts, PreviewFollowUp, PreviewVisibility,
    current_preview_layout, selection_for_generation,
};
use stickymd_app::instruction::PreviewIntent;
use stickymd_app::ports::{PreviewCompletion, PreviewJob, PreviewViewport};

impl<H: Host> DesktopShell<H> {
    pub(super) fn view_geometry(&self) -> Option<ViewGeometry> {
        let (size, scale) = self.shell_view()?;
        Some(self.view_geometry_for(size, scale as f32))
    }

    /// Native size and DPI are observations; mode comes from the actual config.
    /// Content zoom and document length must not enter shell gutter geometry.
    pub(super) fn view_geometry_for(&self, size: PhysicalSize<u32>, dpi: f32) -> ViewGeometry {
        geometry(self.app.config().current().view_mode, size, dpi)
    }

    pub(super) fn dispatch_preview_intent(&mut self, intent: PreviewIntent) {
        match self.app.dispatch_preview_intent(intent) {
            Ok(PreviewEffect::ApplyViewMode(mode)) => self.apply_view_mode(mode),
            Ok(PreviewEffect::OpenTarget { destination, kind }) => {
                if let Err(error) = self.host.open_target(&destination, kind) {
                    self.diagnostic = Some(error.to_string());
                    self.request_redraw();
                }
            }
            Err(error) => {
                self.diagnostic = Some(error.to_string());
                self.request_redraw();
            }
        }
    }

    fn apply_view_mode(&mut self, mode: ViewMode) {
        let mut prepared = None;
        self.with_session(|app, _, out| prepared = app.begin_view_change(mode, out));
        let Some(prepared) = prepared else {
            return;
        };
        // The drags of the old view end, and refresh the guards, before the note sees the change.
        self.cancel_scrollbar_drag();
        self.with_session(|app, _, out| app.commit_view_change(prepared, out));
    }

    /// Lays the shell out for the view the session changed to and gives the keyboard to its pane.
    pub(super) fn lay_out_view(&mut self, mode: ViewMode) {
        self.sync_control_layout();
        if mode == ViewMode::Source {
            self.preview_frame = None;
            self.preview_frame_viewport = None;
        }
        self.preview_focused = mode == ViewMode::Preview;
        self.preview_selection = PreviewSelection::default();
        self.sync_preedit();
        self.configure_viewports();
    }

    pub(super) fn on_preview_document_changed(&mut self, generation: Generation) {
        // A document change resets the horizontal offsets and so ends a horizontal drag (plan 06).
        // It does not itself end a vertical scrollbar drag (plan 09: release, blur, a view switch,
        // a resize); a Preview frame it makes stale shows no thumb, which ends that pane's drag at
        // the next pointer move.
        if self.preview_horizontal.drag.take().is_some() {
            self.refresh_window_guards();
            self.request_redraw();
        }
        self.preview_horizontal.offsets = Default::default();
        if let Some(action) = self.app.preview_document_changed(generation) {
            self.submit_preview_action(action);
        }
    }

    pub(super) fn request_preview_relayout(&mut self) {
        if let Some(action) = self.app.preview_relayout_action() {
            self.submit_preview_action(action);
        }
    }

    pub(super) fn request_preview_paint(&mut self) {
        let Some(frame) = &self.preview_frame else {
            return;
        };
        if frame.generation() != self.app.coordinator().view().generation {
            return;
        }
        let Some(viewport) = self.preview_viewport() else {
            return;
        };
        let generation = frame.generation();
        self.ensure_preview_worker();
        if let Some(worker) = self.app.preview_worker() {
            worker.submit(PreviewJob::Relayout {
                generation,
                viewport,
            });
        }
    }

    pub(super) fn handle_preview_completion(&mut self, completion: PreviewCompletion) {
        let hidden_to_tray = self.app.hidden_to_tray();
        let current = self.app.coordinator().view().generation;
        // The viewport is read only for a visible completion of the current document.
        let viewport = (!hidden_to_tray && completion.generation == current)
            .then(|| self.preview_viewport())
            .flatten();
        self.apply_preview_completion(completion, hidden_to_tray, viewport);
    }

    /// Apply a worker result against current canonical state and observed viewport.
    /// The native wrapper obtains facts only; all admission and mutation stays here.
    fn apply_preview_completion(
        &mut self,
        completion: PreviewCompletion,
        hidden_to_tray: bool,
        viewport: Option<PreviewViewport>,
    ) {
        let facts = PreviewFacts {
            hidden_to_tray,
            current: self.app.coordinator().view().generation,
            visible: self.app.preview_visibility() != PreviewVisibility::Hidden,
            viewport,
        };
        let follow_up = self.app.on_preview_result(
            completion,
            facts,
            &mut self.preview_frame,
            &mut self.preview_frame_viewport,
            &mut self.preview_scroll_y,
            &mut self.preview_selection,
        );
        match follow_up {
            PreviewFollowUp::Nothing => {}
            PreviewFollowUp::ReleaseProjection => {
                if let Some(worker) = self.app.preview_worker() {
                    worker.release_document_projection();
                }
            }
            PreviewFollowUp::Rebuild(generation) => {
                self.submit_preview_action(PreviewAction::Build(generation));
            }
            PreviewFollowUp::Relayout => self.request_preview_relayout(),
            PreviewFollowUp::Redraw => self.request_redraw(),
            PreviewFollowUp::Report(message) => {
                self.diagnostic = Some(message);
                self.request_redraw();
            }
        }
    }

    pub(super) fn configure_viewports(&mut self) {
        let Some(geometry) = self.view_geometry() else {
            return;
        };
        let scale = self.document_scale_factor();
        self.preview_horizontal.offsets = geometry.preview.map_or_else(Default::default, |pane| {
            self.preview_horizontal.offsets.for_layout(
                self.app.coordinator().view().generation,
                pane.width.max(1),
                scale,
            )
        });
        if let (Some(source), Some(projection)) = (geometry.source, &mut self.projection) {
            projection.set_viewport(source.width.max(1), source.height.max(1), scale);
            if !self.frames_released
                && self.source_frame.as_ref().is_none_or(|frame| {
                    frame.width() != source.width || frame.height() != source.height
                })
            {
                self.source_frame = Pixmap::new(source.width.max(1), source.height.max(1));
                self.source_paint_key = None;
            }
        }
    }

    pub(super) fn submit_preview_action(&mut self, action: PreviewAction) {
        // Released frames mean a hidden window: a job now would lay out and paint a window-sized
        // pane for nothing. The show asks again for the current generation
        // (`show_preview_action`), and the schedule keeps no in-flight state to unblock.
        if self.frames_released {
            return;
        }
        match action {
            PreviewAction::Build(generation) => {
                let snapshot = self.app.coordinator().snapshot();
                if snapshot.generation != generation {
                    return;
                }
                let Some(viewport) = self.preview_viewport() else {
                    return;
                };
                self.ensure_preview_worker();
                if let Some(worker) = self.app.preview_worker() {
                    worker.submit(PreviewJob::Build { snapshot, viewport });
                }
            }
            PreviewAction::Relayout(generation) => {
                let Some(viewport) = self.preview_viewport() else {
                    return;
                };
                self.ensure_preview_worker();
                if let Some(worker) = self.app.preview_worker() {
                    worker.submit(PreviewJob::Relayout {
                        generation,
                        viewport,
                    });
                }
            }
        }
    }

    fn ensure_preview_worker(&mut self) {
        if self.app.preview_worker().is_some() {
            return;
        }
        let sink = self.sink.clone();
        match PreviewWorker::start(self.host.preview_images(), move |completion| {
            sink.post(DesktopEvent::Preview(completion));
        }) {
            Ok(worker) => self.app.install_preview_worker(worker),
            Err(error) => self.diagnostic = Some(format!("Preview worker unavailable: {error}")),
        }
    }

    pub(super) fn preview_layout_is_current(&self) -> bool {
        // Keep the expensive viewport projection lazy when no current frame exists.
        let generation = self.preview_frame.as_ref().map(|frame| frame.generation());
        let current = self.app.coordinator().view().generation;
        current_preview_layout(
            generation,
            current,
            self.preview_frame_viewport.clone(),
            (generation == Some(current))
                .then(|| self.preview_viewport())
                .flatten(),
        )
    }

    pub(super) fn preview_viewport(&self) -> Option<PreviewViewport> {
        let pane = self.view_geometry()?.preview?;
        let scale = self.document_scale_factor();
        Some(PreviewViewport {
            horizontal: self.preview_horizontal.offsets.for_layout(
                self.app.coordinator().view().generation,
                pane.width.max(1),
                scale,
            ),
            width_px: pane.width.max(1),
            height_px: pane.height.max(1),
            scale,
            scroll_y: self.preview_scroll_y,
            selection: selection_for_generation(
                self.preview_frame.as_ref().map(|frame| frame.generation()),
                self.app.coordinator().view().generation,
                self.preview_selection,
            ),
            theme: if self.resolved_dark_theme() {
                PreviewTheme::Dark
            } else {
                PreviewTheme::Light
            },
        })
    }
}

#[cfg(test)]
mod runtime_tests;
