//! Windowless fixtures for the real shell's state application paths.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::sync::OnceLock;

use stickymd_core::{DocumentState, LineEnding};
use winit::event_loop::{EventLoop, EventLoopProxy};
use winit::platform::windows::EventLoopBuilderExtWindows;

use super::{AppEvent, StickyApp};
use crate::persistence::PersistenceWorker;
use crate::platform::windows::program_dir::RuntimePaths;
use crate::platform::windows::single_instance::{InstanceDisposition, SingleInstanceGuard};
use crate::startup::{BootstrapOutcome, StartupDiagnostics};

/// The constructor needs an event sink, not a running window or input desktop.
/// Winit permits one event loop per process. Create its proxy once on an owned
/// thread, then destroy the loop there. No application window is ever created;
/// callbacks cannot run or activate anything through this closed proxy.
fn closed_proxy() -> EventLoopProxy<AppEvent> {
    static PROXY: OnceLock<EventLoopProxy<AppEvent>> = OnceLock::new();
    PROXY
        .get_or_init(|| {
            std::thread::spawn(|| {
                let event_loop = EventLoop::<AppEvent>::with_user_event()
                    .with_any_thread(true)
                    .build()
                    .expect("windowless event sink");
                event_loop.create_proxy()
            })
            .join()
            .expect("event sink thread")
        })
        .clone()
}

pub(super) struct HeadlessApp {
    app: Option<StickyApp>,
    root: PathBuf,
}

impl Deref for HeadlessApp {
    type Target = StickyApp;
    fn deref(&self) -> &Self::Target {
        self.app.as_ref().unwrap()
    }
}

impl DerefMut for HeadlessApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.app.as_mut().unwrap()
    }
}

impl Drop for HeadlessApp {
    fn drop(&mut self) {
        // Drop the worker, watcher and instance handles before removing only
        // this exclusively created fixture. Nothing under the checkout is used.
        drop(self.app.take());
        let result = std::fs::remove_dir_all(&self.root);
        if !std::thread::panicking() {
            result.expect("remove owned headless fixture");
        }
    }
}

pub(super) fn fixture(text: &str) -> HeadlessApp {
    let root = crate::test_support::unique_temp_path("headless-shell");
    std::fs::create_dir(&root).unwrap();
    let mut owned = HeadlessApp { app: None, root };
    let paths = RuntimePaths::from_executable(&owned.root.join("StickyMD.exe")).unwrap();
    std::fs::create_dir(&paths.note_dir).unwrap();
    let InstanceDisposition::Primary(instance) =
        SingleInstanceGuard::acquire(&paths.program_dir).unwrap()
    else {
        panic!("unique fixture must own its instance");
    };
    let worker = PersistenceWorker::start(|_| {}).unwrap();
    let mut app = StickyApp::new(
        paths,
        BootstrapOutcome {
            document: DocumentState::loaded(text, LineEnding::Lf, None),
            config: crate::config::RuntimeConfig::default(),
            config_persistence_allowed: false,
            recovery: None,
            recovery_canonical_to_preserve: None,
            warnings: Vec::new(),
        },
        instance,
        worker,
        closed_proxy(),
        StartupDiagnostics::disabled_for_test(),
    );
    // The real constructor's directory watcher is unnecessary here: completions
    // are supplied explicitly, and no test is allowed to use native input.
    app.watcher.take();
    use crate::flow::window::geometry::{
        MonitorGeometry, MonitorIdentity, PhysicalRect, WindowPlacement,
    };
    use crate::flow::window::reducer::WindowShellCoordinator;
    use crate::flow::window::state::StableVisibility;
    app.window_flow = Some(WindowShellCoordinator::new(
        WindowPlacement::new(900.0, 600.0, None, 0.0, 0.5, 0.5),
        StableVisibility::Floating,
        MonitorGeometry::new(
            MonitorIdentity::new("headless"),
            PhysicalRect::new(0, 0, 1920, 1080),
            1.0,
            true,
        ),
        true,
    ));
    assert!(app.window_accepts_editor_mutation());
    assert!(app.window.is_none());
    assert!(app.tray.is_none());
    owned.app = Some(app);
    owned
}
