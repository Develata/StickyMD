//! Translates native shell pointer, keyboard, tray, and control interactions.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton};
use winit::window::{CursorIcon, ResizeDirection};

use super::DesktopShell;
#[cfg(test)]
use super::shell_pins_tests::PressRoute;
use crate::controls::{ControlHit, ControlId, PopupHit};
use crate::host::Host;
use stickymd_app::config::ViewMode;
use stickymd_app::flow::window::state::WindowIntent;
use stickymd_app::instruction::{
    AppIntent, PreviewIntent, WindowPlatformIntent, WindowPreferenceIntent, WindowResizeEdge,
};
use stickymd_app::interaction::rules::{scope_to_selection, theme_rotation};
use stickymd_app::ports::TrayCommand;

#[cfg(test)]
mod runtime_tests;

impl<H: Host> DesktopShell<H> {
    pub(super) fn handle_tray_command(&mut self, command: TrayCommand) {
        match command {
            TrayCommand::ToggleVisibility => {
                let now_ms = self.timestamp_ms();
                let guards = self.window_guards();
                self.with_session(|app, _, out| {
                    app.toggle_visibility_from_tray(now_ms, guards, out);
                });
            }
            TrayCommand::ToggleAlwaysOnTop => self.toggle_topmost(),
            TrayCommand::Quit => self.dispatch_window_intent(WindowIntent::TrayQuitRequested {
                guards: self.window_guards(),
            }),
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
        let Some(layout) = &self.control_layout else {
            return false;
        };
        match state {
            ElementState::Pressed => {
                let cursor = self.cursor_position;
                // A press outside the open popup closes it below, before the control it hits
                // acts; the Opacity button must see the popup as this press found it.
                let popup_was_open = self.controls.opacity_popup_open;
                let hit = layout.hit_test(cursor, popup_was_open);
                let slider_value = layout.opacity_at(cursor.x);
                if popup_was_open {
                    // A press anywhere but the field ends its edit first (plan 09: the field
                    // commits when it loses focus), so a slider drag never overrides typed input.
                    if self.controls.opacity_input_focused
                        && hit != Some(ControlHit::Popup(PopupHit::Field))
                    {
                        self.commit_opacity_input();
                    }
                    // A field edit was committed above; the value is already the committed one.
                    if !matches!(hit, Some(ControlHit::Popup(_))) {
                        self.controls.opacity_popup_open = false;
                        self.refresh_cursor();
                    }
                }
                match hit {
                    Some(ControlHit::Popup(PopupHit::Slider)) => {
                        let intent = self.controls.press_slider(slider_value);
                        self.dispatch_window_preference_intent(intent);
                    }
                    Some(ControlHit::Popup(PopupHit::Field)) => self.focus_opacity_input(),
                    Some(ControlHit::Popup(PopupHit::Blank)) => {
                        press_probe!(self, PressRoute::PopupBlank, true);
                    }
                    Some(ControlHit::Resize(edge)) => {
                        return self.dispatch_window_platform_intent(
                            WindowPlatformIntent::RequestResize(edge),
                        );
                    }
                    Some(ControlHit::SplitSync) => {
                        let enabled = !self.app.config().current().split_scroll_sync;
                        self.dispatch_window_preference_intent(
                            WindowPreferenceIntent::SetSplitScrollSync(enabled),
                        );
                    }
                    Some(ControlHit::Button(control)) => {
                        match control_press(control, popup_was_open) {
                            ControlPress::Activate(control) => self.activate_control(control),
                            ControlPress::PopupClosed => {
                                press_probe!(self, PressRoute::PopupClosed, true);
                                self.request_redraw();
                            }
                        }
                    }
                    Some(ControlHit::DragWindow) => {
                        return self
                            .dispatch_window_platform_intent(WindowPlatformIntent::RequestDrag);
                    }
                    None => return false,
                }
                true
            }
            ElementState::Released => {
                if let Some(intent) = self.controls.release_slider() {
                    self.dispatch_window_preference_intent(intent);
                    return true;
                }
                false
            }
        }
    }

    pub(super) fn handle_shell_cursor_moved(&mut self, position: PhysicalPosition<f64>) -> bool {
        let intent = self
            .control_layout
            .as_ref()
            .and_then(|layout| self.controls.drag_slider(|| layout.opacity_at(position.x)));
        if let Some(intent) = intent {
            self.dispatch_window_preference_intent(intent);
            return true;
        }
        false
    }

    /// Sets the IME for whoever takes the keyboard now (`ime_allowed`). Every site that changes
    /// who that is calls it: window focus, input going away or coming back, a press on Preview or
    /// the note, opening or closing Search, a view change, the opacity field's edit starting or
    /// ending. Search counts as drawn where it has a layout (`search_layout`, as it is painted).
    pub(super) fn sync_ime_allowed(&self) {
        if let Some(window) = &self.window {
            window.set_ime_allowed(self.ime_allowed_for(self.search_layout().is_some()));
        }
    }

    fn ime_allowed_for(&self, search_drawn: bool) -> bool {
        ime_allowed(
            self.app.editor.focused && self.shell_input_enabled,
            self.controls.takes_keys(),
            self.app.search.open && search_drawn,
            self.preview_focused,
        )
    }

    /// The cursor the shell's own surface asks for; `None` off it. The toolbar outside its buttons
    /// moves the window and is no button, so it shows the arrow.
    pub(super) fn shell_cursor_icon(&self) -> Option<CursorIcon> {
        let layout = self.control_layout.as_ref()?;
        match layout.hit_test(self.cursor_position, self.controls.opacity_popup_open)? {
            ControlHit::Popup(_) | ControlHit::SplitSync | ControlHit::Button(_) => {
                Some(CursorIcon::Pointer)
            }
            ControlHit::Resize(edge) => Some(CursorIcon::from(winit_resize_direction(edge))),
            ControlHit::DragWindow => Some(CursorIcon::Default),
        }
    }

    fn activate_control(&mut self, control: ControlId) {
        press_probe!(self, PressRoute::Button(control));
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
            ControlId::Collapse => self.dispatch_window_intent(WindowIntent::ManualCollapse {
                now_ms: self.timestamp_ms(),
            }),
            ControlId::Close => self.dispatch_window_intent(WindowIntent::CloseRequested {
                now_ms: self.timestamp_ms(),
                guards: self.window_guards(),
            }),
        }
    }

    fn convert_math_delimiters(&mut self) {
        let selection = self.app.editor.selection;
        let mode = self.app.config().current().view_mode;
        self.dispatch(AppIntent::ConvertLatexMathDelimiters {
            expected_generation: self.app.coordinator().view().generation,
            selection,
            scope_to_selection: scope_to_selection(mode, selection.is_collapsed()),
            timestamp_ms: self.timestamp_ms(),
        });
    }

    fn toggle_topmost(&mut self) {
        let next = !self.app.config().current().always_on_top;
        self.dispatch_window_preference_intent(WindowPreferenceIntent::SetAlwaysOnTop(next));
    }

    fn cycle_theme(&mut self) {
        let next = theme_rotation(self.app.config().current().theme);
        self.dispatch_window_preference_intent(WindowPreferenceIntent::SetTheme(next));
    }
}

/// Whether the IME is on: only in a focused window that accepts input, and only for a receiver
/// that composes, taken in `handle_key`'s order: the edited opacity field (numeric, no IME), then
/// a drawn Search (composes, even after a press on Preview), then focused Preview (no IME), then
/// the note (composes). Preview parks an open Search (it is closed there, `park_search`); a
/// Search that is open without a layout would not be drawn, so it still gets no IME.
fn ime_allowed(
    window_takes_input: bool,
    field_edited: bool,
    search_drawn: bool,
    preview_focused: bool,
) -> bool {
    window_takes_input && !field_edited && (search_drawn || !preview_focused)
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
    use crate::controls::ControlLayout;
    use winit::dpi::PhysicalSize;

    use super::*;

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
                let layout = ControlLayout::new(ViewMode::Source, size, scale);
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
}
