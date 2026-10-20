//! Translates native shell pointer, keyboard, tray, and control interactions.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, KeyEvent, MouseButton};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{CursorIcon, ResizeDirection};

use super::StickyApp;
use super::controls::{ControlId, ControlLayout, split_sync_rect};
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
                        let intent = self
                            .controls
                            .press_slider(layout.opacity_at(self.cursor_position.x));
                        self.dispatch_window_preference_intent(intent);
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
                    self.activate_control(control);
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
            let split_sync = self.config.current().view_mode == ViewMode::Split
                && self.view_geometry().is_some_and(|geometry| {
                    geometry.divider_x.is_some_and(|divider_x| {
                        split_sync_rect(divider_x, geometry.toolbar_height, window.scale_factor())
                            .contains(self.cursor_position)
                    })
                });
            (layout.control_at(self.cursor_position).is_some()
                || split_sync
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

    /// Closes the popup and drops any uncommitted slider or numeric value.
    pub(super) fn cancel_opacity_popup(&mut self) {
        let committed = self.config.current().opacity;
        self.controls.cancel(committed);
        self.dispatch_window_preference_intent(WindowPreferenceIntent::PreviewOpacity(committed));
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
