//! Every rule at the edges its callers rely on, without a host.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use stickymd_core::{DocumentState, Generation, LineEnding, Selection};

use super::*;
use crate::flow::{ClipboardError, ClipboardPort, EditorCoordinator};
use crate::instruction::{AppIntent, LiteralSearchOptions};

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
fn a_view_without_a_source_pane_parks_search_and_one_with_it_resumes_it() {
    use SearchViewChange::{Keep, Park, Refuse, Resume};

    let idle = SearchPlace::Open { composing: false };
    let composing = SearchPlace::Open { composing: true };
    let cases = [
        (ViewMode::Preview, SearchPlace::Closed, Keep),
        (ViewMode::Preview, idle, Park),
        (ViewMode::Preview, composing, Refuse),
        (ViewMode::Preview, SearchPlace::Parked, Keep),
        (ViewMode::Source, SearchPlace::Closed, Keep),
        (ViewMode::Source, idle, Keep),
        (ViewMode::Source, composing, Keep),
        (ViewMode::Source, SearchPlace::Parked, Resume),
        (ViewMode::Split, SearchPlace::Closed, Keep),
        (ViewMode::Split, idle, Keep),
        (ViewMode::Split, composing, Keep),
        (ViewMode::Split, SearchPlace::Parked, Resume),
    ];
    for (target, place, expected) in cases {
        assert_eq!(
            search_on_view_change(target, place),
            expected,
            "{target:?} from {place:?}"
        );
    }
}

#[test]
fn replace_current_replaces_only_the_selected_active_match() {
    use ReplaceCurrentAction::{Nothing, Replace, Select};

    assert_eq!(replace_current_action(None, 0..0), Nothing);
    assert_eq!(replace_current_action(None, 2..3), Nothing);
    assert_eq!(replace_current_action(Some(2..3), 2..3), Replace(2..3));
    // `a X a` reloaded as `a a X` with the second `a` active: `X` stays selected at 4..5.
    assert_eq!(replace_current_action(Some(2..3), 4..5), Select);
    // The selection's direction does not matter.
    let reversed = Selection::new(3, 2).normalized_range();
    assert_eq!(replace_current_action(Some(2..3), reversed), Replace(2..3));
    // A caret, a selection overlapping or containing the match, or another match.
    for selected in [2..2, 3..3, 2..4, 1..3, 0..5, 0..1] {
        assert_eq!(
            replace_current_action(Some(2..3), selected.clone()),
            Select,
            "{selected:?}"
        );
    }
}

#[test]
fn consecutive_replacements_move_past_text_that_contains_the_query() {
    struct NoClipboard;
    impl ClipboardPort for NoClipboard {
        fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
            Ok(None)
        }
        fn write_text(&mut self, _text: &str) -> Result<(), ClipboardError> {
            Ok(())
        }
    }

    // One Replace Current press, as the shell's `replace_current_search` performs it.
    fn press(
        coordinator: &mut EditorCoordinator<NoClipboard>,
        search: &mut SearchSession,
        replacement: &str,
    ) -> String {
        let range = search.active_range().unwrap();
        let effect = coordinator
            .dispatch(AppIntent::ReplaceLiteralMatch {
                expected_generation: coordinator.view().generation,
                range: range.clone(),
                query: search.query.clone(),
                replacement: replacement.into(),
                options: LiteralSearchOptions::default(),
                timestamp_ms: 0,
            })
            .unwrap();
        let end = replacement_end(&effect, &range);
        let view = coordinator.view();
        search.refresh(view.text, view.generation);
        search.activate_from(end);
        view.text.to_owned()
    }

    let document = DocumentState::loaded("a b a", LineEnding::Lf, None);
    let mut coordinator = EditorCoordinator::new(document, NoClipboard);
    let mut search = SearchSession::default();
    let view = coordinator.view();
    search.open(true, Some("a"), view.text, view.generation);

    // The inserted `aa` contains the query; the next press reaches the second original `a`.
    assert_eq!(press(&mut coordinator, &mut search, "aa"), "aa b a");
    assert_eq!(search.active_range(), Some(5..6));
    assert_eq!(press(&mut coordinator, &mut search, "aa"), "aa b aa");
    // Past the last match, navigation wraps to the first, as Enter does.
    assert_eq!(search.active_range(), Some(0..1));

    // With no later match the wrap lands in the text just inserted, as the contract says.
    let mut search = SearchSession::default();
    let document = DocumentState::loaded("a", LineEnding::Lf, None);
    let mut coordinator = EditorCoordinator::new(document, NoClipboard);
    let view = coordinator.view();
    search.open(true, Some("a"), view.text, view.generation);
    assert_eq!(press(&mut coordinator, &mut search, "aa"), "aa");
    assert_eq!(search.active_range(), Some(0..1));
    assert_eq!(press(&mut coordinator, &mut search, "aa"), "aaa");

    // A replacement equal to the match changes nothing and still moves on.
    let mut search = SearchSession::default();
    let document = DocumentState::loaded("a a", LineEnding::Lf, None);
    let mut coordinator = EditorCoordinator::new(document, NoClipboard);
    let view = coordinator.view();
    search.open(true, Some("a"), view.text, view.generation);
    let generation = coordinator.view().generation;
    assert_eq!(press(&mut coordinator, &mut search, "a"), "a a");
    assert_eq!(coordinator.view().generation, generation);
    assert_eq!(search.active_range(), Some(2..3));

    // Newlines in the replacement are normalized, so the inserted text is shorter than the
    // replacement typed: the next match starts right where the caret is (3), not at 4.
    let mut search = SearchSession::default();
    let document = DocumentState::loaded("aaa", LineEnding::Lf, None);
    let mut coordinator = EditorCoordinator::new(document, NoClipboard);
    let view = coordinator.view();
    search.open(true, Some("a"), view.text, view.generation);
    let replacement = "x\r\ny";
    assert_eq!(replacement.len(), 4);
    assert_eq!(press(&mut coordinator, &mut search, replacement), "x\nyaa");
    assert_eq!(search.active_range(), Some(3..4));
}

#[test]
fn find_only_and_active_preedit_both_reject_replacement_commands() {
    let mut search = SearchSession::default();
    search.open(false, Some("a"), "a", Generation::initial());
    assert!(!replacement_allowed(&search));

    search.show_replace();
    assert!(replacement_allowed(&search));
    search.set_preedit("中".into(), Some((0, "中".len())));
    assert!(!replacement_allowed(&search));
}

#[test]
fn a_conversion_scopes_to_a_selection_the_note_shows() {
    assert!(scope_to_selection(ViewMode::Source, false));
    assert!(scope_to_selection(ViewMode::Split, false));
    assert!(!scope_to_selection(ViewMode::Preview, false));
    for mode in [ViewMode::Source, ViewMode::Split, ViewMode::Preview] {
        assert!(!scope_to_selection(mode, true), "{mode:?}");
    }
}

#[test]
fn the_theme_goes_light_system_dark_and_back() {
    assert_eq!(theme_rotation(ThemeMode::Light), ThemeMode::System);
    assert_eq!(theme_rotation(ThemeMode::System), ThemeMode::Dark);
    assert_eq!(theme_rotation(ThemeMode::Dark), ThemeMode::Light);
}

#[test]
fn a_clamped_selection_stays_in_the_text_on_character_boundaries() {
    let inside = Selection::new(1, 1 + "中".len());
    assert_eq!(clamp_selection("a中b", inside), inside);
    // Past the end, in either direction.
    assert_eq!(
        clamp_selection("ab", Selection::new(9, 1)),
        Selection::new(2, 1)
    );
    assert_eq!(
        clamp_selection("ab", Selection::new(0, 9)),
        Selection::new(0, 2)
    );
    // Inside a character: back to its start.
    assert_eq!(
        clamp_selection("a中b", Selection::new(2, 3)),
        Selection::new(1, 1)
    );
    assert_eq!(
        clamp_selection("", Selection::new(4, 4)),
        Selection::caret(0)
    );
}

#[test]
fn a_placement_rounds_and_stays_inside_the_window_limits() {
    assert_eq!(MIN_WINDOW_WIDTH_DIP, 220);
    assert_eq!(MIN_WINDOW_HEIGHT_DIP, 120);
    assert_eq!(MAX_WINDOW_DIP, 16_384);
    for (width, height, expected) in [
        (1.0, 1.0, (220, 120)),
        (219.4, 119.4, (220, 120)),
        (219.5, 119.5, (220, 120)),
        (220.0, 120.0, (220, 120)),
        (900.4, 600.6, (900, 601)),
        (16_384.0, 16_384.0, (16_384, 16_384)),
        (16_384.6, 16_385.0, (16_384, 16_384)),
        (99_999.0, 99_999.0, (16_384, 16_384)),
    ] {
        assert_eq!(clamp_placement(width, height), expected, "{width}x{height}");
    }
}

/// All 32 combinations: exactly one opens the gate (plan 07, plan 09).
#[test]
fn input_changes_the_note_only_with_every_gate_open() {
    let mut open = 0;
    for mask in 0_u8..32 {
        let on = |bit: u8| mask & (1 << bit) != 0;
        let (recovery, preview, shell, window, reconcile) = (on(0), on(1), on(2), on(3), on(4));
        let allowed = mutation_allowed(recovery, preview, shell, window, reconcile);
        assert_eq!(
            allowed,
            !recovery && !preview && shell && window && !reconcile,
            "{mask:05b}"
        );
        open += usize::from(allowed);
    }
    assert_eq!(open, 1);
}

#[test]
fn recovery_pending_rejects_ime_commits_and_preedit() {
    assert!(!mutation_allowed(true, false, true, true, false));
    assert!(!mutation_allowed(false, true, true, true, false));
    assert!(mutation_allowed(false, false, true, true, false));
}

#[test]
fn quit_and_asset_reconcile_freeze_keyboard_and_ime_mutations() {
    assert!(!mutation_allowed(false, false, true, false, false));
    assert!(!mutation_allowed(false, false, false, true, false));
    assert!(!mutation_allowed(false, false, true, true, true));
}

/// Each fact alone raises exactly the guard it belongs to.
#[test]
fn each_fact_maps_to_its_guard() {
    type Set = fn(&mut GuardFacts);
    type Expect = fn(&mut WindowGuardSnapshot);
    let cases: [(Set, Expect); 15] = [
        (|f| f.window_focused = true, |g| g.window_focused = true),
        (|f| f.editor_composing = true, |g| g.ime_composing = true),
        (|f| f.search_composing = true, |g| g.ime_composing = true),
        (|f| f.move_resize_active = true, |g| g.dragging = true),
        (|f| f.scrollbar_dragging = true, |g| g.dragging = true),
        (|f| f.opacity_popup_open = true, |g| g.popup_open = true),
        (|f| f.search_open = true, |g| g.popup_open = true),
        (|f| f.export_in_flight = true, |g| g.popup_open = true),
        (
            |f| f.recovery_pending = true,
            |g| g.conflict_or_recovery = true,
        ),
        (|f| f.conflict = true, |g| g.conflict_or_recovery = true),
        (|f| f.paste_pending = true, |g| g.paste_pending = true),
        (
            |f| f.asset_sync_in_flight = true,
            |g| g.asset_transaction_pending = true,
        ),
        (|f| f.dirty = true, |g| g.note_save_required = true),
        (
            |f| f.durability_required = true,
            |g| g.note_save_required = true,
        ),
        (
            |f| f.note_save_in_flight = true,
            |g| g.note_save_in_flight = true,
        ),
    ];
    assert_eq!(
        guard_snapshot(GuardFacts::default()),
        WindowGuardSnapshot::default()
    );
    for (set, expect) in cases {
        let mut facts = GuardFacts::default();
        set(&mut facts);
        let mut expected = WindowGuardSnapshot::default();
        expect(&mut expected);
        assert_eq!(guard_snapshot(facts), expected, "{facts:?}");
    }
}

/// A composition in the editor or in the Search field alone keeps a docked window from
/// auto-collapsing (plan 09: never collapse during IME composition).
#[test]
fn a_composition_in_either_field_blocks_auto_collapse() {
    for facts in [
        GuardFacts {
            editor_composing: true,
            ..GuardFacts::default()
        },
        GuardFacts {
            search_composing: true,
            ..GuardFacts::default()
        },
    ] {
        assert!(guard_snapshot(facts).blocks_auto_collapse(), "{facts:?}");
    }
}

/// The quit barrier still needs the note exactly while it is dirty, a durable write is
/// still owed (`durability_required`) or a note save is in flight: all eight combinations,
/// whatever the other facts are (plan 09 quit-cancelled state).
#[test]
fn the_note_barrier_is_dirty_or_owed_or_in_flight() {
    for mask in 0_u8..8 {
        let on = |bit: u8| mask & (1 << bit) != 0;
        for others in [GuardFacts::default(), every_other_fact()] {
            let facts = GuardFacts {
                dirty: on(0),
                durability_required: on(1),
                note_save_in_flight: on(2),
                ..others
            };
            assert_eq!(
                guard_snapshot(facts).requires_note_barrier(),
                on(0) || on(1) || on(2),
                "{facts:?}"
            );
        }
    }
}

/// Every fact the note barrier does not read, raised.
fn every_other_fact() -> GuardFacts {
    GuardFacts {
        window_focused: true,
        editor_composing: true,
        search_composing: true,
        move_resize_active: true,
        scrollbar_dragging: true,
        opacity_popup_open: true,
        search_open: true,
        export_in_flight: true,
        recovery_pending: true,
        conflict: true,
        paste_pending: true,
        asset_sync_in_flight: true,
        dirty: false,
        durability_required: false,
        note_save_in_flight: false,
    }
}
