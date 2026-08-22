//! Translates native shell pointer, keyboard, tray, and control interactions.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorIcon, ResizeDirection};

use super::StickyApp;
use super::controls::{ControlId, ControlLayout};
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
                if self.controls.opacity_popup_open {
                    if layout.opacity_slider.contains(self.cursor_position) {
                        self.controls.opacity_dragging = true;
                        self.preview_opacity(layout.opacity_at(self.cursor_position.x));
                        return true;
                    }
                    if layout.opacity_input.contains(self.cursor_position) {
                        self.controls.opacity_input_focused = true;
                        return true;
                    }
                    if !layout.opacity_popup.contains(self.cursor_position) {
                        self.commit_opacity_input();
                        self.controls.opacity_popup_open = false;
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
                if let Some(control) = layout.control_at(self.cursor_position) {
                    self.activate_control(control);
                    return true;
                }
                if layout.is_drag_region(self.cursor_position) {
                    return self.dispatch_window_platform_intent(WindowPlatformIntent::RequestDrag);
                }
            }
            ElementState::Released => {
                if self.controls.opacity_dragging {
                    self.controls.opacity_dragging = false;
                    self.commit_opacity(self.controls.opacity_preview);
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn handle_shell_cursor_moved(&mut self, position: PhysicalPosition<f64>) -> bool {
        if self.controls.opacity_popup_open
            && self.controls.opacity_dragging
            && let Some(window) = &self.window
        {
            let layout = ControlLayout::new(window.inner_size(), window.scale_factor());
            self.preview_opacity(layout.opacity_at(position.x));
            return true;
        }
        false
    }

    pub(super) fn handle_shell_key(&mut self, event: &KeyEvent) -> bool {
        if event.state != ElementState::Pressed {
            return false;
        }
        if self.controls.opacity_popup_open && self.controls.opacity_input_focused {
            match event.physical_key {
                PhysicalKey::Code(KeyCode::Enter | KeyCode::NumpadEnter) => {
                    self.commit_opacity_input();
                    return true;
                }
                PhysicalKey::Code(KeyCode::Backspace) => {
                    self.controls.opacity_input.pop();
                    return true;
                }
                PhysicalKey::Code(KeyCode::Escape) => {
                    self.controls.opacity_input_focused = false;
                    self.controls.opacity_popup_open = false;
                    self.controls.opacity_input = self.config.current().opacity.to_string();
                    self.controls.opacity_preview = self.config.current().opacity;
                    self.dispatch_window_preference_intent(WindowPreferenceIntent::PreviewOpacity(
                        self.config.current().opacity,
                    ));
                    return true;
                }
                _ => {
                    if let Some(text) = &event.text {
                        self.controls.replace_input(text.to_string());
                        self.request_redraw();
                        return true;
                    }
                }
            }
        }
        false
    }

    pub(super) fn shell_cursor_icon(&self) -> Option<CursorIcon> {
        let window = self.window.as_ref()?;
        resize_direction(
            window.inner_size(),
            self.cursor_position,
            window.scale_factor(),
        )
        .map(|direction| CursorIcon::from(winit_resize_direction(direction)))
        .or_else(|| {
            let layout = ControlLayout::new(window.inner_size(), window.scale_factor());
            (layout.control_at(self.cursor_position).is_some()
                || (self.controls.opacity_popup_open
                    && layout.opacity_popup.contains(self.cursor_position)))
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

    fn preview_opacity(&mut self, opacity: u8) {
        self.dispatch_window_preference_intent(WindowPreferenceIntent::PreviewOpacity(opacity));
    }

    fn commit_opacity_input(&mut self) {
        if let Some(opacity) = self.controls.commit_input() {
            self.commit_opacity(opacity);
        }
        self.controls.opacity_input_focused = false;
    }

    fn commit_opacity(&mut self, opacity: u8) {
        self.dispatch_window_preference_intent(WindowPreferenceIntent::CommitOpacity(opacity));
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
