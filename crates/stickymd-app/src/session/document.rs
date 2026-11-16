//! What a document change means for everything projected from it, in the order it happens:
//! persistence, Preview, the editor session and Search, the Source projection, the title, assets.
//!
//! plan_ref: docs/plan/03_system_architecture.md#flow-coordination

use crate::flow::AppEffect;
use crate::flow::notice::Notice;
use crate::instruction::AppIntent;
use crate::interaction::rules::clamp_selection;
use stickymd_core::{DocumentSnapshot, Generation, TextDelta};

use super::turns::Turn;
use super::{AppSession, ShellEffect, ShellEffects};

/// How the Source projection follows the document.
#[derive(Debug)]
pub enum ProjectionChange {
    Delta {
        generation: Generation,
        delta: TextDelta,
    },
    FullResync(DocumentSnapshot),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalReconcileFollowUp {
    FullProjectionResync,
    RuntimeAssetConvergence,
}

fn external_reconcile_followups(
    reconciled: Generation,
    current: Generation,
) -> [ExternalReconcileFollowUp; 2] {
    debug_assert_eq!(reconciled, current);
    [
        ExternalReconcileFollowUp::FullProjectionResync,
        ExternalReconcileFollowUp::RuntimeAssetConvergence,
    ]
}

impl AppSession {
    /// A read-only copy of the document, for the rebuild a failed delta needs.
    pub fn snapshot(&self) -> DocumentSnapshot {
        self.coordinator.snapshot()
    }

    pub fn dispatch(&mut self, intent: AppIntent, out: &mut ShellEffects) {
        match self.coordinator.dispatch(intent) {
            Ok(effect) => self.apply_app_effect(effect, out),
            Err(error) => self.show_notice(Notice::editor_failure(&error), out),
        }
    }

    pub fn apply_app_effect(&mut self, effect: AppEffect, out: &mut ShellEffects) {
        match effect {
            AppEffect::DocumentChanged {
                generation,
                selection,
                delta,
                asset_effects,
            } => {
                self.persistence
                    .on_document_changed(self.now_ms(), generation);
                out.push(ShellEffect::PreviewDocumentChanged(generation));
                out.turn(Turn::DocumentAccepted {
                    generation,
                    selection,
                });
                out.push(ShellEffect::Projection(ProjectionChange::Delta {
                    generation,
                    delta,
                }));
                out.push(ShellEffect::PresentationChanged);
                out.push(ShellEffect::TitleDirty);
                if !asset_effects.is_empty() {
                    out.turn(Turn::AssetEffects {
                        generation,
                        effects: asset_effects,
                    });
                }
            }
            AppEffect::AssetPasteRequested(request) => self.submit_asset_paste(request, out),
            AppEffect::ExternalDocumentReconciled { generation } => {
                for followup in
                    external_reconcile_followups(generation, self.coordinator.view().generation)
                {
                    match followup {
                        ExternalReconcileFollowUp::FullProjectionResync => {
                            self.resync_projection(out);
                        }
                        ExternalReconcileFollowUp::RuntimeAssetConvergence => {
                            out.turn(Turn::RuntimeAssetConvergence);
                        }
                    }
                }
            }
            // A copy shows "copied" for 3 s, never over a failure (plan 07, plan 04 push rule).
            AppEffect::ClipboardWritten => self.show_notice(Notice::Copied, out),
            AppEffect::NoOp => {}
        }
    }

    /// The document was replaced outside the editor: everything projected from it is rebuilt.
    pub(super) fn resync_projection(&mut self, out: &mut ShellEffects) {
        let snapshot = self.coordinator.snapshot();
        let generation = snapshot.generation;
        self.editor.selection = clamp_selection(&snapshot.text, self.editor.selection);
        self.search.refresh(&snapshot.text, generation);
        out.push(ShellEffect::Projection(ProjectionChange::FullResync(
            snapshot,
        )));
        out.push(ShellEffect::PreviewDocumentChanged(generation));
        out.push(ShellEffect::PresentationChanged);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_reconcile_handler_requires_projection_resync_and_asset_convergence() {
        let generation = Generation::initial();
        assert_eq!(
            external_reconcile_followups(generation, generation),
            [
                ExternalReconcileFollowUp::FullProjectionResync,
                ExternalReconcileFollowUp::RuntimeAssetConvergence,
            ]
        );
    }
}
