//! Preview-mode layout, scheduling, and worker-result coordination.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling
//! plan_ref: docs/plan/06_markdown_math_rendering.md#native-preview-layout

use stickymd_core::Generation;
use stickymd_render::preview::{PreviewSelection, PreviewTheme};
use tiny_skia::Pixmap;
use winit::dpi::PhysicalSize;

use super::search_controller::{SEARCH_COMPOSING, SearchViewChange, search_on_view_change};
use super::{AppEvent, StickyApp};
use crate::config::ViewMode;
use crate::flow::{PreviewAction, PreviewAdmission, PreviewEffect, PreviewVisibility};
use crate::instruction::PreviewIntent;
use crate::preview::{PreviewCompletion, PreviewJob, PreviewViewport, PreviewWorker};

pub(super) const TOOLBAR_HEIGHT_DIP: f32 = 34.0;

/// What a Preview worker completion may do. Only a build for the current document and viewport
/// reaches the admission rule, so an old receipt can neither show stale pixels nor move the
/// view back (plan 09 vertical scrollbars).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum CompletionRoute {
    /// The paper is hidden to the tray: release the projection; never re-admit it.
    ReleaseHidden,
    /// Built for an older document: rebuild the current one.
    Rebuild,
    /// There is no viewport to show it in.
    Drop,
    /// Built for another viewport: keep the semantic tree if the build succeeded, then relayout.
    Relayout { reuse_tree: bool },
    /// For the current document and viewport: offer it to the admission rule.
    Admit,
}

/// `viewport_matches` is `None` when there is no current viewport.
pub(super) fn completion_route(
    hidden_to_tray: bool,
    completed: Generation,
    current: Generation,
    viewport_matches: Option<bool>,
    succeeded: bool,
) -> CompletionRoute {
    if hidden_to_tray {
        CompletionRoute::ReleaseHidden
    } else if completed != current {
        CompletionRoute::Rebuild
    } else {
        match viewport_matches {
            None => CompletionRoute::Drop,
            Some(false) => CompletionRoute::Relayout {
                reuse_tree: succeeded,
            },
            Some(true) => CompletionRoute::Admit,
        }
    }
}

/// A cached layout may drive scroll anchors while only offset/selection changes.
/// Missing state, another generation, or any geometry/theme change forbids it.
pub(super) fn current_preview_layout(
    frame_generation: Option<Generation>,
    document_generation: Generation,
    applied: Option<PreviewViewport>,
    current: Option<PreviewViewport>,
) -> bool {
    frame_generation == Some(document_generation)
        && applied
            .zip(current)
            .is_some_and(|(applied, current)| applied.same_layout(&current))
}
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
        Some(self.view_geometry_for(size, scale))
    }

    /// Native size and DPI are observations; mode comes from the actual config.
    /// Content zoom and document length must not enter shell gutter geometry.
    pub(super) fn view_geometry_for(&self, size: PhysicalSize<u32>, dpi: f32) -> ViewGeometry {
        geometry(self.config.current().view_mode, size, dpi)
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
        // Search lives in the Source pane (plan 07). Decided before anything changes, so a
        // refusal leaves the view untouched; applied only once the new view is committed.
        let search = search_on_view_change(mode, self.search_place());
        if search == SearchViewChange::Refuse {
            self.diagnostic = Some(SEARCH_COMPOSING.into());
            self.request_redraw();
            return;
        }
        self.cancel_scrollbar_drag();
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
            self.preview_frame_viewport = None;
            self.preview_flow.release_projection();
            if let Some(worker) = &self.preview_worker {
                // Preserve the semantic tree for a fast return to Preview;
                // only tray-hidden cache release drops document-sized state.
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
        match search {
            SearchViewChange::Park => self.park_search(),
            SearchViewChange::Resume => self.resume_search(),
            SearchViewChange::Keep | SearchViewChange::Refuse => {}
        }
        self.sync_ime_allowed();
        self.submit_config_if_needed();
        self.request_redraw();
    }

    pub(super) fn on_preview_document_changed(&mut self, generation: Generation) {
        self.cancel_scrollbar_drag();
        self.preview_horizontal.offsets = Default::default();
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
        if frame.generation() != self.coordinator.view().generation {
            return;
        }
        let Some(viewport) = self.preview_viewport() else {
            return;
        };
        let generation = frame.generation();
        self.ensure_preview_worker();
        if let Some(worker) = &self.preview_worker {
            worker.submit(PreviewJob::Relayout {
                generation,
                viewport,
            });
        }
    }

    pub(super) fn handle_preview_completion(&mut self, completion: PreviewCompletion) {
        let hidden_to_tray = self.window_flow.as_ref().is_some_and(|flow| {
            matches!(
                flow.state().visibility(),
                crate::flow::window::state::VisibilityState::HiddenToTray { .. }
            )
        });
        let current = self.coordinator.view().generation;
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
        let current = self.coordinator.view().generation;
        match completion_route(
            hidden_to_tray,
            completion.generation,
            current,
            viewport
                .as_ref()
                .map(|viewport| viewport == &completion.viewport),
            completion.result.is_ok(),
        ) {
            CompletionRoute::ReleaseHidden => {
                // A build can finish after the tray-hidden release effect. Never
                // re-admit that document-sized projection while the paper is not
                // visible; repeat the idempotent worker-owned release.
                self.preview_frame = None;
                self.preview_frame_viewport = None;
                self.preview_flow.release_projection();
                if let Some(worker) = &self.preview_worker {
                    worker.release_document_projection();
                }
                return;
            }
            CompletionRoute::Rebuild => {
                if self.preview_visibility() != PreviewVisibility::Hidden {
                    self.submit_preview_action(PreviewAction::Build(current));
                }
                return;
            }
            CompletionRoute::Drop => return,
            CompletionRoute::Relayout { reuse_tree } => {
                // The worker owns a valid semantic tree after a successful build,
                // even when its pixels are already stale. Reuse it for the latest
                // viewport without displaying or hit-testing the stale frame.
                if reuse_tree {
                    self.preview_flow.admit_completion(current, current);
                }
                self.request_preview_relayout();
                return;
            }
            CompletionRoute::Admit => {}
        }
        let Some(viewport) = viewport else {
            return;
        };
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
                let text_len = frame.text().len();
                self.preview_selection.anchor = self.preview_selection.anchor.min(text_len);
                self.preview_selection.active = self.preview_selection.active.min(text_len);
                self.preview_frame_viewport = Some(viewport);
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
        self.preview_horizontal.offsets = geometry.preview.map_or_else(Default::default, |pane| {
            self.preview_horizontal.offsets.for_layout(
                self.coordinator.view().generation,
                pane.width.max(1),
                scale,
            )
        });
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
                let Some(viewport) = self.preview_viewport() else {
                    return;
                };
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

    pub(super) fn preview_layout_is_current(&self) -> bool {
        // Keep the expensive viewport projection lazy when no current frame exists.
        let generation = self.preview_frame.as_ref().map(|frame| frame.generation());
        let current = self.coordinator.view().generation;
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
                self.coordinator.view().generation,
                pane.width.max(1),
                scale,
            ),
            width_px: pane.width.max(1),
            height_px: pane.height.max(1),
            scale,
            scroll_y: self.preview_scroll_y,
            selection: selection_for_generation(
                self.preview_frame.as_ref().map(|frame| frame.generation()),
                self.coordinator.view().generation,
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
    let mut geometry = match mode {
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
    };
    let gutter = (super::scrollbar::GUTTER_DIP * scale.max(0.5)).round() as u32;
    for pane in [&mut geometry.source, &mut geometry.preview]
        .into_iter()
        .flatten()
    {
        pane.width = pane.width.saturating_sub(gutter).max(1);
    }
    geometry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_layout_requires_generation_and_every_layout_field_but_not_offset_or_selection() {
        let old = Generation::initial();
        let generation = old.checked_next().unwrap();
        let viewport = PreviewViewport {
            horizontal: Default::default(),
            width_px: 500,
            height_px: 400,
            scale: 1.25,
            scroll_y: 42.0,
            selection: PreviewSelection::default(),
            theme: PreviewTheme::Light,
        };
        for current in [
            viewport.clone(),
            PreviewViewport {
                scroll_y: 1000.0,
                selection: PreviewSelection::caret(5),
                ..viewport.clone()
            },
        ] {
            assert!(current_preview_layout(
                Some(generation),
                generation,
                Some(viewport.clone()),
                Some(current)
            ));
        }
        for frame_generation in [None, Some(old), Some(generation.checked_next().unwrap())] {
            assert!(!current_preview_layout(
                frame_generation,
                generation,
                Some(viewport.clone()),
                Some(viewport.clone())
            ));
        }
        assert!(!current_preview_layout(
            Some(generation),
            generation,
            None,
            Some(viewport.clone())
        ));
        assert!(!current_preview_layout(
            Some(generation),
            generation,
            Some(viewport.clone()),
            None
        ));
        for stale in [
            PreviewViewport {
                width_px: 499,
                ..viewport.clone()
            },
            PreviewViewport {
                height_px: 399,
                ..viewport.clone()
            },
            PreviewViewport {
                scale: 1.5,
                ..viewport.clone()
            },
            PreviewViewport {
                theme: PreviewTheme::Dark,
                ..viewport.clone()
            },
        ] {
            assert!(!current_preview_layout(
                Some(generation),
                generation,
                Some(stale.clone()),
                Some(viewport.clone())
            ));
            assert!(!current_preview_layout(
                Some(generation),
                generation,
                Some(viewport.clone()),
                Some(stale.clone())
            ));
        }
    }

    /// Only a completion for the current document and viewport is offered for display; the
    /// tray-hidden release wins over every other condition.
    #[test]
    fn completion_route_admits_only_the_current_build() {
        let current = Generation::initial().checked_next().unwrap();
        let older = Generation::initial();
        for generation in [current, older] {
            for viewport in [None, Some(false), Some(true)] {
                for succeeded in [true, false] {
                    assert_eq!(
                        completion_route(true, generation, current, viewport, succeeded),
                        CompletionRoute::ReleaseHidden
                    );
                }
            }
        }
        for viewport in [None, Some(false), Some(true)] {
            assert_eq!(
                completion_route(false, older, current, viewport, true),
                CompletionRoute::Rebuild
            );
        }
        assert_eq!(
            completion_route(false, current, current, None, true),
            CompletionRoute::Drop
        );
        for succeeded in [true, false] {
            assert_eq!(
                completion_route(false, current, current, Some(false), succeeded),
                CompletionRoute::Relayout {
                    reuse_tree: succeeded
                }
            );
        }
        for succeeded in [true, false] {
            assert_eq!(
                completion_route(false, current, current, Some(true), succeeded),
                CompletionRoute::Admit
            );
        }
    }

    /// Plan 09: an old scroll request's background receipt never makes the new position jump
    /// back. The receipt carries the viewport it was built for (the worker echoes it), which
    /// differs from the current viewport in its offset alone while its layout is the same; the
    /// identity sees the difference, so the receipt is only re-laid out for the current
    /// viewport (reusing its tree when it built one) and never admitted for display. The receipt
    /// for the current offset is admitted.
    #[test]
    fn a_receipt_for_an_older_offset_is_relaid_out_and_never_shown() {
        let generation = Generation::initial().checked_next().unwrap();
        let current = PreviewViewport {
            horizontal: Default::default(),
            width_px: 500,
            height_px: 646,
            scale: 1.25,
            scroll_y: 1200.0,
            selection: PreviewSelection::default(),
            theme: PreviewTheme::Light,
        };
        for older in [0.0, 600.0, 1199.0, 1199.75, 1200.25, 4000.0] {
            let receipt = PreviewViewport {
                scroll_y: older,
                ..current.clone()
            };
            assert!(receipt.same_layout(&current), "{older}");
            assert_ne!(
                receipt, current,
                "the offset alone tells them apart: {older}"
            );
            for succeeded in [true, false] {
                assert_eq!(
                    completion_route(
                        false,
                        generation,
                        generation,
                        Some(receipt == current),
                        succeeded
                    ),
                    CompletionRoute::Relayout {
                        reuse_tree: succeeded
                    },
                    "{older}"
                );
            }
        }
        assert_eq!(
            completion_route(
                false,
                generation,
                generation,
                Some(current == current),
                true
            ),
            CompletionRoute::Admit
        );
    }

    #[test]
    fn split_is_fixed_fifty_fifty_while_semantic_scroll_mapping_stays_separate() {
        let geometry = geometry(ViewMode::Split, PhysicalSize::new(901, 700), 1.0);
        let source = geometry.source.unwrap();
        let preview = geometry.preview.unwrap();
        assert_eq!(
            geometry.divider_x,
            Some(source.width + super::super::scrollbar::GUTTER_DIP as u32)
        );
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
            let gutter = (super::super::scrollbar::GUTTER_DIP * scale).round() as u32;
            assert_eq!(right.x, left.width + gutter + divider);
            assert!((left.width as i64 - right.width as i64).abs() <= 1);
            assert_eq!(left.width + divider + right.width + 2 * gutter, size.width);
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
