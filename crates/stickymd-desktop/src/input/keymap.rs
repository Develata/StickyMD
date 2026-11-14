//! The key table a Platform Host selects: clipboard aliases, window commands and save keys.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use stickymd_app::config::ViewMode;
use stickymd_app::instruction::{PersistenceIntent, SaveReason};
use winit::keyboard::{KeyCode, ModifiersState};

/// What each key means, as data a Platform Host chooses (plan 03, rule 9). `shortcut_held` says
/// whether the held modifiers make a key a shortcut; `command_key` names the commands whose keys
/// differ between desktops. `window_key` decides zoom and view keys through `zoom_key_command` and
/// `view_key_mode`, so a table that changes either supplies its own `window_key` too.
#[derive(Clone, Copy)]
pub struct Keymap {
    pub shortcut_held: fn(ModifiersState) -> bool,
    pub command_key: fn(Option<KeyCode>, ModifiersState) -> Option<KeyCommand>,
    pub clipboard_alias: fn(Option<KeyCode>, bool, bool) -> Option<ClipboardAlias>,
    pub window_key: fn(Option<KeyCode>, bool, bool) -> Option<WindowKey>,
    pub view_key_mode: fn(Option<KeyCode>) -> Option<ViewMode>,
    pub zoom_key_command: fn(Option<KeyCode>) -> Option<ZoomCommand>,
    pub global_persistence_shortcut: fn(Option<KeyCode>, bool, bool) -> Option<PersistenceIntent>,
}

impl Keymap {
    /// The table with Ctrl as the shortcut modifier: the Windows keys, unchanged.
    pub const fn ctrl_standard() -> Self {
        Self {
            shortcut_held: ctrl_shortcut_held,
            command_key: ctrl_command_key,
            clipboard_alias,
            window_key,
            view_key_mode,
            zoom_key_command,
            global_persistence_shortcut,
        }
    }

    /// The table with Cmd as the shortcut modifier (plan 07, macOS input).
    pub const fn cmd_standard() -> Self {
        Self {
            shortcut_held: cmd_shortcut_held,
            command_key: cmd_command_key,
            clipboard_alias: no_clipboard_alias,
            window_key,
            view_key_mode,
            zoom_key_command,
            global_persistence_shortcut,
        }
    }
}

/// A key's meaning where desktops disagree about which key means it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyCommand {
    OpenFind,
    OpenReplace,
    Undo,
    Redo,
    LineBoundary { end: bool },
    DocumentBoundary { end: bool },
    Quit,
}

fn ctrl_shortcut_held(modifiers: ModifiersState) -> bool {
    modifiers.control_key() && !modifiers.alt_key()
}

fn ctrl_command_key(code: Option<KeyCode>, modifiers: ModifiersState) -> Option<KeyCommand> {
    if !ctrl_shortcut_held(modifiers) {
        return None;
    }
    match code {
        Some(KeyCode::KeyF) => Some(KeyCommand::OpenFind),
        Some(KeyCode::KeyH) => Some(KeyCommand::OpenReplace),
        Some(KeyCode::KeyZ) => Some(KeyCommand::Undo),
        Some(KeyCode::KeyY) => Some(KeyCommand::Redo),
        Some(KeyCode::Home) => Some(KeyCommand::DocumentBoundary { end: false }),
        Some(KeyCode::End) => Some(KeyCommand::DocumentBoundary { end: true }),
        _ => None,
    }
}

fn cmd_shortcut_held(modifiers: ModifiersState) -> bool {
    modifiers.super_key() && !modifiers.control_key() && !modifiers.alt_key()
}

fn cmd_command_key(code: Option<KeyCode>, modifiers: ModifiersState) -> Option<KeyCommand> {
    if code == Some(KeyCode::KeyF)
        && modifiers.super_key()
        && modifiers.alt_key()
        && !modifiers.control_key()
    {
        return Some(KeyCommand::OpenReplace);
    }
    if !cmd_shortcut_held(modifiers) {
        return None;
    }
    match (code, modifiers.shift_key()) {
        (Some(KeyCode::KeyF), false) => Some(KeyCommand::OpenFind),
        (Some(KeyCode::KeyZ), false) => Some(KeyCommand::Undo),
        (Some(KeyCode::KeyZ), true) => Some(KeyCommand::Redo),
        (Some(KeyCode::KeyQ), false) => Some(KeyCommand::Quit),
        (Some(KeyCode::ArrowLeft), _) => Some(KeyCommand::LineBoundary { end: false }),
        (Some(KeyCode::ArrowRight), _) => Some(KeyCommand::LineBoundary { end: true }),
        (Some(KeyCode::ArrowUp), _) => Some(KeyCommand::DocumentBoundary { end: false }),
        (Some(KeyCode::ArrowDown), _) => Some(KeyCommand::DocumentBoundary { end: true }),
        _ => None,
    }
}

fn no_clipboard_alias(_: Option<KeyCode>, _: bool, _: bool) -> Option<ClipboardAlias> {
    None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipboardAlias {
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
pub enum ZoomCommand {
    Step(i16),
    Reset,
}

/// A window command decided before Search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowKey {
    Zoom(ZoomCommand),
    View(ViewMode),
}

/// Zoom and the view shortcuts are window commands like save and export: an open Search does
/// not take them (plan 07), and a view change decides what happens to Search. A Search-field
/// composition still owns every such key until it ends, as it does every other key.
fn window_key(code: Option<KeyCode>, shortcut: bool, search_composing: bool) -> Option<WindowKey> {
    if !shortcut || search_composing {
        return None;
    }
    zoom_key_command(code)
        .map(WindowKey::Zoom)
        .or_else(|| view_key_mode(code).map(WindowKey::View))
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn window_keys_pass_an_open_search_but_not_its_composition() {
        let zoom_in = Some(KeyCode::Equal);
        let preview = Some(KeyCode::Digit3);
        assert_eq!(
            window_key(zoom_in, true, false),
            Some(WindowKey::Zoom(ZoomCommand::Step(10)))
        );
        assert_eq!(
            window_key(preview, true, false),
            Some(WindowKey::View(ViewMode::Preview))
        );
        // Ctrl+0 resets zoom; no digit is both.
        assert_eq!(
            window_key(Some(KeyCode::Digit0), true, false),
            Some(WindowKey::Zoom(ZoomCommand::Reset))
        );
        for code in [zoom_in, preview, Some(KeyCode::Digit0)] {
            assert_eq!(
                window_key(code, true, true),
                None,
                "{code:?} while composing"
            );
            assert_eq!(
                window_key(code, false, false),
                None,
                "{code:?} without Ctrl"
            );
        }
        assert_eq!(window_key(Some(KeyCode::KeyV), true, false), None);
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
    fn phase10_zoom_keys_are_deterministic() {
        for key in [KeyCode::Equal, KeyCode::NumpadAdd] {
            assert_eq!(zoom_key_command(Some(key)), Some(ZoomCommand::Step(10)));
        }
        for key in [KeyCode::Minus, KeyCode::NumpadSubtract] {
            assert_eq!(zoom_key_command(Some(key)), Some(ZoomCommand::Step(-10)));
        }
        for key in [KeyCode::Digit0, KeyCode::Numpad0] {
            assert_eq!(zoom_key_command(Some(key)), Some(ZoomCommand::Reset));
        }
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

    /// Pins the Windows keys now that they come from a table.
    #[test]
    fn the_ctrl_table_keeps_the_windows_commands() {
        let table = Keymap::ctrl_standard();
        let ctrl = ModifiersState::CONTROL;
        for (code, command) in [
            (KeyCode::KeyF, KeyCommand::OpenFind),
            (KeyCode::KeyH, KeyCommand::OpenReplace),
            (KeyCode::KeyZ, KeyCommand::Undo),
            (KeyCode::KeyY, KeyCommand::Redo),
            (KeyCode::Home, KeyCommand::DocumentBoundary { end: false }),
            (KeyCode::End, KeyCommand::DocumentBoundary { end: true }),
        ] {
            assert_eq!((table.command_key)(Some(code), ctrl), Some(command));
            // Ctrl+Shift+Z has always undone, and Ctrl+Alt is not a shortcut at all.
            assert_eq!(
                (table.command_key)(Some(code), ctrl | ModifiersState::SHIFT),
                Some(command)
            );
            assert_eq!(
                (table.command_key)(Some(code), ctrl | ModifiersState::ALT),
                None
            );
            assert_eq!(
                (table.command_key)(Some(code), ModifiersState::empty()),
                None
            );
        }
        assert!((table.shortcut_held)(ctrl));
        assert!(!(table.shortcut_held)(ctrl | ModifiersState::ALT));
        assert!(!(table.shortcut_held)(ModifiersState::SUPER));
        assert_eq!((table.command_key)(Some(KeyCode::KeyQ), ctrl), None);
        assert_eq!((table.command_key)(Some(KeyCode::ArrowLeft), ctrl), None);
    }

    #[test]
    fn the_cmd_table_follows_macos_conventions() {
        let table = Keymap::cmd_standard();
        let cmd = ModifiersState::SUPER;
        let key = |code, modifiers| (table.command_key)(Some(code), modifiers);
        assert_eq!(key(KeyCode::KeyF, cmd), Some(KeyCommand::OpenFind));
        assert_eq!(
            key(KeyCode::KeyF, cmd | ModifiersState::ALT),
            Some(KeyCommand::OpenReplace)
        );
        assert_eq!(key(KeyCode::KeyZ, cmd), Some(KeyCommand::Undo));
        assert_eq!(
            key(KeyCode::KeyZ, cmd | ModifiersState::SHIFT),
            Some(KeyCommand::Redo)
        );
        assert_eq!(key(KeyCode::KeyQ, cmd), Some(KeyCommand::Quit));
        assert_eq!(
            key(KeyCode::ArrowRight, cmd | ModifiersState::SHIFT),
            Some(KeyCommand::LineBoundary { end: true })
        );
        assert_eq!(
            key(KeyCode::ArrowUp, cmd),
            Some(KeyCommand::DocumentBoundary { end: false })
        );
        // Cmd+H belongs to macOS, Ctrl is not a shortcut, and there is no Insert key.
        assert_eq!(key(KeyCode::KeyH, cmd), None);
        assert_eq!(key(KeyCode::KeyZ, ModifiersState::CONTROL), None);
        assert!((table.shortcut_held)(cmd));
        assert!(!(table.shortcut_held)(ModifiersState::CONTROL));
        assert_eq!(
            (table.clipboard_alias)(Some(KeyCode::Insert), true, false),
            None
        );
    }

    /// The table a host selects is these functions, field by field.
    #[test]
    fn the_standard_table_answers_as_its_functions_do() {
        let table = Keymap::ctrl_standard();
        let insert = Some(KeyCode::Insert);
        assert_eq!(
            (table.clipboard_alias)(insert, true, false),
            Some(ClipboardAlias::Copy)
        );
        assert_eq!(
            (table.window_key)(Some(KeyCode::Digit2), true, false),
            Some(WindowKey::View(ViewMode::Split))
        );
        assert_eq!(
            (table.view_key_mode)(Some(KeyCode::Digit3)),
            Some(ViewMode::Preview)
        );
        assert_eq!(
            (table.zoom_key_command)(Some(KeyCode::Digit0)),
            Some(ZoomCommand::Reset)
        );
        assert_eq!(
            (table.global_persistence_shortcut)(Some(KeyCode::KeyS), true, true),
            Some(PersistenceIntent::Export)
        );
    }
}
