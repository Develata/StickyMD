//! The macOS Platform Host: the capabilities the shell asks for, over the native adapters.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use std::path::PathBuf;
use std::sync::Arc;

use stickymd_app::flow::window::geometry::PhysicalRect;
use stickymd_app::ports::{
    ExportTargetPicker, FailureKind, FatalNotice, HostFailure, PreviewImages, ShellLauncher,
    TrayView,
};
use stickymd_desktop::host::{
    CaretOverlay, DesktopEvent, EventSink, Host, HostSignal, StartupProbe, TrayHost,
};
use stickymd_desktop::input::Keymap;
use stickymd_render::image::PreviewImageSource;
use stickymd_render::preview::LinkKind;
use winit::window::{Window, WindowAttributes};

use crate::paths::RuntimePaths;
use crate::platform::macos::app::{install_hooks, termination_abandoned, termination_finished};
use crate::platform::macos::caret_layer::CaretLayer;
use crate::platform::macos::dialog::{choose_markdown_export, show_error};
use crate::platform::macos::file::DirectoryHandle;
use crate::platform::macos::file_watch::{FileWatchSignal, NoteDirectoryWatcher};
use crate::platform::macos::instance::InstanceGuard;
use crate::platform::macos::pointer_band::PointerBandWatch;
use crate::platform::macos::shell::open_target;
use crate::platform::macos::tray::{TrayController, tray_icon};
use crate::platform::macos::window::{PointerProbe, join_all_desktops};
use crate::preview::LocalImageSource;

mod monitor_source;
mod window;

/// This host's keys (plan 07, macOS input).
const KEYMAP: Keymap = Keymap::cmd_standard();

pub struct MacHost {
    paths: RuntimePaths,
    /// The Note Directory as the note's storage holds it, for the reads that must follow it.
    note_directory: DirectoryHandle,
    sink: EventSink,
    watcher: Option<NoteDirectoryWatcher>,
    _instance: InstanceGuard,
    tray: Option<TrayController>,
    tray_view: TrayView,
    window: Option<Arc<Window>>,
    caret: CaretLayer,
    pointer_band: PointerBandWatch,
    /// What the user chose, apart from the level a window over the menu bar has to take.
    wants_topmost: bool,
}

impl MacHost {
    pub fn new(
        paths: RuntimePaths,
        note_directory: DirectoryHandle,
        instance: InstanceGuard,
        sink: EventSink,
    ) -> Self {
        Self {
            paths,
            note_directory,
            sink,
            watcher: None,
            _instance: instance,
            tray: None,
            tray_view: TrayView {
                window_visible: true,
                always_on_top: false,
            },
            window: None,
            caret: CaretLayer::default(),
            pointer_band: PointerBandWatch::default(),
            wants_topmost: false,
        }
    }
}

fn failed(error: impl ToString) -> HostFailure {
    HostFailure::new(FailureKind::Failed, error.to_string())
}

impl FatalNotice for MacHost {
    fn show_error(&self, title: &str, body: &str) {
        show_error(title, body);
    }
}

impl ShellLauncher for MacHost {
    fn open_target(&mut self, target: &str, kind: LinkKind) -> Result<(), HostFailure> {
        open_target(target, kind, &self.paths.note_dir).map_err(failed)
    }
}

impl ExportTargetPicker for MacHost {
    fn choose_markdown_export(&mut self) -> Result<Option<PathBuf>, HostFailure> {
        Ok(choose_markdown_export())
    }
}

impl TrayHost for MacHost {
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

    /// Runs when the shell has finished a quit: the moment a pending system request can be told
    /// that the process may end.
    fn destroy(&mut self) {
        self.tray = None;
        termination_finished();
    }
}

impl CaretOverlay for MacHost {
    fn toggle(&mut self, rect: PhysicalRect) -> Result<(), HostFailure> {
        let window = self
            .window
            .as_ref()
            .ok_or_else(|| HostFailure::new(FailureKind::Unavailable, "window is unavailable"))?;
        if self.caret.toggle(window, rect) {
            Ok(())
        } else {
            Err(failed("the caret layer could not be created"))
        }
    }

    fn release(&mut self) {
        self.caret.release();
    }

    /// The layer carries a compositor animation (`caret_layer::blink_animation`).
    fn blinks_itself(&self) -> bool {
        true
    }
}

impl PreviewImages for MacHost {
    fn preview_images(&self) -> Option<Box<dyn PreviewImageSource + Send>> {
        let directory = self.note_directory.try_clone().ok()?;
        Some(Box::new(LocalImageSource::following(directory)))
    }
}

impl StartupProbe for MacHost {}

impl Host for MacHost {
    fn window_attributes(&self, base: WindowAttributes) -> WindowAttributes {
        base
    }

    fn window_created(&mut self, window: &Arc<Window>) {
        join_all_desktops(window);
        self.window = Some(Arc::clone(window));
        install_hooks(self.sink.clone());
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

    fn quit_abandoned(&mut self) {
        termination_abandoned();
    }

    /// A top-docked window expands below the menu bar, away from the edge the pointer reached.
    /// The pointer resting up there has not left yet.
    fn keeps_pointer_after_leave(&mut self) -> bool {
        let Some(probe) = self.window.as_ref().and_then(PointerProbe::new) else {
            return false;
        };
        if !probe.in_band_above() {
            self.pointer_band.stop();
            return false;
        }
        self.pointer_band.start(probe, self.sink.clone());
        true
    }
}
