//! What a Preview worker result may do. Only a build for the current document and viewport reaches
//! the admission rule, so an old receipt can neither show stale pixels nor move the view back
//! (plan 09 vertical scrollbars).
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling

use stickymd_core::Generation;
use stickymd_render::preview::{PreviewFrame, PreviewSelection};

use super::{PreviewAdmission, PreviewCoordinator};
use crate::ports::{PreviewCompletion, PreviewViewport};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompletionRoute {
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
fn completion_route(
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
pub fn current_preview_layout(
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

pub fn selection_for_generation(
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

/// What the host observed when a result arrived.
#[derive(Debug, Clone)]
pub struct PreviewFacts {
    pub hidden_to_tray: bool,
    pub current: Generation,
    /// Whether Preview is on screen in the current view mode.
    pub visible: bool,
    pub viewport: Option<PreviewViewport>,
}

/// The one thing left for the host after a result is applied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewFollowUp {
    Nothing,
    /// Repeat the idempotent worker-owned release of the document projection.
    ReleaseProjection,
    Rebuild(Generation),
    /// Lay the current document out again for the latest viewport.
    Relayout,
    Redraw,
    /// Show this diagnostic, then redraw.
    Report(String),
}

/// The Preview state a result may change, borrowed for one step.
pub struct PreviewIo<'a> {
    pub flow: &'a mut PreviewCoordinator,
    pub frame: &'a mut Option<PreviewFrame>,
    pub frame_viewport: &'a mut Option<PreviewViewport>,
    pub scroll_y: &'a mut f32,
    pub selection: &'a mut PreviewSelection,
}

impl PreviewIo<'_> {
    pub fn on_result(
        &mut self,
        completion: PreviewCompletion,
        facts: PreviewFacts,
    ) -> PreviewFollowUp {
        let current = facts.current;
        match completion_route(
            facts.hidden_to_tray,
            completion.generation,
            current,
            facts
                .viewport
                .as_ref()
                .map(|viewport| viewport == &completion.viewport),
            completion.result.is_ok(),
        ) {
            CompletionRoute::ReleaseHidden => {
                // A build can finish after the tray-hidden release effect. Never
                // re-admit that document-sized projection while the paper is not
                // visible.
                *self.frame = None;
                *self.frame_viewport = None;
                self.flow.release_projection();
                return PreviewFollowUp::ReleaseProjection;
            }
            CompletionRoute::Rebuild => {
                return if facts.visible {
                    PreviewFollowUp::Rebuild(current)
                } else {
                    PreviewFollowUp::Nothing
                };
            }
            CompletionRoute::Drop => return PreviewFollowUp::Nothing,
            CompletionRoute::Relayout { reuse_tree } => {
                // The worker owns a valid semantic tree after a successful build,
                // even when its pixels are already stale. Reuse it for the latest
                // viewport without displaying or hit-testing the stale frame.
                if reuse_tree {
                    self.flow.admit_completion(current, current);
                }
                return PreviewFollowUp::Relayout;
            }
            CompletionRoute::Admit => {}
        }
        let Some(viewport) = facts.viewport else {
            return PreviewFollowUp::Nothing;
        };
        match completion.result {
            Ok(frame)
                if self.flow.admit_completion(completion.generation, current)
                    == PreviewAdmission::Apply =>
            {
                *self.scroll_y = frame.scroll_y();
                *self.selection = selection_for_generation(
                    self.frame.as_ref().map(|previous| previous.generation()),
                    completion.generation,
                    *self.selection,
                );
                let text_len = frame.text().len();
                self.selection.anchor = self.selection.anchor.min(text_len);
                self.selection.active = self.selection.active.min(text_len);
                *self.frame_viewport = Some(viewport);
                *self.frame = Some(frame);
                PreviewFollowUp::Redraw
            }
            Ok(_) => PreviewFollowUp::Redraw,
            Err(error) if completion.generation == current => {
                PreviewFollowUp::Report(format!("Preview failed: {error}"))
            }
            Err(_) => PreviewFollowUp::Redraw,
        }
    }
}

#[cfg(test)]
mod tests;
