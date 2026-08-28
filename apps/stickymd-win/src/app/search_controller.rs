//! Source search/replace event translation and document-flow coordination.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-find-replace

use winit::event::{ElementState, Ime, KeyEvent, MouseButton};
use winit::keyboard::KeyCode;

use super::StickyApp;
use super::search_runtime::{SearchHit, SearchLayout, search_field_display};
use crate::config::ViewMode;
use crate::instruction::{AppIntent, PreviewIntent};
use crate::interaction::SearchField;

impl StickyApp {
    pub(super) fn search_layout(&self) -> Option<SearchLayout> {
        let pane = self.view_geometry()?.source?;
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);
        Some(SearchLayout::new(pane, scale, self.search.replace_visible))
    }

    pub(super) fn open_search(&mut self, replace: bool) {
        if self.search.open && self.search.is_composing() {
            self.diagnostic = Some("请先完成当前查找输入法组合。".into());
            self.request_redraw();
            return;
        }
        let action = search_open_action(self.search.open, replace);
        match action {
            SearchOpenAction::Close => {
                self.search.close();
                self.update_ime_area();
                self.request_redraw();
                return;
            }
            SearchOpenAction::ExpandReplace => {
                self.search.show_replace();
                self.update_ime_area();
                self.request_redraw();
                return;
            }
            SearchOpenAction::Open { .. } => {}
        }
        if self.session.is_composing() {
            self.diagnostic = Some("请先完成当前输入法组合，再打开查找。".into());
            self.request_redraw();
            return;
        }
        if self.config.current().view_mode == ViewMode::Preview {
            self.dispatch_preview_intent(PreviewIntent::SetViewMode(ViewMode::Source));
        }
        self.preview_focused = false;
        let view = self.coordinator.view();
        let selected = view
            .text
            .get(self.session.selection.normalized_range())
            .filter(|_| !self.session.selection.is_collapsed())
            .map(str::to_owned);
        let SearchOpenAction::Open { replace } = action else {
            return;
        };
        self.search
            .open(replace, selected.as_deref(), view.text, view.generation);
        self.update_search_selection();
        if let Some(window) = &self.window {
            window.set_ime_allowed(true);
        }
        self.update_ime_area();
        self.request_redraw();
    }

    pub(super) fn handle_search_key(
        &mut self,
        event: &KeyEvent,
        code: Option<KeyCode>,
        shortcut: bool,
        shift: bool,
    ) -> bool {
        if !self.search.open {
            return false;
        }
        // An active IME owns keyboard interpretation until commit/cancel.
        // Search shortcuts, navigation and field switching must not create a
        // second transition path while preedit is only a projection.
        if self.search.is_composing() {
            return true;
        }
        let generation = self.coordinator.view().generation;
        match code {
            Some(KeyCode::Escape) => self.search.close(),
            Some(KeyCode::Tab) => self.search.focus_next(),
            Some(KeyCode::F3) => self.navigate_search(shift),
            Some(KeyCode::Enter | KeyCode::NumpadEnter)
                if shortcut && shift && replacement_allowed(&self.search) =>
            {
                self.replace_all_search()
            }
            Some(KeyCode::Enter | KeyCode::NumpadEnter)
                if shortcut && replacement_allowed(&self.search) =>
            {
                self.replace_current_search()
            }
            Some(KeyCode::Enter | KeyCode::NumpadEnter) if shortcut => {}
            Some(KeyCode::Enter | KeyCode::NumpadEnter) => self.navigate_search(shift),
            Some(KeyCode::Backspace) => {
                let text = self.coordinator.view().text;
                self.search.backspace(text, generation);
            }
            Some(KeyCode::Delete) => {
                let text = self.coordinator.view().text;
                self.search.delete(text, generation);
            }
            Some(KeyCode::ArrowLeft) => self.search.move_cursor(false),
            Some(KeyCode::ArrowRight) => self.search.move_cursor(true),
            Some(KeyCode::ArrowUp) => self.navigate_search(true),
            Some(KeyCode::ArrowDown) => self.navigate_search(false),
            Some(KeyCode::KeyC) if self.modifiers.alt_key() => {
                let text = self.coordinator.view().text;
                self.search.toggle_case(text, generation);
            }
            Some(KeyCode::KeyV) if shortcut => match self.coordinator.read_clipboard_text() {
                Ok(Some(inserted)) => {
                    let text = self.coordinator.view().text;
                    self.search.insert(&inserted, text, generation);
                }
                Ok(None) => {}
                Err(error) => self.diagnostic = Some(error.to_string()),
            },
            _ if !shortcut
                && !self.modifiers.alt_key()
                && !self.modifiers.super_key()
                && event.text.as_ref().is_some_and(|value| {
                    !value.is_empty() && !value.chars().any(char::is_control)
                }) =>
            {
                if let Some(inserted) = &event.text {
                    let text = self.coordinator.view().text;
                    self.search.insert(inserted, text, generation);
                }
            }
            _ => return true,
        }
        self.update_search_selection();
        self.update_ime_area();
        self.request_redraw();
        true
    }

    pub(super) fn handle_search_ime(&mut self, event: &Ime) -> bool {
        if !self.search.open {
            return false;
        }
        let generation = self.coordinator.view().generation;
        match event {
            Ime::Preedit(preedit, cursor) => self.search.set_preedit(preedit.clone(), *cursor),
            Ime::Commit(commit) => {
                let text = self.coordinator.view().text;
                self.search.commit_preedit(commit, text, generation);
            }
            Ime::Disabled => self.search.set_preedit(String::new(), None),
            Ime::Enabled => {}
        }
        self.update_search_selection();
        self.update_ime_area();
        self.request_redraw();
        true
    }

    pub(super) fn handle_search_mouse_button(
        &mut self,
        state: ElementState,
        button: MouseButton,
    ) -> bool {
        if !self.search.open || state != ElementState::Pressed || button != MouseButton::Left {
            return false;
        }
        if self.search.is_composing() {
            return true;
        }
        let Some(layout) = self.search_layout() else {
            return false;
        };
        let Some(hit) = layout.hit(self.cursor_position, self.search.replace_visible) else {
            return false;
        };
        match hit {
            SearchHit::Query => self.focus_search_field_at_pointer(SearchField::Query, layout),
            SearchHit::Replacement => {
                self.focus_search_field_at_pointer(SearchField::Replacement, layout)
            }
            SearchHit::ToggleCase => {
                let generation = self.coordinator.view().generation;
                let text = self.coordinator.view().text;
                self.search.toggle_case(text, generation);
            }
            SearchHit::Previous => self.navigate_search(true),
            SearchHit::Next => self.navigate_search(false),
            SearchHit::Close => self.search.close(),
            SearchHit::Replace => self.replace_current_search(),
            SearchHit::ReplaceAll => self.replace_all_search(),
        }
        self.update_ime_area();
        self.request_redraw();
        true
    }

    fn navigate_search(&mut self, reverse: bool) {
        let generation = self.coordinator.view().generation;
        if !self.search.is_current(generation) {
            self.search
                .refresh(self.coordinator.view().text, generation);
        }
        if let Some(selection) = self.search.next(reverse) {
            self.session.selection = selection;
            self.after_presentation_change();
        }
    }

    fn focus_search_field_at_pointer(&mut self, field: SearchField, layout: SearchLayout) {
        self.search.focused = field;
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);
        let (display, cursor) = search_field_display(&self.search, field);
        let spec = layout.field_spec(field, scale);
        let hit = self.projection.as_mut().and_then(|projection| {
            projection.ui_text_field_hit(&display, cursor, spec, self.cursor_position.x as f32)
        });
        if let Some(hit) = hit {
            let committed_len = match field {
                SearchField::Query => self.search.query.len(),
                SearchField::Replacement => self.search.replacement.len(),
            };
            self.search.set_focused_cursor(hit.min(committed_len));
        }
    }

    fn update_search_selection(&mut self) {
        if let Some(range) = self.search.active_range() {
            self.session.selection = stickymd_core::Selection::new(range.start, range.end);
            self.after_presentation_change();
        }
    }

    fn replace_current_search(&mut self) {
        if !replacement_allowed(&self.search) {
            return;
        }
        let generation = self.coordinator.view().generation;
        if !self.search.is_current(generation) {
            self.search
                .refresh(self.coordinator.view().text, generation);
        }
        let Some(range) = self.search.active_range() else {
            return;
        };
        self.dispatch(AppIntent::ReplaceLiteralMatch {
            expected_generation: generation,
            range,
            query: self.search.query.clone(),
            replacement: self.search.replacement.clone(),
            options: self.search.options(),
            timestamp_ms: self.timestamp_ms(),
        });
    }

    fn replace_all_search(&mut self) {
        if !replacement_allowed(&self.search) || self.search.query.is_empty() {
            return;
        }
        self.dispatch(AppIntent::ReplaceAllLiteral {
            expected_generation: self.coordinator.view().generation,
            query: self.search.query.clone(),
            replacement: self.search.replacement.clone(),
            options: self.search.options(),
            timestamp_ms: self.timestamp_ms(),
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchOpenAction {
    Open { replace: bool },
    Close,
    ExpandReplace,
}

const fn search_open_action(open: bool, replace_shortcut: bool) -> SearchOpenAction {
    match (open, replace_shortcut) {
        (false, replace) => SearchOpenAction::Open { replace },
        (true, false) => SearchOpenAction::Close,
        (true, true) => SearchOpenAction::ExpandReplace,
    }
}

fn replacement_allowed(search: &crate::interaction::SearchSession) -> bool {
    search.replace_visible && !search.is_composing()
}

#[cfg(test)]
mod tests {
    use stickymd_core::Generation;

    use super::*;

    #[test]
    fn ctrl_f_toggles_and_ctrl_h_expands_one_search_session() {
        assert_eq!(
            search_open_action(false, false),
            SearchOpenAction::Open { replace: false }
        );
        assert_eq!(search_open_action(true, false), SearchOpenAction::Close);
        assert_eq!(
            search_open_action(false, true),
            SearchOpenAction::Open { replace: true }
        );
        assert_eq!(
            search_open_action(true, true),
            SearchOpenAction::ExpandReplace
        );
    }

    #[test]
    fn find_only_and_active_preedit_both_reject_replacement_commands() {
        let mut search = crate::interaction::SearchSession::default();
        search.open(false, Some("a"), "a", Generation::initial());
        assert!(!replacement_allowed(&search));

        search.show_replace();
        assert!(replacement_allowed(&search));
        search.set_preedit("中".into(), Some((0, "中".len())));
        assert!(!replacement_allowed(&search));
    }
}
