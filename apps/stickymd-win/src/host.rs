//! The Windows Platform Host: the capabilities the shell asks for, over the native adapters.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping

use std::path::PathBuf;
use std::sync::Arc;

use stickymd_app::flow::window::geometry::PhysicalRect;
use stickymd_app::ports::{
    ExportTargetPicker, FailureKind, FatalNotice, HostFailure, PreviewImages, ShellLauncher,
    TrayView,
};
use stickymd_desktop::host::{
    CaretOverlay, DesktopEvent, EventSink, Host, HostSignal, PresentFacts, StartupDetail,
    StartupProbe, TrayHost,
};
use stickymd_desktop::input::Keymap;
use stickymd_render::image::PreviewImageSource;
use stickymd_render::preview::LinkKind;
use winit::dpi::PhysicalSize;
use winit::platform::windows::{CornerPreference, WindowAttributesExtWindows};
use winit::window::{Window, WindowAttributes};

use crate::platform::windows::caret_overlay::{CaretOverlayRect, toggle_caret_overlay};
use crate::platform::windows::export_dialog::choose_markdown_export;
use crate::platform::windows::file_watch::{FileWatchSignal, NoteDirectoryWatcher};
use crate::platform::windows::message_box;
use crate::platform::windows::program_dir::RuntimePaths;
use crate::platform::windows::shell::open_target;
use crate::platform::windows::single_instance::SingleInstanceGuard;
use crate::platform::windows::tray::{TrayController, tray_icon};
use crate::preview::LocalImageSource;
use crate::startup::StartupDiagnostics;

mod monitor_source;
mod window;

/// This host's keys: the shared Ctrl table (plan 03, rule 9).
const KEYMAP: Keymap = Keymap::ctrl_standard();

/// Declared after `app` in the shell, so the worker stops before any of this drops and in
/// particular before the single-instance guard goes. The tray is usually gone by then: the exit
/// destroys it before the event loop ends.
pub struct WinHost {
    paths: RuntimePaths,
    watcher: Option<NoteDirectoryWatcher>,
    _instance: SingleInstanceGuard,
    tray: Option<TrayController>,
    /// What the tray shows, so `update` touches only the item that changed.
    tray_view: TrayView,
    window: Option<Arc<Window>>,
    startup_diagnostics: StartupDiagnostics,
}

impl WinHost {
    pub fn new(
        paths: RuntimePaths,
        instance: SingleInstanceGuard,
        startup_diagnostics: StartupDiagnostics,
    ) -> Self {
        Self {
            paths,
            watcher: None,
            _instance: instance,
            tray: None,
            tray_view: TrayView {
                window_visible: true,
                always_on_top: false,
            },
            window: None,
            startup_diagnostics,
        }
    }

    pub fn startup_diagnostic_exit_code(&self) -> std::process::ExitCode {
        self.startup_diagnostics.exit_code()
    }
}

fn failed(error: impl ToString) -> HostFailure {
    HostFailure::new(FailureKind::Failed, error.to_string())
}

impl FatalNotice for WinHost {
    fn show_error(&self, title: &str, body: &str) {
        message_box::show_error(title, body);
    }
}

impl ShellLauncher for WinHost {
    fn open_target(&mut self, target: &str, kind: LinkKind) -> Result<(), HostFailure> {
        open_target(target, kind, &self.paths.note_dir).map_err(failed)
    }
}

impl ExportTargetPicker for WinHost {
    fn choose_markdown_export(&mut self) -> Result<Option<PathBuf>, HostFailure> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| HostFailure::new(FailureKind::Unavailable, "window is unavailable"))?;
        choose_markdown_export(window.as_ref()).map_err(failed)
    }
}

impl TrayHost for WinHost {
    fn create(&mut self, view: TrayView, sink: EventSink) -> Result<(), HostFailure> {
        let tray = TrayController::create(
            tray_icon(),
            view.window_visible,
            view.always_on_top,
            move |command| sink.post(DesktopEvent::Host(HostSignal::Tray(command))),
        )
        .map_err(failed)?;
        self.tray = Some(tray);
        self.tray_view = view;
        Ok(())
    }

    fn update(&mut self, view: TrayView) {
        let Some(tray) = &self.tray else { return };
        if view.window_visible != self.tray_view.window_visible {
            tray.set_window_visible(view.window_visible);
        }
        if view.always_on_top != self.tray_view.always_on_top {
            tray.set_always_on_top(view.always_on_top);
        }
        self.tray_view = view;
    }

    fn destroy(&mut self) {
        self.tray = None;
    }
}

impl CaretOverlay for WinHost {
    fn toggle(&mut self, rect: PhysicalRect) -> Result<(), HostFailure> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| HostFailure::new(FailureKind::Unavailable, "window is unavailable"))?;
        let rect = CaretOverlayRect {
            x: rect.x,
            y: rect.y,
            width: i32::try_from(rect.width)
                .map_err(|_| failed("caret width exceeds Win32 range"))?,
            height: i32::try_from(rect.height)
                .map_err(|_| failed("caret height exceeds Win32 range"))?,
        };
        toggle_caret_overlay(window.as_ref(), rect).map_err(failed)
    }
}

impl PreviewImages for WinHost {
    fn preview_images(&self) -> Option<Box<dyn PreviewImageSource + Send>> {
        Some(Box::new(LocalImageSource::new(self.paths.note_dir.clone())))
    }
}

impl StartupProbe for WinHost {
    fn milestone(&mut self, name: &'static str) {
        self.startup_diagnostics.record(name);
    }

    fn detail(&mut self, detail: StartupDetail) {
        self.startup_diagnostics.record_detail(detail);
    }
}

impl Host for WinHost {
    fn window_attributes(&self, base: WindowAttributes) -> WindowAttributes {
        base.with_undecorated_shadow(true)
            .with_corner_preference(CornerPreference::RoundSmall)
    }

    fn window_created(&mut self, window: &Arc<Window>) {
        self.window = Some(Arc::clone(window));
    }

    fn keymap(&self) -> &Keymap {
        &KEYMAP
    }

    fn start_watch(&mut self, sink: &EventSink) -> Result<(), HostFailure> {
        if self.watcher.is_some() {
            return Ok(());
        }
        let sink = sink.clone();
        let watcher = NoteDirectoryWatcher::start(&self.paths.note_dir, move |signal| {
            sink.post(DesktopEvent::Host(match signal {
                FileWatchSignal::NoteHint => HostSignal::NoteDirectoryChanged,
                FileWatchSignal::Failed(error) => HostSignal::WatchFailed(error),
            }));
        })
        .map_err(failed)?;
        self.watcher = Some(watcher);
        Ok(())
    }

    fn stop_watch(&mut self) {
        self.watcher = None;
    }

    fn acknowledge_watch(&mut self) {
        if let Some(watcher) = &self.watcher {
            watcher.acknowledge_hint();
        }
    }

    fn caret_overlay(&mut self) -> Option<&mut dyn CaretOverlay> {
        Some(self)
    }

    fn after_present(&mut self, facts: &PresentFacts<'_>) -> bool {
        if let Err(error) = self.startup_diagnostics.editor_ready() {
            eprintln!("startup diagnostics failed: {error}");
        }
        let measured = !self.startup_diagnostics.measures_frames() || facts.view_complete;
        let exit = self.startup_diagnostics.exit_requested() && measured;
        if exit {
            let size = PhysicalSize::new(facts.width, facts.height);
            self.startup_diagnostics.record_frames(size, facts.ledger);
        }
        exit
    }
}
