//! Carries out the note decisions of `NoteIo`.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use crate::flow::window::state::{QuitBarrier, WindowIntent};
use crate::flow::{NoteEffect, NoteEffects, NoteIo, SaveTrigger};

use super::turns::Turn;
use super::{AppSession, SessionClipboard, ShellEffect, ShellEffects, ViewFacts};

impl AppSession {
    pub(super) fn note_io(&mut self) -> NoteIo<'_, SessionClipboard> {
        NoteIo::new(
            &mut self.coordinator,
            &mut self.persistence,
            &mut self.recovery,
        )
    }

    /// The note directory changed: the external check waits for the debounce, counted from now.
    pub fn note_directory_changed(&mut self) {
        self.persistence.on_watch_hint(self.now_ms());
    }

    pub fn request_save(&mut self, trigger: SaveTrigger, out: &mut ShellEffects) {
        let effects = self.note_io().request_save(trigger);
        Self::queue_note(effects, out);
    }

    /// Whether a conflict or a recovery choice consumed the key.
    pub fn resolve(&mut self, primary: bool, out: &mut ShellEffects) -> bool {
        let Some(effects) = self.note_io().resolve(primary) else {
            return false;
        };
        Self::queue_note(effects, out);
        true
    }

    pub(super) fn queue_note(effects: NoteEffects, out: &mut ShellEffects) {
        for effect in effects {
            out.turn(Turn::Note(effect));
        }
    }

    pub(super) fn apply_note_effect(
        &mut self,
        effect: NoteEffect,
        view: ViewFacts,
        out: &mut ShellEffects,
    ) {
        match effect {
            NoteEffect::SubmitSave(request) => {
                #[cfg(debug_assertions)]
                eprintln!("save requested generation={}", request.generation.value());
                self.worker.submit_note(request);
            }
            NoteEffect::InspectExternal { requested_base } => {
                self.worker.inspect_external(requested_base);
            }
            NoteEffect::RemoveTemporary(purpose) => self.worker.remove_temporary(purpose),
            NoteEffect::PreserveCanonical {
                operation,
                expected,
            } => self.worker.preserve_canonical(expected, operation),
            NoteEffect::ApplyDocument(effect) => self.apply_app_effect(effect, out),
            NoteEffect::ResyncProjection => self.resync_projection(out),
            NoteEffect::AssetsAfterRecovery => self.sync_assets_after_recovery(out),
            NoteEffect::NoteBarriers { succeeded } => {
                let guards = self.guards(view);
                let now_ms = self.now_ms();
                self.dispatch_window_intent(
                    WindowIntent::HideSaveCompleted {
                        succeeded,
                        guards,
                        now_ms,
                    },
                    out,
                );
                out.turn(Turn::QuitBarrier {
                    barrier: QuitBarrier::NoteSave,
                    succeeded,
                });
            }
            NoteEffect::Diagnostic(message) => out.push(ShellEffect::Diagnostic(message)),
            NoteEffect::RefreshTitle => out.push(ShellEffect::TitleDirty),
            NoteEffect::Redraw => out.push(ShellEffect::RedrawNeeded),
        }
    }
}
