//! Opacity field routing through the real shell, using native observations as inputs.
//!
//! plan_ref: docs/plan/09_windows_shell.md#theme-opacity-and-content-zoom

use super::*;
use crate::app::headless_tests::fixture;
use crate::app::input::ShellKey;
use crate::interaction::ImeSignal;

fn key(code: KeyCode, text: Option<&str>) -> ShellKey<'_> {
    ShellKey {
        physical_key: PhysicalKey::Code(code),
        state: ElementState::Pressed,
        repeat: false,
        ime_owned: false,
        text,
    }
}

fn focus_field(app: &mut StickyApp) {
    app.controls.opacity_popup_open = true;
    let size = PhysicalSize::new(800, 500);
    let field = ControlLayout::new(size, 1.0).opacity_input;
    app.cursor_position =
        PhysicalPosition::new(field.x + field.width / 2.0, field.y + field.height / 2.0);
    assert!(app.apply_shell_mouse_button(ElementState::Pressed, MouseButton::Left, size, 1.0));
}

#[test]
fn runtime_opacity_enter_keeps_repeats_and_synthetic_release_from_note_and_search() {
    for search in [false, true] {
        for code in [KeyCode::Enter, KeyCode::NumpadEnter] {
            let mut app = fixture("canonical text text");
            app.session.focused = true;
            app.search.open = search;
            if search {
                // A query with matches, so an Enter leaking into Search would move its match.
                app.search.query = "text".into();
                let (text, generation) = {
                    let view = app.coordinator.view();
                    (view.text.to_owned(), view.generation)
                };
                app.search.refresh(&text, generation);
            }
            focus_field(&mut app);
            assert!(app.controls.takes_keys());
            assert!(!app.ime_allowed_for(search));
            let snapshot = app.coordinator.snapshot();
            let previous = app.config.current().opacity;
            app.apply_key(key(KeyCode::Digit7, Some("70")), false);
            assert_eq!(app.controls.opacity_input, "70");
            assert_eq!(
                app.config.current().opacity,
                previous,
                "typing does not commit"
            );
            for ignored in [
                ShellKey {
                    ime_owned: true,
                    ..key(code, None)
                },
                key(code, None),
            ] {
                app.apply_key(ignored, !ignored.ime_owned);
                assert!(app.controls.takes_keys());
            }
            app.apply_key(key(code, None), false);
            assert_eq!(app.config.current().opacity, 70);
            assert!(!app.controls.takes_keys());
            assert!(app.ime_allowed_for(search));
            let repeat = ShellKey {
                repeat: true,
                ..key(code, Some("\r"))
            };
            let release = ShellKey {
                state: ElementState::Released,
                ..key(code, None)
            };
            let active = app.search.active_range();
            let selection = app.session.selection;
            for _ in 0..3 {
                app.apply_key(repeat, false);
            }
            // Focus-synthesized release must not release ownership of a held Enter.
            app.apply_key(release, true);
            app.apply_key(repeat, false);
            assert_eq!(app.coordinator.snapshot(), snapshot);
            assert_eq!(app.search.query, if search { "text" } else { "" });
            assert_eq!(
                app.search.active_range(),
                active,
                "a repeat moved the match"
            );
            assert_eq!(
                app.session.selection, selection,
                "a repeat moved the selection"
            );
            app.apply_key(release, false);
            if !search {
                app.apply_key(key(code, None), false);
                assert_ne!(
                    app.coordinator.snapshot(),
                    snapshot,
                    "fresh Enter reaches editor"
                );
            }
        }
    }
}

#[test]
fn runtime_opacity_blur_commits_before_other_click_hide_or_quit() {
    for reason in ["window", "blank", "outside", "slider", "hide", "quit"] {
        let mut app = fixture("canonical text");
        app.session.focused = true;
        focus_field(&mut app);
        app.apply_key(key(KeyCode::Digit7, Some("70")), false);
        let snapshot = app.coordinator.snapshot();
        match reason {
            "window" => app.apply_focus_change(None, false),
            "blank" | "outside" | "slider" => {
                let size = PhysicalSize::new(800, 500);
                let layout = ControlLayout::new(size, 1.0);
                app.cursor_position = match reason {
                    "blank" => PhysicalPosition::new(
                        layout.opacity_popup.x + 2.0,
                        layout.opacity_popup.y + 2.0,
                    ),
                    "slider" => PhysicalPosition::new(
                        layout.opacity_slider.x + 2.0,
                        layout.opacity_slider.y + 2.0,
                    ),
                    _ => PhysicalPosition::new(200.0, 300.0),
                };
                app.apply_shell_mouse_button(ElementState::Pressed, MouseButton::Left, size, 1.0);
            }
            "hide" => {
                let guards = app.window_guards();
                app.dispatch_window_intent(
                    None,
                    WindowIntent::TrayToggleRequested { now_ms: 1, guards },
                );
            }
            "quit" => {
                let guards = app.window_guards();
                app.dispatch_window_intent(None, WindowIntent::TrayQuitRequested { guards });
            }
            _ => unreachable!(),
        }
        assert_eq!(app.config.current().opacity, 70, "{reason}");
        assert!(!app.controls.opacity_input_focused, "{reason}");
        assert_eq!(app.coordinator.snapshot(), snapshot);
        if reason == "slider" {
            assert!(app.controls.opacity_dragging);
            assert_ne!(
                app.controls.opacity_preview, 70,
                "new slider preview follows field commit"
            );
        }
    }
}

#[test]
fn runtime_opacity_refuses_composition_and_routes_late_ime_to_field() {
    for editor in [false, true] {
        let mut app = fixture("canonical text");
        app.session.focused = true;
        let snapshot = app.coordinator.snapshot();
        if editor {
            app.session
                .handle_ime(ImeSignal::Enabled, snapshot.generation, 1);
            app.session.handle_ime(
                ImeSignal::Preedit {
                    text: "nihao".into(),
                    cursor: Some(0..5),
                },
                snapshot.generation,
                2,
            );
        } else {
            app.search.open = true;
            app.search.set_preedit("nihao".into(), Some((0, 5)));
        }
        focus_field(&mut app);
        assert!(!app.controls.takes_keys());
        assert!(app.window_guards().ime_composing);
        app.apply_focus_change(None, false);
        app.apply_focus_change(None, true);
        focus_field(&mut app);
        assert!(app.controls.takes_keys());
        assert!(!app.ime_allowed_for(true));
        app.handle_ime(Ime::Preedit("zhong".into(), Some((0, 5))));
        assert!(!app.window_guards().ime_composing);
        app.handle_ime(Ime::Commit("73".into()));
        assert_eq!(app.controls.opacity_input, "73");
        assert_eq!(app.coordinator.snapshot(), snapshot);
        assert_eq!(app.search.query, "");
        app.commit_opacity_input();
        assert_eq!(app.config.current().opacity, 73);
        assert!(app.ime_allowed_for(true));
        app.preview_focused = true;
        assert!(!app.ime_allowed_for(false));
        app.shell_input_enabled = false;
        assert!(!app.ime_allowed_for(true));
    }
}

#[test]
fn runtime_opacity_key_priority_keeps_escape_and_save_above_field() {
    let mut app = fixture("canonical text");
    app.session.focused = true;
    focus_field(&mut app);
    let snapshot = app.coordinator.snapshot();
    let initial = app.config.current().opacity;
    app.apply_key(key(KeyCode::Digit7, Some("77")), false);
    let selection = app.session.selection;
    app.modifiers = ModifiersState::CONTROL;
    // Ctrl+2/3 would leave the default Source view and Ctrl+A would select the note, so each
    // leak shows; Ctrl+1 alone could not (the view already is Source).
    for code in [
        KeyCode::KeyF,
        KeyCode::KeyH,
        KeyCode::Digit1,
        KeyCode::Digit2,
        KeyCode::Digit3,
        KeyCode::KeyA,
    ] {
        app.apply_key(key(code, None), false);
    }
    assert!(!app.search.open);
    assert_eq!(app.config.current().view_mode, ViewMode::Source);
    assert_eq!(app.session.selection, selection, "Ctrl+A reached the note");
    assert_eq!(app.controls.opacity_input, "77");
    let submitted = app.worker.metrics().note_submitted;
    app.apply_key(key(KeyCode::KeyS, None), false);
    assert_eq!(app.worker.metrics().note_submitted, submitted + 1);
    assert!(app.controls.takes_keys(), "save does not end field editing");
    app.apply_key(key(KeyCode::Escape, None), false);
    assert!(!app.controls.opacity_popup_open);
    assert!(!app.controls.opacity_input_focused);
    assert_eq!(
        app.config.current().opacity,
        initial,
        "Esc reverts instead of committing"
    );
    assert_eq!(app.coordinator.snapshot(), snapshot);
}

#[test]
fn runtime_opacity_recovery_and_busy_export_shortcuts_precede_field() {
    use stickymd_core::{LoadedDocument, RecoveryCandidate};
    for code in [KeyCode::F6, KeyCode::F7] {
        let mut app = fixture("original");
        focus_field(&mut app);
        app.apply_key(key(KeyCode::Digit7, Some("77")), false);
        app.recovery = crate::flow::RecoveryCoordinator::new(
            Some(RecoveryCandidate {
                canonical: Some(LoadedDocument::from_durable_bytes(b"durable").unwrap()),
                temporary: LoadedDocument::from_durable_bytes(b"temporary").unwrap(),
                temporary_is_newer: true,
            }),
            false,
        );
        app.persistence.set_recovery_pending(true);
        app.apply_key(key(code, None), false);
        assert_eq!(
            app.coordinator.view().text,
            if code == KeyCode::F6 {
                "temporary"
            } else {
                "durable"
            }
        );
        assert!(app.recovery.operation_pending());
        assert_eq!(app.controls.opacity_input, "77");
        assert!(app.controls.takes_keys());
    }
    let mut app = fixture("canonical text");
    focus_field(&mut app);
    app.export_in_flight = true; // Stops at the real busy-export branch; opens no native dialog.
    app.modifiers = ModifiersState::CONTROL | ModifiersState::SHIFT;
    app.apply_key(key(KeyCode::KeyS, None), false);
    assert_eq!(app.diagnostic.as_deref(), Some("已有导出任务正在进行。"));
    assert!(app.controls.takes_keys());
    assert_eq!(app.coordinator.view().text, "canonical text");
}
