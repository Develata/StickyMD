//! Preview-mode layout, scheduling, and worker-result coordination.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use stickymd_core::Generation;
use stickymd_render::preview::{PreviewSelection, PreviewTheme};
use tiny_skia::Pixmap;
use winit::dpi::PhysicalSize;

use super::{AppEvent, StickyApp};
use crate::config::ViewMode;
use crate::flow::{PreviewAction, PreviewAdmission, PreviewEffect, PreviewVisibility};
use crate::instruction::PreviewIntent;
use crate::preview::{PreviewCompletion, PreviewJob, PreviewViewport, PreviewWorker};

pub(super) const TOOLBAR_HEIGHT_DIP: f32 = 34.0;
const SPLIT_DIVIDER_DIP: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PaneRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PaneRect {
    pub fn contains(self, x: f32, y: f32) -> bool {
        x >= self.x as f32
            && x < self.x.saturating_add(self.width) as f32
            && y >= self.y as f32
            && y < self.y.saturating_add(self.height) as f32
    }
}

#[derive(Debug, Clone, Copy)]
pub(super) struct ViewGeometry {
    pub toolbar_height: u32,
    pub source: Option<PaneRect>,
    pub preview: Option<PaneRect>,
    pub divider_x: Option<u32>,
}

impl StickyApp {
    pub(super) fn view_geometry(&self) -> Option<ViewGeometry> {
        let window = self.window.as_ref()?;
        let size = window.inner_size();
        let scale = window.scale_factor() as f32;
        Some(geometry(self.config.current().view_mode, size, scale))
    }

    pub(super) fn preview_visibility(&self) -> PreviewVisibility {
        match self.config.current().view_mode {
            ViewMode::Source => PreviewVisibility::Hidden,
            ViewMode::Split => PreviewVisibility::Split,
            ViewMode::Preview => PreviewVisibility::Preview,
        }
    }

    pub(super) fn dispatch_preview_intent(&mut self, intent: PreviewIntent) {
        match self.preview_flow.dispatch(intent) {
            Ok(PreviewEffect::ApplyViewMode(mode)) => self.apply_view_mode(mode),
            Ok(PreviewEffect::OpenTarget { destination, kind }) => {
                if let Err(error) = crate::platform::windows::shell::open_target(
                    &destination,
                    kind,
                    &self.paths.note_dir,
                ) {
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
        if self.config.current().view_mode == mode {
            return;
        }
        self.session.cancel_preedit();
        self.dispatch_window_intent(
            None,
            crate::flow::window::state::WindowIntent::SplitModeChanged {
                split: mode == ViewMode::Split,
            },
        );
        if let Err(error) = self.config.update(|config| config.view_mode = mode) {
            self.diagnostic = Some(error.to_string());
            return;
        }
        if mode == ViewMode::Source {
            self.preview_frame = None;
            self.preview_flow.release_projection();
            if let Some(worker) = &self.preview_worker {
                worker.release_raster_caches();
            }
        }
        self.preview_focused = mode == ViewMode::Preview;
        self.preview_selection = PreviewSelection::default();
        self.sync_preedit();
        self.configure_viewports();
        let generation = self.coordinator.view().generation;
        if let Some(action) = self
            .preview_flow
            .show(generation, self.preview_visibility())
        {
            self.submit_preview_action(action);
        }
        if let Some(window) = &self.window {
            window.set_ime_allowed(mode != ViewMode::Preview && self.session.focused);
        }
        self.submit_config_if_needed();
        self.request_redraw();
    }

    pub(super) fn on_preview_document_changed(&mut self, generation: Generation) {
        if let Some(action) = self.preview_flow.on_document_changed(
            self.timestamp_ms(),
            generation,
            self.preview_visibility(),
        ) {
            self.submit_preview_action(action);
        }
    }

    pub(super) fn tick_preview(&mut self, now_ms: u64) {
        if let Some(action) = self.preview_flow.tick(now_ms) {
            self.submit_preview_action(action);
        }
    }

    pub(super) fn preview_deadline(&self) -> Option<u64> {
        self.preview_flow.deadline()
    }

    pub(super) fn request_preview_relayout(&mut self) {
        if self.preview_visibility() == PreviewVisibility::Hidden {
            return;
        }
        let generation = self.coordinator.view().generation;
        if self.preview_flow.applied_generation() == Some(generation) {
            if let Some(viewport) = self.preview_viewport() {
                self.ensure_preview_worker();
                if let Some(worker) = &self.preview_worker {
                    worker.submit(PreviewJob::Relayout {
                        generation,
                        viewport,
                    });
                }
            }
        } else {
            self.submit_preview_action(PreviewAction::Build(generation));
        }
    }

    pub(super) fn request_preview_paint(&mut self) {
        let Some(frame) = &self.preview_frame else {
            return;
        };
        let Some(viewport) = self.preview_viewport() else {
            return;
        };
        let generation = frame.generation();
        self.ensure_preview_worker();
        if let Some(worker) = &self.preview_worker {
            worker.submit(PreviewJob::Paint {
                generation,
                height_px: viewport.height_px,
                scroll_y: viewport.scroll_y,
                selection: viewport.selection,
                theme: viewport.theme,
            });
        }
    }

    pub(super) fn handle_preview_completion(&mut self, completion: PreviewCompletion) {
        let current = self.coordinator.view().generation;
        if completion.generation != current {
            if self.preview_visibility() != PreviewVisibility::Hidden {
                self.submit_preview_action(PreviewAction::Build(current));
            }
            return;
        }
        match completion.result {
            Ok(frame)
                if self
                    .preview_flow
                    .admit_completion(completion.generation, current)
                    == PreviewAdmission::Apply =>
            {
                self.preview_scroll_y = frame.scroll_y();
                self.preview_selection = selection_for_generation(
                    self.preview_frame
                        .as_ref()
                        .map(|previous| previous.generation()),
                    completion.generation,
                    self.preview_selection,
                );
                let text_len = frame.index().text().len();
                self.preview_selection.anchor = self.preview_selection.anchor.min(text_len);
                self.preview_selection.active = self.preview_selection.active.min(text_len);
                self.preview_frame = Some(frame);
            }
            Ok(_) => {}
            Err(error) if completion.generation == current => {
                self.diagnostic = Some(format!("Preview failed: {error}"));
            }
            Err(_) => {}
        }
        self.request_redraw();
    }

    pub(super) fn configure_viewports(&mut self) {
        let Some(geometry) = self.view_geometry() else {
            return;
        };
        let scale = self.document_scale_factor();
        if let (Some(source), Some(projection)) = (geometry.source, &mut self.projection) {
            projection.set_viewport(source.width.max(1), source.height.max(1), scale);
            if self.source_frame.as_ref().is_none_or(|frame| {
                frame.width() != source.width || frame.height() != source.height
            }) {
                self.source_frame = Pixmap::new(source.width.max(1), source.height.max(1));
                self.source_paint_key = None;
            }
        }
    }

    pub(super) fn submit_preview_action(&mut self, action: PreviewAction) {
        match action {
            PreviewAction::Build(generation) => {
                let snapshot = self.coordinator.snapshot();
                if snapshot.generation != generation {
                    return;
                }
                let Some(mut viewport) = self.preview_viewport() else {
                    return;
                };
                viewport.selection = selection_for_generation(
                    self.preview_frame.as_ref().map(|frame| frame.generation()),
                    generation,
                    viewport.selection,
                );
                self.ensure_preview_worker();
                if let Some(worker) = &self.preview_worker {
                    worker.submit(PreviewJob::Build { snapshot, viewport });
                }
            }
            PreviewAction::Relayout(generation) => {
                let Some(viewport) = self.preview_viewport() else {
                    return;
                };
                self.ensure_preview_worker();
                if let Some(worker) = &self.preview_worker {
                    worker.submit(PreviewJob::Relayout {
                        generation,
                        viewport,
                    });
                }
            }
        }
    }

    fn ensure_preview_worker(&mut self) {
        if self.preview_worker.is_some() {
            return;
        }
        let proxy = self.proxy.clone();
        match PreviewWorker::start_with_image_base(self.paths.note_dir.clone(), move |completion| {
            let _ = proxy.send_event(AppEvent::Preview(completion));
        }) {
            Ok(worker) => self.preview_worker = Some(worker),
            Err(error) => self.diagnostic = Some(format!("Preview worker unavailable: {error}")),
        }
    }

    fn preview_viewport(&self) -> Option<PreviewViewport> {
        let pane = self.view_geometry()?.preview?;
        let scale = self.document_scale_factor();
        Some(PreviewViewport {
            width_px: pane.width.max(1),
            height_px: pane.height.max(1),
            scale,
            scroll_y: self.preview_scroll_y,
            selection: self.preview_selection,
            theme: if self.resolved_dark_theme() {
                PreviewTheme::Dark
            } else {
                PreviewTheme::Light
            },
        })
    }
}

fn selection_for_generation(
    previous: Option<Generation>,
    target: Generation,
    selection: PreviewSelection,
) -> PreviewSelection {
    if previous == Some(target) {
        selection
    } else {
        PreviewSelection::default()
    }
}

pub(super) fn geometry(mode: ViewMode, size: PhysicalSize<u32>, scale: f32) -> ViewGeometry {
    let toolbar_height = (TOOLBAR_HEIGHT_DIP * scale.max(0.5)).round().max(1.0) as u32;
    let content_y = toolbar_height.min(size.height);
    let content_height = size.height.saturating_sub(content_y).max(1);
    let full = PaneRect {
        x: 0,
        y: content_y,
        width: size.width.max(1),
        height: content_height,
    };
    match mode {
        ViewMode::Source => ViewGeometry {
            toolbar_height,
            source: Some(full),
            preview: None,
            divider_x: None,
        },
        ViewMode::Preview => ViewGeometry {
            toolbar_height,
            source: None,
            preview: Some(full),
            divider_x: None,
        },
        ViewMode::Split => {
            let divider = (SPLIT_DIVIDER_DIP * scale.max(0.5)).round().max(1.0) as u32;
            let available = size.width.saturating_sub(divider);
            let source_width = available / 2;
            let preview_width = available.saturating_sub(source_width);
            ViewGeometry {
                toolbar_height,
                source: Some(PaneRect {
                    width: source_width.max(1),
                    ..full
                }),
                preview: Some(PaneRect {
                    x: source_width.saturating_add(divider),
                    width: preview_width.max(1),
                    ..full
                }),
                divider_x: Some(source_width),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_is_fixed_fifty_fifty_and_source_preview_scroll_are_not_coupled() {
        let geometry = geometry(ViewMode::Split, PhysicalSize::new(901, 700), 1.0);
        let source = geometry.source.unwrap();
        let preview = geometry.preview.unwrap();
        assert_eq!(geometry.divider_x, Some(source.width));
        assert!((source.width as i64 - preview.width as i64).abs() <= 1);
        assert_eq!(source.y, preview.y);
        assert_eq!(source.height, preview.height);
    }

    #[test]
    fn toolbar_is_reserved_in_every_view_mode() {
        for mode in [ViewMode::Source, ViewMode::Split, ViewMode::Preview] {
            let geometry = geometry(mode, PhysicalSize::new(520, 680), 1.5);
            for pane in [geometry.source, geometry.preview].into_iter().flatten() {
                assert_eq!(pane.y, geometry.toolbar_height);
                assert_eq!(pane.height + pane.y, 680);
            }
        }
    }

    #[test]
    fn phase10_minimum_window_keeps_all_modes_operable_and_split_is_fifty_fifty() {
        for scale in [1.0_f32, 1.25, 1.5, 2.0] {
            let size = PhysicalSize::new(
                (220.0 * scale).round() as u32,
                (120.0 * scale).round() as u32,
            );
            let source = geometry(ViewMode::Source, size, scale)
                .source
                .expect("Source pane");
            let preview = geometry(ViewMode::Preview, size, scale)
                .preview
                .expect("Preview pane");
            assert!(source.width > 0 && source.height > 0);
            assert!(preview.width > 0 && preview.height > 0);

            let split = geometry(ViewMode::Split, size, scale);
            let left = split.source.expect("Split source pane");
            let right = split.preview.expect("Split preview pane");
            let divider = (SPLIT_DIVIDER_DIP * scale).round().max(1.0) as u32;
            assert_eq!(right.x, left.width + divider);
            assert!((left.width as i64 - right.width as i64).abs() <= 1);
            assert_eq!(left.width + divider + right.width, size.width);
            assert!(left.height > 0 && right.height > 0);
        }
    }

    #[test]
    fn new_preview_generation_clears_old_selection_but_repaint_keeps_it() {
        let old = Generation::initial();
        let new = old.checked_next().unwrap();
        let selection = PreviewSelection {
            anchor: 2,
            active: 8,
        };
        assert_eq!(
            selection_for_generation(Some(old), old, selection),
            selection
        );
        assert_eq!(
            selection_for_generation(Some(old), new, selection),
            PreviewSelection::default()
        );
    }
}
