//! Opacity field routing through the real shell, using native observations as inputs.
//!
//! plan_ref: docs/plan/09_windows_shell.md#theme-opacity-and-content-zoom

use super::*;
use crate::controls::ControlLayout;
use crate::shell::headless_tests::{TestApp, fixture};
use crate::shell::input::ShellKey;
use stickymd_app::interaction::ImeSignal;
use winit::dpi::PhysicalSize;
use winit::event::Ime;
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

fn key(code: KeyCode, text: Option<&str>) -> ShellKey<'_> {
    ShellKey {
        physical_key: PhysicalKey::Code(code),
        state: ElementState::Pressed,
        repeat: false,
        ime_owned: false,
        text,
    }
}

fn layout() -> ControlLayout {
    ControlLayout::new(ViewMode::Source, PhysicalSize::new(800, 500), 1.0)
}

fn focus_field(app: &mut TestApp) {
    app.controls.opacity_popup_open = true;
    let layout = layout();
    let field = layout.opacity_input;
    app.control_layout = Some(layout);
    app.cursor_position =
        PhysicalPosition::new(field.x + field.width / 2.0, field.y + field.height / 2.0);
    assert!(app.handle_shell_mouse_button(ElementState::Pressed, MouseButton::Left));
}

#[test]
fn runtime_opacity_enter_keeps_repeats_and_synthetic_release_from_note_and_search() {
    for search in [false, true] {
        for code in [KeyCode::Enter, KeyCode::NumpadEnter] {
            let mut app = fixture("canonical text text");
            app.app.editor.focused = true;
            app.app.search.open = search;
            if search {
                // A query with matches, so an Enter leaking into Search would move its match.
                app.app.search.query = "text".into();
                let (text, generation) = {
                    let view = app.app.coordinator.view();
                    (view.text.to_owned(), view.generation)
                };
                app.app.search.refresh(&text, generation);
            }
            focus_field(&mut app);
            assert!(app.controls.takes_keys());
            assert!(!app.ime_allowed_for(search));
            let snapshot = app.app.coordinator.snapshot();
            let previous = app.app.config.current().opacity;
            app.apply_key(key(KeyCode::Digit7, Some("70")), false);
            assert_eq!(app.controls.opacity_input, "70");
            assert_eq!(
                app.app.config.current().opacity,
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
            assert_eq!(app.app.config.current().opacity, 70);
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
            let active = app.app.search.active_range();
            let selection = app.app.editor.selection;
            for _ in 0..3 {
                app.apply_key(repeat, false);
            }
            // Focus-synthesized release must not release ownership of a held Enter.
            app.apply_key(release, true);
            app.apply_key(repeat, false);
            assert_eq!(app.app.coordinator.snapshot(), snapshot);
            assert_eq!(app.app.search.query, if search { "text" } else { "" });
            assert_eq!(
                app.app.search.active_range(),
                active,
                "a repeat moved the match"
            );
            assert_eq!(
                app.app.editor.selection, selection,
                "a repeat moved the selection"
            );
            app.apply_key(release, false);
            if !search {
                app.apply_key(key(code, None), false);
                assert_ne!(
                    app.app.coordinator.snapshot(),
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
        app.app.editor.focused = true;
        focus_field(&mut app);
        app.apply_key(key(KeyCode::Digit7, Some("70")), false);
        let snapshot = app.app.coordinator.snapshot();
        match reason {
            "window" => app.apply_focus_change(false),
            "blank" | "outside" | "slider" => {
                let layout = layout();
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
                app.handle_shell_mouse_button(ElementState::Pressed, MouseButton::Left);
            }
            "hide" => {
                let guards = app.window_guards();
                app.dispatch_window_intent(WindowIntent::TrayToggleRequested { now_ms: 1, guards });
            }
            "quit" => {
                let guards = app.window_guards();
                app.dispatch_window_intent(WindowIntent::TrayQuitRequested { guards });
            }
            _ => unreachable!(),
        }
        assert_eq!(app.app.config.current().opacity, 70, "{reason}");
        assert!(!app.controls.opacity_input_focused, "{reason}");
        assert_eq!(app.app.coordinator.snapshot(), snapshot);
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
        app.app.editor.focused = true;
        let snapshot = app.app.coordinator.snapshot();
        if editor {
            app.app
                .editor
                .handle_ime(ImeSignal::Enabled, snapshot.generation, 1);
            app.app.editor.handle_ime(
                ImeSignal::Preedit {
                    text: "nihao".into(),
                    cursor: Some(0..5),
                },
                snapshot.generation,
                2,
            );
        } else {
            app.app.search.open = true;
            app.app.search.set_preedit("nihao".into(), Some((0, 5)));
        }
        focus_field(&mut app);
        assert!(!app.controls.takes_keys());
        assert!(app.window_guards().ime_composing);
        app.apply_focus_change(false);
        app.apply_focus_change(true);
        focus_field(&mut app);
        assert!(app.controls.takes_keys());
        assert!(!app.ime_allowed_for(true));
        app.handle_ime(Ime::Preedit("zhong".into(), Some((0, 5))));
        assert!(!app.window_guards().ime_composing);
        app.handle_ime(Ime::Commit("73".into()));
        assert_eq!(app.controls.opacity_input, "73");
        assert_eq!(app.app.coordinator.snapshot(), snapshot);
        assert_eq!(app.app.search.query, "");
        app.commit_opacity_input();
        assert_eq!(app.app.config.current().opacity, 73);
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
    app.app.editor.focused = true;
    focus_field(&mut app);
    let snapshot = app.app.coordinator.snapshot();
    let initial = app.app.config.current().opacity;
    app.apply_key(key(KeyCode::Digit7, Some("77")), false);
    let selection = app.app.editor.selection;
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
    assert!(!app.app.search.open);
    assert_eq!(app.app.config.current().view_mode, ViewMode::Source);
    assert_eq!(
        app.app.editor.selection, selection,
        "Ctrl+A reached the note"
    );
    assert_eq!(app.controls.opacity_input, "77");
    let submitted = app.app.worker.metrics().note_submitted;
    app.apply_key(key(KeyCode::KeyS, None), false);
    assert_eq!(app.app.worker.metrics().note_submitted, submitted + 1);
    assert!(app.controls.takes_keys(), "save does not end field editing");
    app.apply_key(key(KeyCode::Escape, None), false);
    assert!(!app.controls.opacity_popup_open);
    assert!(!app.controls.opacity_input_focused);
    assert_eq!(
        app.app.config.current().opacity,
        initial,
        "Esc reverts instead of committing"
    );
    assert_eq!(app.app.coordinator.snapshot(), snapshot);
}

#[test]
fn runtime_opacity_recovery_and_busy_export_shortcuts_precede_field() {
    use stickymd_core::{LoadedDocument, RecoveryCandidate};
    for code in [KeyCode::F6, KeyCode::F7] {
        let mut app = fixture("original");
        focus_field(&mut app);
        app.apply_key(key(KeyCode::Digit7, Some("77")), false);
        app.app.recovery = stickymd_app::flow::RecoveryCoordinator::new(
            Some(RecoveryCandidate {
                canonical: Some(LoadedDocument::from_durable_bytes(b"durable").unwrap()),
                temporary: LoadedDocument::from_durable_bytes(b"temporary").unwrap(),
                temporary_is_newer: true,
            }),
            None,
        );
        app.app.persistence.set_recovery_pending(true);
        app.apply_key(key(code, None), false);
        assert_eq!(
            app.app.coordinator.view().text,
            if code == KeyCode::F6 {
                "temporary"
            } else {
                "durable"
            }
        );
        assert!(app.app.recovery.operation_pending());
        assert_eq!(app.controls.opacity_input, "77");
        assert!(app.controls.takes_keys());
    }
    let mut app = fixture("canonical text");
    focus_field(&mut app);
    app.app.export_in_flight = true; // Stops at the real busy-export branch; opens no native dialog.
    app.modifiers = ModifiersState::CONTROL | ModifiersState::SHIFT;
    app.apply_key(key(KeyCode::KeyS, None), false);
    assert_eq!(app.diagnostic.as_deref(), Some("已有导出任务正在进行。"));
    assert!(app.controls.takes_keys());
    assert_eq!(app.app.coordinator.view().text, "canonical text");
}
