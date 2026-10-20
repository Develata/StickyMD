//! Translates native shell pointer, keyboard, tray, and control interactions.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, Ime, KeyEvent, MouseButton};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, ModifiersState};
use winit::window::{CursorIcon, ResizeDirection};

use super::StickyApp;
use super::controls::{ControlId, ControlLayout, PopupHit, split_sync_rect};
use crate::config::{ThemeMode, ViewMode};
use crate::flow::window::state::{ShowReason, WindowIntent};
use crate::instruction::{
    AppIntent, PreviewIntent, WindowPlatformIntent, WindowPreferenceIntent, WindowResizeEdge,
};
use crate::platform::windows::tray::TrayPlatformEvent;

const RESIZE_BORDER_DIP: f64 = 6.0;

impl StickyApp {
    pub(super) fn handle_tray_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        event: TrayPlatformEvent,
    ) {
        match event {
            TrayPlatformEvent::ShowHideRequested if self.window_is_hidden_to_tray() => self
                .dispatch_window_intent(
                    Some(event_loop),
                    WindowIntent::ShowRequested {
                        reason: ShowReason::Tray,
                        now_ms: self.timestamp_ms(),
                    },
                ),
            TrayPlatformEvent::ShowHideRequested => self.dispatch_window_intent(
                Some(event_loop),
                WindowIntent::TrayToggleRequested {
                    now_ms: self.timestamp_ms(),
                    guards: self.window_guards(),
                },
            ),
            TrayPlatformEvent::AlwaysOnTopToggled => self.toggle_topmost(),
            TrayPlatformEvent::QuitRequested => self.dispatch_window_intent(
                Some(event_loop),
                WindowIntent::TrayQuitRequested {
                    guards: self.window_guards(),
                },
            ),
        }
    }

    pub(super) fn handle_shell_mouse_button(
        &mut self,
        state: ElementState,
        button: MouseButton,
    ) -> bool {
        if button != MouseButton::Left || !self.shell_input_enabled {
            return false;
        }
        let Some(window) = self.window.as_ref().cloned() else {
            return false;
        };
        let layout = ControlLayout::new(window.inner_size(), window.scale_factor());
        match state {
            ElementState::Pressed => {
                // A press outside the open popup closes it below, before the control it hits
                // acts; the Opacity button must see the popup as this press found it.
                let popup_was_open = self.controls.opacity_popup_open;
                if self.controls.opacity_popup_open {
                    let hit = layout.popup_hit(self.cursor_position);
                    // A press anywhere but the field ends its edit first (plan 09: the field
                    // commits when it loses focus), so a slider drag never overrides typed input.
                    if self.controls.opacity_input_focused && hit != Some(PopupHit::Field) {
                        self.commit_opacity_input();
                    }
                    match hit {
                        Some(PopupHit::Slider) => {
                            let intent = self
                                .controls
                                .press_slider(layout.opacity_at(self.cursor_position.x));
                            self.dispatch_window_preference_intent(intent);
                            return true;
                        }
                        Some(PopupHit::Field) => {
                            self.focus_opacity_input();
                            return true;
                        }
                        Some(PopupHit::Blank) => return true,
                        // A field edit was committed above; the value is already the committed one.
                        None => {
                            self.controls.opacity_popup_open = false;
                            self.refresh_cursor();
                        }
                    }
                }
                if let Some(direction) = resize_direction(
                    window.inner_size(),
                    self.cursor_position,
                    window.scale_factor(),
                ) {
                    return self.dispatch_window_platform_intent(
                        WindowPlatformIntent::RequestResize(direction),
                    );
                }
                if self.config.current().view_mode == ViewMode::Split
                    && let Some(geometry) = self.view_geometry()
                    && let Some(divider_x) = geometry.divider_x
                    && split_sync_rect(divider_x, geometry.toolbar_height, window.scale_factor())
                        .contains(self.cursor_position)
                {
                    let enabled = !self.config.current().split_scroll_sync;
                    self.dispatch_window_preference_intent(
                        WindowPreferenceIntent::SetSplitScrollSync(enabled),
                    );
                    return true;
                }
                if let Some(control) = layout.control_at(self.cursor_position) {
                    match control_press(control, popup_was_open) {
                        ControlPress::Activate(control) => self.activate_control(control),
                        ControlPress::PopupClosed => self.request_redraw(),
                    }
                    return true;
                }
                if layout.is_drag_region(self.cursor_position) {
                    return self.dispatch_window_platform_intent(WindowPlatformIntent::RequestDrag);
                }
            }
            ElementState::Released => {
                if let Some(intent) = self.controls.release_slider() {
                    self.dispatch_window_preference_intent(intent);
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn handle_shell_cursor_moved(&mut self, position: PhysicalPosition<f64>) -> bool {
        // The layout is read only while a slider drag is in progress.
        let intent = self.window.as_ref().and_then(|window| {
            self.controls.drag_slider(|| {
                ControlLayout::new(window.inner_size(), window.scale_factor())
                    .opacity_at(position.x)
            })
        });
        if let Some(intent) = intent {
            self.dispatch_window_preference_intent(intent);
            return true;
        }
        false
    }

    /// A key-down while the field is edited. `handle_key` runs it where Search's keys run, so Esc,
    /// F6/F7, save and export keep their window-wide meaning first; every key it reaches is the
    /// field's, and a key the field does not use is dropped, so none edits the note or Search.
    pub(super) fn handle_opacity_key(&mut self, event: &KeyEvent, code: Option<KeyCode>) {
        match opacity_key(code, event.text.as_deref(), self.modifiers) {
            OpacityKey::Commit => {
                self.controls.claim_commit_key(event.physical_key);
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
    fn focus_opacity_input(&mut self) {
        if self.session.is_composing() || self.search.is_composing() {
            self.diagnostic = Some("请先完成当前输入法组合。".into());
        } else {
            self.controls.focus_input();
            self.sync_ime_allowed();
        }
        self.request_redraw();
    }

    /// Sets the IME for whoever takes the keyboard now (`ime_allowed`). Every site that changes
    /// who that is calls it: window focus, input going away or coming back, a press on Preview or
    /// the note, opening or closing Search, a view change, the opacity field's edit starting or
    /// ending. Search counts as drawn where it has a layout (`search_layout`, as it is painted).
    pub(super) fn sync_ime_allowed(&self) {
        if let Some(window) = &self.window {
            window.set_ime_allowed(ime_allowed(
                self.session.focused && self.shell_input_enabled,
                self.controls.takes_keys(),
                self.search.open && self.search_layout().is_some(),
                self.preview_focused,
            ));
        }
    }

    pub(super) fn shell_cursor_icon(&self) -> Option<CursorIcon> {
        let window = self.window.as_ref()?;
        let layout = ControlLayout::new(window.inner_size(), window.scale_factor());
        // The open popup takes every press inside it before the resize border does
        // (`handle_shell_mouse_button`), so its cursor comes first too.
        if self.controls.opacity_popup_open && layout.opacity_popup.contains(self.cursor_position) {
            return Some(CursorIcon::Pointer);
        }
        resize_direction(
            window.inner_size(),
            self.cursor_position,
            window.scale_factor(),
        )
        .map(|direction| CursorIcon::from(winit_resize_direction(direction)))
        .or_else(|| {
            let split_sync = self.config.current().view_mode == ViewMode::Split
                && self.view_geometry().is_some_and(|geometry| {
                    geometry.divider_x.is_some_and(|divider_x| {
                        split_sync_rect(divider_x, geometry.toolbar_height, window.scale_factor())
                            .contains(self.cursor_position)
                    })
                });
            (layout.control_at(self.cursor_position).is_some() || split_sync)
                .then_some(CursorIcon::Pointer)
        })
    }

    fn activate_control(&mut self, control: ControlId) {
        match control {
            ControlId::Source => {
                self.dispatch_preview_intent(PreviewIntent::SetViewMode(ViewMode::Source))
            }
            ControlId::Split => {
                self.dispatch_preview_intent(PreviewIntent::SetViewMode(ViewMode::Split))
            }
            ControlId::Preview => {
                self.dispatch_preview_intent(PreviewIntent::SetViewMode(ViewMode::Preview))
            }
            ControlId::ConvertMath => self.convert_math_delimiters(),
            ControlId::Topmost => self.toggle_topmost(),
            ControlId::Theme => self.cycle_theme(),
            ControlId::Opacity => {
                self.controls.opacity_popup_open = !self.controls.opacity_popup_open;
                self.controls.opacity_input_focused = false;
                self.refresh_cursor();
                self.request_redraw();
            }
            ControlId::Collapse => self.dispatch_window_intent(
                None,
                WindowIntent::ManualCollapse {
                    now_ms: self.timestamp_ms(),
                },
            ),
            ControlId::Close => self.dispatch_window_intent(
                None,
                WindowIntent::CloseRequested {
                    now_ms: self.timestamp_ms(),
                    guards: self.window_guards(),
                },
            ),
        }
    }

    fn convert_math_delimiters(&mut self) {
        let selection = self.session.selection;
        let mode = self.config.current().view_mode;
        self.dispatch(AppIntent::ConvertLatexMathDelimiters {
            expected_generation: self.coordinator.view().generation,
            selection,
            scope_to_selection: mode != ViewMode::Preview && !selection.is_collapsed(),
            timestamp_ms: self.timestamp_ms(),
        });
    }

    fn toggle_topmost(&mut self) {
        let next = !self.config.current().always_on_top;
        self.dispatch_window_preference_intent(WindowPreferenceIntent::SetAlwaysOnTop(next));
    }

    fn cycle_theme(&mut self) {
        let next = match self.config.current().theme {
            ThemeMode::Light => ThemeMode::System,
            ThemeMode::System => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
        };
        self.dispatch_window_preference_intent(WindowPreferenceIntent::SetTheme(next));
    }

    /// Closes the popup and drops any uncommitted slider or numeric value.
    pub(super) fn cancel_opacity_popup(&mut self) {
        let editing = self.controls.takes_keys();
        let committed = self.config.current().opacity;
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

/// Whether the IME is on: only in a focused window that accepts input, and only for a receiver
/// that composes, taken in `handle_key`'s order: the edited opacity field (numeric, no IME), then
/// a drawn Search (composes, even after a press on Preview), then focused Preview (no IME), then
/// the note (composes). A Search left open in Preview mode is not drawn, so a composition there
/// would show nowhere: it gets no IME.
fn ime_allowed(
    window_takes_input: bool,
    field_edited: bool,
    search_drawn: bool,
    preview_focused: bool,
) -> bool {
    window_takes_input && !field_edited && (search_drawn || !preview_focused)
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

/// What a press on a toolbar control does.
#[derive(Debug, PartialEq, Eq)]
enum ControlPress {
    Activate(ControlId),
    /// The press found the popup open and has closed it, a field edit committed first: the
    /// Opacity button's one transition is done.
    PopupClosed,
}

/// The Opacity button toggles the popup as the press found it (`popup_was_open`). A press on
/// it while the popup is open lands outside the popup, which closes it before any control
/// acts, so activating the button then would reopen it. Every other control acts after the
/// close.
fn control_press(control: ControlId, popup_was_open: bool) -> ControlPress {
    match control {
        ControlId::Opacity if popup_was_open => ControlPress::PopupClosed,
        control => ControlPress::Activate(control),
    }
}

fn resize_direction(
    size: PhysicalSize<u32>,
    point: PhysicalPosition<f64>,
    scale: f64,
) -> Option<WindowResizeEdge> {
    let border = RESIZE_BORDER_DIP * scale.max(0.5);
    let left = point.x >= 0.0 && point.x < border;
    let right = point.x <= size.width as f64 && point.x >= size.width as f64 - border;
    let top = point.y >= 0.0 && point.y < border;
    let bottom = point.y <= size.height as f64 && point.y >= size.height as f64 - border;
    match (left, right, top, bottom) {
        (true, _, true, _) => Some(WindowResizeEdge::NorthWest),
        (_, true, true, _) => Some(WindowResizeEdge::NorthEast),
        (true, _, _, true) => Some(WindowResizeEdge::SouthWest),
        (_, true, _, true) => Some(WindowResizeEdge::SouthEast),
        (true, _, _, _) => Some(WindowResizeEdge::West),
        (_, true, _, _) => Some(WindowResizeEdge::East),
        (_, _, true, _) => Some(WindowResizeEdge::North),
        (_, _, _, true) => Some(WindowResizeEdge::South),
        _ => None,
    }
}

fn winit_resize_direction(direction: WindowResizeEdge) -> ResizeDirection {
    match direction {
        WindowResizeEdge::North => ResizeDirection::North,
        WindowResizeEdge::NorthEast => ResizeDirection::NorthEast,
        WindowResizeEdge::East => ResizeDirection::East,
        WindowResizeEdge::SouthEast => ResizeDirection::SouthEast,
        WindowResizeEdge::South => ResizeDirection::South,
        WindowResizeEdge::SouthWest => ResizeDirection::SouthWest,
        WindowResizeEdge::West => ResizeDirection::West,
        WindowResizeEdge::NorthWest => ResizeDirection::NorthWest,
    }
}

#[cfg(test)]
mod phase8_interaction_tests {
    use super::*;

    /// Plan 09 gutter geometry: in every view, at every DPI and window size, `geometry()`
    /// reserves exactly the scaled gutter right of each pane (before the divider in a Split,
    /// before the window edge otherwise), and each pane's scrollbar track lies inside its
    /// gutter, clear of the native resize border (the real hit test finds no edge on any track
    /// pixel) and of the Split sync button.
    #[test]
    fn every_view_reserves_the_gutter_clear_of_the_resize_border_and_sync_button() {
        use super::super::preview_runtime::geometry;
        use super::super::scrollbar::{GUTTER_DIP, ScrollPane, Scrollbar};
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
                                resize_direction(
                                    size,
                                    PhysicalPosition::new(x, y),
                                    f64::from(scale)
                                ),
                                None,
                                "{label}: {side:?} track pixel ({x},{y}) is a resize edge"
                            );
                        }
                        if let (ScrollPane::Source, Some(divider)) = (side, view.divider_x) {
                            let sync =
                                split_sync_rect(divider, view.toolbar_height, f64::from(scale));
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

    /// The IME follows the keyboard's receiver in `handle_key`'s order: the edited field, a drawn
    /// Search, focused Preview, then the note; only the Search and the note compose, and only in a
    /// focused window that accepts input. Applying it at each site is not exercised here.
    #[test]
    fn the_ime_follows_whoever_takes_the_keyboard() {
        for bits in 0..16_u8 {
            let [input, field, search, preview] = [0, 1, 2, 3].map(|bit| bits & (1 << bit) != 0);
            let composes = if field {
                false
            } else if search {
                true
            } else {
                !preview
            };
            assert_eq!(
                ime_allowed(input, field, search, preview),
                input && composes,
                "input {input} field {field} search {search} preview {preview}"
            );
        }
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

    /// The Opacity button toggles the popup as the press found it: open, the press has already
    /// closed it (the whole button lies above the popup at every scale and size), and nothing
    /// reopens it; closed, the button opens it. Every other control acts either way. This checks
    /// the decision at four scales and three sizes; that the handler applies it is checked by the
    /// copied-Release `WindowShell` runtime (`stickymd-smoke` `runtime/opacity_popup.rs`), which
    /// sends the presses as window messages, not native pointer delivery.
    #[test]
    fn the_opacity_button_closes_the_popup_it_finds_open() {
        for scale in [1.0, 1.25, 1.5, 2.0] {
            for (width, height) in [(520.0, 680.0), (220.0, 120.0), (1200.0, 900.0)] {
                let size = PhysicalSize::new((width * scale) as u32, (height * scale) as u32);
                let layout = ControlLayout::new(size, scale);
                let button = layout.rect(ControlId::Opacity);
                let center = PhysicalPosition::new(
                    button.x + button.width * 0.5,
                    button.y + button.height * 0.5,
                );
                assert_eq!(layout.control_at(center), Some(ControlId::Opacity));
                assert!(
                    button.y + button.height <= layout.opacity_popup.y,
                    "{size:?} at {scale}"
                );
            }
        }
        for control in [
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
            assert_eq!(
                control_press(control, false),
                ControlPress::Activate(control)
            );
            let open = if control == ControlId::Opacity {
                ControlPress::PopupClosed
            } else {
                ControlPress::Activate(control)
            };
            assert_eq!(control_press(control, true), open, "{control:?}");
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
}
