//! Winit lifecycle and event-loop scheduling for the native shell.
//!
//! plan_ref: docs/plan/03_system_architecture.md#interaction-shell

use std::sync::Arc;
use std::time::{Duration, Instant};

use stickymd_render::source::SourceProjection;
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow};
use winit::window::{WindowAttributes, WindowId};

use super::{AppEvent, CARET_BLINK, StickyApp};
use crate::config::ViewMode;
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
        let attributes = WindowAttributes::default()
            .with_title("StickyMD")
            .with_inner_size(LogicalSize::new(
                self.config.window.width_dip as f64,
                self.config.window.height_dip as f64,
            ))
            .with_min_inner_size(LogicalSize::new(360.0, 240.0));
        let window = match event_loop.create_window(attributes) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("window creation failed: {error}");
                event_loop.exit();
                return;
            }
        };
        window.set_ime_allowed(self.config.view_mode != ViewMode::Preview);
        let size = window.inner_size();
        let surface = match SoftwareSurface::new(Arc::clone(&window)) {
            Ok(surface) => surface,
            Err(error) => {
                eprintln!("surface creation failed: {error}");
                event_loop.exit();
                return;
            }
        };
        let initial_snapshot = self.coordinator.snapshot();
        let projection = SourceProjection::new(
            &initial_snapshot,
            size.width,
            size.height,
            window.scale_factor() as f32,
        );
        let fonts = projection.fonts();
        eprintln!(
            "font selection: CJK={} found={} Latin={} found={}",
            fonts.cjk_family, fonts.cjk_found, fonts.latin_family, fonts.latin_found
        );
        self.window = Some(window);
        self.surface = Some(surface);
        self.projection = Some(projection);
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
                self.dispatch_persistence_intent(Some(event_loop), PersistenceIntent::RequestQuit);
            }
            WindowEvent::Resized(size) => self.resize(size),
            WindowEvent::ScaleFactorChanged { .. } => {
                if let Some(window) = &self.window {
                    self.resize(window.inner_size());
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
                    window.set_ime_allowed(focused && !self.preview_focused);
                }
                self.after_presentation_change();
            }
            WindowEvent::ModifiersChanged(modifiers) => self.modifiers = modifiers.state(),
            WindowEvent::KeyboardInput { event, .. } => self.handle_key(event),
            WindowEvent::Ime(event) => self.handle_ime(event),
            WindowEvent::CursorMoved { position, .. } => self.handle_cursor_moved(position),
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse_button(state, button);
            }
            WindowEvent::MouseWheel { delta, .. } => self.handle_scroll(delta),
            WindowEvent::RedrawRequested => self.render(),
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

        let now = Instant::now();
        let mut next_wake = None;
        for deadline in [
            self.persistence.autosave_deadline(),
            self.persistence.external_deadline(),
            self.preview_deadline(),
        ]
        .into_iter()
        .flatten()
        {
            let wake = self.started + Duration::from_millis(deadline);
            next_wake = Some(next_wake.map_or(wake, |current: Instant| current.min(wake)));
        }
        if self.session.focused && !self.preview_focused && !self.session.is_composing() {
            if now >= self.next_blink {
                self.session.caret_visible = !self.session.caret_visible;
                self.next_blink = now + CARET_BLINK;
                self.request_redraw();
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
                if let Some(window) = &self.window {
                    window.set_visible(true);
                    window.set_minimized(false);
                    window.focus_window();
                    window.request_redraw();
                }
            }
            AppEvent::Preview(completion) => self.handle_preview_completion(completion),
        }
    }
}
