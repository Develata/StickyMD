//! Keyboard and IME input translation for the interaction shell; the pointer is `pointer.rs`.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use stickymd_core::{EditKind, Selection};
use winit::event::{ElementState, Ime, KeyEvent};
use winit::keyboard::{Key, KeyCode, NamedKey, PhysicalKey};

use super::DesktopShell;
use crate::host::Host;
use crate::input::{ClipboardAlias, WindowKey, ZoomCommand, is_escape};
use stickymd_app::config::ContentZoomPercent;
use stickymd_app::instruction::{
    AppIntent, PersistenceIntent, PreviewIntent, WindowPreferenceIntent,
};
use stickymd_app::interaction::ImeSignal;
use stickymd_app::session::ZoomInput;

/// Only the native key observations consumed by the shell; borrows text without copying.
#[derive(Clone, Copy)]
pub(super) struct ShellKey<'a> {
    pub physical_key: PhysicalKey,
    pub state: ElementState,
    pub repeat: bool,
    pub ime_owned: bool,
    pub text: Option<&'a str>,
}

impl<H: Host> DesktopShell<H> {
    pub(super) fn handle_ime(&mut self, event: Ime) {
        if self.handle_opacity_ime(&event) || self.handle_search_ime(&event) {
            return;
        }
        if !self
            .app
            .mutation_allowed(self.preview_focused, self.shell_input_enabled)
        {
            self.app.editor.cancel_preedit();
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
                self.app.coordinator().view().generation.value()
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
        let generation = self.app.coordinator().view().generation;
        let intent = self
            .app
            .editor
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
            stickymd_app::flow::window::state::WindowIntent::EscapePressed {
                now_ms: self.timestamp_ms(),
            },
        );
    }

    pub(super) fn handle_key(&mut self, event: KeyEvent, is_synthetic: bool) {
        self.apply_key(
            ShellKey {
                physical_key: event.physical_key,
                state: event.state,
                repeat: event.repeat,
                ime_owned: event.logical_key == Key::Named(NamedKey::Process),
                text: event.text.as_deref(),
            },
            is_synthetic,
        );
    }

    pub(super) fn apply_key(&mut self, event: ShellKey<'_>, is_synthetic: bool) {
        if is_synthetic {
            return;
        }
        // The Enter that ended an opacity field edit keeps its repeats and release (`controls.rs`).
        if self.controls.takes_commit_key_event(
            event.physical_key,
            event.state == ElementState::Pressed,
            event.repeat,
        ) {
            return;
        }
        if event.state != ElementState::Pressed {
            return;
        }
        // Windows can deliver the IME-owned key after empty preedit/commit and
        // Disabled (observed with WeType Escape). Composition state is then
        // already clear, but VK_PROCESSKEY still belongs exclusively to the IME.
        // Its physical scancode must not close Search, collapse the dock, or edit.
        if event.ime_owned {
            return;
        }
        let code = match event.physical_key {
            PhysicalKey::Code(code) => Some(code),
            PhysicalKey::Unidentified(_) => None,
        };
        // Every Esc is decided in one place, in the plan 09 order (`escape.rs`).
        if is_escape(code, self.modifiers) {
            self.handle_escape();
            return;
        }
        let (generation, text_len) = {
            let view = self.app.coordinator().view();
            (view.generation, view.text.len())
        };
        let timestamp_ms = self.timestamp_ms();
        let shift = self.modifiers.shift_key();
        let shortcut = self.modifiers.control_key() && !self.modifiers.alt_key();

        if code == Some(KeyCode::F6)
            && self.dispatch_persistence_intent(PersistenceIntent::ResolvePrimary)
        {
            return;
        }
        if code == Some(KeyCode::F7)
            && self.dispatch_persistence_intent(PersistenceIntent::ResolveSecondary)
        {
            return;
        }
        // Not the focus: Preview takes its keys below, and window-wide shortcuts reach it first.
        if !self.app.mutation_allowed(false, self.shell_input_enabled) {
            return;
        }
        if let Some(intent) =
            (self.host.keymap().global_persistence_shortcut)(code, shortcut, shift)
        {
            self.dispatch_persistence_intent(intent);
            return;
        }
        // The edited opacity field takes every key past this point, before Search and the note.
        if self.controls.takes_keys() {
            self.handle_opacity_key(event.physical_key, event.text, code);
            return;
        }
        if shortcut && matches!(code, Some(KeyCode::KeyF | KeyCode::KeyH)) {
            self.open_search(code == Some(KeyCode::KeyH));
            return;
        }
        match (self.host.keymap().window_key)(code, shortcut, self.app.search.is_composing()) {
            Some(WindowKey::Zoom(command)) => {
                self.apply_zoom_command(command, ZoomInput::Key);
                return;
            }
            Some(WindowKey::View(mode)) => {
                self.dispatch_preview_intent(PreviewIntent::SetViewMode(mode));
                return;
            }
            None => {}
        }
        if self.app.search.open && self.handle_search_key(event.text, code, shortcut, shift) {
            return;
        }

        if self.preview_focused {
            self.handle_preview_key(code, shortcut, shift);
            return;
        }

        if let Some(command) = (self.host.keymap().clipboard_alias)(code, shortcut, shift) {
            self.dispatch_clipboard_alias(command, generation, timestamp_ms);
            return;
        }

        if shortcut && self.handle_shortcut(code, generation, text_len, timestamp_ms, shift) {
            return;
        }

        match code {
            Some(KeyCode::Backspace) => {
                let intent = self.app.editor.backspace(
                    self.app.coordinator().view().text,
                    generation,
                    timestamp_ms,
                );
                if let Some(intent) = intent {
                    self.dispatch(intent);
                }
                return;
            }
            Some(KeyCode::Delete) => {
                let intent = self.app.editor.delete_forward(
                    self.app.coordinator().view().text,
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
                let (editor, text) = self.app.editor_over_text();
                editor.move_horizontal(text, direction, shift);
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
                let (editor, text) = self.app.editor_over_text();
                editor.move_line_boundary(text, code == Some(KeyCode::End), shift);
                self.after_presentation_change();
                return;
            }
            Some(KeyCode::Enter) | Some(KeyCode::NumpadEnter) => {
                if !self.app.editor.is_composing() {
                    self.dispatch(self.app.editor.insert(
                        generation,
                        "\n",
                        EditKind::Newline,
                        timestamp_ms,
                    ));
                }
                return;
            }
            Some(KeyCode::Tab) => {
                if !self.app.editor.is_composing() {
                    self.dispatch(self.app.editor.insert(
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
            && let Some(intent) =
                self.app
                    .editor
                    .handle_keyboard_text(text, generation, timestamp_ms)
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
                selection: self.app.editor.selection,
            }),
            ClipboardAlias::Cut => {
                self.app.editor.cancel_preedit();
                self.dispatch(AppIntent::CutSelection {
                    expected_generation: generation,
                    selection: self.app.editor.selection,
                    timestamp_ms,
                });
            }
            ClipboardAlias::Paste => {
                self.app.editor.cancel_preedit();
                self.dispatch(AppIntent::PasteClipboard {
                    expected_generation: generation,
                    selection: self.app.editor.selection,
                    timestamp_ms,
                });
            }
        }
    }

    pub(super) fn apply_zoom_command(&mut self, command: ZoomCommand, input: ZoomInput) {
        let current = self.app.config().current().content_zoom_percent;
        let next = match command {
            ZoomCommand::Step(delta) => current.stepped(delta),
            ZoomCommand::Reset => ContentZoomPercent::default(),
        };
        self.dispatch_window_preference_intent(WindowPreferenceIntent::SetContentZoom(next));
        self.app.finish_zoom_input(input);
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
                self.app.editor.selection = Selection::new(0, text_len);
                self.after_presentation_change();
            }
            Some(KeyCode::KeyC) => self.dispatch(AppIntent::CopySelection {
                expected_generation: generation,
                selection: self.app.editor.selection,
            }),
            Some(KeyCode::KeyX) => {
                self.app.editor.cancel_preedit();
                self.dispatch(AppIntent::CutSelection {
                    expected_generation: generation,
                    selection: self.app.editor.selection,
                    timestamp_ms,
                });
            }
            Some(KeyCode::KeyV) => {
                self.app.editor.cancel_preedit();
                self.dispatch(AppIntent::PasteClipboard {
                    expected_generation: generation,
                    selection: self.app.editor.selection,
                    timestamp_ms,
                });
            }
            Some(KeyCode::KeyZ) => self.undo_or_cancel(true),
            Some(KeyCode::KeyY) => self.undo_or_cancel(false),
            Some(KeyCode::Home) | Some(KeyCode::End) => {
                self.app
                    .editor
                    .move_document_boundary(text_len, code == Some(KeyCode::End), shift);
                self.after_presentation_change();
            }
            _ => return false,
        }
        true
    }

    fn undo_or_cancel(&mut self, undo: bool) {
        if self.app.editor.is_composing() {
            self.app.editor.cancel_preedit();
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
        if self.app.editor.is_composing() {
            return;
        }
        let Some(projection) = &self.projection else {
            return;
        };
        let active = self.app.editor.selection.active.byte;
        let preferred_x = self
            .app
            .editor
            .preferred_x
            .or_else(|| projection.caret_rect(active).map(|rect| rect.x))
            .unwrap_or(0.0);
        let target = projection.vertical_neighbor(active, direction, preferred_x);
        self.app.editor.selection = if extend {
            Selection::new(self.app.editor.selection.anchor.byte, target)
        } else {
            Selection::caret(target)
        };
        self.app.editor.preferred_x = Some(preferred_x);
        self.after_presentation_change();
    }
}
