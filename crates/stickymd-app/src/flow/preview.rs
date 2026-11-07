//! Deterministic preview generation scheduling and stale-result admission.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling

use stickymd_core::Generation;
use stickymd_render::preview::{LinkKind, SpanAction};
use thiserror::Error;

use crate::config::ViewMode;
use crate::instruction::PreviewIntent;

pub const PREVIEW_DEBOUNCE_MS: u64 = 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewVisibility {
    Hidden,
    Split,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewAction {
    Build(Generation),
    Relayout(Generation),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PreviewAdmission {
    Apply,
    DropStale,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewEffect {
    ApplyViewMode(ViewMode),
    OpenTarget { destination: String, kind: LinkKind },
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum PreviewFlowError {
    #[error("preview destination uses a blocked URI scheme")]
    BlockedScheme,
}

#[derive(Debug, Default)]
pub struct PreviewCoordinator {
    dirty_generation: Option<Generation>,
    scheduled: Option<(Generation, u64)>,
    applied_generation: Option<Generation>,
}

impl PreviewCoordinator {
    pub fn dispatch(&self, intent: PreviewIntent) -> Result<PreviewEffect, PreviewFlowError> {
        match intent {
            PreviewIntent::SetViewMode(mode) => Ok(PreviewEffect::ApplyViewMode(mode)),
            PreviewIntent::Activate(SpanAction::OpenLink { destination, kind }) => {
                if !kind.may_open() {
                    return Err(PreviewFlowError::BlockedScheme);
                }
                Ok(PreviewEffect::OpenTarget { destination, kind })
            }
            PreviewIntent::Activate(SpanAction::RemoteImageLink { destination, kind }) => {
                if !matches!(kind, LinkKind::Http | LinkKind::Https) {
                    return Err(PreviewFlowError::BlockedScheme);
                }
                Ok(PreviewEffect::OpenTarget { destination, kind })
            }
        }
    }

    pub fn on_document_changed(
        &mut self,
        now_ms: u64,
        generation: Generation,
        visibility: PreviewVisibility,
    ) -> Option<PreviewAction> {
        self.dirty_generation = Some(generation);
        match visibility {
            PreviewVisibility::Hidden => {
                self.scheduled = None;
                None
            }
            PreviewVisibility::Split => {
                self.scheduled = now_ms
                    .checked_add(PREVIEW_DEBOUNCE_MS)
                    .map(|deadline| (generation, deadline));
                None
            }
            PreviewVisibility::Preview => {
                self.scheduled = None;
                Some(PreviewAction::Build(generation))
            }
        }
    }

    pub fn show(
        &mut self,
        generation: Generation,
        visibility: PreviewVisibility,
    ) -> Option<PreviewAction> {
        if visibility == PreviewVisibility::Hidden {
            return None;
        }
        self.scheduled = None;
        if self.applied_generation == Some(generation) && self.dirty_generation != Some(generation)
        {
            Some(PreviewAction::Relayout(generation))
        } else {
            Some(PreviewAction::Build(generation))
        }
    }

    pub fn tick(&mut self, now_ms: u64) -> Option<PreviewAction> {
        let (generation, deadline) = self.scheduled?;
        if now_ms < deadline {
            return None;
        }
        self.scheduled = None;
        Some(PreviewAction::Build(generation))
    }

    pub fn deadline(&self) -> Option<u64> {
        self.scheduled.map(|(_, deadline)| deadline)
    }

    pub fn admit_completion(
        &mut self,
        completed: Generation,
        current: Generation,
    ) -> PreviewAdmission {
        if completed != current {
            return PreviewAdmission::DropStale;
        }
        self.applied_generation = Some(completed);
        if self.dirty_generation == Some(completed) {
            self.dirty_generation = None;
        }
        PreviewAdmission::Apply
    }

    pub fn applied_generation(&self) -> Option<Generation> {
        self.applied_generation
    }

    pub fn release_projection(&mut self) {
        self.applied_generation = None;
        self.scheduled = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn next(generation: Generation) -> Generation {
        generation.checked_next().unwrap()
    }

    #[test]
    fn split_resets_fixed_one_second_debounce() {
        let mut flow = PreviewCoordinator::default();
        let first = next(Generation::initial());
        let second = next(first);
        assert_eq!(
            flow.on_document_changed(0, first, PreviewVisibility::Split),
            None
        );
        assert_eq!(flow.tick(999), None);
        flow.on_document_changed(500, second, PreviewVisibility::Split);
        assert_eq!(flow.tick(1_499), None);
        assert_eq!(flow.tick(1_500), Some(PreviewAction::Build(second)));
    }

    #[test]
    fn one_hundred_rapid_split_edits_emit_only_the_latest_build() {
        let mut flow = PreviewCoordinator::default();
        let mut generation = Generation::initial();
        for now_ms in 0..100 {
            generation = next(generation);
            assert_eq!(
                flow.on_document_changed(now_ms, generation, PreviewVisibility::Split),
                None
            );
        }

        assert_eq!(flow.tick(1_098), None);
        assert_eq!(flow.tick(1_099), Some(PreviewAction::Build(generation)));
        assert_eq!(flow.tick(2_000), None);
    }

    #[test]
    fn preview_entry_and_preview_visible_changes_are_immediate() {
        let mut flow = PreviewCoordinator::default();
        let generation = next(Generation::initial());
        assert_eq!(
            flow.show(generation, PreviewVisibility::Preview),
            Some(PreviewAction::Build(generation))
        );
        assert_eq!(
            flow.on_document_changed(0, generation, PreviewVisibility::Preview),
            Some(PreviewAction::Build(generation))
        );
    }

    #[test]
    fn clean_preview_relayouts_when_a_visible_mode_change_alters_its_viewport() {
        let mut flow = PreviewCoordinator::default();
        let generation = next(Generation::initial());
        assert_eq!(
            flow.admit_completion(generation, generation),
            PreviewAdmission::Apply
        );

        assert_eq!(
            flow.show(generation, PreviewVisibility::Preview),
            Some(PreviewAction::Relayout(generation))
        );
        assert_eq!(
            flow.show(generation, PreviewVisibility::Split),
            Some(PreviewAction::Relayout(generation))
        );
    }

    #[test]
    fn stale_completion_cannot_replace_current_preview() {
        let mut flow = PreviewCoordinator::default();
        let old = Generation::initial();
        let current = next(old);
        assert_eq!(
            flow.admit_completion(old, current),
            PreviewAdmission::DropStale
        );
        assert_eq!(flow.applied_generation(), None);
        assert_eq!(
            flow.admit_completion(current, current),
            PreviewAdmission::Apply
        );
        assert_eq!(flow.applied_generation(), Some(current));
    }

    #[test]
    fn hidden_edits_remain_dirty_until_preview_is_shown() {
        let mut flow = PreviewCoordinator::default();
        let generation = next(Generation::initial());
        flow.on_document_changed(0, generation, PreviewVisibility::Hidden);
        assert_eq!(
            flow.show(generation, PreviewVisibility::Preview),
            Some(PreviewAction::Build(generation))
        );
    }

    #[test]
    fn releasing_projection_drops_applied_and_scheduled_state_but_keeps_dirty_work() {
        let mut flow = PreviewCoordinator::default();
        let generation = next(Generation::initial());
        flow.on_document_changed(0, generation, PreviewVisibility::Split);
        assert!(flow.deadline().is_some());
        assert_eq!(
            flow.admit_completion(generation, generation),
            PreviewAdmission::Apply
        );
        flow.on_document_changed(1, generation, PreviewVisibility::Split);
        flow.release_projection();
        assert_eq!(flow.applied_generation(), None);
        assert_eq!(flow.deadline(), None);
        assert_eq!(
            flow.show(generation, PreviewVisibility::Preview),
            Some(PreviewAction::Build(generation))
        );
    }

    #[test]
    fn link_activation_is_validated_before_shell_effects_are_emitted() {
        let flow = PreviewCoordinator::default();
        assert!(matches!(
            flow.dispatch(PreviewIntent::Activate(SpanAction::OpenLink {
                destination: "javascript:alert(1)".into(),
                kind: LinkKind::Blocked,
            })),
            Err(PreviewFlowError::BlockedScheme)
        ));
        assert_eq!(
            flow.dispatch(PreviewIntent::Activate(SpanAction::OpenLink {
                destination: "https://example.com".into(),
                kind: LinkKind::Https,
            })),
            Ok(PreviewEffect::OpenTarget {
                destination: "https://example.com".into(),
                kind: LinkKind::Https,
            })
        );
    }
}
