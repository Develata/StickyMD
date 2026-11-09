//! The decisions the shell handed to the session (Phase 15 C6), without a shell: what each one
//! changes and what it asks of the shell, in order. The shell's side runs in its own tests.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use crate::config::{
    ContentZoomPercent, DockEdge as ConfigDockEdge, RuntimeConfig, ThemeMode, ViewMode,
};
use crate::flow::window::geometry::{MonitorGeometry, MonitorIdentity, PhysicalRect};
use crate::flow::window::state::{
    DockEdge, StableVisibility, VisibilityState, WindowGuardSnapshot, WindowIntent,
};
use crate::instruction::WindowPreferenceIntent;
use crate::interaction::ImeSignal;
use crate::interaction::rules::{SEARCH_COMPOSING, SearchViewChange};
use crate::ports::{FailureKind, FrameFact};
use stickymd_core::Selection;

use super::tests::{run, session};
use super::{AppSession, PreparedViewChange, SearchOpenStep, ZoomInput};

/// Search open on the first "a" of "a b a".
fn searching() -> AppSession {
    let mut session = session("a b a");
    let view = session.coordinator.view();
    session
        .search
        .open(false, Some("a"), view.text, view.generation);
    session
}

/// A note whose IME is composing.
fn composing() -> AppSession {
    let mut session = session("text");
    let generation = session.coordinator.view().generation;
    session.editor.handle_ime(ImeSignal::Enabled, generation, 0);
    session.editor.handle_ime(
        ImeSignal::Preedit {
            text: "ni".into(),
            cursor: Some(2..2),
        },
        generation,
        1,
    );
    session
}

fn begin(session: &mut AppSession, mode: ViewMode) -> (Option<PreparedViewChange>, Vec<String>) {
    let mut decided = None;
    let seen = run(session, |session, out| {
        decided = session.begin_view_change(mode, out);
    });
    (decided, seen)
}

/// What a begun view change does to Search, or `None` when it was not allowed.
fn decided(prepared: &Option<PreparedViewChange>) -> Option<SearchViewChange> {
    prepared.as_ref().map(PreparedViewChange::search)
}

fn commit(session: &mut AppSession, prepared: Option<PreparedViewChange>) -> Vec<String> {
    let prepared = prepared.expect("an allowed view change");
    run(session, |session, out| {
        session.commit_view_change(prepared, out);
    })
}

#[test]
fn the_view_shown_does_not_change_again() {
    let mut session = session("text");
    let (prepared, seen) = begin(&mut session, ViewMode::Source);
    assert_eq!((decided(&prepared), seen), (None, Vec::new()));
}

#[test]
fn a_composing_search_keeps_the_view_that_has_its_pane() {
    let mut session = searching();
    session.search.set_preedit("zh".into(), Some((0, 2)));
    let (prepared, seen) = begin(&mut session, ViewMode::Preview);
    assert_eq!(
        (decided(&prepared), seen),
        (
            None,
            vec![SEARCH_COMPOSING.to_owned(), "RedrawNeeded".to_owned()]
        )
    );
    assert_eq!(session.config.current().view_mode, ViewMode::Source);
    assert!(session.search.open);
}

/// Preview parks the open Search and Source puts it back, its match selected again; the shell
/// lays the view out before the Preview is asked for and before Search moves.
#[test]
fn a_view_without_a_source_pane_parks_search_and_the_way_back_resumes_it() {
    let mut session = searching();
    let (prepared, _) = begin(&mut session, ViewMode::Preview);
    assert_eq!(decided(&prepared), Some(SearchViewChange::Park));
    let seen = commit(&mut session, prepared);
    assert_eq!(
        seen,
        [
            "ViewModeChanged(Preview)",
            "SubmitPreview",
            "ImeAreaChanged",
            "RedrawNeeded",
            "KeyboardOwnerChanged",
            "RedrawNeeded",
        ]
    );
    assert_eq!(session.config.current().view_mode, ViewMode::Preview);
    assert!(!session.search.open);
    assert!(session.parked_search.is_some());

    session.editor.selection = Selection::caret(0);
    let (prepared, _) = begin(&mut session, ViewMode::Source);
    assert_eq!(decided(&prepared), Some(SearchViewChange::Resume));
    let seen = commit(&mut session, prepared);
    assert_eq!(
        seen,
        [
            "ViewModeChanged(Source)",
            "PresentationChanged",
            "ImeAreaChanged",
            "RedrawNeeded",
            "KeyboardOwnerChanged",
            "RedrawNeeded",
        ]
    );
    assert!(session.search.open);
    assert!(session.parked_search.is_none());
    assert_eq!(session.editor.selection, Selection::new(0, 1));
}

/// A view change ends the note's composition; a closed Search stays as it is.
#[test]
fn a_view_change_ends_the_composition_and_keeps_a_closed_search() {
    let mut session = composing();
    assert!(session.editor.is_composing());
    let (prepared, _) = begin(&mut session, ViewMode::Split);
    assert_eq!(decided(&prepared), Some(SearchViewChange::Keep));
    let seen = commit(&mut session, prepared);
    assert_eq!(
        seen,
        [
            "ViewModeChanged(Split)",
            "SubmitPreview",
            "KeyboardOwnerChanged",
            "RedrawNeeded",
        ]
    );
    assert!(!session.editor.is_composing());
    assert!(!session.search.open);
    assert_eq!(session.config.current().view_mode, ViewMode::Split);
}

fn prefer(session: &mut AppSession, intent: WindowPreferenceIntent) -> Vec<String> {
    run(session, |session, out| {
        session.dispatch_window_preference(intent, out);
    })
}

/// A changed theme, Split sync or committed opacity is written at once; a previewed
/// opacity and a zoom are not, and a value already held asks the shell for nothing.
#[test]
fn a_preference_is_shown_and_written_unless_it_is_previewed_or_a_zoom() {
    let zoom = ContentZoomPercent::new_clamped(150);
    for (intent, shown, written) in [
        (
            WindowPreferenceIntent::SetTheme(ThemeMode::Dark),
            Some("ApplyTheme(Dark)"),
            true,
        ),
        (
            WindowPreferenceIntent::SetSplitScrollSync(false),
            Some("ApplySplitScrollSync(false)"),
            true,
        ),
        (
            WindowPreferenceIntent::CommitOpacity(60),
            Some("ApplyOpacity { opacity: 60, persist: true }"),
            true,
        ),
        (
            WindowPreferenceIntent::PreviewOpacity(10),
            Some("ApplyOpacity { opacity: 40, persist: false }"),
            false,
        ),
        (
            WindowPreferenceIntent::SetContentZoom(zoom),
            Some("ApplyContentZoom(ContentZoomPercent(150))"),
            false,
        ),
        (
            WindowPreferenceIntent::SetTheme(RuntimeConfig::default().theme),
            None,
            false,
        ),
    ] {
        let mut session = session("text");
        session.config_persistence_allowed = true;
        let seen = prefer(&mut session, intent);
        let expected: Vec<String> = shown
            .into_iter()
            .map(str::to_owned)
            .chain(shown.map(|_| "RedrawNeeded".to_owned()))
            .collect();
        assert_eq!(seen, expected, "{intent:?}");
        assert_eq!(session.config.is_saving(), written, "{intent:?}");
        assert_eq!(session.zoom_config_deadline, None, "{intent:?}");
    }
}

fn answer_topmost(
    session: &mut AppSession,
    id: u64,
    outcome: Result<(), FailureKind>,
) -> Vec<String> {
    run(session, |session, out| {
        session.dispatch_window_intent(
            WindowIntent::ActionCompleted {
                id,
                outcome,
                now_ms: 9,
            },
            out,
        );
    })
}

/// The user's topmost reaches the configuration, its write and the shell only after the window
/// confirms it; a failure writes nothing and the same value can be asked for again. Each request
/// is the next action id of this fresh flow.
#[test]
fn a_topmost_is_written_only_after_the_window_confirmed_it() {
    let mut session = session("text");
    session.config_persistence_allowed = true;
    let seen = prefer(&mut session, WindowPreferenceIntent::SetAlwaysOnTop(true));
    assert_eq!(
        seen,
        ["ApplyTopmost(TopmostRequest { id: 1, desired_effective: true, source: User })"]
    );
    assert!(!session.config.current().always_on_top);
    assert!(!session.config.is_saving());

    let seen = answer_topmost(&mut session, 1, Err(FailureKind::Failed));
    assert_eq!(
        seen,
        [
            "TopmostRejected { source: User, kind: Failed }",
            "ApplyTopmost(TopmostRequest { id: 2, desired_effective: false, source: Reconcile })",
        ]
    );
    assert!(!session.config.current().always_on_top);
    assert!(!session.config.is_saving());

    prefer(&mut session, WindowPreferenceIntent::SetAlwaysOnTop(true));
    let seen = answer_topmost(&mut session, 3, Ok(()));
    assert_eq!(seen, ["ApplyAlwaysOnTop(true)", "RedrawNeeded"]);
    assert!(session.config.current().always_on_top);
    assert!(session.config.is_saving());
}

/// Before the window flow starts there is no window to ask: the configured value is the one the
/// startup applies.
#[test]
fn a_topmost_before_the_window_flow_goes_straight_to_the_configuration() {
    let mut session = session("text");
    session.window_flow = None;
    let seen = prefer(&mut session, WindowPreferenceIntent::SetAlwaysOnTop(true));
    assert_eq!(seen, ["ApplyAlwaysOnTop(true)", "RedrawNeeded"]);
    assert!(session.config.current().always_on_top);
}

/// The sensor's temporary value never reaches the configuration, whatever its receipts say.
#[test]
fn a_sensor_receipt_writes_no_configuration() {
    let mut session = session("text");
    session.config_persistence_allowed = true;
    session.window_flow = Some(crate::flow::window::reducer::WindowShellCoordinator::new(
        crate::flow::window::geometry::WindowPlacement::new(900.0, 600.0, None, 0.0, 0.5, 0.5),
        StableVisibility::DockedExpanded(DockEdge::Left),
        display("primary", true),
        true,
    ));
    let seen = run(&mut session, |session, out| {
        session.dispatch_window_intent(WindowIntent::ManualCollapse { now_ms: 10 }, out);
    });
    let request = seen
        .iter()
        .find(|effect| effect.starts_with("ApplyTopmost"))
        .expect("the collapse asks for the sensor's topmost");
    assert!(request.contains("source: Sensor"), "{request}");
    answer_topmost(&mut session, 1, Ok(()));
    assert!(!session.config.current().always_on_top);
    assert!(!session.config.is_saving());
}

/// A drag's end reaches the configuration only with the frame the window was observed at, and a
/// placement that never gets there writes nothing.
#[test]
fn a_drag_is_written_only_after_its_frame_was_observed() {
    let drag = |session: &mut AppSession| {
        run(session, |session, out| {
            session.dispatch_window_intent(
                WindowIntent::DragEnded {
                    frame: PhysicalRect::new(10, 100, 780, 900),
                    monitor: display("primary", true),
                    now_ms: 10,
                },
                out,
            );
        })
    };
    let request = |seen: &[String]| {
        seen.iter()
            .find(|effect| effect.starts_with("ApplyFrame"))
            .expect("the drag places the window")
            .clone()
    };
    let observe = |session: &mut AppSession, observed, now_ms| {
        let id = session
            .window_flow
            .as_ref()
            .unwrap()
            .state()
            .pending_placement_id()
            .expect("a placement waits");
        run(session, |session, out| {
            session.dispatch_window_intent(
                WindowIntent::PlacementObserved {
                    id,
                    observed,
                    now_ms,
                },
                out,
            );
        })
    };

    let mut session = session("text");
    let seen = drag(&mut session);
    assert_eq!(
        session.config.current().window.dock_edge,
        ConfigDockEdge::None,
        "{seen:?}"
    );
    let target = session.window_flow.as_ref().unwrap().state().frame();
    assert!(request(&seen).contains("last: true"), "{seen:?}");
    let seen = observe(&mut session, Ok(FrameFact { outer: target }), 11);
    assert!(seen.is_empty(), "{seen:?}");
    assert_eq!(
        session.config.current().window.dock_edge,
        ConfigDockEdge::Left
    );

    let mut session = self::session("text");
    drag(&mut session);
    let seen = observe(&mut session, Err(FailureKind::Unsupported), 11);
    assert_eq!(seen, ["PlacementFailed(Unsupported)"]);
    assert_eq!(
        session.config.current().window.dock_edge,
        ConfigDockEdge::None
    );
}

/// The zoom's write waits 250 ms from when the shell has shown it.
#[test]
fn a_shown_zoom_is_written_once_it_has_been_still() {
    let mut session = session("text");
    session.defer_zoom_config();
    assert_eq!(session.zoom_config_deadline, Some(1_250));
}

/// A zoom from a key is written at once and leaves no settle deadline behind to write it a second
/// time; a wheel's keeps the deadline its shown zoom took and is not written yet.
#[test]
fn a_zoom_written_now_drops_its_settle_deadline() {
    let zoom = ContentZoomPercent::default().stepped(1);
    for (input, deadline, written) in [
        (ZoomInput::Key, None, true),
        (ZoomInput::Wheel, Some(1_250), false),
    ] {
        let mut session = session("text");
        session.config_persistence_allowed = true;
        prefer(&mut session, WindowPreferenceIntent::SetContentZoom(zoom));
        session.defer_zoom_config();
        session.finish_zoom_input(input);
        assert_eq!(session.zoom_config_deadline, deadline, "{input:?}");
        assert_eq!(session.config.is_saving(), written, "{input:?}");
    }
}

fn display(name: &str, primary: bool) -> MonitorGeometry {
    MonitorGeometry::new(
        MonitorIdentity::new(name),
        PhysicalRect::new(0, 0, 1920, 1080),
        1.0,
        primary,
    )
}

fn opens_on(session: &AppSession, displays: &[MonitorGeometry]) -> Option<String> {
    session
        .startup_display(displays)
        .map(|display| display.identity.as_str().to_owned())
}

#[test]
fn the_window_opens_on_the_configured_display_else_the_primary_else_any() {
    let mut session = session("text");
    let displays = [display("a", false), display("b", true), display("c", false)];
    assert_eq!(opens_on(&session, &displays).as_deref(), Some("b"));
    for (configured, expected) in [("c", "c"), ("gone", "b")] {
        session
            .config
            .update(|config| config.window.monitor_id = configured.into())
            .unwrap();
        assert_eq!(opens_on(&session, &displays).as_deref(), Some(expected));
    }
    let no_primary = [display("a", false), display("c", false)];
    session
        .config
        .update(|config| config.window.monitor_id = String::new())
        .unwrap();
    assert_eq!(opens_on(&session, &no_primary).as_deref(), Some("a"));
    assert_eq!(opens_on(&session, &[]), None);
}

/// The flow starts docked where the configuration says, and the frame the window takes first is
/// the flow's.
#[test]
fn the_window_flow_starts_where_the_configuration_put_the_window() {
    let mut session = session("text");
    session.window_flow = None;
    session
        .config
        .update(|config| config.window.dock_edge = ConfigDockEdge::Right)
        .unwrap();
    let frame = session.start_window_flow(display("b", true), true);
    let state = session.window_flow.as_ref().unwrap().state();
    assert_eq!(state.frame(), frame);
    assert!(matches!(
        state.visibility(),
        VisibilityState::Presented(StableVisibility::DockedExpanded(DockEdge::Right))
    ));
}

fn open(session: &mut AppSession, replace: bool) -> (SearchOpenStep, Vec<String>) {
    let mut step = SearchOpenStep::Done;
    let seen = run(session, |session, out| {
        step = session.open_search(replace, out);
    });
    (step, seen)
}

/// Search opens on the selected text, takes the keyboard and selects its first match.
#[test]
fn search_opens_on_the_selected_text_and_selects_its_match() {
    let mut session = session("a b a");
    session.editor.selection = Selection::new(4, 5);
    assert_eq!(
        open(&mut session, false),
        (
            SearchOpenStep::Done,
            [
                "SourceFocused",
                "PresentationChanged",
                "KeyboardOwnerChanged",
                "ImeAreaChanged",
                "RedrawNeeded",
            ]
            .map(str::to_owned)
            .to_vec()
        )
    );
    assert!(session.search.open);
    assert_eq!(session.search.query, "a");
    assert_eq!(
        session.editor.selection,
        session
            .search
            .active_range()
            .map(|range| Selection::new(range.start, range.end))
            .unwrap()
    );
}

/// On an open Search, Ctrl+F closes it and Ctrl+H shows its replacement row.
#[test]
fn the_shortcut_closes_an_open_search_or_shows_its_replacement_row() {
    let mut session = searching();
    assert_eq!(
        open(&mut session, false),
        (SearchOpenStep::Close, Vec::new())
    );
    assert!(session.search.open);
    assert_eq!(
        open(&mut session, true),
        (
            SearchOpenStep::Done,
            vec!["ImeAreaChanged".to_owned(), "RedrawNeeded".to_owned()]
        )
    );
    assert!(session.search.replace_visible);
}

/// A composition refuses opening Search: in Search the status line says so, as it does in the note.
#[test]
fn a_composition_refuses_opening_search() {
    let mut session = searching();
    session.search.set_preedit("zh".into(), Some((0, 2)));
    assert_eq!(
        open(&mut session, true),
        (
            SearchOpenStep::Done,
            vec![SEARCH_COMPOSING.to_owned(), "RedrawNeeded".to_owned()]
        )
    );
    assert!(!session.search.replace_visible);

    let mut session = composing();
    assert_eq!(
        open(&mut session, false),
        (
            SearchOpenStep::Done,
            vec![
                "请先完成当前输入法组合，再打开查找。".to_owned(),
                "RedrawNeeded".to_owned()
            ]
        )
    );
    assert!(!session.search.open);
}

/// From Preview the view changes first. A view that stayed Preview opens nothing; a parked Search
/// that came back keeps its query and only shows its row on Ctrl+H.
#[test]
fn search_from_preview_waits_for_the_source_view() {
    let mut session = searching();
    let (prepared, _) = begin(&mut session, ViewMode::Preview);
    commit(&mut session, prepared);
    assert_eq!(
        open(&mut session, true),
        (SearchOpenStep::SourceViewFirst, Vec::new())
    );
    let seen = run(&mut session, |session, out| {
        session.open_search_in_source_view(true, out);
    });
    assert!(seen.is_empty());
    assert!(!session.search.open);

    let (prepared, _) = begin(&mut session, ViewMode::Source);
    commit(&mut session, prepared);
    let seen = run(&mut session, |session, out| {
        session.open_search_in_source_view(true, out);
    });
    assert_eq!(seen, ["ImeAreaChanged", "RedrawNeeded"]);
    assert!(session.search.replace_visible);
    assert_eq!(session.search.query, "a");
}

/// Search with its replacement row open on "a" in "a b a", replacing with `replacement`; the
/// note shows the active match, the first "a", selected.
fn replacing(replacement: &str) -> AppSession {
    let mut session = session("a b a");
    let view = session.coordinator.view();
    session
        .search
        .open(true, Some("a"), view.text, view.generation);
    session.search.replacement = replacement.into();
    session.editor.selection = Selection::new(0, 1);
    session
}

fn replace_current(session: &mut AppSession) -> Vec<String> {
    run(session, |session, out| session.replace_current_match(out))
}

fn text(session: &AppSession) -> &str {
    session.coordinator.view().text
}

/// Without the replacement row, or while Search composes, neither replace touches the note.
#[test]
fn replacing_is_refused_while_it_is_not_allowed() {
    let mut hidden_row = searching();
    hidden_row.editor.selection = Selection::new(0, 1);
    let mut composing_search = replacing("x");
    composing_search
        .search
        .set_preedit("zh".into(), Some((0, 2)));
    for mut session in [hidden_row, composing_search] {
        assert!(replace_current(&mut session).is_empty());
        let seen = run(&mut session, |session, out| {
            session.replace_all_matches(out)
        });
        assert!(seen.is_empty());
        assert_eq!(text(&session), "a b a");
    }
}

/// With the note's selection elsewhere, the first press selects the active match and replaces
/// nothing; the next press replaces it.
#[test]
fn replace_current_first_selects_an_unselected_match() {
    let mut session = replacing("x");
    session.editor.selection = Selection::caret(3);
    assert_eq!(replace_current(&mut session), ["PresentationChanged"]);
    assert_eq!(text(&session), "a b a");
    assert_eq!(session.editor.selection, Selection::new(0, 1));
    replace_current(&mut session);
    assert_eq!(text(&session), "x b a");
}

/// The change reaches the note and Search first; then the next match is selected.
#[test]
fn replace_current_replaces_the_selected_match_and_selects_the_next() {
    let mut session = replacing("x");
    assert_eq!(
        replace_current(&mut session),
        [
            "PreviewDocumentChanged",
            "Delta",
            "PresentationChanged",
            "TitleDirty",
            "PresentationChanged",
        ]
    );
    assert_eq!(text(&session), "x b a");
    assert_eq!(session.editor.selection, Selection::new(4, 5));
    assert_eq!(session.search.active_range(), Some(4..5));
}

/// A replacement that contains the query is not found again in place: the next match is the one
/// after the inserted text.
#[test]
fn a_replacement_containing_the_query_is_not_replaced_again_in_place() {
    let mut session = replacing("aa");
    replace_current(&mut session);
    assert_eq!(text(&session), "aa b a");
    assert_eq!(session.editor.selection, Selection::new(5, 6));
    replace_current(&mut session);
    assert_eq!(text(&session), "aa b aa");
}

/// Replace All needs a query; an empty one asks nothing of the note or the shell.
#[test]
fn replace_all_with_an_empty_query_does_nothing() {
    let mut session = session("a b a");
    let view = session.coordinator.view();
    session.search.open(true, None, view.text, view.generation);
    session.search.replacement = "x".into();
    assert!(session.search.query.is_empty());
    let seen = run(&mut session, |session, out| {
        session.replace_all_matches(out)
    });
    assert!(seen.is_empty());
    assert_eq!(text(&session), "a b a");

    let mut session = replacing("x");
    run(&mut session, |session, out| {
        session.replace_all_matches(out)
    });
    assert_eq!(text(&session), "x b x");
}

fn tray_toggle(session: &mut AppSession, guards: WindowGuardSnapshot) -> Vec<String> {
    run(session, |session, out| {
        session.toggle_visibility_from_tray(7, guards, out);
    })
}

/// A shown window is toggled by the flow, which takes the guards from then on; one hidden to the
/// tray is shown, and the guards the flow holds stay as they were.
#[test]
fn the_tray_toggles_a_shown_window_and_shows_a_hidden_one() {
    let mut session = session("text");
    let focused = WindowGuardSnapshot {
        window_focused: true,
        ..Default::default()
    };
    let seen = tray_toggle(&mut session, focused);
    let state = session.window_flow.as_ref().unwrap().state();
    assert_eq!(state.guards(), focused);
    let VisibilityState::HidePending { id, .. } = state.visibility() else {
        panic!("a clean note hides at once: {:?}", state.visibility());
    };
    assert!(
        seen.contains(&format!("HideWindow {{ id: {id} }}")),
        "{seen:?}"
    );
    run(&mut session, |session, out| {
        session.dispatch_window_intent(
            WindowIntent::ActionCompleted {
                id,
                outcome: Ok(()),
                now_ms: 8,
            },
            out,
        );
    });
    assert!(session.hidden_to_tray());

    let unsaved = WindowGuardSnapshot {
        note_save_required: true,
        ..Default::default()
    };
    let seen = tray_toggle(&mut session, unsaved);
    let state = session.window_flow.as_ref().unwrap().state();
    let VisibilityState::ShowPending { id: show, .. } = state.visibility() else {
        panic!("a hidden window is asked to show: {:?}", state.visibility());
    };
    assert_eq!(state.guards(), focused);
    assert!(
        seen.contains(&format!("ShowWindow {{ id: {show} }}")),
        "{seen:?}"
    );
    run(&mut session, |session, out| {
        session.dispatch_window_intent(
            WindowIntent::ActionCompleted {
                id: show,
                outcome: Ok(()),
                now_ms: 9,
            },
            out,
        );
    });
    let state = session.window_flow.as_ref().unwrap().state();
    assert_eq!(
        state.visibility(),
        VisibilityState::Presented(StableVisibility::Floating)
    );
}
