//! Non-authoritative Source find/replace interaction session.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-find-replace

use stickymd_core::{Generation, Selection};

use super::navigation::{next_grapheme_boundary, previous_grapheme_boundary};
use crate::instruction::LiteralSearchOptions;
use crate::source_search::{LiteralMatch, find_literal_matches};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchField {
    Query,
    Replacement,
}

#[derive(Debug, Clone)]
pub struct SearchSession {
    pub open: bool,
    pub replace_visible: bool,
    pub focused: SearchField,
    pub query: String,
    pub replacement: String,
    pub case_sensitive: bool,
    pub preedit: String,
    generation: Option<Generation>,
    matches: Vec<LiteralMatch>,
    active: Option<usize>,
    query_cursor: usize,
    replacement_cursor: usize,
    truncated: bool,
}

impl Default for SearchSession {
    fn default() -> Self {
        Self {
            open: false,
            replace_visible: false,
            focused: SearchField::Query,
            query: String::new(),
            replacement: String::new(),
            case_sensitive: false,
            preedit: String::new(),
            generation: None,
            matches: Vec::new(),
            active: None,
            query_cursor: 0,
            replacement_cursor: 0,
            truncated: false,
        }
    }
}

impl SearchSession {
    pub fn open(
        &mut self,
        replace: bool,
        selected: Option<&str>,
        text: &str,
        generation: Generation,
    ) {
        self.open = true;
        self.replace_visible = replace;
        self.focused = SearchField::Query;
        if let Some(selected) = selected.filter(|value| !value.contains(['\r', '\n'])) {
            self.query.clear();
            self.query.push_str(selected);
            self.query_cursor = self.query.len();
        }
        self.refresh(text, generation);
    }

    pub fn close(&mut self) {
        self.open = false;
        self.active = None;
        self.preedit.clear();
    }

    pub fn options(&self) -> LiteralSearchOptions {
        LiteralSearchOptions {
            case_sensitive: self.case_sensitive,
        }
    }

    pub fn refresh(&mut self, text: &str, generation: Generation) {
        let result = find_literal_matches(text, &self.query, self.options());
        self.generation = Some(generation);
        self.matches = result.ranges;
        self.truncated = result.truncated;
        self.active = match (self.active, self.matches.is_empty()) {
            (_, true) => None,
            (Some(active), false) => Some(active.min(self.matches.len() - 1)),
            (None, false) => Some(0),
        };
    }

    pub fn is_current(&self, generation: Generation) -> bool {
        self.generation == Some(generation)
    }

    pub fn active_range(&self) -> Option<std::ops::Range<usize>> {
        self.active
            .and_then(|active| self.matches.get(active))
            .copied()
            .map(LiteralMatch::range)
    }

    pub fn match_summary(&self) -> (usize, usize, bool) {
        (
            self.active.map_or(0, |active| active + 1),
            self.matches.len(),
            self.truncated,
        )
    }

    pub fn next(&mut self, reverse: bool) -> Option<Selection> {
        if self.matches.is_empty() {
            self.active = None;
            return None;
        }
        let next = match (self.active, reverse) {
            (Some(0), true) | (None, true) => self.matches.len() - 1,
            (Some(active), true) => active - 1,
            (Some(active), false) => (active + 1) % self.matches.len(),
            (None, false) => 0,
        };
        self.active = Some(next);
        self.active_range()
            .map(|range| Selection::new(range.start, range.end))
    }

    pub fn toggle_case(&mut self, text: &str, generation: Generation) {
        self.case_sensitive = !self.case_sensitive;
        self.refresh(text, generation);
    }

    pub fn focus_next(&mut self) {
        if self.replace_visible {
            self.focused = match self.focused {
                SearchField::Query => SearchField::Replacement,
                SearchField::Replacement => SearchField::Query,
            };
        }
    }

    pub fn set_preedit(&mut self, text: String) {
        self.preedit = text;
    }

    pub fn commit_preedit(&mut self, text: &str, source: &str, generation: Generation) {
        self.preedit.clear();
        self.insert(text, source, generation);
    }

    pub fn is_composing(&self) -> bool {
        !self.preedit.is_empty()
    }

    pub fn insert(&mut self, inserted: &str, text: &str, generation: Generation) {
        if inserted.chars().any(char::is_control) {
            return;
        }
        let (value, cursor) = self.focused_value_mut();
        value.insert_str(*cursor, inserted);
        *cursor += inserted.len();
        if self.focused == SearchField::Query {
            self.refresh(text, generation);
        }
    }

    pub fn backspace(&mut self, text: &str, generation: Generation) {
        let focused = self.focused;
        let (value, cursor) = self.focused_value_mut();
        let previous = previous_grapheme_boundary(value, *cursor);
        value.replace_range(previous..*cursor, "");
        *cursor = previous;
        if focused == SearchField::Query {
            self.refresh(text, generation);
        }
    }

    pub fn delete(&mut self, text: &str, generation: Generation) {
        let focused = self.focused;
        let (value, cursor) = self.focused_value_mut();
        let next = next_grapheme_boundary(value, *cursor);
        value.replace_range(*cursor..next, "");
        if focused == SearchField::Query {
            self.refresh(text, generation);
        }
    }

    pub fn move_cursor(&mut self, right: bool) {
        let (value, cursor) = self.focused_value_mut();
        *cursor = if right {
            next_grapheme_boundary(value, *cursor)
        } else {
            previous_grapheme_boundary(value, *cursor)
        };
    }

    fn focused_value_mut(&mut self) -> (&mut String, &mut usize) {
        match self.focused {
            SearchField::Query => (&mut self.query, &mut self.query_cursor),
            SearchField::Replacement => (&mut self.replacement, &mut self.replacement_cursor),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_session_invalidates_generation_and_wraps_navigation() {
        let mut session = SearchSession::default();
        session.open(false, Some("a"), "a b a", Generation::initial());
        assert_eq!(session.match_summary(), (1, 2, false));
        assert_eq!(session.next(false), Some(Selection::new(4, 5)));
        assert_eq!(session.next(false), Some(Selection::new(0, 1)));
        let next = Generation::initial().checked_next().unwrap();
        assert!(!session.is_current(next));
        session.refresh("a", next);
        assert!(session.is_current(next));
        assert_eq!(session.active_range(), Some(0..1));
    }

    #[test]
    fn query_edits_are_grapheme_safe_and_replacement_is_independent() {
        let mut session = SearchSession::default();
        session.open(true, None, "🙂a🙂", Generation::initial());
        session.insert("🙂", "🙂a🙂", Generation::initial());
        assert_eq!(session.match_summary().1, 2);
        session.backspace("🙂a🙂", Generation::initial());
        assert!(session.query.is_empty());
        session.focus_next();
        session.insert("中", "🙂a🙂", Generation::initial());
        assert_eq!(session.replacement, "中");
        assert!(session.query.is_empty());
    }

    #[test]
    fn search_session_defaults_to_case_insensitive_matching() {
        let mut session = SearchSession::default();
        session.open(false, Some("rust"), "Rust rust", Generation::initial());
        assert!(!session.case_sensitive);
        assert_eq!(session.match_summary(), (1, 2, false));
    }
}
