//! The opacity popup's numeric field: its keys and IME events, its focus, and the commit or
//! cancel that ends its edit (plan 09).
//!
//! plan_ref: docs/plan/09_windows_shell.md#theme-opacity-and-content-zoom

use winit::event::Ime;
use winit::keyboard::{KeyCode, ModifiersState, PhysicalKey};

use super::DesktopShell;
#[cfg(test)]
use super::shell_pins_tests::PressRoute;
use crate::host::Host;
use stickymd_app::instruction::WindowPreferenceIntent;

impl<H: Host> DesktopShell<H> {
    /// A key-down while the field is edited. `handle_key` runs it where Search's keys run, so Esc,
    /// F6/F7, save and export keep their window-wide meaning first; every key it reaches is the
    /// field's, and a key the field does not use is dropped, so none edits the note or Search.
    pub(super) fn handle_opacity_key(
        &mut self,
        physical_key: PhysicalKey,
        text: Option<&str>,
        code: Option<KeyCode>,
    ) {
        match opacity_key(code, text, self.modifiers) {
            OpacityKey::Commit => {
                self.controls.claim_commit_key(physical_key);
                self.commit_opacity_input();
            }
            OpacityKey::Backspace => self.controls.backspace_input(),
            OpacityKey::Type(text) => self.controls.type_input(text),
            OpacityKey::Drop => {}
        }
        self.request_redraw();
    }

    /// An IME event while the field is edited. The IME is off then (`focus_opacity_input`); should
    /// the platform still deliver one, a commit is typed into the field and a composition is
    /// dropped, so neither reaches the note or Search. Returns whether the field took it.
    pub(super) fn handle_opacity_ime(&mut self, event: &Ime) -> bool {
        if !self.controls.takes_keys() || !field_takes_ime(event) {
            return false;
        }
        if let Ime::Commit(text) = event {
            self.controls.type_input(text);
        }
        self.request_redraw();
        true
    }

    /// Focuses the field, unless the note or Search is composing: as for opening Search, the
    /// keyboard moves only once the composition ends. The field is numeric and takes no IME, so
    /// focusing it turns the IME off, as Preview focus does: no composition can start in it and
    /// reach the note or Search after the edit ends.
    pub(super) fn focus_opacity_input(&mut self) {
        press_probe!(self, PressRoute::PopupField);
        if self.app.editor.is_composing() || self.app.search.is_composing() {
            self.diagnostic = Some("请先完成当前输入法组合。".into());
        } else {
            self.controls.focus_input();
            self.sync_ime_allowed();
        }
        self.request_redraw();
    }

    /// Closes the popup and drops any uncommitted slider or numeric value.
    pub(super) fn cancel_opacity_popup(&mut self) {
        let editing = self.controls.takes_keys();
        let committed = self.app.config().current().opacity;
        self.controls.cancel(committed);
        if editing {
            self.sync_ime_allowed();
        }
        self.refresh_cursor();
        self.dispatch_window_preference_intent(WindowPreferenceIntent::PreviewOpacity(committed));
    }

    /// Ends the field edit, committing an integer: on Enter, or when the field loses focus (a
    /// press elsewhere, the window losing focus, hiding or quitting).
    pub(super) fn commit_opacity_input(&mut self) {
        if let Some(opacity) = self.controls.finish_input() {
            self.commit_opacity(opacity);
        }
        self.sync_ime_allowed();
        self.request_redraw();
    }

    fn commit_opacity(&mut self, opacity: u8) {
        self.dispatch_window_preference_intent(WindowPreferenceIntent::CommitOpacity(opacity));
    }
}

/// What a key-down does to the edited field.
#[derive(Debug, PartialEq, Eq)]
enum OpacityKey<'a> {
    Commit,
    Backspace,
    Type(&'a str),
    Drop,
}

/// Enter commits (not Ctrl+Enter, as in Search) and Backspace deletes; text is typed only
/// without Ctrl, Alt or Win, as in Search, because winit's text ignores Ctrl (Ctrl+0 would type
/// a 0). Every other key is dropped.
fn opacity_key<'a>(
    code: Option<KeyCode>,
    text: Option<&'a str>,
    modifiers: ModifiersState,
) -> OpacityKey<'a> {
    let shortcut = modifiers.control_key() && !modifiers.alt_key();
    match code {
        Some(KeyCode::Enter | KeyCode::NumpadEnter) if !shortcut => OpacityKey::Commit,
        Some(KeyCode::Backspace) => OpacityKey::Backspace,
        _ if !modifiers.control_key() && !modifiers.alt_key() && !modifiers.super_key() => text
            .filter(|text| !text.is_empty() && !text.chars().any(char::is_control))
            .map_or(OpacityKey::Drop, OpacityKey::Type),
        _ => OpacityKey::Drop,
    }
}

/// Whether the edited field takes an IME event: a commit or a composition, never the end of one
/// (an empty preedit, `Disabled`) or `Enabled`, which go on to whoever composes, so their state
/// still ends.
fn field_takes_ime(event: &Ime) -> bool {
    match event {
        Ime::Commit(_) => true,
        Ime::Preedit(text, _) => !text.is_empty(),
        Ime::Enabled | Ime::Disabled => false,
    }
}

#[cfg(test)]
mod phase8_interaction_tests {
    use super::*;

    /// Plan 09 integer input, in Search's key order: Enter commits (not Ctrl+Enter), Backspace
    /// deletes, text is typed only without Ctrl, Alt or Win, and every other key is dropped.
    /// Save, export and F6/F7 never reach this decision (`handle_key` runs them first); that
    /// order, like real key delivery, is not exercised here.
    #[test]
    fn the_edited_opacity_field_types_only_plain_text() {
        use OpacityKey::{Backspace, Commit, Drop, Type};
        let none = ModifiersState::empty();
        let (ctrl, alt, win, shift) = (
            ModifiersState::CONTROL,
            ModifiersState::ALT,
            ModifiersState::SUPER,
            ModifiersState::SHIFT,
        );
        for (code, text, modifiers, expected) in [
            (KeyCode::Enter, Some("\r"), none, Commit),
            (KeyCode::NumpadEnter, Some("\r"), shift, Commit),
            (KeyCode::Enter, Some("\r"), ctrl | alt, Commit),
            (KeyCode::Enter, Some("\r"), ctrl, Drop),
            (KeyCode::Backspace, Some("\u{8}"), none, Backspace),
            (KeyCode::Backspace, Some("\u{8}"), ctrl, Backspace),
            (KeyCode::Digit5, Some("5"), none, Type("5")),
            (KeyCode::Numpad5, Some("5"), shift, Type("5")),
            (KeyCode::Minus, Some("-"), none, Type("-")),
            // The field drops what is not an integer character.
            (KeyCode::KeyA, Some("a"), none, Type("a")),
            // Ctrl+0 resets zoom elsewhere; here it types nothing.
            (KeyCode::Digit0, Some("0"), ctrl, Drop),
            (KeyCode::Digit5, Some("5"), alt, Drop),
            (KeyCode::Digit5, Some("5"), win, Drop),
            (KeyCode::Digit5, Some("5"), ctrl | alt, Drop),
            (KeyCode::KeyV, Some("v"), ctrl, Drop),
            (KeyCode::KeyZ, Some("z"), ctrl, Drop),
            (KeyCode::KeyF, Some("f"), ctrl, Drop),
            (KeyCode::Delete, Some("\u{7f}"), none, Drop),
            (KeyCode::Tab, Some("\t"), none, Drop),
            (KeyCode::ArrowLeft, None, none, Drop),
            (KeyCode::Digit5, Some(""), none, Drop),
        ] {
            assert_eq!(
                opacity_key(Some(code), text, modifiers),
                expected,
                "{code:?} {modifiers:?}"
            );
        }
        assert_eq!(opacity_key(None, None, none), Drop);
    }

    /// The edited field takes a commit or a composition but never the end of one or `Enabled`,
    /// so a note or Search composition still ends. Real IME delivery is not exercised here.
    #[test]
    fn the_edited_opacity_field_leaves_the_end_of_a_composition_to_its_owner() {
        for (event, taken) in [
            (Ime::Commit("5".into()), true),
            (Ime::Preedit("ni".into(), Some((2, 2))), true),
            (Ime::Preedit(String::new(), None), false),
            (Ime::Disabled, false),
            (Ime::Enabled, false),
        ] {
            assert_eq!(field_takes_ime(&event), taken, "{event:?}");
        }
    }
}
