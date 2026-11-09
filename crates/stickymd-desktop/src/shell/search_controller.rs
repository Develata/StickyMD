//! Source search/replace event translation and document-flow coordination.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-find-replace

use winit::event::{ElementState, Ime, MouseButton};
use winit::keyboard::KeyCode;

use super::DesktopShell;
#[cfg(test)]
use super::shell_pins_tests::PressRoute;
use crate::host::Host;
use crate::search::{SearchHit, SearchLayout, search_field_display};
use stickymd_app::config::ViewMode;
use stickymd_app::instruction::PreviewIntent;
use stickymd_app::interaction::SearchField;
use stickymd_app::interaction::rules::replacement_allowed;
use stickymd_app::session::SearchOpenStep;

impl<H: Host> DesktopShell<H> {
    pub(super) fn search_layout(&self) -> Option<SearchLayout> {
        let pane = self.view_geometry()?.source?;
        let scale = self.shell_dpi().map_or(1.0, |dpi| dpi as f32);
        Some(SearchLayout::new(
            pane,
            scale,
            self.app.search.replace_visible,
        ))
    }

    pub(super) fn open_search(&mut self, replace: bool) {
        let mut step = SearchOpenStep::Done;
        self.with_session(|app, _, out| step = app.open_search(replace, out));
        match step {
            SearchOpenStep::Done => {}
            SearchOpenStep::Close => self.close_search(),
            SearchOpenStep::SourceViewFirst => {
                self.dispatch_preview_intent(PreviewIntent::SetViewMode(ViewMode::Source));
                self.with_session(|app, _, out| app.open_search_in_source_view(replace, out));
            }
        }
    }

    /// Closes Search and refreshes what its overlay projected; the keyboard, and with it the IME,
    /// goes back to Preview or the note. Every way of closing it (Esc, Ctrl+F, its close button)
    /// comes here.
    pub(super) fn close_search(&mut self) {
        self.app.search.close();
        self.sync_ime_allowed();
        self.update_search_selection();
        self.update_ime_area();
        self.request_redraw();
    }

    pub(super) fn handle_search_key(
        &mut self,
        key_text: Option<&str>,
        code: Option<KeyCode>,
        shortcut: bool,
        shift: bool,
    ) -> bool {
        if !self.app.search.open {
            return false;
        }
        // An active IME owns keyboard interpretation until commit/cancel.
        // Search shortcuts, navigation and field switching must not create a
        // second transition path while preedit is only a projection.
        if self.app.search.is_composing() {
            return true;
        }
        let generation = self.app.coordinator().view().generation;
        match code {
            Some(KeyCode::Tab) => self.app.search.focus_next(),
            Some(KeyCode::F3) => self.navigate_search(shift),
            Some(KeyCode::Enter | KeyCode::NumpadEnter)
                if shortcut && shift && replacement_allowed(&self.app.search) =>
            {
                self.replace_all_search()
            }
            Some(KeyCode::Enter | KeyCode::NumpadEnter)
                if shortcut && replacement_allowed(&self.app.search) =>
            {
                self.replace_current_search()
            }
            Some(KeyCode::Enter | KeyCode::NumpadEnter) if shortcut => {}
            Some(KeyCode::Enter | KeyCode::NumpadEnter) => self.navigate_search(shift),
            Some(KeyCode::Backspace) => {
                let (search, text) = self.app.search_over_text();
                search.backspace(text, generation);
            }
            Some(KeyCode::Delete) => {
                let (search, text) = self.app.search_over_text();
                search.delete(text, generation);
            }
            Some(KeyCode::ArrowLeft) => self.app.search.move_cursor(false),
            Some(KeyCode::ArrowRight) => self.app.search.move_cursor(true),
            Some(KeyCode::ArrowUp) => self.navigate_search(true),
            Some(KeyCode::ArrowDown) => self.navigate_search(false),
            Some(KeyCode::KeyC) if self.modifiers.alt_key() => {
                let (search, text) = self.app.search_over_text();
                search.toggle_case(text, generation);
            }
            Some(KeyCode::KeyV) if shortcut => match self.app.read_clipboard_text() {
                Ok(Some(inserted)) => {
                    let (search, text) = self.app.search_over_text();
                    search.insert(&inserted, text, generation);
                }
                Ok(None) => {}
                Err(error) => self.diagnostic = Some(error.to_string()),
            },
            _ if !shortcut
                && !self.modifiers.alt_key()
                && !self.modifiers.super_key()
                && key_text.is_some_and(|value| {
                    !value.is_empty() && !value.chars().any(char::is_control)
                }) =>
            {
                if let Some(inserted) = key_text {
                    let (search, text) = self.app.search_over_text();
                    search.insert(inserted, text, generation);
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
        if !self.app.search.open {
            return false;
        }
        let generation = self.app.coordinator().view().generation;
        match event {
            Ime::Preedit(preedit, cursor) => self.app.search.set_preedit(preedit.clone(), *cursor),
            Ime::Commit(commit) => {
                let (search, text) = self.app.search_over_text();
                search.commit_preedit(commit, text, generation);
            }
            Ime::Disabled => self.app.search.set_preedit(String::new(), None),
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
        if !self.app.search.open || state != ElementState::Pressed || button != MouseButton::Left {
            return false;
        }
        if self.app.search.is_composing() {
            return true;
        }
        let Some(layout) = self.search_layout() else {
            return false;
        };
        let Some(hit) = layout.hit(self.cursor_position, self.app.search.replace_visible) else {
            return false;
        };
        press_probe!(self, PressRoute::Search(hit), true);
        match hit {
            SearchHit::Query => self.focus_search_field_at_pointer(SearchField::Query, layout),
            SearchHit::Replacement => {
                self.focus_search_field_at_pointer(SearchField::Replacement, layout)
            }
            SearchHit::ToggleCase => {
                let generation = self.app.coordinator().view().generation;
                let (search, text) = self.app.search_over_text();
                search.toggle_case(text, generation);
                // The rescanned active match is selected, as Alt+C does from the keyboard.
                self.update_search_selection();
            }
            SearchHit::Previous => self.navigate_search(true),
            SearchHit::Next => self.navigate_search(false),
            SearchHit::Close => self.close_search(),
            SearchHit::Replace => self.replace_current_search(),
            SearchHit::ReplaceAll => self.replace_all_search(),
        }
        self.update_ime_area();
        self.request_redraw();
        true
    }

    fn navigate_search(&mut self, reverse: bool) {
        let generation = self.app.coordinator().view().generation;
        if !self.app.search.is_current(generation) {
            let (search, text) = self.app.search_over_text();
            search.refresh(text, generation);
        }
        if let Some(selection) = self.app.search.next(reverse) {
            self.app.editor.selection = selection;
            self.after_presentation_change();
        }
    }

    fn focus_search_field_at_pointer(&mut self, field: SearchField, layout: SearchLayout) {
        // The press lands on the field as it was painted before it: from the start when it was
        // not focused, scrolled to its cursor when it was.
        let was_focused = self.app.search.focused == field;
        let (display, cursor) = search_field_display(&self.app.search, field);
        self.app.search.focused = field;
        let scale = self
            .window
            .as_ref()
            .map_or(1.0, |window| window.scale_factor() as f32);
        let spec = layout.field_spec(field, scale);
        let x = self.cursor_position.x as f32;
        let hit = self.projection.as_mut().and_then(|projection| {
            if was_focused {
                projection.ui_text_field_hit(&display, cursor, spec, x)
            } else {
                projection.ui_text_hit(&display, spec, x)
            }
        });
        if let Some(hit) = hit {
            let committed_len = match field {
                SearchField::Query => self.app.search.query.len(),
                SearchField::Replacement => self.app.search.replacement.len(),
            };
            self.app.search.set_focused_cursor(hit.min(committed_len));
        }
    }

    fn update_search_selection(&mut self) {
        self.with_session(|app, _, out| app.select_active_match(out));
    }

    fn replace_current_search(&mut self) {
        self.with_session(|app, _, out| app.replace_current_match(out));
    }

    fn replace_all_search(&mut self) {
        self.with_session(|app, _, out| app.replace_all_matches(out));
    }
}

#[cfg(test)]
mod tests {
    use winit::dpi::PhysicalSize;

    use super::*;
    use crate::geometry::geometry;

    /// `search_on_view_change` keys on the view having no Source pane: only Preview.
    #[test]
    fn only_preview_has_no_source_pane() {
        for (mode, has_source_pane) in [
            (ViewMode::Source, true),
            (ViewMode::Split, true),
            (ViewMode::Preview, false),
        ] {
            let pane = geometry(mode, PhysicalSize::new(900, 680), 1.0).source;
            assert_eq!(pane.is_some(), has_source_pane, "{mode:?}");
        }
    }
}
