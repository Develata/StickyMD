//! Search as the session keeps it: opening it, where it is when the view changes, the note's
//! selection following its active match, and replacing matches. The shell draws it and places the
//! IME over its field.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-find-replace

use stickymd_core::Selection;

use crate::config::ViewMode;
use crate::instruction::AppIntent;
use crate::interaction::rules::{
    ReplaceCurrentAction, SEARCH_COMPOSING, SearchOpenAction, SearchPlace, replace_current_action,
    replacement_allowed, replacement_end, search_open_action,
};

use super::turns::Turn;
use super::{AppSession, ShellEffect, ShellEffects};

/// What opening Search leaves to the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOpenStep {
    Done,
    /// The shortcut closes the open Search (`close_search` in the shell, as every close).
    Close,
    /// Search needs a Source pane: change the view to Source, then `open_search_in_source_view`.
    SourceViewFirst,
}

impl AppSession {
    /// Ctrl+F (`replace` false) or Ctrl+H. It opens a closed Search on the selected text, closes
    /// an open one or shows its replacement row; a composition in the note or in Search refuses it.
    pub fn open_search(&mut self, replace: bool, out: &mut ShellEffects) -> SearchOpenStep {
        if self.search.open && self.search.is_composing() {
            out.push(ShellEffect::Diagnostic(SEARCH_COMPOSING.into()));
            out.push(ShellEffect::RedrawNeeded);
            return SearchOpenStep::Done;
        }
        let replace = match search_open_action(self.search.open, replace) {
            SearchOpenAction::Close => return SearchOpenStep::Close,
            SearchOpenAction::ExpandReplace => {
                self.search.show_replace();
                out.push(ShellEffect::ImeAreaChanged);
                out.push(ShellEffect::RedrawNeeded);
                return SearchOpenStep::Done;
            }
            SearchOpenAction::Open { replace } => replace,
        };
        if self.editor.is_composing() {
            out.push(ShellEffect::Diagnostic(
                "请先完成当前输入法组合，再打开查找。".into(),
            ));
            out.push(ShellEffect::RedrawNeeded);
            return SearchOpenStep::Done;
        }
        if self.config.current().view_mode == ViewMode::Preview {
            return SearchOpenStep::SourceViewFirst;
        }
        self.open_search_on_selection(replace, out);
        SearchOpenStep::Done
    }

    /// Opening Search from Preview, once the shell asked for the Source view.
    pub fn open_search_in_source_view(&mut self, replace: bool, out: &mut ShellEffects) {
        if self.config.current().view_mode == ViewMode::Preview {
            // The view did not change (its diagnostic is shown), and Search needs a Source pane.
            return;
        }
        if self.search.open {
            // The Search that Preview parked came back as it was; opening it again would
            // reset its row and focus and replace its query with the selected match.
            if replace {
                self.search.show_replace();
            }
            out.push(ShellEffect::ImeAreaChanged);
            out.push(ShellEffect::RedrawNeeded);
            return;
        }
        self.open_search_on_selection(replace, out);
    }

    /// Opens Search with the note's selected text as its query, and the keyboard goes to it.
    fn open_search_on_selection(&mut self, replace: bool, out: &mut ShellEffects) {
        out.push(ShellEffect::SourceFocused);
        let view = self.coordinator.view();
        let selected = view
            .text
            .get(self.editor.selection.normalized_range())
            .filter(|_| !self.editor.selection.is_collapsed())
            .map(str::to_owned);
        self.search
            .open(replace, selected.as_deref(), view.text, view.generation);
        self.select_active_match(out);
        out.push(ShellEffect::KeyboardOwnerChanged);
        out.push(ShellEffect::ImeAreaChanged);
        out.push(ShellEffect::RedrawNeeded);
    }

    /// Where Search is, as a view change reads it.
    pub fn search_place(&self) -> SearchPlace {
        debug_assert!(
            !(self.search.open && self.parked_search.is_some()),
            "a parked Search exists only while the live one is closed"
        );
        if self.parked_search.is_some() {
            SearchPlace::Parked
        } else if self.search.open {
            SearchPlace::Open {
                composing: self.search.is_composing(),
            }
        } else {
            SearchPlace::Closed
        }
    }

    /// Selects the active match in the note, so what is selected is what Replace Current
    /// replaces.
    pub fn select_active_match(&mut self, out: &mut ShellEffects) {
        if let Some(range) = self.search.active_range() {
            self.editor.selection = Selection::new(range.start, range.end);
            out.push(ShellEffect::PresentationChanged);
        }
    }

    /// Replace Current: the active match is replaced only while the note shows it selected; a
    /// first press with the selection elsewhere selects it instead (`replace_current_action`).
    /// Matches made stale by an edit are rescanned first.
    pub fn replace_current_match(&mut self, out: &mut ShellEffects) {
        if !replacement_allowed(&self.search) {
            return;
        }
        let generation = self.coordinator.view().generation;
        if !self.search.is_current(generation) {
            self.search
                .refresh(self.coordinator.view().text, generation);
        }
        let action = replace_current_action(
            self.search.active_range(),
            self.editor.selection.normalized_range(),
        );
        let range = match action {
            ReplaceCurrentAction::Nothing => return,
            ReplaceCurrentAction::Select => {
                self.select_active_match(out);
                return;
            }
            ReplaceCurrentAction::Replace(range) => range,
        };
        let result = self.coordinator.dispatch(AppIntent::ReplaceLiteralMatch {
            expected_generation: generation,
            range: range.clone(),
            query: self.search.query.clone(),
            replacement: self.search.replacement.clone(),
            options: self.search.options(),
            timestamp_ms: self.now_ms(),
        });
        match result {
            Ok(effect) => {
                let end = replacement_end(&effect, &range);
                // Applying the change rescans Search against the new text, in its own turn; the
                // next match is looked for once that rescan is done (`Turn::MatchReplaced`).
                self.apply_app_effect(effect, out);
                out.turn(Turn::MatchReplaced { end });
            }
            Err(error) => {
                out.push(ShellEffect::Diagnostic(error.to_string()));
                out.push(ShellEffect::RedrawNeeded);
                // The active match is selected for the keyboard and the Replace button alike.
                self.select_active_match(out);
            }
        }
    }

    /// After Replace Current's change and its rescan: the next match is searched for after the
    /// inserted text, so while a later match exists a replacement that contains the query is not
    /// replaced again in place. It is selected, so each further press replaces one match.
    pub(super) fn after_match_replaced(&mut self, end: usize, out: &mut ShellEffects) {
        self.search.activate_from(end);
        self.select_active_match(out);
    }

    /// Replace All, for a non-empty query while replacing is allowed.
    pub fn replace_all_matches(&mut self, out: &mut ShellEffects) {
        if !replacement_allowed(&self.search) || self.search.query.is_empty() {
            return;
        }
        self.dispatch(
            AppIntent::ReplaceAllLiteral {
                expected_generation: self.coordinator.view().generation,
                query: self.search.query.clone(),
                replacement: self.search.replacement.clone(),
                options: self.search.options(),
                timestamp_ms: self.now_ms(),
            },
            out,
        );
    }

    /// Takes the open Search off the screen once the view has no Source pane: the live session is
    /// left closed, so no key, IME event, press, Esc or auto-collapse guard finds it, and it holds
    /// no matches until `resume_search` puts it back.
    pub(super) fn park_search(&mut self, out: &mut ShellEffects) {
        self.parked_search = self.search.park();
        out.push(ShellEffect::ImeAreaChanged);
        out.push(ShellEffect::RedrawNeeded);
    }

    /// Puts the parked Search back as it was once the view has a Source pane again, rescanned
    /// against the current document. The note's selection follows the active match, as it does
    /// on opening and navigating, so what is selected is what Replace Current replaces even when
    /// the document changed while Search was parked.
    pub(super) fn resume_search(&mut self, out: &mut ShellEffects) {
        let Some(parked) = self.parked_search.take() else {
            return;
        };
        let view = self.coordinator.view();
        self.search.resume(parked, view.text, view.generation);
        self.select_active_match(out);
        out.push(ShellEffect::ImeAreaChanged);
        out.push(ShellEffect::RedrawNeeded);
    }
}
