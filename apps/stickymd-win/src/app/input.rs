//! Window input translation for the Phase 3 interaction shell.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use stickymd_core::{EditKind, Selection};
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, Ime, KeyEvent, MouseButton, MouseScrollDelta};
use winit::keyboard::{KeyCode, PhysicalKey};

use super::StickyApp;
use crate::config::ViewMode;
use crate::instruction::{AppIntent, PersistenceIntent, PreviewIntent, SaveReason};
use crate::interaction::ImeSignal;

impl StickyApp {
    pub(super) fn handle_ime(&mut self, event: Ime) {
        if !ime_event_allowed(self.recovery.is_pending(), self.preview_focused) {
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

    pub(super) fn handle_key(&mut self, event: KeyEvent) {
        if event.state != ElementState::Pressed {
            return;
        }
        let (generation, text_len) = {
            let view = self.coordinator.view();
            (view.generation, view.text.len())
        };
        let timestamp_ms = self.timestamp_ms();
        let shift = self.modifiers.shift_key();
        let shortcut = self.modifiers.control_key() && !self.modifiers.alt_key();
        let code = match event.physical_key {
            PhysicalKey::Code(code) => Some(code),
            PhysicalKey::Unidentified(_) => None,
        };

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
        if self.recovery.is_pending() {
            return;
        }

        if shortcut {
            let mode = match code {
                Some(KeyCode::Digit1) => Some(ViewMode::Source),
                Some(KeyCode::Digit2) => Some(ViewMode::Split),
                Some(KeyCode::Digit3) => Some(ViewMode::Preview),
                _ => None,
            };
            if let Some(mode) = mode {
                self.dispatch_preview_intent(PreviewIntent::SetViewMode(mode));
                return;
            }
        }

        if self.preview_focused {
            self.handle_preview_key(code, shortcut);
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
            Some(KeyCode::Escape) if self.session.is_composing() => {
                self.session.cancel_preedit();
                self.after_presentation_change();
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
                self.dispatch(AppIntent::PasteText {
                    expected_generation: generation,
                    selection: self.session.selection,
                    timestamp_ms,
                });
            }
            Some(KeyCode::KeyZ) => self.undo_or_cancel(true),
            Some(KeyCode::KeyY) => self.undo_or_cancel(false),
            Some(KeyCode::KeyS) => {
                self.dispatch_persistence_intent(
                    None,
                    PersistenceIntent::SaveNow(SaveReason::Manual),
                );
            }
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
        let cursor = self.cursor_icon_for_position(position);
        if let Some(window) = &self.window {
            window.set_cursor(cursor);
        }
        let math_tooltip = self.preview_at_cursor().and_then(|(x, y)| {
            self.preview_frame.as_ref().and_then(|frame| {
                frame
                    .index()
                    .tooltip_at(x, y + frame.scroll_y())
                    .map(str::to_owned)
            })
        });
        if let (Some(window), Some(detail)) = (&self.window, math_tooltip) {
            window.set_title(&format!("StickyMD — 公式错误：{detail}"));
        } else {
            self.update_window_title();
        }
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
        let pixels = match delta {
            MouseScrollDelta::LineDelta(_, lines) => -lines * 48.0,
            MouseScrollDelta::PixelDelta(position) => -position.y as f32,
        };
        if self.preview_at_cursor().is_some() {
            self.preview_scroll_y = (self.preview_scroll_y + pixels).max(0.0);
            self.request_preview_paint();
            return;
        }
        let Some(projection) = &mut self.projection else {
            return;
        };
        let scroll = projection.scroll_by(pixels);
        self.session.scroll.line = scroll.line;
        self.session.scroll.vertical_px = scroll.vertical;
        self.session.scroll.horizontal_px = scroll.horizontal;
        self.update_ime_area();
        self.request_redraw();
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

fn ime_event_allowed(recovery_pending: bool, preview_only: bool) -> bool {
    !recovery_pending && !preview_only
}

#[cfg(test)]
mod recovery_tests {
    use super::ime_event_allowed;

    #[test]
    fn recovery_pending_rejects_ime_commits_and_preedit() {
        assert!(!ime_event_allowed(true, false));
        assert!(!ime_event_allowed(false, true));
        assert!(ime_event_allowed(false, false));
    }
}
