//! Runs session steps and carries out what they ask of the shell.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use stickymd_app::instruction::AppIntent;

use super::DesktopShell;
use crate::host::Host;
use stickymd_app::session::{AppSession, ProjectionChange, ShellEffect, ShellEffects, ViewFacts};

impl<H: Host> DesktopShell<H> {
    /// Runs one session step, then applies everything it asked for, in order, before returning:
    /// a caller sees the finished state, as it would from a direct call. A step that an effect
    /// runs in turn finishes before the next effect of this one.
    pub(super) fn with_session(
        &mut self,
        step: impl FnOnce(&mut AppSession, ViewFacts, &mut ShellEffects),
    ) {
        let frame = self.effects.open();
        let view = self.view_facts();
        step(&mut self.app, view, &mut self.effects);
        self.effects.seal(frame);
        while let Some(effect) = self.effects.next(frame) {
            self.apply_shell_effect(effect);
        }
    }

    pub(super) fn dispatch(&mut self, intent: AppIntent) {
        self.with_session(|app, _, out| app.dispatch(intent, out));
    }

    /// Asset steps as the receipt tests start them.
    #[cfg(test)]
    pub(super) fn submit_asset_sync(
        &mut self,
        generation: stickymd_core::Generation,
        effects: Vec<stickymd_core::AssetEffect>,
        reconcile: Option<stickymd_app::ports::AssetReconcileMode>,
        for_quit: bool,
    ) {
        self.with_session(|app, _, out| {
            app.submit_asset_sync(generation, effects, reconcile, for_quit, out);
        });
    }

    #[cfg(test)]
    pub(super) fn submit_asset_paste(&mut self, pending: stickymd_app::flow::PendingAssetPaste) {
        self.with_session(|app, _, out| app.submit_asset_paste(pending, out));
    }

    #[cfg(test)]
    pub(super) fn sync_assets_after_recovery(&mut self) {
        self.with_session(|app, _, out| app.sync_assets_after_recovery(out));
    }

    fn apply_shell_effect(&mut self, effect: ShellEffect) {
        match effect {
            ShellEffect::Window(effect) => self.run_window_effect(effect),
            ShellEffect::Resume(turn) => {
                self.with_session(|app, view, out| app.resume(turn, view, out));
            }
            ShellEffect::Projection(change) => self.apply_projection_change(change),
            ShellEffect::PreviewDocumentChanged(generation) => {
                self.on_preview_document_changed(generation);
            }
            ShellEffect::SubmitPreview(action) => self.submit_preview_action(action),
            ShellEffect::PresentationChanged => self.after_presentation_change(),
            ShellEffect::ViewModeChanged(mode) => self.lay_out_view(mode),
            ShellEffect::KeyboardOwnerChanged => self.sync_ime_allowed(),
            ShellEffect::SourceFocused => self.preview_focused = false,
            ShellEffect::ImeAreaChanged => self.update_ime_area(),
            ShellEffect::Preference(effect) => self.show_window_preference(effect),
            ShellEffect::TitleDirty => self.update_window_title(),
            ShellEffect::RedrawNeeded => self.request_redraw(),
            ShellEffect::Diagnostic(message) => self.diagnostic = Some(message),
            ShellEffect::StartWatcher => self.start_watcher(),
            ShellEffect::FocusLost => self.end_view_focus(),
            ShellEffect::Exit => {
                self.host.destroy();
                self.exit_pending = true;
            }
        }
    }

    fn apply_projection_change(&mut self, change: ProjectionChange) {
        let Some(projection) = &mut self.projection else {
            return;
        };
        match change {
            ProjectionChange::Delta { generation, delta } => {
                if let Err(error) = projection.apply_delta(generation, &delta) {
                    #[cfg(debug_assertions)]
                    eprintln!(
                        "source projection resync: error={error} target_generation={}",
                        generation.value()
                    );
                    let snapshot = self.app.snapshot();
                    if let Err(resync_error) = projection.resync(&snapshot) {
                        self.diagnostic = Some(format!(
                            "projection update failed: {error}; resync failed: {resync_error}"
                        ));
                    }
                }
            }
            ProjectionChange::FullResync(snapshot) => {
                let _ = projection.resync(&snapshot);
            }
        }
    }
}
