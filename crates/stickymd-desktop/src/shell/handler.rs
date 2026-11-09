//! The winit event loop's callbacks: the window's creation, its events, the end of each pass and
//! what the workers and the host post. Each callback ends with the exit its effects asked for.
//!
//! plan_ref: docs/plan/03_system_architecture.md#interaction-shell

use std::sync::Arc;
use std::time::{Duration, Instant};

use stickymd_render::source::{SourceInitializationMilestone, SourceProjection};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::{ElementState, MouseButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{Theme, WindowAttributes, WindowId};

use super::window_geometry_runtime::geometry_event;
use super::{CARET_BLINK, DesktopShell};
use crate::frame::SoftwareSurface;
use crate::host::{DesktopEvent, Host};
use stickymd_app::config::{MIN_WINDOW_HEIGHT_DIP, MIN_WINDOW_WIDTH_DIP, ViewMode};

impl<H: Host> ApplicationHandler<DesktopEvent> for DesktopShell<H> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_blink));
        if self.window.is_some() {
            return;
        }
        let configured_theme = match self.app.config().current().theme {
            stickymd_app::config::ThemeMode::Light => Some(Theme::Light),
            stickymd_app::config::ThemeMode::Dark => Some(Theme::Dark),
            stickymd_app::config::ThemeMode::System => None,
        };
        let base = WindowAttributes::default()
            .with_title("StickyMD")
            .with_theme(configured_theme)
            .with_decorations(false)
            .with_resizable(true)
            .with_visible(false)
            .with_inner_size(LogicalSize::new(
                self.app.config().current().window.width_dip as f64,
                self.app.config().current().window.height_dip as f64,
            ))
            .with_min_inner_size(LogicalSize::new(
                f64::from(MIN_WINDOW_WIDTH_DIP),
                f64::from(MIN_WINDOW_HEIGHT_DIP),
            ));
        let window = match event_loop.create_window(self.host.window_attributes(base)) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("window creation failed: {error}");
                event_loop.exit();
                return;
            }
        };
        self.host.window_created(&window);
        self.host.milestone("window_created");
        window.set_ime_allowed(self.app.config().current().view_mode != ViewMode::Preview);
        self.system_theme = window.theme().unwrap_or(Theme::Light);
        let size = window.inner_size();
        let surface = match SoftwareSurface::new(Arc::clone(&window)) {
            Ok(surface) => surface,
            Err(error) => {
                eprintln!("surface creation failed: {error}");
                event_loop.exit();
                return;
            }
        };
        self.host.milestone("surface_ready");
        self.host.milestone("display_ready");
        let initial_snapshot = self.app.coordinator().snapshot();
        self.host.milestone("font_system_begin");
        let document_scale = window.scale_factor() as f32
            * self.app.config().current().content_zoom_percent.factor();
        let initial_geometry = crate::geometry::geometry(
            self.app.config().current().view_mode,
            size,
            window.scale_factor() as f32,
        );
        let (initial_width, initial_height) = initial_geometry
            .source
            .or(initial_geometry.preview)
            .map_or((size.width.max(1), size.height.max(1)), |pane| {
                (pane.width.max(1), pane.height.max(1))
            });
        self.host.milestone("source_layout_begin");
        let host = &mut self.host;
        let projection = SourceProjection::new_observed(
            &initial_snapshot,
            initial_width,
            initial_height,
            document_scale,
            |milestone| match milestone {
                SourceInitializationMilestone::FontSystemReady => host.milestone("font_system_end"),
                SourceInitializationMilestone::SourceBufferReady => {
                    host.milestone("source_buffer_ready")
                }
                SourceInitializationMilestone::SourceShaped => {
                    host.milestone("source_layout_end");
                    host.milestone("source_projection_ready")
                }
            },
        );
        let fonts = projection.fonts();
        eprintln!(
            "font selection: CJK={} found={} Latin={} found={}",
            fonts.cjk_family, fonts.cjk_found, fonts.latin_family, fonts.latin_found
        );
        self.window = Some(window);
        self.surface = Some(surface);
        self.sync_control_layout();
        self.projection = Some(projection);
        if !self.initialize_window_shell() {
            event_loop.exit();
            return;
        }
        self.configure_viewports();
        if let Some(action) = self.app.show_preview_action() {
            self.submit_preview_action(action);
        }
        self.update_window_title();
        self.request_redraw();
        self.finish_callback(|| event_loop.exit());
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        // Debug-only native routing diagnostics; never log document or IME text.
        #[cfg(debug_assertions)]
        match &event {
            WindowEvent::Ime(ime) => {
                let (kind, len) = match ime {
                    winit::event::Ime::Enabled => ("enabled", 0),
                    winit::event::Ime::Disabled => ("disabled", 0),
                    winit::event::Ime::Preedit(text, _) => ("preedit", text.len()),
                    winit::event::Ime::Commit(text) => ("commit", text.len()),
                };
                eprintln!(
                    "INPUT_TRACE t={} ime={kind} len={len} search={} composing={}",
                    self.timestamp_ms(),
                    self.app.search.open,
                    self.app.search.is_composing()
                );
            }
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                eprintln!(
                    "INPUT_TRACE t={} key={:?} state={:?} repeat={} synthetic={is_synthetic} search={} composing={} ime_processed={}",
                    self.timestamp_ms(),
                    event.physical_key,
                    event.state,
                    event.repeat,
                    self.app.search.open,
                    self.app.search.is_composing(),
                    event.logical_key
                        == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Process)
                );
            }
            _ => {}
        }
        if let Some(geometry) = geometry_event(&event) {
            self.on_window_geometry(geometry);
        }
        match event {
            WindowEvent::CloseRequested => {
                self.dispatch_window_intent(
                    stickymd_app::flow::window::state::WindowIntent::CloseRequested {
                        now_ms: self.timestamp_ms(),
                        guards: self.window_guards(),
                    },
                );
            }
            WindowEvent::ThemeChanged(theme) => {
                self.system_theme = theme;
                if self.app.config().current().theme == stickymd_app::config::ThemeMode::System {
                    self.request_preview_relayout();
                    self.request_redraw();
                }
            }
            WindowEvent::Focused(focused) => {
                self.apply_focus_change(focused);
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            // winit synthesizes a press for every key held when the window gains focus and a
            // release when it loses it; neither is a key pressed or released here, so neither
            // edits anything nor ends what a held key owns (the opacity field's Enter).
            WindowEvent::KeyboardInput {
                event,
                is_synthetic,
                ..
            } => {
                self.handle_key(event, is_synthetic);
            }
            WindowEvent::Ime(event) => {
                self.handle_ime(event);
                self.refresh_window_guards();
            }
            WindowEvent::CursorMoved { position, .. } => self.handle_cursor_moved(position),
            WindowEvent::CursorEntered { .. } => {
                self.pointer_inside_window = true;
                // What lies under the pointer may have changed while it was away (the opacity
                // popup closing), and a re-entry at the same position brings no `CursorMoved`.
                self.refresh_cursor();
                self.dispatch_window_intent(
                    stickymd_app::flow::window::state::WindowIntent::SensorEntered {
                        now_ms: self.timestamp_ms(),
                    },
                );
                self.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer_inside_window = false;
                self.scrollbars.hovered = None;
                self.dispatch_window_intent(
                    stickymd_app::flow::window::state::WindowIntent::PointerLeft {
                        now_ms: self.timestamp_ms(),
                    },
                );
                self.request_redraw();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let move_resize_finished =
                    completes_move_resize(self.move_resize_active, state, button);
                self.handle_mouse_button(state, button);
                if move_resize_finished {
                    self.complete_window_drag();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => self.handle_scroll(delta),
            WindowEvent::RedrawRequested => self.render(event_loop),
            _ => {}
        }
        self.finish_callback(|| event_loop.exit());
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.run_deferred_display_recovery();
        let now_ms = self.timestamp_ms();
        self.settle_event_pass(now_ms);

        let now = Instant::now();
        let mut next_wake = self
            .app
            .next_deadline()
            .map(|deadline| self.clock_origin + Duration::from_millis(deadline));
        if self.caret_animation_active() {
            if now >= self.next_blink {
                self.next_blink = now + CARET_BLINK;
                self.advance_caret_blink();
            }
            next_wake =
                Some(next_wake.map_or(self.next_blink, |current| current.min(self.next_blink)));
        }
        if let Some(next_wake) = next_wake {
            event_loop.set_control_flow(ControlFlow::WaitUntil(next_wake));
        } else {
            event_loop.set_control_flow(ControlFlow::Wait);
        }
        self.finish_callback(|| event_loop.exit());
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: DesktopEvent) {
        match event {
            DesktopEvent::Io(completion) => self.handle_io_completion(completion),
            DesktopEvent::Preview(completion) => self.handle_preview_completion(completion),
            DesktopEvent::Host(signal) => self.handle_host_signal(signal),
        }
        self.finish_callback(|| event_loop.exit());
    }
}

fn completes_move_resize(active: bool, state: ElementState, button: MouseButton) -> bool {
    active && state == ElementState::Released && button == MouseButton::Left
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_winit_left_release_completes_only_an_active_native_move_resize() {
        assert!(completes_move_resize(
            true,
            ElementState::Released,
            MouseButton::Left
        ));
        assert!(!completes_move_resize(
            false,
            ElementState::Released,
            MouseButton::Left
        ));
        assert!(!completes_move_resize(
            true,
            ElementState::Pressed,
            MouseButton::Left
        ));
        assert!(!completes_move_resize(
            true,
            ElementState::Released,
            MouseButton::Right
        ));
    }
}
