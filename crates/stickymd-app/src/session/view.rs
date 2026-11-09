//! Which view the note is shown in, and what changing it does to the window flow, the Preview, the
//! configuration and Search. The shell lays the new view out (`ShellEffect::ViewModeChanged`).
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#preview-scheduling

use crate::config::ViewMode;
use crate::flow::window::state::WindowIntent;
use crate::interaction::rules::{SEARCH_COMPOSING, SearchViewChange, search_on_view_change};

use super::{AppSession, ShellEffect, ShellEffects};

/// A view change `begin_view_change` allowed: the view to change to and what that does to
/// Search. Only `begin_view_change` makes one, never with a refusal, and `commit_view_change`
/// consumes it, so a change is committed at most once and only as it was allowed.
#[derive(Debug)]
pub struct PreparedViewChange {
    mode: ViewMode,
    search: SearchViewChange,
}

#[cfg(test)]
impl PreparedViewChange {
    /// What the prepared change does to Search, for the decision tests to read.
    pub(super) fn search(&self) -> SearchViewChange {
        self.search
    }
}

impl AppSession {
    /// Whether the view may change to `mode`, and what that does to Search; `None` leaves the view
    /// as it is, and a refusal is on the status line.
    pub fn begin_view_change(
        &self,
        mode: ViewMode,
        out: &mut ShellEffects,
    ) -> Option<PreparedViewChange> {
        if self.config.current().view_mode == mode {
            return None;
        }
        // Search lives in the Source pane (plan 07). Decided before anything changes, so a
        // refusal leaves the view untouched; applied only once the new view is committed.
        let search = search_on_view_change(mode, self.search_place());
        if search == SearchViewChange::Refuse {
            out.push(ShellEffect::Diagnostic(SEARCH_COMPOSING.into()));
            out.push(ShellEffect::RedrawNeeded);
            return None;
        }
        Some(PreparedViewChange { mode, search })
    }

    /// Changes the view as `begin_view_change` allowed, once the shell has ended the drags of the
    /// old one. A configuration that cannot take the change leaves the view as it was.
    pub fn commit_view_change(&mut self, prepared: PreparedViewChange, out: &mut ShellEffects) {
        let PreparedViewChange { mode, search } = prepared;
        self.editor.cancel_preedit();
        self.dispatch_window_intent(
            WindowIntent::SplitModeChanged {
                split: mode == ViewMode::Split,
            },
            out,
        );
        if let Err(error) = self.config.update(|config| config.view_mode = mode) {
            out.push(ShellEffect::Diagnostic(error.to_string()));
            return;
        }
        if mode == ViewMode::Source {
            self.preview_flow.release_projection();
            if let Some(worker) = &self.preview_worker {
                // Preserve the semantic tree for a fast return to Preview;
                // only tray-hidden cache release drops document-sized state.
                worker.release_raster_caches();
            }
        }
        out.push(ShellEffect::ViewModeChanged(mode));
        if let Some(action) = self.show_preview_action() {
            out.push(ShellEffect::SubmitPreview(action));
        }
        match search {
            SearchViewChange::Park => self.park_search(out),
            SearchViewChange::Resume => self.resume_search(out),
            // A refusal is never prepared.
            SearchViewChange::Keep | SearchViewChange::Refuse => {}
        }
        out.push(ShellEffect::KeyboardOwnerChanged);
        self.submit_config_if_needed();
        out.push(ShellEffect::RedrawNeeded);
    }
}
