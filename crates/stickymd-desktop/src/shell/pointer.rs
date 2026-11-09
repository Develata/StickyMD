//! Pointer input translation for the interaction shell: presses, the cursor, moves and the wheel
//! over the note, Preview, Search and the scrollbars. The keyboard is `input.rs`.
//!
//! plan_ref: docs/plan/07_editor_and_ime.md#source-editor

use stickymd_core::Selection;
use winit::dpi::PhysicalPosition;
use winit::event::{ElementState, MouseButton, MouseScrollDelta};

use super::DesktopShell;
#[cfg(test)]
use super::shell_pins_tests::PressRoute;
use crate::host::Host;
use crate::input::ZoomCommand;
use crate::preview::{meaningful_preview_drag, pane_local};
use crate::scroll::ScrollPane;
use stickymd_app::instruction::PersistenceIntent;
use stickymd_app::session::ZoomInput;

impl<H: Host> DesktopShell<H> {
    pub(super) fn handle_mouse_button(&mut self, state: ElementState, button: MouseButton) {
        if state == ElementState::Released
            && self.preview_horizontal.drag.is_some()
            && self.handle_horizontal_button(state, button)
        {
            return;
        }
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
        if self.handle_scrollbar_button(state, button)
            || self.handle_horizontal_button(state, button)
        {
            press_probe!(self, PressRoute::Scroll);
            return;
        }
        if button != MouseButton::Left {
            return;
        }
        match state {
            ElementState::Pressed => {
                if self.preview_at_cursor().is_some() {
                    press_probe!(self, PressRoute::Preview);
                    self.preview_focused = true;
                    self.sync_ime_allowed();
                    self.press_preview_selection();
                    return;
                }
                // Only a press on the Source pane gives the note the keyboard; one that lands on no
                // pane (outside the client area while the mouse is captured) changes no focus.
                let Some(source_position) = self.source_local_cursor() else {
                    return;
                };
                press_probe!(self, PressRoute::Source);
                self.preview_focused = false;
                self.sync_ime_allowed();
                self.app.editor.cancel_preedit();
                let diagnostic_action = self.projection.as_ref().and_then(|projection| {
                    projection.diagnostic_action_at(source_position.0, source_position.1)
                });
                if let Some(primary) = diagnostic_action
                    && self.dispatch_persistence_intent(if primary {
                        PersistenceIntent::ResolvePrimary
                    } else {
                        PersistenceIntent::ResolveSecondary
                    })
                {
                    return;
                }
                let Some(projection) = &self.projection else {
                    return;
                };
                let hit = projection.hit_test(source_position.0, source_position.1);
                self.app.editor.selection = if self.modifiers.shift_key() {
                    Selection::new(self.app.editor.selection.anchor.byte, hit)
                } else {
                    Selection::caret(hit)
                };
                self.app.editor.dragging_selection = true;
                self.after_presentation_change();
            }
            ElementState::Released => {
                if self.preview_dragging {
                    self.release_preview_selection();
                }
                self.app.editor.dragging_selection = false;
            }
        }
    }

    /// Sets the cursor again when what lies under a still pointer changes (the opacity popup
    /// opening or closing), only while the pointer is over the window: the cursor is shared and
    /// belongs to the window under the pointer (Microsoft `SetCursor`).
    pub(super) fn refresh_cursor(&self) {
        if self.pointer_inside_window {
            self.apply_cursor();
        }
    }

    /// The cursor for what lies under the pointer.
    pub(super) fn cursor_at_pointer(&self) -> winit::window::CursorIcon {
        self.shell_cursor_icon()
            .or_else(|| {
                self.scrollbar_gutter_at_cursor()
                    .map(|_| winit::window::CursorIcon::Default)
            })
            .unwrap_or_else(|| self.cursor_icon_for_position(self.cursor_position))
    }

    /// Sets the cursor for what lies under the pointer.
    fn apply_cursor(&self) {
        let cursor = self.cursor_at_pointer();
        if let Some(window) = &self.window {
            window.set_cursor(cursor);
        }
    }

    pub(super) fn handle_cursor_moved(&mut self, position: PhysicalPosition<f64>) {
        self.cursor_position = position;
        if self.handle_shell_cursor_moved(position) {
            return;
        }
        if self.handle_horizontal_cursor() || self.handle_scrollbar_cursor() {
            return;
        }
        self.apply_cursor();
        // The formula-error hover title is derived from the pointer and the current frame when
        // the event pass ends (`update_window_title`), so it cannot outlive either.
        if self.preview_dragging {
            if let Some(start) = self.preview_press_position {
                let scale = self
                    .window
                    .as_ref()
                    .map_or(1.0, |window| window.scale_factor());
                self.preview_drag_moved |= meaningful_preview_drag(start, position, scale);
            }
            self.extend_preview_selection(position);
            return;
        }
        if !self.app.editor.dragging_selection {
            return;
        }
        let Some(projection) = &self.projection else {
            return;
        };
        let Some(source_position) = self.source_local_position(position) else {
            return;
        };
        let active = projection.hit_test(source_position.0, source_position.1);
        self.app.editor.selection = Selection::new(self.app.editor.selection.anchor.byte, active);
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
                self.apply_zoom_command(ZoomCommand::Step(delta as i16), ZoomInput::Wheel);
            }
            return;
        }
        if self.scroll_preview_horizontally(delta) {
            return;
        }
        let pixels = match delta {
            MouseScrollDelta::LineDelta(_, lines) => -lines * 48.0,
            MouseScrollDelta::PixelDelta(position) => -position.y as f32,
        };
        let pane = if self.preview_at_cursor().is_some()
            || self.scrollbar_gutter_at_cursor() == Some(ScrollPane::Preview)
        {
            ScrollPane::Preview
        } else {
            ScrollPane::Source
        };
        self.scroll_pane_by(pane, pixels);
    }

    fn source_local_cursor(&self) -> Option<(f32, f32)> {
        self.source_local_position(self.cursor_position)
    }

    fn source_local_position(&self, position: PhysicalPosition<f64>) -> Option<(f32, f32)> {
        pane_local(self.view_geometry()?.source?, position)
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

#[cfg(test)]
mod wheel_tests {
    use super::ZoomWheelAccumulator;
    use winit::dpi::PhysicalPosition;
    use winit::event::MouseScrollDelta;

    #[test]
    fn phase10_high_resolution_wheel_is_deterministic() {
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
}
