//! What the shared shell needs from a Platform Host, and how a host's events reach the shell.
//!
//! plan_ref: docs/plan/03_system_architecture.md#shared-crates

use std::sync::Arc;
use std::time::Duration;

use stickymd_app::execution::IoCompletion;
use stickymd_app::flow::window::geometry::PhysicalRect;
use stickymd_app::ports::{
    ExportTargetPicker, FailureKind, FatalNotice, HostFailure, MonitorSource, PreviewCompletion,
    PreviewImages, ShellLauncher, TrayCommand, TrayView, WindowHost,
};
use winit::window::{Window, WindowAttributes};

use crate::frame::FrameLedger;
use crate::input::Keymap;

/// Where other threads post what the event loop must see. A host builds it around its event-loop
/// proxy, a test around a plain closure. A post after the loop ended is lost, like the event.
#[derive(Clone)]
pub struct EventSink(Arc<dyn Fn(DesktopEvent) + Send + Sync>);

impl EventSink {
    pub fn new(post: impl Fn(DesktopEvent) + Send + Sync + 'static) -> Self {
        Self(Arc::new(post))
    }

    pub fn post(&self, event: DesktopEvent) {
        (self.0)(event);
    }
}

/// The event loop's user event.
#[derive(Debug)]
pub enum DesktopEvent {
    Io(IoCompletion),
    Preview(PreviewCompletion),
    Host(HostSignal),
}

/// A fact from the host, neutral of how the host learned it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HostSignal {
    Tray(TrayCommand),
    /// A second start of the program asks this one to come forward.
    ShowRequested,
    DisplayTopologyChanged,
    NoteDirectoryChanged,
    WatchFailed(String),
    /// See `Host::keeps_pointer_after_leave`.
    PointerLeft,
    /// A requested quit has run longer than the host can wait for it.
    QuitDeadlinePassed,
}

/// The tray. It takes `EventSink`, so it lives here and not with the ports in `stickymd-app`.
pub trait TrayHost {
    /// A host without a tray keeps the default.
    fn create(&mut self, _view: TrayView, _sink: EventSink) -> Result<(), HostFailure> {
        Err(HostFailure::new(
            FailureKind::Unsupported,
            "this desktop has no tray",
        ))
    }

    fn update(&mut self, view: TrayView);

    /// Must run on the main thread, before the event loop ends.
    fn destroy(&mut self);
}

/// How long the caret stays shown, and then hidden, in each blink.
pub const CARET_BLINK: Duration = Duration::from_millis(550);

/// A native way to blink the caret without a new frame. Without one (`Host::caret_overlay`) the
/// shell draws the caret into the frame.
pub trait CaretOverlay {
    /// Inverts `rect` of the window's client area; the same call again puts the pixels back.
    fn toggle(&mut self, rect: PhysicalRect) -> Result<(), HostFailure>;

    /// A present replaced the pixels, so no inverted rectangle is left on the screen.
    fn release(&mut self) {}

    /// Whether an inverted rectangle blinks by itself, `CARET_BLINK` shown and `CARET_BLINK`
    /// hidden, starting shown each time it is inverted. The shell then neither wakes nor toggles
    /// for each blink (plan 10: macOS plays it as a system compositor animation). By default the
    /// shell toggles it.
    fn blinks_itself(&self) -> bool {
        false
    }
}

/// The substeps of the startup a host measures separately, in the order they happen.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartupDetail {
    SplitModeBegin,
    SplitModeEnd,
    WindowShowBegin,
    WindowShowEnd,
    ToolIdentityBegin,
    ToolIdentityEnd,
}

/// Where the startup has got to, for a host that measures it; the names are the stable trace.
pub trait StartupProbe {
    fn milestone(&mut self, _name: &'static str) {}
    fn detail(&mut self, _detail: StartupDetail) {}
}

/// A full frame reached the window. It only borrows: `after_present` runs on every frame.
pub struct PresentFacts<'a> {
    pub width: u32,
    pub height: u32,
    /// Every pane of the view shows a real frame, not a placeholder.
    pub view_complete: bool,
    pub ledger: &'a FrameLedger,
}

/// Everything the shell uses of a host, as one bound: it adds no business of its own. The host is
/// fixed per target at compile time, so this is a generic bound and never `dyn`.
pub trait Host:
    FatalNotice
    + ShellLauncher
    + ExportTargetPicker
    + TrayHost
    + MonitorSource
    + WindowHost
    + StartupProbe
    + PreviewImages
    + 'static
{
    /// Adds what only this desktop's windows have to the attributes the shell chose.
    fn window_attributes(&self, base: WindowAttributes) -> WindowAttributes;

    /// The window exists and nothing of it is shown yet: the host keeps what its ports need.
    fn window_created(&mut self, window: &Arc<Window>);

    fn keymap(&self) -> &Keymap;

    /// Starts watching the note directory, whose changes come back as `HostSignal`s. A watch that
    /// already runs stays.
    fn start_watch(&mut self, sink: &EventSink) -> Result<(), HostFailure>;

    fn stop_watch(&mut self);

    /// The shell took in a `NoteDirectoryChanged`: the host may send the next one, so a storm of
    /// file events keeps at most one in the event queue.
    fn acknowledge_watch(&mut self);

    fn caret_overlay(&mut self) -> Option<&mut dyn CaretOverlay> {
        None
    }

    /// The pointer left the window but still counts as over it, in an area of the desktop only
    /// the host can see. The host posts `HostSignal::PointerLeft` when it really has left.
    fn keeps_pointer_after_leave(&mut self) -> bool {
        false
    }

    /// A requested quit will not happen: it was blocked, or its barrier failed. A host that answered
    /// the system's termination request with "later" has to take that answer back.
    fn quit_abandoned(&mut self) {}

    /// For a host's verification probe; `true` ends the event loop.
    fn after_present(&mut self, _facts: &PresentFacts<'_>) -> bool {
        false
    }

    /// Whether the shell gives up its frame storage while the window is hidden and rebuilds it
    /// before the window shows again (`09` macOS mapping). A host that keeps it, as Windows does,
    /// leaves this `false`.
    fn releases_hidden_frames(&self) -> bool {
        false
    }

    /// The shell has just released (`true`) or is about to rebuild (`false`) its frame storage.
    fn hidden_frames_released(&mut self, _released: bool) {}
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn every_clone_of_a_sink_posts_to_the_same_place() {
        let posted = Arc::new(Mutex::new(Vec::new()));
        let log = Arc::clone(&posted);
        let sink = EventSink::new(move |event| {
            if let DesktopEvent::Host(signal) = event {
                log.lock().unwrap().push(signal);
            }
        });
        sink.clone()
            .post(DesktopEvent::Host(HostSignal::ShowRequested));
        sink.post(DesktopEvent::Host(HostSignal::Tray(TrayCommand::Quit)));
        assert_eq!(
            *posted.lock().unwrap(),
            [
                HostSignal::ShowRequested,
                HostSignal::Tray(TrayCommand::Quit)
            ]
        );
    }

    struct NoTray;

    impl TrayHost for NoTray {
        fn update(&mut self, _view: TrayView) {}
        fn destroy(&mut self) {}
    }

    #[test]
    fn a_host_without_a_tray_says_so() {
        let view = TrayView {
            window_visible: true,
            always_on_top: false,
        };
        let result = NoTray.create(view, EventSink::new(|_| {}));
        assert_eq!(result.unwrap_err().kind(), FailureKind::Unsupported);
    }
}
