//! Windowless fixtures for the real shell's state application paths.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::ops::{Deref, DerefMut};
#[cfg(test)]
use std::path::Path;
use std::path::PathBuf;
#[cfg(test)]
use std::sync::Arc;
#[cfg(test)]
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::Instant;

use crate::host::EventSink;
#[cfg(test)]
use crate::testing::PortableDiskPorts;
use crate::testing::{FakeHost, MemoryClipboard, unique_temp_dir};
use stickymd_app::flow::window::geometry::{
    MonitorGeometry, MonitorIdentity, PhysicalRect, WindowPlacement,
};
use stickymd_app::flow::window::reducer::WindowShellCoordinator;
use stickymd_app::flow::window::state::StableVisibility;
use stickymd_app::session::{AppSession, InstantClock, MonotonicClock, SessionParts};
use stickymd_core::{
    DocumentState, ExternalFileFact, LineEnding, LoadedDocument, PreserveExpectation,
    RecoveryCandidate,
};
use winit::event::ElementState;
use winit::keyboard::{KeyCode, PhysicalKey};

use super::DesktopShell;
use super::input::ShellKey;
#[cfg(test)]
use super::storage_conformance::{Completion, StorageSite};
use super::storage_conformance::{StorageFactory, StoragePaths};
use stickymd_app::execution::IoCompletion;
#[cfg(test)]
use stickymd_app::execution::PersistenceWorker;

/// The shell these tests drive.
pub(super) type TestApp = DesktopShell<FakeHost>;

pub(super) struct HeadlessApp {
    shell: Option<TestApp>,
    root: PathBuf,
    /// Where the fixture's storage keeps the note.
    pub(super) paths: StoragePaths,
    /// Whether the constructor started a directory watcher.
    #[cfg(test)]
    pub(super) started_watcher: bool,
}

impl Deref for HeadlessApp {
    type Target = TestApp;
    fn deref(&self) -> &Self::Target {
        self.shell.as_ref().unwrap()
    }
}

impl DerefMut for HeadlessApp {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.shell.as_mut().unwrap()
    }
}

impl Drop for HeadlessApp {
    fn drop(&mut self) {
        // Stop the worker before removing only this exclusively created
        // fixture. Nothing under the checkout is used.
        drop(self.shell.take());
        let result = std::fs::remove_dir_all(&self.root);
        if !std::thread::panicking() {
            result.expect("remove owned headless fixture");
        }
    }
}

/// The disk every fixture uses unless a test names another storage.
#[cfg(test)]
pub(super) fn portable_storage(root: &Path, on_completion: Completion) -> StorageSite {
    let disk = PortableDiskPorts::new(root.join("note"));
    std::fs::create_dir(disk.note_dir()).unwrap();
    let paths = StoragePaths {
        note_file: disk.note_file(),
        note_tmp: disk.note_tmp(),
        note_dir: disk.note_dir().to_path_buf(),
    };
    let worker = PersistenceWorker::start(disk.io_ports(), on_completion).unwrap();
    StorageSite { worker, paths }
}

/// A coordinator on one fixed monitor, without a native window.
pub(super) fn window_flow(visibility: StableVisibility) -> WindowShellCoordinator {
    WindowShellCoordinator::new(
        WindowPlacement::new(900.0, 600.0, None, 0.0, 0.5, 0.5),
        visibility,
        MonitorGeometry::new(
            MonitorIdentity::new("headless"),
            PhysicalRect::new(0, 0, 1920, 1080),
            1.0,
            true,
        ),
        true,
    )
}

#[cfg(test)]
pub(super) fn fixture(text: &str) -> HeadlessApp {
    started(text, None, |_| {})
}

/// A shell whose startup found a recovery candidate.
#[cfg(test)]
pub(super) fn recovering_at_start() -> HeadlessApp {
    let candidate = RecoveryCandidate {
        canonical: None,
        temporary: LoadedDocument::from_durable_bytes(b"temporary").unwrap(),
        temporary_is_newer: true,
    };
    started("disk", Some(candidate), |_| {})
}

/// A fixture whose worker hands every receipt to the test instead of dropping it, so a test can
/// see what was really requested and deliver the receipt itself.
#[cfg(test)]
pub(super) fn receipt_fixture(text: &str) -> (HeadlessApp, mpsc::Receiver<IoCompletion>) {
    receipt_fixture_on(portable_storage, text)
}

/// `receipt_fixture` on the storage `storage` builds.
pub(super) fn receipt_fixture_on(
    storage: StorageFactory,
    text: &str,
) -> (HeadlessApp, mpsc::Receiver<IoCompletion>) {
    let (sender, receiver) = mpsc::channel();
    let app = started_with(
        storage,
        FakeHost::default(),
        text,
        None,
        move |completion| {
            let _ = sender.send(completion);
        },
    );
    (app, receiver)
}

#[cfg(test)]
fn started(
    text: &str,
    recovery: Option<RecoveryCandidate>,
    on_completion: impl Fn(IoCompletion) + Send + Sync + 'static,
) -> HeadlessApp {
    started_on(FakeHost::default(), text, recovery, on_completion)
}

/// The stock fixture, on a host with this desktop's answers.
#[cfg(test)]
pub(super) fn fixture_on(host: FakeHost, text: &str) -> HeadlessApp {
    started_on(host, text, None, |_| {})
}

#[cfg(test)]
pub(super) fn started_on(
    host: FakeHost,
    text: &str,
    recovery: Option<RecoveryCandidate>,
    on_completion: impl Fn(IoCompletion) + Send + Sync + 'static,
) -> HeadlessApp {
    started_with(portable_storage, host, text, recovery, on_completion)
}

/// A shell as startup leaves it, without a window, on `storage` in a directory of its own: the
/// configuration is the default and is never written, and the clipboard is in memory.
pub(super) fn started_with(
    storage: StorageFactory,
    host: FakeHost,
    text: &str,
    recovery: Option<RecoveryCandidate>,
    on_completion: impl Fn(IoCompletion) + Send + Sync + 'static,
) -> HeadlessApp {
    started_on_clock(storage, host, text, recovery, on_completion, None)
}

/// A clock a test moves by hand.
#[cfg(test)]
#[derive(Clone, Default)]
pub(super) struct TestClock(Arc<AtomicU64>);

#[cfg(test)]
impl TestClock {
    pub(super) fn set(&self, now_ms: u64) {
        self.0.store(now_ms, Ordering::SeqCst);
    }
}

#[cfg(test)]
impl MonotonicClock for TestClock {
    fn now_ms(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
}

/// The stock fixture, on a clock the test moves.
#[cfg(test)]
pub(super) fn clocked_fixture() -> (HeadlessApp, TestClock) {
    let clock = TestClock::default();
    let app = started_on_clock(
        portable_storage,
        FakeHost::default(),
        "start",
        None,
        |_| {},
        Some(Box::new(clock.clone())),
    );
    (app, clock)
}

fn started_on_clock(
    storage: StorageFactory,
    host: FakeHost,
    text: &str,
    recovery: Option<RecoveryCandidate>,
    on_completion: impl Fn(IoCompletion) + Send + Sync + 'static,
    clock: Option<Box<dyn MonotonicClock>>,
) -> HeadlessApp {
    let root = unique_temp_dir("headless-shell");
    let site = storage(&root, Box::new(on_completion));
    let mut owned = HeadlessApp {
        shell: None,
        root,
        paths: site.paths,
        #[cfg(test)]
        started_watcher: false,
    };
    let started_at = Instant::now();
    let session = AppSession::new(SessionParts {
        document: DocumentState::loaded(text, LineEnding::Lf, None),
        clipboard: Box::new(MemoryClipboard::default()),
        config: stickymd_app::config::RuntimeConfig::default(),
        config_persistence_allowed: false,
        recovery,
        recovery_canonical_to_preserve: None,
        worker: site.worker,
        clock: clock.unwrap_or_else(|| Box::new(InstantClock::starting_at(started_at))),
    });
    // There is no event loop: what a worker posts here is dropped.
    let sink = EventSink::new(|_| {});
    let mut app = DesktopShell::new(host, session, sink, started_at, None);
    #[cfg(test)]
    {
        app.headless_window_gesture = Some(true);
        owned.started_watcher = app.host.watching;
    }
    // The constructor's watch is not wanted here: completions are supplied explicitly.
    app.host.watching = false;
    app.host.take_calls();
    app.app.window_flow = Some(window_flow(StableVisibility::Floating));
    assert!(app.window_accepts_editor_mutation());
    assert!(app.window.is_none());
    owned.shell = Some(app);
    owned
}

pub(super) fn loaded(text: &str) -> LoadedDocument {
    LoadedDocument::from_durable_bytes(text.as_bytes()).unwrap()
}

pub(super) fn external(text: &str) -> ExternalFileFact {
    loaded(text).into_external_fact()
}

/// Offers a recovery choice between `canonical` and a temp holding "temporary".
pub(super) fn offer_recovery(
    app: &mut HeadlessApp,
    canonical: Option<&str>,
    preserve: Option<PreserveExpectation>,
) {
    app.app.recovery = stickymd_app::flow::RecoveryCoordinator::new(
        Some(RecoveryCandidate {
            canonical: canonical.map(loaded),
            temporary: loaded("temporary"),
            temporary_is_newer: true,
        }),
        preserve,
    );
    app.app.persistence.set_recovery_pending(true);
}

pub(super) fn saves(app: &HeadlessApp) -> u64 {
    app.app.worker.metrics().note_submitted
}

pub(super) fn type_letter(app: &mut HeadlessApp) {
    app.apply_key(
        ShellKey {
            physical_key: PhysicalKey::Code(KeyCode::KeyA),
            state: ElementState::Pressed,
            repeat: false,
            ime_owned: false,
            text: Some("a"),
        },
        false,
    );
}
