//! Winit lifecycle and event-loop scheduling for the native shell.
//!
//! plan_ref: docs/plan/03_system_architecture.md#interaction-shell

use std::sync::Arc;
use std::time::{Duration, Instant};

use stickymd_render::source::{SourceInitializationMilestone, SourceProjection};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::platform::windows::{CornerPreference, WindowAttributesExtWindows};
use winit::window::{Theme, WindowAttributes, WindowId};

use super::{AppEvent, CARET_BLINK, StickyApp};
use crate::config::{MIN_WINDOW_HEIGHT_DIP, MIN_WINDOW_WIDTH_DIP, ViewMode};
use crate::instruction::{PersistenceIntent, SaveReason};
use crate::platform::windows::file_watch::{FileWatchSignal, NoteDirectoryWatcher};
use crate::surface::SoftwareSurface;

impl StickyApp {
    pub(super) fn start_watcher(&mut self) {
        if self.watcher.is_some() || self.recovery.is_pending() {
            return;
        }
        let proxy = self.proxy.clone();
        match NoteDirectoryWatcher::start(&self.paths.note_dir, move |signal| {
            let event = match signal {
                FileWatchSignal::NoteHint => AppEvent::NoteFsHint,
                FileWatchSignal::Failed(error) => AppEvent::WatchFailed(error),
            };
            let _ = proxy.send_event(event);
        }) {
            Ok(watcher) => self.watcher = Some(watcher),
            Err(error) => {
                self.diagnostic = Some(format!(
                    "外部文件自动检测不可用；安全保存仍受指纹校验保护：{error}"
                ));
            }
        }
    }

    pub(super) fn timestamp_ms(&self) -> u64 {
        self.started.elapsed().as_millis().min(u64::MAX as u128) as u64
    }
}

impl ApplicationHandler<AppEvent> for StickyApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_blink));
        if self.window.is_some() {
            return;
        }
        let configured_theme = match self.config.current().theme {
            crate::config::ThemeMode::Light => Some(Theme::Light),
            crate::config::ThemeMode::Dark => Some(Theme::Dark),
            crate::config::ThemeMode::System => None,
        };
        let attributes = WindowAttributes::default()
            .with_title("StickyMD")
            .with_theme(configured_theme)
            .with_decorations(false)
            .with_resizable(true)
            .with_visible(false)
            .with_undecorated_shadow(true)
            .with_corner_preference(CornerPreference::RoundSmall)
            .with_inner_size(LogicalSize::new(
                self.config.current().window.width_dip as f64,
                self.config.current().window.height_dip as f64,
            ))
            .with_min_inner_size(LogicalSize::new(
                f64::from(MIN_WINDOW_WIDTH_DIP),
                f64::from(MIN_WINDOW_HEIGHT_DIP),
            ));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("window creation failed: {error}");
                event_loop.exit();
                return;
            }
        };
        self.startup_diagnostics.record("window_created");
        window.set_ime_allowed(self.config.current().view_mode != ViewMode::Preview);
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
        self.startup_diagnostics.record("surface_ready");
        self.startup_diagnostics.record("display_ready");
        let initial_snapshot = self.coordinator.snapshot();
        self.startup_diagnostics.record("font_system_begin");
        let document_scale =
            window.scale_factor() as f32 * self.config.current().content_zoom_percent.factor();
        let initial_geometry = super::preview_runtime::geometry(
            self.config.current().view_mode,
            size,
            window.scale_factor() as f32,
        );
        let (initial_width, initial_height) = initial_geometry
            .source
            .or(initial_geometry.preview)
            .map_or((size.width.max(1), size.height.max(1)), |pane| {
                (pane.width.max(1), pane.height.max(1))
            });
        self.startup_diagnostics.record("source_layout_begin");
        let diagnostics = &mut self.startup_diagnostics;
        let projection = SourceProjection::new_observed(
            &initial_snapshot,
            initial_width,
            initial_height,
            document_scale,
            |milestone| match milestone {
                SourceInitializationMilestone::FontSystemReady => {
                    diagnostics.record("font_system_end")
                }
                SourceInitializationMilestone::SourceBufferReady => {
                    diagnostics.record("source_buffer_ready")
                }
                SourceInitializationMilestone::SourceShaped => {
                    diagnostics.record("source_layout_end");
                    diagnostics.record("source_projection_ready")
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
        self.projection = Some(projection);
        if !self.initialize_window_shell(event_loop) {
            event_loop.exit();
            return;
        }
        self.configure_viewports();
        let generation = self.coordinator.view().generation;
        if let Some(action) = self
            .preview_flow
            .show(generation, self.preview_visibility())
        {
            self.submit_preview_action(action);
        }
        self.update_window_title();
        self.request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                self.dispatch_window_intent(
                    Some(event_loop),
                    crate::flow::window::state::WindowIntent::CloseRequested {
                        now_ms: self.timestamp_ms(),
                        guards: self.window_guards(),
                    },
                );
            }
            WindowEvent::Resized(size) => self.resize(size),
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = &self.window {
                    self.resize(window.inner_size());
                }
                self.recover_display_topology();
            }
            WindowEvent::ThemeChanged(theme) => {
                self.system_theme = theme;
                if self.config.current().theme == crate::config::ThemeMode::System {
                    self.request_preview_relayout();
                    self.request_redraw();
                }
            }
            WindowEvent::Focused(focused) => {
                self.session.focused = focused;
                if !focused {
                    self.session.cancel_preedit();
                    if self.coordinator.view().dirty {
                        self.dispatch_persistence_intent(
                            None,
                            PersistenceIntent::SaveNow(SaveReason::FocusLoss),
                        );
                    }
                }
                if let Some(window) = &self.window {
                    window.set_ime_allowed(
                        focused && !self.preview_focused && self.shell_input_enabled,
                    );
                }
                self.refresh_window_guards(Some(event_loop));
                self.after_presentation_change();
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => self.handle_key(event),
            WindowEvent::Ime(event) => {
                self.handle_ime(event);
                self.refresh_window_guards(Some(event_loop));
            }
            WindowEvent::CursorMoved { position, .. } => self.handle_cursor_moved(position),
            WindowEvent::CursorEntered { .. } => {
                self.pointer_inside_window = true;
                self.dispatch_window_intent(
                    Some(event_loop),
                    crate::flow::window::state::WindowIntent::SensorEntered {
                        now_ms: self.timestamp_ms(),
                    },
                );
                self.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                self.pointer_inside_window = false;
                self.dispatch_window_intent(
                    Some(event_loop),
                    crate::flow::window::state::WindowIntent::PointerLeft {
                        now_ms: self.timestamp_ms(),
                    },
                );
                self.request_redraw();
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_button(state, button);
            }
            WindowEvent::Moved(_) => {}
            WindowEvent::MouseWheel { delta, .. } => self.handle_scroll(delta),
            WindowEvent::RedrawRequested => self.render(event_loop),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now_ms = self.timestamp_ms();
        if let Some(action) = self.persistence.tick_autosave(now_ms) {
            self.submit_save(action.trigger, None);
        }
        if self.persistence.take_external_check(now_ms) {
            self.worker.inspect_external(self.paths.note_file.clone());
        }
        self.tick_preview(now_ms);
        if self
            .zoom_config_deadline
            .is_some_and(|deadline| now_ms >= deadline)
        {
            self.zoom_config_deadline = None;
            self.submit_config_if_needed();
        }
        self.dispatch_window_intent(
            Some(event_loop),
            crate::flow::window::state::WindowIntent::Tick { now_ms },
        );

        let now = Instant::now();
        let mut next_wake = None;
        for deadline in [
            self.persistence.autosave_deadline(),
            self.persistence.external_deadline(),
            self.preview_deadline(),
            self.window_next_deadline(),
            self.zoom_config_deadline,
        ]
        .into_iter()
        .flatten()
        {
            let wake = self.started + Duration::from_millis(deadline);
            next_wake = Some(next_wake.map_or(wake, |current: Instant| current.min(wake)));
        }
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
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: AppEvent) {
        match event {
            AppEvent::Io(completion) => self.handle_io_completion(event_loop, completion),
            AppEvent::NoteFsHint => {
                if let Some(watcher) = &self.watcher {
                    watcher.acknowledge_hint();
                }
                self.persistence.on_watch_hint(self.timestamp_ms());
            }
            AppEvent::WatchFailed(error) => {
                self.watcher = None;
                self.diagnostic = Some(format!(
                    "外部文件自动检测已停止；安全保存仍受指纹校验保护：{error}"
                ));
                self.request_redraw();
            }
            AppEvent::ShowRequested => {
                self.dispatch_window_intent(
                    Some(event_loop),
                    crate::flow::window::state::WindowIntent::ShowRequested {
                        reason: crate::flow::window::state::ShowReason::SecondInstance,
                        now_ms: self.timestamp_ms(),
                    },
                );
            }
            AppEvent::Tray(event) => self.handle_tray_event(event_loop, event),
            AppEvent::Native(signal) => match signal {
                crate::platform::windows::native_message::NativeWindowSignal::MoveSizeStarted => {
                    self.move_resize_active = true;
                    self.refresh_window_guards(Some(event_loop));
                }
                crate::platform::windows::native_message::NativeWindowSignal::MoveSizeFinished => {
                    self.complete_window_drag();
                }
                crate::platform::windows::native_message::NativeWindowSignal::DisplayTopologyChanged => {
                    self.recover_display_topology();
                }
            },
            AppEvent::Preview(completion) => self.handle_preview_completion(completion),
        }
    }
}
