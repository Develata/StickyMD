//! Window input translation for the Phase 3 interaction shell.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use stickymd_core::{EditKind, Selection};
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta};
use winit::keyboard::{KeyCode, PhysicalKey};

use super::StickyApp;
use crate::config::{ContentZoomPercent, ViewMode};
use crate::instruction::{
    AppIntent, PersistenceIntent, PreviewIntent, SaveReason, WindowPreferenceIntent,
};
use crate::interaction::ImeSignal;

impl StickyApp {
    pub(super) fn handle_ime(&mut self, event: Ime) {
        if self.handle_search_ime(&event) {
            return;
        }
        if !mutation_input_allowed(
            self.recovery.is_pending(),
            self.preview_focused,
            !self.window_accepts_editor_mutation(),
            self.asset_reconcile_pending,
        ) {
            self.session.cancel_preedit();
            self.sync_preedit();
            return;
        }
        #[cfg(debug_assertions)]
        match &event {
            Ime::Enabled => eprintln!("IME Enabled"),
            Ime::Disabled => eprintln!("IME Disabled"),
            Ime::Preedit(text, cursor) => {
                eprintln!("IME Preedit len={} cursor={cursor:?}", text.len());
            }
            Ime::Commit(text) => eprintln!(
                "IME Commit len={} generation={}",
                text.len(),
                self.coordinator.view().generation.value()
            ),
        }
        let signal = match event {
            Ime::Enabled => ImeSignal::Enabled,
            Ime::Disabled => ImeSignal::Disabled,
            Ime::Preedit(text, cursor) => ImeSignal::Preedit {
                text,
                cursor: cursor.map(|(start, end)| start..end),
            },
            Ime::Commit(text) => ImeSignal::Commit(text),
        };
        let generation = self.coordinator.view().generation;
        let intent = self
            .session
            .handle_ime(signal, generation, self.timestamp_ms());
        self.sync_preedit();
        if let Some(intent) = intent {
            self.dispatch(intent);
        } else {
            self.after_presentation_change();
        }
    }

    /// Esc that no input state consumed collapses the window (plan 09, last row of the Esc order).
    pub(super) fn dispatch_escape_to_window(&mut self) {
        self.dispatch_window_intent(
            None,
            crate::flow::window::state::WindowIntent::EscapePressed {
                now_ms: self.timestamp_ms(),
            },
        );
    }

    pub(super) fn handle_key(&mut self, event: KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }
        let code = match event.physical_key {
            PhysicalKey::Code(code) => Some(code),
            PhysicalKey::Unidentified(_) => None,
        };
        // Every Esc is decided in one place, in the plan 09 order (`escape.rs`).
        if super::escape::is_escape(code, self.modifiers) {
            self.handle_escape();
            return;
        }
        if self.handle_shell_key(&event) {
            return;
        }
        let (generation, text_len) = {
            let view = self.coordinator.view();
            (view.generation, view.text.len())
        };
        let timestamp_ms = self.timestamp_ms();
        let shift = self.modifiers.shift_key();
        let shortcut = self.modifiers.control_key() && !self.modifiers.alt_key();

        if code == Some(KeyCode::F6)
            && self.dispatch_persistence_intent(None, PersistenceIntent::ResolvePrimary)
        {
            return;
        }
        if code == Some(KeyCode::F7)
            && self.dispatch_persistence_intent(None, PersistenceIntent::ResolveSecondary)
        {
            return;
        }
        if !mutation_input_allowed(
            self.recovery.is_pending(),
            false,
            !self.window_accepts_editor_mutation(),
            self.asset_reconcile_pending,
        ) {
            return;
        }
        if let Some(intent) = global_persistence_shortcut(code, shortcut, shift) {
            self.dispatch_persistence_intent(None, intent);
            return;
        }
        if shortcut && matches!(code, Some(KeyCode::KeyF | KeyCode::KeyH)) {
            self.open_search(code == Some(KeyCode::KeyH));
            return;
        }
        // Zoom and the view shortcuts are window commands like save and export: an open Search
        // does not take them (plan 07), and a view change decides what happens to Search.
        if shortcut && let Some(command) = zoom_key_command(code) {
            self.apply_zoom_command(command, true);
            return;
        }
        if shortcut && let Some(mode) = view_key_mode(code) {
            self.dispatch_preview_intent(PreviewIntent::SetViewMode(mode));
            return;
        }
        if self.search.open && self.handle_search_key(&event, code, shortcut, shift) {
            return;
        }

        if self.preview_focused {
            self.handle_preview_key(code, shortcut, shift);
            return;
        }

        if let Some(command) = clipboard_alias(code, shortcut, shift) {
            self.dispatch_clipboard_alias(command, generation, timestamp_ms);
            return;
        }

        if shortcut && self.handle_shortcut(code, generation, text_len, timestamp_ms, shift) {
            return;
        }

        match code {
            Some(KeyCode::Backspace) => {
                let intent =
                    self.session
                        .backspace(self.coordinator.view().text, generation, timestamp_ms);
                if let Some(intent) = intent {
                    self.dispatch(intent);
                }
                return;
            }
            Some(KeyCode::Delete) => {
                let intent = self.session.delete_forward(
                    self.coordinator.view().text,
                    generation,
                    timestamp_ms,
                );
                if let Some(intent) = intent {
                    self.dispatch(intent);
                }
                return;
            }
            Some(KeyCode::ArrowLeft) | Some(KeyCode::ArrowRight) => {
                let direction = if code == Some(KeyCode::ArrowLeft) {
                    -1
                } else {
                    1
                };
                self.session
                    .move_horizontal(self.coordinator.view().text, direction, shift);
                self.after_presentation_change();
                return;
            }
            Some(KeyCode::ArrowUp) | Some(KeyCode::ArrowDown) => {
                let direction = if code == Some(KeyCode::ArrowUp) {
                    -1
                } else {
                    1
                };
                self.move_vertical(direction, shift);
                return;
            }
            Some(KeyCode::Home) | Some(KeyCode::End) => {
                self.session.move_line_boundary(
                    self.coordinator.view().text,
                    code == Some(KeyCode::End),
                    shift,
                );
                self.after_presentation_change();
                return;
            }
            Some(KeyCode::Enter) | Some(KeyCode::NumpadEnter) => {
                if !self.session.is_composing() {
                    self.dispatch(self.session.insert(
                        generation,
                        "\n",
                        EditKind::Newline,
                        timestamp_ms,
                    ));
                }
                return;
            }
            Some(KeyCode::Tab) => {
                if !self.session.is_composing() {
                    self.dispatch(self.session.insert(
                        generation,
                        "    ",
                        EditKind::Typing,
                        timestamp_ms,
                    ));
                }
                return;
            }
            _ => {}
        }

        if !shortcut
            && !self.modifiers.super_key()
            && let Some(text) = event.text
            && !text.chars().any(char::is_control)
            && let Some(intent) = self
                .session
                .handle_keyboard_text(&text, generation, timestamp_ms)
        {
            self.dispatch(intent);
        }
    }

    fn dispatch_clipboard_alias(
        &mut self,
        command: ClipboardAlias,
        generation: stickymd_core::Generation,
        timestamp_ms: u64,
    ) {
        match command {
            ClipboardAlias::Copy => self.dispatch(AppIntent::CopySelection {
                expected_generation: generation,
                selection: self.session.selection,
            }),
            ClipboardAlias::Cut => {
                self.session.cancel_preedit();
                self.dispatch(AppIntent::CutSelection {
                    expected_generation: generation,
                    selection: self.session.selection,
                    timestamp_ms,
                });
            }
            ClipboardAlias::Paste => {
                self.session.cancel_preedit();
                self.dispatch(AppIntent::PasteClipboard {
                    expected_generation: generation,
                    selection: self.session.selection,
                    timestamp_ms,
                });
            }
        }
    }

    fn apply_zoom_command(&mut self, command: ZoomCommand, persist_now: bool) {
        let current = self.config.current().content_zoom_percent;
        let next = match command {
            ZoomCommand::Step(delta) => current.stepped(delta),
            ZoomCommand::Reset => ContentZoomPercent::default(),
        };
        self.dispatch_window_preference_intent(WindowPreferenceIntent::SetContentZoom(next));
        if persist_now {
            self.zoom_config_deadline = None;
            self.submit_config_if_needed();
        }
    }

    fn handle_shortcut(
        &mut self,
        code: Option<KeyCode>,
        generation: stickymd_core::Generation,
        text_len: usize,
        timestamp_ms: u64,
        shift: bool,
    ) -> bool {
        match code {
            Some(KeyCode::KeyA) => {
                self.session.selection = Selection::new(0, text_len);
                self.after_presentation_change();
            }
            Some(KeyCode::KeyC) => self.dispatch(AppIntent::CopySelection {
                expected_generation: generation,
                selection: self.session.selection,
            }),
            Some(KeyCode::KeyX) => {
                self.session.cancel_preedit();
                self.dispatch(AppIntent::CutSelection {
                    expected_generation: generation,
                    selection: self.session.selection,
                    timestamp_ms,
                });
            }
            Some(KeyCode::KeyV) => {
                self.session.cancel_preedit();
                self.dispatch(AppIntent::PasteClipboard {
                    expected_generation: generation,
                    selection: self.session.selection,
                    timestamp_ms,
                });
            }
            Some(KeyCode::KeyZ) => self.undo_or_cancel(true),
            Some(KeyCode::KeyY) => self.undo_or_cancel(false),
            Some(KeyCode::Home) | Some(KeyCode::End) => {
                self.session
                    .move_document_boundary(text_len, code == Some(KeyCode::End), shift);
                self.after_presentation_change();
            }
            _ => return false,
        }
        true
    }

    fn undo_or_cancel(&mut self, undo: bool) {
        if self.session.is_composing() {
            self.session.cancel_preedit();
            self.after_presentation_change();
        } else {
            self.dispatch(if undo {
                AppIntent::Undo
            } else {
                AppIntent::Redo
            });
        }
    }

    fn move_vertical(&mut self, direction: i32, extend: bool) {
        if self.session.is_composing() {
            return;
        }
        let Some(projection) = &self.projection else {
            return;
        };
        let active = self.session.selection.active.byte;
        let preferred_x = self
            .session
            .preferred_x
            .or_else(|| projection.caret_rect(active).map(|rect| rect.x))
            .unwrap_or(0.0);
        let target = projection.vertical_neighbor(active, direction, preferred_x);
        self.session.selection = if extend {
            Selection::new(self.session.selection.anchor.byte, target)
        } else {
            Selection::caret(target)
        };
        self.session.preferred_x = Some(preferred_x);
        self.after_presentation_change();
    }

    pub(super) fn handle_mouse_button(&mut self, state: ElementState, button: MouseButton) {
        if state == ElementState::Released
            && self.scrollbars.drag.is_some()
            && self.handle_scrollbar_button(state, button)
        {
            return;
        }
        if self.handle_shell_mouse_button(state, button) {
            return;
        }
        if self.handle_search_mouse_button(state, button) {
            return;
        }
        if !self.window_accepts_editor_mutation() {
            return;
        }
        if self.handle_scrollbar_button(state, button) {
            return;
        }
        if button != MouseButton::Left {
            return;
        }
        match state {
            ElementState::Pressed => {
                if let Some(mode) = self.toolbar_mode_at_cursor() {
                    self.dispatch_preview_intent(PreviewIntent::SetViewMode(mode));
                    return;
                }
                if self.preview_at_cursor().is_some() {
                    self.preview_focused = true;
                    if let Some(window) = &self.window {
                        window.set_ime_allowed(false);
                    }
                    self.press_preview_selection();
                    return;
                }
                self.preview_focused = false;
                if let Some(window) = &self.window {
                    window.set_ime_allowed(self.session.focused);
                }
                self.session.cancel_preedit();
                let Some(source_position) = self.source_local_cursor() else {
                    return;
                };
                let diagnostic_action = self.projection.as_ref().and_then(|projection| {
                    projection.diagnostic_action_at(source_position.0, source_position.1)
                });
                if let Some(primary) = diagnostic_action
                    && self.dispatch_persistence_intent(
                        None,
                        if primary {
                            PersistenceIntent::ResolvePrimary
                        } else {
                            PersistenceIntent::ResolveSecondary
                        },
                    )
                {
                    return;
                }
                let Some(projection) = &self.projection else {
                    return;
                };
                let hit = projection.hit_test(source_position.0, source_position.1);
                self.session.selection = if self.modifiers.shift_key() {
                    Selection::new(self.session.selection.anchor.byte, hit)
                } else {
                    Selection::caret(hit)
                };
                self.session.dragging_selection = true;
                self.after_presentation_change();
            }
            ElementState::Released => {
                if self.preview_dragging {
                    self.release_preview_selection();
                }
                self.session.dragging_selection = false;
            }
        }
    }

    pub(super) fn handle_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        self.cursor_position = position;
        if self.handle_shell_cursor_moved(position) {
            return;
        }
        if self.handle_scrollbar_cursor() {
            return;
        }
        let cursor = self
            .shell_cursor_icon()
            .or_else(|| {
                self.scrollbar_gutter_at_cursor()
                    .map(|_| winit::window::CursorIcon::Default)
            })
            .unwrap_or_else(|| self.cursor_icon_for_position(position));
        if let Some(window) = &self.window {
            window.set_cursor(cursor);
        }
        // The formula-error hover title is derived from the pointer and the current frame when
        // the event pass ends (`update_window_title`), so it cannot outlive either.
        if self.preview_dragging {
            if let Some(start) = self.preview_press_position {
                let scale = self
                    .window
                    .as_ref()
                    .map_or(1.0, |window| window.scale_factor());
                self.preview_drag_moved |=
                    super::preview_input::meaningful_preview_drag(start, position, scale);
            }
            self.extend_preview_selection(position);
            return;
        }
        if !self.session.dragging_selection {
            return;
        }
        let Some(projection) = &self.projection else {
            return;
        };
        let Some(source_position) = self.source_local_position(position) else {
            return;
        };
        let active = projection.hit_test(source_position.0, source_position.1);
        self.session.selection = Selection::new(self.session.selection.anchor.byte, active);
        self.after_presentation_change();
    }

    pub(super) fn handle_scroll(&mut self, delta: MouseScrollDelta) {
        if self.modifiers.control_key() && !self.modifiers.alt_key() {
            let dpi = self
                .window
                .as_ref()
                .map_or(1.0, |window| window.scale_factor());
            let steps = self.zoom_wheel.push(delta, dpi);
            if steps != 0 {
                let delta =
                    (steps.saturating_mul(5)).clamp(i32::from(i16::MIN), i32::from(i16::MAX));
                self.apply_zoom_command(ZoomCommand::Step(delta as i16), false);
            }
            return;
        }
        let pixels = match delta {
            MouseScrollDelta::LineDelta(_, lines) => -lines * 48.0,
            MouseScrollDelta::PixelDelta(position) => -position.y as f32,
        };
        let pane = if self.preview_at_cursor().is_some()
            || self.scrollbar_gutter_at_cursor() == Some(super::scrollbar::ScrollPane::Preview)
        {
            super::scrollbar::ScrollPane::Preview
        } else {
            super::scrollbar::ScrollPane::Source
        };
        self.scroll_pane_by(pane, pixels);
    }

    fn source_local_cursor(&self) -> Option<(f32, f32)> {
        self.source_local_position(self.cursor_position)
    }

    fn source_local_position(&self, position: PhysicalPosition<f64>) -> Option<(f32, f32)> {
        let pane = self.view_geometry()?.source?;
        let x = position.x as f32;
        let y = position.y as f32;
        pane.contains(x, y)
            .then_some((x - pane.x as f32, y - pane.y as f32))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ClipboardAlias {
    Copy,
    Cut,
    Paste,
}

fn clipboard_alias(code: Option<KeyCode>, shortcut: bool, shift: bool) -> Option<ClipboardAlias> {
    match (code, shortcut, shift) {
        (Some(KeyCode::Insert), true, _) => Some(ClipboardAlias::Copy),
        (Some(KeyCode::Delete), false, true) => Some(ClipboardAlias::Cut),
        (Some(KeyCode::Insert), false, true) => Some(ClipboardAlias::Paste),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZoomCommand {
    Step(i16),
    Reset,
}

/// The view a Ctrl+digit shortcut switches to.
fn view_key_mode(code: Option<KeyCode>) -> Option<ViewMode> {
    match code {
        Some(KeyCode::Digit1) => Some(ViewMode::Source),
        Some(KeyCode::Digit2) => Some(ViewMode::Split),
        Some(KeyCode::Digit3) => Some(ViewMode::Preview),
        _ => None,
    }
}

fn zoom_key_command(code: Option<KeyCode>) -> Option<ZoomCommand> {
    match code {
        Some(KeyCode::Equal | KeyCode::NumpadAdd) => Some(ZoomCommand::Step(10)),
        Some(KeyCode::Minus | KeyCode::NumpadSubtract) => Some(ZoomCommand::Step(-10)),
        Some(KeyCode::Digit0 | KeyCode::Numpad0) => Some(ZoomCommand::Reset),
        _ => None,
    }
}

fn global_persistence_shortcut(
    code: Option<KeyCode>,
    shortcut: bool,
    shift: bool,
) -> Option<PersistenceIntent> {
    match (code, shortcut, shift) {
        (Some(KeyCode::KeyS), true, false) => Some(PersistenceIntent::SaveNow(SaveReason::Manual)),
        (Some(KeyCode::KeyS), true, true) => Some(PersistenceIntent::Export),
        _ => None,
    }
}

#[derive(Debug, Default)]
pub(super) struct ZoomWheelAccumulator {
    units: f64,
}

impl ZoomWheelAccumulator {
    const UNITS_PER_NOTCH: f64 = 120.0;

    fn push(&mut self, delta: MouseScrollDelta, dpi_scale: f64) -> i32 {
        self.units += match delta {
            MouseScrollDelta::LineDelta(_, lines) => f64::from(lines) * Self::UNITS_PER_NOTCH,
            MouseScrollDelta::PixelDelta(position) => position.y / dpi_scale.max(0.5),
        };
        let steps = (self.units / Self::UNITS_PER_NOTCH).trunc() as i32;
        self.units -= f64::from(steps) * Self::UNITS_PER_NOTCH;
        steps
    }
}

fn mutation_input_allowed(
    recovery_pending: bool,
    preview_only: bool,
    quit_pending: bool,
    asset_reconcile_pending: bool,
) -> bool {
    !recovery_pending && !preview_only && !quit_pending && !asset_reconcile_pending
}

#[cfg(test)]
mod recovery_tests {
    use super::{
        ClipboardAlias, ZoomCommand, ZoomWheelAccumulator, clipboard_alias,
        global_persistence_shortcut, mutation_input_allowed, view_key_mode, zoom_key_command,
    };
    use crate::config::ViewMode;
    use crate::instruction::{PersistenceIntent, SaveReason};
    use winit::dpi::PhysicalPosition;
    use winit::event::MouseScrollDelta;
    use winit::keyboard::KeyCode;

    #[test]
    fn recovery_pending_rejects_ime_commits_and_preedit() {
        assert!(!mutation_input_allowed(true, false, false, false));
        assert!(!mutation_input_allowed(false, true, false, false));
        assert!(mutation_input_allowed(false, false, false, false));
    }

    #[test]
    fn quit_and_asset_reconcile_freeze_keyboard_and_ime_mutations() {
        assert!(!mutation_input_allowed(false, false, true, false));
        assert!(!mutation_input_allowed(false, false, false, true));
    }

    #[test]
    fn view_and_zoom_shortcuts_take_no_key_that_search_binds() {
        assert_eq!(view_key_mode(Some(KeyCode::Digit1)), Some(ViewMode::Source));
        assert_eq!(view_key_mode(Some(KeyCode::Digit2)), Some(ViewMode::Split));
        assert_eq!(
            view_key_mode(Some(KeyCode::Digit3)),
            Some(ViewMode::Preview)
        );
        assert_eq!(view_key_mode(Some(KeyCode::Digit0)), None);
        assert_eq!(view_key_mode(Some(KeyCode::Digit4)), None);
        assert_eq!(view_key_mode(None), None);
        // Decided before Search, so neither may claim a Ctrl key Search or its shortcuts use.
        for key in [
            KeyCode::KeyF,
            KeyCode::KeyH,
            KeyCode::KeyV,
            KeyCode::Enter,
            KeyCode::NumpadEnter,
            KeyCode::Tab,
            KeyCode::F3,
            KeyCode::Backspace,
            KeyCode::Delete,
            KeyCode::ArrowLeft,
            KeyCode::ArrowRight,
            KeyCode::ArrowUp,
            KeyCode::ArrowDown,
        ] {
            assert_eq!(view_key_mode(Some(key)), None, "{key:?}");
            assert_eq!(zoom_key_command(Some(key)), None, "{key:?}");
        }
    }

    #[test]
    fn phase10_clipboard_aliases_are_unambiguous() {
        assert_eq!(
            clipboard_alias(Some(KeyCode::Insert), true, false),
            Some(ClipboardAlias::Copy)
        );
        assert_eq!(
            clipboard_alias(Some(KeyCode::Delete), false, true),
            Some(ClipboardAlias::Cut)
        );
        assert_eq!(
            clipboard_alias(Some(KeyCode::Insert), false, true),
            Some(ClipboardAlias::Paste)
        );
        assert_eq!(clipboard_alias(Some(KeyCode::Delete), false, false), None);
    }

    #[test]
    fn phase10_zoom_keys_and_high_resolution_wheel_are_deterministic() {
        for key in [KeyCode::Equal, KeyCode::NumpadAdd] {
            assert_eq!(zoom_key_command(Some(key)), Some(ZoomCommand::Step(10)));
        }
        for key in [KeyCode::Minus, KeyCode::NumpadSubtract] {
            assert_eq!(zoom_key_command(Some(key)), Some(ZoomCommand::Step(-10)));
        }
        for key in [KeyCode::Digit0, KeyCode::Numpad0] {
            assert_eq!(zoom_key_command(Some(key)), Some(ZoomCommand::Reset));
        }
        let mut accumulator = ZoomWheelAccumulator::default();
        for _ in 0..3 {
            assert_eq!(
                accumulator.push(
                    MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 30.0)),
                    1.0,
                ),
                0
            );
        }
        assert_eq!(
            accumulator.push(
                MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 30.0)),
                1.0,
            ),
            1
        );
        assert_eq!(
            accumulator.push(MouseScrollDelta::LineDelta(0.0, -0.5), 1.0),
            0
        );
        assert_eq!(
            accumulator.push(MouseScrollDelta::LineDelta(0.0, -0.5), 1.0),
            -1
        );
    }

    #[test]
    fn phase14_save_and_export_shortcuts_remain_global_with_search_focus() {
        assert_eq!(
            global_persistence_shortcut(Some(KeyCode::KeyS), true, false),
            Some(PersistenceIntent::SaveNow(SaveReason::Manual))
        );
        assert_eq!(
            global_persistence_shortcut(Some(KeyCode::KeyS), true, true),
            Some(PersistenceIntent::Export)
        );
        assert_eq!(
            global_persistence_shortcut(Some(KeyCode::KeyS), false, false),
            None
        );
    }
}
