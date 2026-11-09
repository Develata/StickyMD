//! Layout and state tests of the toolbar controls.
//!
//! plan_ref: docs/plan/09_windows_shell.md#window-geometry
//! plan_ref: docs/plan/09_windows_shell.md#theme-opacity-and-content-zoom

use std::time::Instant;

use stickymd_app::config::{ConfigCoordinator, RuntimeConfig, ViewMode};
use stickymd_app::flow::{WindowPreferenceEffect, coordinate_window_preference};
use stickymd_app::instruction::{WindowPreferenceIntent, WindowResizeEdge};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::keyboard::PhysicalKey;

use super::*;

#[test]
fn cancelling_the_opacity_popup_ends_every_edit_and_restores_the_committed_value() {
    for (focused, dragging) in [(false, false), (true, false), (false, true), (true, true)] {
        let mut state = ControlState::new(80);
        state.opacity_popup_open = true;
        state.opacity_input_focused = focused;
        state.opacity_dragging = dragging;
        state.preview(55);
        state.type_input("4");
        state.cancel(80);
        assert!(!state.opacity_popup_open && !state.opacity_input_focused);
        assert!(!state.opacity_dragging, "a later release must not commit");
        assert_eq!(
            (state.opacity_preview, state.opacity_input.as_str()),
            (80, "80")
        );
    }
}

/// The open popup takes every press inside it: on the slider, on the field, or on its own
/// surface, which covers the note, Search and scrollbars under it; a press outside is not its.
#[test]
fn the_open_popup_takes_every_press_inside_it() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        for (width, height) in [(520, 680), (330, 360), (1200, 900), (220, 120)] {
            let size = PhysicalSize::new(
                (f64::from(width) * scale).round() as u32,
                (f64::from(height) * scale).round() as u32,
            );
            let layout = ControlLayout::new(ViewMode::Source, size, scale);
            let center = |rect: ControlRect| {
                PhysicalPosition::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0)
            };
            let popup = layout.opacity_popup;
            let label = format!("{size:?} at {scale}");
            assert_eq!(
                layout.popup_hit(center(layout.opacity_slider)),
                Some(PopupHit::Slider),
                "{label}"
            );
            assert_eq!(
                layout.popup_hit(center(layout.opacity_input)),
                Some(PopupHit::Field),
                "{label}"
            );
            let gap = PhysicalPosition::new(
                (layout.opacity_slider.x + layout.opacity_slider.width + layout.opacity_input.x)
                    / 2.0,
                center(layout.opacity_input).y,
            );
            for point in [
                PhysicalPosition::new(popup.x, popup.y),
                PhysicalPosition::new(popup.x + popup.width - 1.0, popup.y + popup.height - 1.0),
                gap,
            ] {
                assert_eq!(
                    layout.popup_hit(point),
                    Some(PopupHit::Blank),
                    "{label} {point:?}"
                );
            }
            for point in [
                PhysicalPosition::new(popup.x, popup.y - 1.0),
                PhysicalPosition::new(popup.x, popup.y + popup.height),
                PhysicalPosition::new(popup.x - 1.0, popup.y),
            ] {
                assert_eq!(layout.popup_hit(point), None, "{label} {point:?}");
            }
        }
    }
    // The round 2 review press: inside the popup, on neither control.
    let layout = ControlLayout::new(ViewMode::Source, PhysicalSize::new(520, 680), 1.0);
    assert_eq!(
        layout.popup_hit(PhysicalPosition::new(300.0, 40.0)),
        Some(PopupHit::Blank)
    );
    // The round 3 review press: the minimum window's popup covers its left resize border.
    let layout = ControlLayout::new(ViewMode::Source, PhysicalSize::new(220, 120), 1.0);
    assert_eq!(
        layout.popup_hit(PhysicalPosition::new(1.0, 40.0)),
        Some(PopupHit::Blank)
    );
}

#[test]
fn phase8_controls_share_layout_between_hit_testing_and_painting() {
    let layout = ControlLayout::new(ViewMode::Source, PhysicalSize::new(520, 680), 1.0);
    for id in [
        ControlId::Source,
        ControlId::Split,
        ControlId::Preview,
        ControlId::ConvertMath,
        ControlId::Topmost,
        ControlId::Theme,
        ControlId::Opacity,
        ControlId::Collapse,
        ControlId::Close,
    ] {
        let rect = layout.rect(id);
        let center = PhysicalPosition::new(rect.x + rect.width / 2.0, rect.y + rect.height / 2.0);
        assert_eq!(layout.control_at(center), Some(id));
    }
}

/// The pointer decisions through the real preference reducer: every drag step is a
/// `PreviewOpacity` the reducer does not persist; a valid release yields one `CommitOpacity`,
/// which the reducer persists when the value changed; after Esc a release yields nothing.
/// The runtime's config submission and the bytes on disk are not exercised here.
#[test]
fn phase8_a_slider_drag_commits_at_most_once_on_release() {
    let mut config = ConfigCoordinator::loaded(RuntimeConfig::default());
    let mut controls = ControlState::new(96);
    controls.opacity_popup_open = true;
    // Applies one intent as the runtime does; returns the value the reducer asks to persist.
    let apply = |config: &mut ConfigCoordinator, controls: &mut ControlState, intent| {
        let effect = coordinate_window_preference(config, intent).unwrap();
        let WindowPreferenceEffect::ApplyOpacity { opacity, persist } = effect else {
            panic!("{effect:?}");
        };
        controls.preview(opacity);
        persist.then_some(opacity)
    };
    // The runtime writes the config whenever it is dirty, from whichever event comes next
    // (`submit_config_if_needed`); a drag must therefore leave it clean at every step.
    let assert_clean = |config: &mut ConfigCoordinator, step: &str| {
        assert!(!config.is_dirty(), "{step} dirtied the config");
        assert_eq!(config.current().opacity, 96, "{step}");
        assert!(config.begin_persist().is_none(), "{step} would be written");
    };
    let mut persisted = Vec::new();
    let press = controls.press_slider(80);
    persisted.extend(apply(&mut config, &mut controls, press));
    assert_clean(&mut config, "the press");
    for value in [70, 60, 55] {
        let drag = controls.drag_slider(|| value).expect("dragging previews");
        persisted.extend(apply(&mut config, &mut controls, drag));
        assert_clean(&mut config, "a drag step");
    }
    assert!(persisted.is_empty(), "{persisted:?}");
    let release = controls.release_slider().expect("the release commits");
    persisted.extend(apply(&mut config, &mut controls, release));
    assert_eq!(persisted, [55]);
    assert!(config.is_dirty(), "the release is written once");
    assert_eq!(config.current().opacity, 55);
    // No drag in progress: moves preview nothing and a release commits nothing.
    assert!(controls.drag_slider(|| 60).is_none());
    assert!(controls.release_slider().is_none());
    // Esc mid-drag cancels it; the later release commits nothing.
    controls.opacity_popup_open = true;
    controls.press_slider(70);
    controls.cancel(55);
    assert!(controls.drag_slider(|| 65).is_none());
    assert!(controls.release_slider().is_none());
}

/// Plan 09 opacity range and input: the slider maps every position to an integer from 40
/// to 100 in steps of one and reaches each, at every DPI; the field keeps whatever is typed
/// key by key, a commit clamps an integer into the range and a non-integer commits nothing
/// (plan 09, AC-024; USER decision D5, 2026-10-20); the configured default is 96.
#[test]
fn the_opacity_range_is_40_to_100_in_integer_steps() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let layout = ControlLayout::new(ViewMode::Source, PhysicalSize::new(520, 680), scale);
        let slider = layout.opacity_slider;
        let mut previous = layout.opacity_at(slider.x - 40.0);
        assert_eq!(previous, 40, "left of the slider at {scale}");
        let mut reached = std::collections::BTreeSet::from([previous]);
        let mut x = slider.x - 40.0;
        while x <= slider.x + slider.width + 40.0 {
            let value = layout.opacity_at(x);
            assert!(
                value >= previous && value - previous <= 1,
                "{previous} -> {value} at x={x}, scale {scale}"
            );
            reached.insert(value);
            previous = value;
            x += 0.25;
        }
        assert_eq!(previous, 100, "right of the slider at {scale}");
        assert_eq!(reached, (40..=100).collect(), "every value at {scale}");
    }

    // Each entry is typed one key at a time into a freshly focused field.
    let typed = |keys: &str| {
        let mut state = ControlState::new(70);
        state.opacity_popup_open = true;
        state.focus_input();
        for key in keys.chars() {
            state.type_input(&key.to_string());
        }
        state
    };
    for (keys, field, committed) in [
        ("0", "0", Some(40)),
        ("39", "39", Some(40)),
        ("40", "40", Some(40)),
        ("96", "96", Some(96)),
        ("100", "100", Some(100)),
        ("101", "101", Some(100)),
        ("999", "999", Some(100)),
        ("1000", "1000", Some(100)),
        ("007", "007", Some(40)),
        ("096", "096", Some(96)),
        ("0096", "0096", Some(96)),
        ("0000", "0000", Some(40)),
        ("-1", "-1", Some(40)),
        ("-0", "-0", Some(40)),
        ("-250", "-250", Some(40)),
        // More digits than any integer type holds still clamp.
        (
            "123456789012345678901234567890",
            "123456789012345678901234567890",
            Some(100),
        ),
        (
            "-123456789012345678901234567890",
            "-123456789012345678901234567890",
            Some(40),
        ),
        // AC-024: out of range clamps; a non-integer commits nothing.
        ("35", "35", Some(40)),
        ("105", "105", Some(100)),
        ("96.5", "96.5", None),
        // Every other character is kept and makes the entry a non-integer.
        ("4.5", "4.5", None),
        ("9%", "9%", None),
        (" 9", " 9", None),
        ("9x", "9x", None),
        ("+5", "+5", None),
        ("5-5", "5-5", None),
        ("--5", "--5", None),
        ("-", "-", None),
        ("x", "x", None),
        // An integer is ASCII digits.
        ("\u{663}\u{665}", "\u{663}\u{665}", None),
        ("\u{ff15}\u{ff10}", "\u{ff15}\u{ff10}", None),
        // So does a control character an IME commit carries.
        ("9\t6", "9\t6", None),
    ] {
        let mut state = typed(keys);
        assert_eq!(state.opacity_input, field, "{keys:?}");
        let before = state.opacity_preview;
        assert_eq!(state.commit_input(), committed, "{keys:?}");
        match committed {
            Some(value) => {
                assert_eq!(state.opacity_preview, value, "{keys:?}");
                assert_eq!(state.opacity_input, value.to_string(), "{keys:?}");
            }
            None => assert_eq!(state.opacity_preview, before, "{keys:?} changed the value"),
        }
    }
    // A whole entry delivered as one text event types the same.
    let mut state = typed("");
    state.type_input("-1000");
    assert_eq!(state.opacity_input, "-1000");
    // An empty field commits nothing.
    let mut state = typed("");
    state.backspace_input();
    assert_eq!(state.opacity_input, "");
    assert_eq!(state.commit_input(), None, "an empty field commits nothing");
    assert_eq!(state.opacity_preview, 70);

    let default = RuntimeConfig::default().opacity;
    assert_eq!(default, 96);
    assert_eq!(ControlState::new(default).opacity_preview, 96);

    // The preference reducer clamps both previews and commits into the range again.
    let mut config = ConfigCoordinator::loaded(RuntimeConfig::default());
    for (requested, applied) in [
        (0, 40),
        (39, 40),
        (40, 40),
        (100, 100),
        (101, 100),
        (255, 100),
    ] {
        for intent in [
            WindowPreferenceIntent::PreviewOpacity(requested),
            WindowPreferenceIntent::CommitOpacity(requested),
        ] {
            let effect = coordinate_window_preference(&mut config, intent).unwrap();
            assert!(
                matches!(effect, WindowPreferenceEffect::ApplyOpacity { opacity, .. } if opacity == applied),
                "{intent:?} -> {effect:?}"
            );
        }
        assert_eq!(config.current().opacity, applied, "{requested} committed");
    }
}

#[test]
fn phase8_opacity_slider_and_input_clamp_only_on_commit() {
    let layout = ControlLayout::new(ViewMode::Source, PhysicalSize::new(520, 680), 1.0);
    assert_eq!(layout.opacity_at(layout.opacity_slider.x - 100.0), 40);
    assert_eq!(
        layout.opacity_at(layout.opacity_slider.x + layout.opacity_slider.width + 100.0),
        100
    );
    let mut state = ControlState::new(96);
    state.focus_input();
    state.type_input("7");
    assert_eq!(state.opacity_input, "7");
    assert_eq!(state.opacity_preview, 96);
    assert_eq!(state.commit_input(), Some(40));
    state.type_input("x");
    assert_eq!(state.opacity_input, "x");
    assert_eq!(state.commit_input(), None, "a non-integer commits nothing");
    assert_eq!(state.opacity_preview, 40);
}

/// Editing the field: focusing selects its value, so the first typed key or Backspace
/// replaces it and later keys append or remove one character; a value the slider or a
/// commit writes is selected again. The focused field paints its typed text.
#[test]
fn the_opacity_field_edits_like_a_selected_text_field() {
    let mut state = ControlState::new(96);
    assert_eq!(
        state.focused_field(),
        None,
        "an unfocused field paints its value"
    );
    state.focus_input();
    assert_eq!(state.focused_field().unwrap().text(), b"96");
    state.type_input("5");
    assert_eq!(
        state.opacity_input, "5",
        "the first key replaces the selected value"
    );
    state.type_input("0");
    assert_eq!(state.opacity_input, "50", "later keys append");
    state.type_input(".");
    assert_eq!(state.opacity_input, "50.", "every character is typed");
    state.backspace_input();
    state.backspace_input();
    assert_eq!(state.opacity_input, "5");
    assert_eq!(state.focused_field().unwrap().text(), b"5");
    // Any character replaces a selected value, a minus sign as any other.
    state.focus_input();
    state.type_input("x");
    assert_eq!(state.opacity_input, "x");
    state.focus_input();
    state.type_input("-");
    assert_eq!(
        state.opacity_input, "-",
        "a minus sign replaces the selected value"
    );
    // Backspace on a selected value clears all of it.
    state.type_input("23");
    assert_eq!(state.opacity_input, "-23");
    state.focus_input();
    state.backspace_input();
    assert_eq!(state.opacity_input, "");
    // The slider writes a value, selected again.
    state.type_input("4");
    state.preview(60);
    state.type_input("7");
    assert_eq!(state.opacity_input, "7");
    // So does a commit.
    state.type_input("5");
    assert_eq!(state.commit_input(), Some(75));
    state.type_input("8");
    assert_eq!(state.opacity_input, "8");
    // A long entry keeps its last characters for painting.
    state.focus_input();
    state.type_input("-12345678901234567890123456789012345");
    let field = state.focused_field().unwrap();
    assert_eq!(field.text().len(), OpacityField::TAIL);
    assert_eq!(field.text(), b"45678901234567890123456789012345");
    // Digits, `-` and `.` paint as themselves, any other character as `?`, a character at a
    // time however many bytes it takes.
    state.focus_input();
    state.type_input("-9.6x\u{663}");
    assert_eq!(state.focused_field().unwrap().text(), b"-9.6??");
    state.focus_input();
    state.type_input(&format!("{}5", "\u{663}".repeat(40)));
    let field = state.focused_field().unwrap();
    assert_eq!(field.text().len(), OpacityField::TAIL);
    assert_eq!(field.text(), format!("{}5", "?".repeat(31)).as_bytes());
}

/// The Enter that ends an edit keeps its auto-repeats and its release, so holding it does not
/// type newlines into the note behind the field; a fresh press of it, or its release, ends
/// the claim, and other keys are not touched.
#[test]
fn the_key_that_ends_an_edit_keeps_its_repeats_and_release() {
    use winit::keyboard::KeyCode;
    let enter = PhysicalKey::Code(KeyCode::Enter);
    let other = PhysicalKey::Code(KeyCode::KeyA);
    let mut state = ControlState::new(96);
    assert!(
        !state.takes_commit_key_event(enter, true, true),
        "nothing is claimed"
    );
    state.claim_commit_key(enter);
    assert!(
        !state.takes_commit_key_event(other, true, false),
        "another key"
    );
    assert!(
        !state.takes_commit_key_event(other, true, true),
        "another repeat"
    );
    assert!(state.takes_commit_key_event(enter, true, true), "a repeat");
    assert!(
        state.takes_commit_key_event(enter, true, true),
        "every repeat"
    );
    assert!(
        state.takes_commit_key_event(enter, false, false),
        "the release"
    );
    assert!(
        !state.takes_commit_key_event(enter, true, true),
        "the release ended the claim"
    );
    // A fresh press (its release was missed) ends the claim and goes on.
    state.claim_commit_key(enter);
    assert!(
        !state.takes_commit_key_event(enter, true, false),
        "a fresh press"
    );
    assert!(
        !state.takes_commit_key_event(enter, true, true),
        "the fresh press ended the claim"
    );
}

/// While edited the field takes every key and IME commit; ending the edit (Enter, or the
/// field losing focus) commits an integer, or shows the value again with nothing committed.
#[test]
fn the_opacity_field_takes_keys_while_edited_and_commits_when_it_ends() {
    for (open, focused) in [(false, false), (false, true), (true, false), (true, true)] {
        let mut state = ControlState::new(96);
        state.opacity_popup_open = open;
        if focused {
            state.focus_input();
        }
        assert_eq!(
            state.takes_keys(),
            open && focused,
            "open {open} focused {focused}"
        );
    }
    let mut state = ControlState::new(96);
    state.opacity_popup_open = true;
    state.focus_input();
    state.type_input("50");
    assert_eq!(state.finish_input(), Some(50));
    assert!(!state.opacity_input_focused && !state.takes_keys());
    assert_eq!(
        (state.opacity_preview, state.opacity_input.as_str()),
        (50, "50")
    );
    assert!(
        state.opacity_input_selected,
        "the committed value is selected"
    );
    // A field without an integer commits nothing and shows the value again, selected.
    state.focus_input();
    state.type_input("-");
    assert_eq!(state.finish_input(), None);
    assert!(!state.opacity_input_focused);
    assert_eq!(
        (state.opacity_preview, state.opacity_input.as_str()),
        (50, "50")
    );
    assert!(state.opacity_input_selected, "the shown value is selected");
    state.focus_input();
    state.type_input("7");
    assert_eq!(
        state.opacity_input, "7",
        "the shown value is selected again"
    );
}

#[test]
fn phase10_compact_layout_keeps_every_control_inside_without_overlap() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let width = (220.0 * scale) as u32;
        let layout = ControlLayout::new(
            ViewMode::Source,
            PhysicalSize::new(width, (120.0 * scale) as u32),
            scale,
        );
        let mut previous_right = 0.0;
        for id in [
            ControlId::Source,
            ControlId::Split,
            ControlId::Preview,
            ControlId::ConvertMath,
            ControlId::Topmost,
            ControlId::Theme,
            ControlId::Opacity,
            ControlId::Collapse,
            ControlId::Close,
        ] {
            let rect = layout.rect(id);
            assert!(rect.width / scale >= 23.0);
            assert!(rect.x + f64::EPSILON * 256.0 >= previous_right);
            assert!(rect.x + rect.width <= f64::from(width) + f64::EPSILON);
            previous_right = rect.x + rect.width;
        }
    }
}

#[test]
fn phase14_split_sync_hit_target_is_centered_on_divider() {
    for scale in [1.0, 1.25, 1.5, 2.0] {
        let rect = split_sync_rect(260, 34, scale);
        assert!(((rect.x + rect.width / 2.0) - 260.0).abs() < f64::EPSILON * 32.0);
        assert!(rect.width / scale >= 22.0);
        assert!(rect.y >= 34.0);
    }
}

#[test]
#[ignore = "Release-only Phase 8 control-layout performance receipt"]
fn phase8_performance_control_layout_and_hit_test() {
    let mut samples = Vec::with_capacity(25);
    for _ in 0..25 {
        let started = Instant::now();
        let mut hits = 0usize;
        for index in 0..100_000 {
            let layout = ControlLayout::new(
                ViewMode::Source,
                PhysicalSize::new(520 + (index % 400), 680),
                [1.0, 1.25, 1.5, 2.0][index as usize % 4],
            );
            hits += layout
                .control_at(PhysicalPosition::new((index % 520) as f64, 17.0))
                .is_some() as usize;
        }
        std::hint::black_box(hits);
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    eprintln!(
        "phase8 control_layout_100k median={:?} p95={:?} max={:?}",
        samples[samples.len() / 2],
        samples[samples.len() * 95 / 100],
        samples[samples.len() - 1]
    );
}

/// Plan 09 gutter geometry: in every view, at every DPI and window size, `geometry()`
/// reserves exactly the scaled gutter right of each pane (before the divider in a Split,
/// before the window edge otherwise), and each pane's scrollbar track lies inside its
/// gutter, clear of the native resize border (the real hit test finds no edge on any track
/// pixel) and of the Split sync button.
#[test]
fn every_view_reserves_the_gutter_clear_of_the_resize_border_and_sync_button() {
    use crate::geometry::{GUTTER_DIP, geometry};
    use crate::scroll::{ScrollPane, Scrollbar};
    for scale in [1.0_f32, 1.25, 1.5, 2.0] {
        let gutter = (GUTTER_DIP * scale).round() as u32;
        for (width, height) in [(520, 680), (330, 360), (1200, 900), (1921, 1081)] {
            let size = PhysicalSize::new(
                (width as f32 * scale).round() as u32,
                (height as f32 * scale).round() as u32,
            );
            for mode in [ViewMode::Source, ViewMode::Preview, ViewMode::Split] {
                let view = geometry(mode, size, scale);
                let label = format!("{mode:?} {size:?} at {scale}");
                let panes = [
                    (ScrollPane::Source, view.source),
                    (ScrollPane::Preview, view.preview),
                ];
                assert_eq!(
                    panes.iter().filter(|(_, pane)| pane.is_some()).count(),
                    if mode == ViewMode::Split { 2 } else { 1 },
                    "{label}"
                );
                for (side, pane) in panes {
                    let Some(pane) = pane else { continue };
                    let right = pane.x + pane.width;
                    let limit = match (side, view.divider_x) {
                        (ScrollPane::Source, Some(divider)) => divider,
                        _ => size.width,
                    };
                    assert_eq!(right + gutter, limit, "{label}: {side:?} gutter");
                    let Some(bar) = Scrollbar::new(
                        side,
                        pane,
                        scale,
                        0.5,
                        0.2,
                        side == ScrollPane::Source && view.divider_x.is_some(),
                    ) else {
                        continue;
                    };
                    let track = bar.track;
                    assert!(
                        track.x >= right && track.x + track.width <= limit,
                        "{label}: {side:?} track {track:?} outside its gutter"
                    );
                    let (left, top) = (f64::from(track.x), f64::from(track.y));
                    let (last_x, last_y) = (
                        f64::from(track.x + track.width - 1),
                        f64::from(track.y + track.height - 1),
                    );
                    for (x, y) in [
                        (left, top),
                        (last_x, top),
                        (left, last_y),
                        (last_x, last_y),
                        ((left + last_x) / 2.0, (top + last_y) / 2.0),
                    ] {
                        assert_eq!(
                            resize_direction(size, PhysicalPosition::new(x, y), f64::from(scale)),
                            None,
                            "{label}: {side:?} track pixel ({x},{y}) is a resize edge"
                        );
                    }
                    if let (ScrollPane::Source, Some(divider)) = (side, view.divider_x) {
                        let sync = split_sync_rect(divider, view.toolbar_height, f64::from(scale));
                        let apart = sync.x + sync.width <= left
                            || sync.x >= f64::from(track.x + track.width)
                            || sync.y + sync.height <= top
                            || sync.y >= f64::from(track.y + track.height);
                        assert!(
                            apart,
                            "{label}: the Source track {track:?} meets the sync button {sync:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn phase8_resize_hit_test_prioritizes_all_eight_border_regions() {
    let size = PhysicalSize::new(520, 680);
    assert_eq!(
        resize_direction(size, PhysicalPosition::new(1.0, 1.0), 1.0),
        Some(WindowResizeEdge::NorthWest)
    );
    assert_eq!(
        resize_direction(size, PhysicalPosition::new(519.0, 679.0), 1.0),
        Some(WindowResizeEdge::SouthEast)
    );
    assert_eq!(
        resize_direction(size, PhysicalPosition::new(260.0, 340.0), 1.0),
        None
    );
}

/// The order the press path always had: the popup covers the border it lies over, the border
/// covers the button rows inside it, the Split control sits under the toolbar, and what is left of
/// the toolbar moves the window.
#[test]
fn a_hit_is_the_first_of_popup_border_split_control_button_and_toolbar() {
    let at = |x, y| PhysicalPosition::new(x, y);
    let layout = ControlLayout::new(ViewMode::Split, PhysicalSize::new(520, 680), 1.0);
    let source = layout.rect(ControlId::Source);
    assert_eq!(
        layout.hit_test(at(20.0, 4.0), false),
        Some(ControlHit::Resize(WindowResizeEdge::North)),
        "the border takes the button's top rows"
    );
    assert!(source.contains(at(20.0, 4.0)));
    assert_eq!(
        layout.hit_test(at(20.0, 17.0), false),
        Some(ControlHit::Button(ControlId::Source))
    );
    assert_eq!(
        layout.hit_test(at(35.0, 17.0), false),
        Some(ControlHit::DragWindow)
    );
    let sync = layout.split_sync.unwrap();
    assert_eq!(
        layout.hit_test(
            at(sync.x + sync.width / 2.0, sync.y + sync.height / 2.0),
            false
        ),
        Some(ControlHit::SplitSync)
    );
    assert_eq!(layout.hit_test(at(100.0, 300.0), false), None);
    // The far edges are inclusive, and the top and bottom bands reach past the sides.
    assert_eq!(
        layout.hit_test(at(520.0, 300.0), false),
        Some(ControlHit::Resize(WindowResizeEdge::East))
    );
    assert_eq!(
        layout.hit_test(at(-1.0, 3.0), false),
        Some(ControlHit::Resize(WindowResizeEdge::North))
    );

    // Only an open popup answers; at the minimum size it covers the left border.
    let small = ControlLayout::new(ViewMode::Source, PhysicalSize::new(220, 120), 1.0);
    assert_eq!(small.split_sync, None);
    assert_eq!(
        small.hit_test(at(1.0, 40.0), true),
        Some(ControlHit::Popup(PopupHit::Blank))
    );
    assert_eq!(
        small.hit_test(at(1.0, 40.0), false),
        Some(ControlHit::Resize(WindowResizeEdge::West))
    );
}
