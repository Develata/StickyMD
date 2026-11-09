//! Pure rules of the interaction: what a fact decides, with no state and no shell. A host reads
//! its own facts and applies the answer; the rule itself stays here, once, for every host.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-find-replace
//! plan_ref: docs/plan/09_windows_shell.md#dock-auto-hide

#[cfg(test)]
mod tests;

use std::ops::Range;

use stickymd_core::Selection;

use super::SearchSession;
use crate::config::{
    MAX_WINDOW_DIP, MIN_WINDOW_HEIGHT_DIP, MIN_WINDOW_WIDTH_DIP, ThemeMode, ViewMode,
};
use crate::flow::AppEffect;
use crate::flow::window::geometry::MonitorGeometry;
use crate::flow::window::state::WindowGuardSnapshot;

pub const SEARCH_COMPOSING: &str = "请先完成当前查找输入法组合。";

/// Where Search is when the view changes. A parked Search exists only while the live one is
/// closed, so the two never coexist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchPlace {
    Closed,
    Open { composing: bool },
    Parked,
}

/// What a view change does to Search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchViewChange {
    Keep,
    Park,
    Resume,
    /// The Search field is composing: the view does not change, and the composition goes on.
    Refuse,
}

/// Search lives in the Source pane (plan 07): a view without one (Preview) parks an open Search
/// and refuses while it composes, as opening Search does; a view with one resumes a parked Search.
pub const fn search_on_view_change(target: ViewMode, place: SearchPlace) -> SearchViewChange {
    let has_source_pane = !matches!(target, ViewMode::Preview);
    match (has_source_pane, place) {
        (false, SearchPlace::Open { composing: true }) => SearchViewChange::Refuse,
        (false, SearchPlace::Open { composing: false }) => SearchViewChange::Park,
        (true, SearchPlace::Parked) => SearchViewChange::Resume,
        _ => SearchViewChange::Keep,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchOpenAction {
    Open { replace: bool },
    Close,
    ExpandReplace,
}

pub const fn search_open_action(open: bool, replace_shortcut: bool) -> SearchOpenAction {
    match (open, replace_shortcut) {
        (false, replace) => SearchOpenAction::Open { replace },
        (true, false) => SearchOpenAction::Close,
        (true, true) => SearchOpenAction::ExpandReplace,
    }
}

/// What Replace Current does (plan 07).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplaceCurrentAction {
    /// There is no active match.
    Nothing,
    /// The note's selection is not the active match: select it, and replace on the next press.
    Select,
    Replace(Range<usize>),
}

/// Replace Current replaces only the match the note shows selected. A reload, a toolbar
/// conversion or a click in the note can leave the selection elsewhere while Search is open;
/// the first press then selects the active match instead of replacing text that is not selected.
pub fn replace_current_action(
    active: Option<Range<usize>>,
    selected: Range<usize>,
) -> ReplaceCurrentAction {
    match active {
        None => ReplaceCurrentAction::Nothing,
        Some(active) if active == selected => ReplaceCurrentAction::Replace(active),
        Some(_) => ReplaceCurrentAction::Select,
    }
}

/// Where the text that replaced `matched` ends: at the caret the edit leaves after it, or, when
/// the replacement equals the match and nothing changed, at the match's end.
pub fn replacement_end(effect: &AppEffect, matched: &Range<usize>) -> usize {
    match effect {
        AppEffect::DocumentChanged { selection, .. } => selection.normalized_range().end,
        _ => matched.end,
    }
}

pub fn replacement_allowed(search: &SearchSession) -> bool {
    search.replace_visible && !search.is_composing()
}

/// A delimiter conversion follows the selection only where the note shows it: Preview has no
/// Source selection to scope to.
pub const fn scope_to_selection(mode: ViewMode, selection_collapsed: bool) -> bool {
    !matches!(mode, ViewMode::Preview) && !selection_collapsed
}

/// The order the theme button goes through.
pub const fn theme_rotation(theme: ThemeMode) -> ThemeMode {
    match theme {
        ThemeMode::Light => ThemeMode::System,
        ThemeMode::System => ThemeMode::Dark,
        ThemeMode::Dark => ThemeMode::Light,
    }
}

/// A selection that survived a document replacement: inside the text, on character boundaries.
pub fn clamp_selection(text: &str, selection: Selection) -> Selection {
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

/// The display a window goes to: the one `configured` names, else the primary, else the first;
/// `None` without any. An empty `configured` names none.
pub fn window_display(displays: &[MonitorGeometry], configured: &str) -> Option<MonitorGeometry> {
    displays
        .iter()
        .find(|display| !configured.is_empty() && display.identity.as_str() == configured)
        .or_else(|| displays.iter().find(|display| display.primary))
        .or_else(|| displays.first())
        .cloned()
}

/// The width and height a placement commits, in whole DIPs inside the window limits.
pub fn clamp_placement(width_dip: f64, height_dip: f64) -> (u32, u32) {
    let limit = f64::from(MAX_WINDOW_DIP);
    (
        width_dip
            .round()
            .clamp(f64::from(MIN_WINDOW_WIDTH_DIP), limit) as u32,
        height_dip
            .round()
            .clamp(f64::from(MIN_WINDOW_HEIGHT_DIP), limit) as u32,
    )
}

/// Whether input may change the note. `preview_focused` is what the caller says it is: the IME
/// passes the real focus, because composing into a focused Preview edits nothing, while a key
/// passes `false`, because Preview takes its own keys further on and window-wide shortcuts must
/// still reach it.
pub const fn mutation_allowed(
    recovery_pending: bool,
    preview_focused: bool,
    shell_input_enabled: bool,
    window_accepts_mutation: bool,
    asset_reconcile_pending: bool,
) -> bool {
    !recovery_pending
        && !preview_focused
        && shell_input_enabled
        && window_accepts_mutation
        && !asset_reconcile_pending
}

/// The state the window guards are projected from.
#[derive(Clone, Copy, Debug, Default)]
pub struct GuardFacts {
    pub window_focused: bool,
    pub editor_composing: bool,
    pub search_composing: bool,
    pub move_resize_active: bool,
    pub scrollbar_dragging: bool,
    pub opacity_popup_open: bool,
    pub search_open: bool,
    pub export_in_flight: bool,
    pub recovery_pending: bool,
    pub conflict: bool,
    pub paste_pending: bool,
    pub asset_sync_in_flight: bool,
    pub dirty: bool,
    pub durability_required: bool,
    pub note_save_in_flight: bool,
}

/// Which facts make a guard (for example that a Search composition counts as composing) is
/// decided here, once.
pub const fn guard_snapshot(facts: GuardFacts) -> WindowGuardSnapshot {
    WindowGuardSnapshot {
        window_focused: facts.window_focused,
        ime_composing: facts.editor_composing || facts.search_composing,
        dragging: facts.move_resize_active || facts.scrollbar_dragging,
        popup_open: facts.opacity_popup_open || facts.search_open || facts.export_in_flight,
        conflict_or_recovery: facts.recovery_pending || facts.conflict,
        paste_pending: facts.paste_pending,
        asset_transaction_pending: facts.asset_sync_in_flight,
        note_save_required: facts.dirty || facts.durability_required,
        note_save_in_flight: facts.note_save_in_flight,
    }
}
