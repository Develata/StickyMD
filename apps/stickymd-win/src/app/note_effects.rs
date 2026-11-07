//! Carries out the note decisions of `NoteIo` in the native shell.
//!
//! plan_ref: docs/plan/05_document_persistence.md#external-change-conflict

use stickymd_app::flow::window::state::{QuitBarrier, WindowIntent};
use stickymd_app::flow::{NoteEffect, NoteEffects, NoteIo, SaveTrigger};
use stickymd_core::Selection;
use winit::event_loop::ActiveEventLoop;

use super::StickyApp;
use crate::platform::windows::ArboardClipboard;

impl StickyApp {
    pub(super) fn note_io(&mut self) -> NoteIo<'_, ArboardClipboard> {
        NoteIo::new(
            &mut self.coordinator,
            &mut self.persistence,
            &mut self.recovery,
        )
    }

    pub(super) fn request_immediate_save(
        &mut self,
        event_loop: Option<&ActiveEventLoop>,
        trigger: SaveTrigger,
    ) {
        let effects = self.note_io().request_save(trigger);
        self.run_note(event_loop, effects);
    }

    /// Whether a conflict or a recovery choice consumed the key.
    pub(super) fn handle_resolution_key(
        &mut self,
        event_loop: Option<&ActiveEventLoop>,
        primary: bool,
    ) -> bool {
        let Some(effects) = self.note_io().resolve(primary) else {
            return false;
        };
        self.run_note(event_loop, effects);
        true
    }

    pub(super) fn run_note(&mut self, event_loop: Option<&ActiveEventLoop>, effects: NoteEffects) {
        for effect in effects {
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
                NoteEffect::ApplyDocument(effect) => self.apply_effect(effect),
                NoteEffect::ResyncProjection => self.full_projection_resync(),
                NoteEffect::AssetsAfterRecovery => self.sync_assets_after_recovery(),
                NoteEffect::NoteBarriers { succeeded } => {
                    self.complete_window_note_barriers(event_loop, succeeded);
                }
                NoteEffect::Diagnostic(message) => self.diagnostic = Some(message),
                NoteEffect::RefreshTitle => self.update_window_title(),
                NoteEffect::Redraw => self.request_redraw(),
            }
        }
    }

    pub(super) fn full_projection_resync(&mut self) {
        let snapshot = self.coordinator.snapshot();
        self.session.selection = clamp_selection(&snapshot.text, self.session.selection);
        self.search.refresh(&snapshot.text, snapshot.generation);
        if let Some(projection) = &mut self.projection {
            let _ = projection.resync(&snapshot);
        }
        self.on_preview_document_changed(snapshot.generation);
        self.after_presentation_change();
    }

    fn complete_window_note_barriers(
        &mut self,
        event_loop: Option<&ActiveEventLoop>,
        succeeded: bool,
    ) {
        let guards = self.window_guards();
        let now_ms = self.timestamp_ms();
        self.dispatch_window_intent(
            event_loop,
            WindowIntent::HideSaveCompleted {
                succeeded,
                guards,
                now_ms,
            },
        );
        self.dispatch_window_intent(
            event_loop,
            WindowIntent::QuitBarrierCompleted {
                barrier: QuitBarrier::NoteSave,
                succeeded,
                guards: self.window_guards(),
            },
        );
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
