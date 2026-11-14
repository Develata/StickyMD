//! A host for tests of the shell: it records every call in order and answers what the test set.
//! It proves the shell's side of a port, never that a native implementation behaves. Beside it, a
//! clipboard and a disk (`PortableDiskPorts`) that need no platform.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use std::any::Any;
use std::cell::RefCell;
use std::collections::VecDeque;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use stickymd_app::flow::window::geometry::PhysicalRect;
use stickymd_app::flow::{ClipboardError, ClipboardPort};
use stickymd_app::ports::{
    ExportTargetPicker, FailureKind, FatalNotice, FrameFact, HostFailure, MonitorSnapshot,
    MonitorSource, PreviewImages, ShellLauncher, TopmostFact, TrayView, Visibility, WindowHost,
};
use stickymd_render::image::PreviewImageSource;
use stickymd_render::preview::LinkKind;
use winit::window::{Window, WindowAttributes};

use crate::host::{CaretOverlay, EventSink, Host, PresentFacts, StartupProbe, TrayHost};
use crate::input::Keymap;

pub mod contracts;
mod portable_disk;

pub use portable_disk::{PortableDiskPorts, unique_temp_dir};

pub use crate::storage_conformance;

/// One call that reached the host.
#[derive(Clone, Debug, PartialEq)]
pub enum HostCall {
    Show,
    Hide,
    Focus,
    SetTopmost(bool),
    SetOpacity(u8),
    ApplyFrame(PhysicalRect),
    ObserveFrame,
    ReassertIdentity,
    RestoreOrdinaryEntry,
    TrayCreated(TrayView),
    TrayUpdated(TrayView),
    TrayDestroyed,
    OpenTarget(String, LinkKind),
    ChooseExport,
    ShowError(String),
    StartWatch,
    StopWatch,
    AcknowledgeWatch,
    CaretToggled(PhysicalRect),
    CaretReleased,
    Presented {
        width: u32,
        height: u32,
    },
    HiddenFramesReleased(bool),
    PresentedFrame {
        width: u32,
        height: u32,
    },
    Milestone(&'static str),
    /// Recorded by a test through `calls`: a step that does not go through the host.
    WindowRevealed,
    /// Recorded by a test through `calls`.
    WindowFocused,
    /// Recorded by a test through `calls`.
    EventLoopExited,
}

/// The answers are public so a test sets what the desktop reports. The defaults are a desktop
/// where every request works and the window was never placed: no display and no frame to read.
pub struct FakeHost {
    /// Shared so that a test can place in the same order what does not go through the host.
    pub calls: Rc<RefCell<Vec<HostCall>>>,
    /// Whether a watch runs, after `start_watch` and `stop_watch`.
    pub watching: bool,
    pub visibility: Result<Visibility, FailureKind>,
    /// What the next `show` calls answer, one per call, before `visibility` takes over.
    pub show_answers: VecDeque<Result<Visibility, FailureKind>>,
    /// What the next `focus` calls answer, one per call, before `visibility` takes over.
    pub focus_answers: VecDeque<Result<Visibility, FailureKind>>,
    pub hide_result: Result<(), FailureKind>,
    /// What the next `set_topmost` calls answer, one per call, before `topmost_result`.
    pub topmost_answers: VecDeque<Result<TopmostFact, HostFailure>>,
    /// A fixed answer for `set_topmost`; `None` is a window whose bit takes what it was asked.
    pub topmost_result: Option<Result<TopmostFact, HostFailure>>,
    pub opacity_result: Result<(), HostFailure>,
    pub frame_result: Result<(), FailureKind>,
    /// A fixed answer for `observe_frame`; `None` is a window that is where it was last sent, and
    /// one never sent anywhere has no frame to read.
    pub observed_frame: Option<Result<FrameFact, FailureKind>>,
    /// What the next `observe_frame` calls answer, one per call, before `observed_frame`.
    pub observe_answers: RefCell<VecDeque<Result<FrameFact, FailureKind>>>,
    /// Every observation must come from `observe_answers`; anything else is a test failure, for
    /// a test that wants no answer by default.
    pub strict_observations: bool,
    /// Where `apply_frame` last sent the window.
    pub applied_frame: Option<PhysicalRect>,
    pub monitors: Result<MonitorSnapshot, FailureKind>,
    pub identity_result: Result<(), HostFailure>,
    /// What the next `reassert_identity` calls answer, one per call, before `identity_result`.
    pub identity_answers: VecDeque<Result<(), HostFailure>>,
    pub tray_result: Result<(), HostFailure>,
    pub open_result: Result<(), HostFailure>,
    pub export_result: Result<Option<PathBuf>, HostFailure>,
    pub watch_result: Result<(), HostFailure>,
    /// Whether this host has a caret overlay at all.
    pub caret_overlay: bool,
    /// Whether that overlay blinks by itself (`CaretOverlay::blinks_itself`).
    pub caret_blinks: bool,
    pub caret_result: Result<(), HostFailure>,
    pub exit_after_present: bool,
    /// Whether this host asks the shell to release frames while hidden (macOS does).
    pub releases_hidden_frames: bool,
    /// Whether this host presents frames itself (macOS does).
    pub presents_frames: bool,
    /// What the next `present_frame` calls answer, one per call, before `Presented`.
    pub present_answers: VecDeque<crate::host::PresentOutcome>,
    pub keymap: Keymap,
    /// Goes when the host goes: a test's probe of the drop order.
    pub dropped_with_host: Option<Box<dyn Any>>,
}

impl Default for FakeHost {
    fn default() -> Self {
        Self {
            calls: Rc::default(),
            watching: false,
            visibility: Ok(Visibility {
                visible: Some(true),
                minimized: Some(false),
            }),
            show_answers: VecDeque::new(),
            focus_answers: VecDeque::new(),
            hide_result: Ok(()),
            topmost_answers: VecDeque::new(),
            topmost_result: None,
            opacity_result: Ok(()),
            frame_result: Ok(()),
            observed_frame: None,
            observe_answers: RefCell::default(),
            strict_observations: false,
            applied_frame: None,
            monitors: Err(FailureKind::Unavailable),
            identity_result: Ok(()),
            identity_answers: VecDeque::new(),
            tray_result: Ok(()),
            open_result: Ok(()),
            export_result: Ok(None),
            watch_result: Ok(()),
            caret_overlay: true,
            caret_blinks: false,
            caret_result: Ok(()),
            exit_after_present: false,
            releases_hidden_frames: false,
            presents_frames: false,
            present_answers: VecDeque::new(),
            keymap: Keymap::ctrl_standard(),
            dropped_with_host: None,
        }
    }
}

impl FakeHost {
    fn record(&self, call: HostCall) {
        self.calls.borrow_mut().push(call);
    }

    /// The calls so far, which starts the record anew.
    pub fn take_calls(&mut self) -> Vec<HostCall> {
        self.calls.take()
    }
}

/// A clipboard that only remembers the last text written to it.
#[derive(Debug, Default)]
pub struct MemoryClipboard {
    pub text: Option<String>,
}

impl ClipboardPort for MemoryClipboard {
    fn read_text(&mut self) -> Result<Option<String>, ClipboardError> {
        Ok(self.text.clone())
    }

    fn write_text(&mut self, text: &str) -> Result<(), ClipboardError> {
        self.text = Some(text.to_owned());
        Ok(())
    }
}

impl FatalNotice for FakeHost {
    fn show_error(&self, _title: &str, body: &str) {
        self.record(HostCall::ShowError(body.to_owned()));
    }
}

impl ShellLauncher for FakeHost {
    fn open_target(&mut self, target: &str, kind: LinkKind) -> Result<(), HostFailure> {
        self.record(HostCall::OpenTarget(target.to_owned(), kind));
        self.open_result.clone()
    }
}

impl ExportTargetPicker for FakeHost {
    fn choose_markdown_export(&mut self) -> Result<Option<PathBuf>, HostFailure> {
        self.record(HostCall::ChooseExport);
        self.export_result.clone()
    }
}

impl TrayHost for FakeHost {
    fn create(&mut self, view: TrayView, _sink: EventSink) -> Result<(), HostFailure> {
        self.record(HostCall::TrayCreated(view));
        self.tray_result.clone()
    }

    fn update(&mut self, view: TrayView) {
        self.record(HostCall::TrayUpdated(view));
    }

    fn destroy(&mut self) {
        self.record(HostCall::TrayDestroyed);
    }
}

impl MonitorSource for FakeHost {
    fn snapshot(&self) -> Result<MonitorSnapshot, FailureKind> {
        self.monitors.clone()
    }
}

impl WindowHost for FakeHost {
    fn show(&mut self) -> Result<Visibility, FailureKind> {
        self.record(HostCall::Show);
        self.show_answers.pop_front().unwrap_or(self.visibility)
    }

    fn hide(&mut self) -> Result<(), FailureKind> {
        self.record(HostCall::Hide);
        self.hide_result
    }

    fn focus(&mut self) -> Result<Visibility, FailureKind> {
        self.record(HostCall::Focus);
        self.focus_answers.pop_front().unwrap_or(self.visibility)
    }

    fn visibility(&self) -> Result<Visibility, FailureKind> {
        self.visibility
    }

    fn set_topmost(&mut self, on: bool) -> Result<TopmostFact, HostFailure> {
        self.record(HostCall::SetTopmost(on));
        self.topmost_answers.pop_front().unwrap_or_else(|| {
            self.topmost_result
                .clone()
                .unwrap_or(Ok(TopmostFact { topmost: Some(on) }))
        })
    }

    fn set_opacity(&mut self, percent: u8) -> Result<(), HostFailure> {
        self.record(HostCall::SetOpacity(percent));
        self.opacity_result.clone()
    }

    fn apply_frame(&mut self, target: PhysicalRect) -> Result<(), FailureKind> {
        self.record(HostCall::ApplyFrame(target));
        self.applied_frame = Some(target);
        self.frame_result
    }

    fn observe_frame(&self) -> Result<FrameFact, FailureKind> {
        self.record(HostCall::ObserveFrame);
        if let Some(answer) = self.observe_answers.borrow_mut().pop_front() {
            return answer;
        }
        assert!(
            !self.strict_observations,
            "an observation this test did not plan"
        );
        self.observed_frame.clone().unwrap_or_else(|| {
            self.applied_frame
                .map(|outer| FrameFact { outer })
                .ok_or(FailureKind::Unavailable)
        })
    }

    fn reassert_identity(&mut self) -> Result<(), HostFailure> {
        self.record(HostCall::ReassertIdentity);
        self.identity_answers
            .pop_front()
            .unwrap_or_else(|| self.identity_result.clone())
    }

    fn restore_ordinary_entry(&mut self) {
        self.record(HostCall::RestoreOrdinaryEntry);
    }
}

impl CaretOverlay for FakeHost {
    fn toggle(&mut self, rect: PhysicalRect) -> Result<(), HostFailure> {
        self.record(HostCall::CaretToggled(rect));
        self.caret_result.clone()
    }

    fn release(&mut self) {
        self.record(HostCall::CaretReleased);
    }

    fn blinks_itself(&self) -> bool {
        self.caret_blinks
    }
}

impl StartupProbe for FakeHost {
    fn milestone(&mut self, name: &'static str) {
        self.record(HostCall::Milestone(name));
    }
}

impl PreviewImages for FakeHost {
    fn preview_images(&self) -> Option<Box<dyn PreviewImageSource + Send>> {
        None
    }
}

impl Host for FakeHost {
    fn window_attributes(&self, base: WindowAttributes) -> WindowAttributes {
        base
    }

    fn window_created(&mut self, _window: &Arc<Window>) {}

    fn keymap(&self) -> &Keymap {
        &self.keymap
    }

    fn start_watch(&mut self, _sink: &EventSink) -> Result<(), HostFailure> {
        self.record(HostCall::StartWatch);
        self.watching = self.watch_result.is_ok() || self.watching;
        self.watch_result.clone()
    }

    fn stop_watch(&mut self) {
        self.record(HostCall::StopWatch);
        self.watching = false;
    }

    fn acknowledge_watch(&mut self) {
        self.record(HostCall::AcknowledgeWatch);
    }

    fn caret_overlay(&mut self) -> Option<&mut dyn CaretOverlay> {
        self.caret_overlay.then_some(self as &mut dyn CaretOverlay)
    }

    fn after_present(&mut self, facts: &PresentFacts<'_>) -> bool {
        self.record(HostCall::Presented {
            width: facts.width,
            height: facts.height,
        });
        self.exit_after_present
    }

    fn releases_hidden_frames(&self) -> bool {
        self.releases_hidden_frames
    }

    fn hidden_frames_released(&mut self, released: bool) {
        self.record(HostCall::HiddenFramesReleased(released));
    }

    fn presents_frames(&self) -> bool {
        self.presents_frames
    }

    fn present_frame(&mut self, frame: &tiny_skia::Pixmap) -> crate::host::PresentOutcome {
        self.record(HostCall::PresentedFrame {
            width: frame.width(),
            height: frame.height(),
        });
        self.present_answers
            .pop_front()
            .unwrap_or(crate::host::PresentOutcome::Presented)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::FrameLedger;

    fn failure(detail: &str) -> HostFailure {
        HostFailure::new(FailureKind::Failed, detail)
    }

    #[test]
    fn every_call_is_recorded_in_order_and_taking_them_starts_anew() {
        let mut host = FakeHost::default();
        let frame = PhysicalRect::new(1, 2, 3, 4);
        host.apply_frame(frame).unwrap();
        host.show().unwrap();
        host.reassert_identity().unwrap();
        host.show_error("StickyMD", "body");
        assert_eq!(
            host.take_calls(),
            [
                HostCall::ApplyFrame(frame),
                HostCall::Show,
                HostCall::ReassertIdentity,
                HostCall::ShowError("body".into()),
            ]
        );
        assert!(host.take_calls().is_empty());
    }

    #[test]
    fn a_read_back_is_what_the_test_set() {
        let mut host = FakeHost {
            hide_result: Err(FailureKind::Unsupported),
            visibility: Err(FailureKind::Unavailable),
            opacity_result: Err(failure("no alpha")),
            ..FakeHost::default()
        };
        assert_eq!(host.hide(), Err(FailureKind::Unsupported));
        assert_eq!(host.focus(), Err(FailureKind::Unavailable));
        assert_eq!(host.set_opacity(50).unwrap_err().to_string(), "no alpha");
    }

    #[test]
    fn set_topmost_echoes_unless_answers_or_a_result_are_set() {
        let mut host = FakeHost::default();
        assert_eq!(host.set_topmost(true).unwrap().topmost, Some(true));
        host.topmost_answers
            .push_back(Ok(TopmostFact { topmost: None }));
        host.topmost_answers.push_back(Err(failure("refused")));
        assert_eq!(host.set_topmost(true).unwrap().topmost, None);
        assert_eq!(host.set_topmost(true).unwrap_err().to_string(), "refused");
        host.topmost_result = Some(Ok(TopmostFact {
            topmost: Some(false),
        }));
        assert_eq!(host.set_topmost(true).unwrap().topmost, Some(false));
    }

    #[test]
    fn show_and_focus_answers_change_per_call_then_fall_back() {
        let hidden = Visibility {
            visible: Some(false),
            minimized: Some(false),
        };
        let mut host = FakeHost::default();
        host.show_answers.push_back(Ok(hidden));
        host.show_answers.push_back(Err(FailureKind::Failed));
        host.focus_answers.push_back(Err(FailureKind::Unsupported));
        assert_eq!(host.show(), Ok(hidden));
        assert_eq!(host.show(), Err(FailureKind::Failed));
        assert_eq!(host.show(), host.visibility);
        assert_eq!(host.focus(), Err(FailureKind::Unsupported));
        assert_eq!(host.focus(), host.visibility);
    }

    #[test]
    fn by_default_nothing_says_where_the_window_is() {
        let mut host = FakeHost::default();
        assert_eq!(host.snapshot(), Err(FailureKind::Unavailable));
        assert_eq!(host.observe_frame(), Err(FailureKind::Unavailable));
        assert_eq!(host.take_calls(), [HostCall::ObserveFrame]);
    }

    #[test]
    #[should_panic(expected = "an observation this test did not plan")]
    fn a_strict_host_fails_an_unplanned_observation() {
        let host = FakeHost {
            strict_observations: true,
            ..FakeHost::default()
        };
        let _ = host.observe_frame();
    }

    #[test]
    fn observe_frame_echoes_the_applied_frame_unless_told_otherwise() {
        let frame = PhysicalRect::new(1, 2, 3, 4);
        let mut host = FakeHost::default();
        host.apply_frame(frame).unwrap();
        assert_eq!(host.observe_frame().unwrap().outer, frame);
        host.observe_answers
            .borrow_mut()
            .push_back(Err(FailureKind::Unsupported));
        assert_eq!(host.observe_frame(), Err(FailureKind::Unsupported));
        host.observed_frame = Some(Err(FailureKind::Failed));
        assert_eq!(host.observe_frame(), Err(FailureKind::Failed));
    }

    #[test]
    fn a_failed_watch_start_leaves_no_watch_and_a_running_one_stays() {
        let sink = EventSink::new(|_| {});
        let mut host = FakeHost {
            watch_result: Err(failure("no watcher")),
            ..FakeHost::default()
        };
        assert!(host.start_watch(&sink).is_err());
        assert!(!host.watching);
        host.watch_result = Ok(());
        host.start_watch(&sink).unwrap();
        assert!(host.watching);
        host.watch_result = Err(failure("no second watcher"));
        assert!(host.start_watch(&sink).is_err());
        assert!(host.watching);
        host.stop_watch();
        assert!(!host.watching);
    }

    #[test]
    fn a_host_may_have_no_caret_overlay() {
        let mut host = FakeHost::default();
        let rect = PhysicalRect::new(0, 0, 1, 1);
        host.caret_overlay().unwrap().toggle(rect).unwrap();
        assert_eq!(host.take_calls(), [HostCall::CaretToggled(rect)]);
        host.caret_overlay = false;
        assert!(host.caret_overlay().is_none());
    }

    #[test]
    fn a_present_tells_the_host_and_the_host_may_end_the_loop() {
        let ledger = FrameLedger::default();
        let facts = PresentFacts {
            width: 900,
            height: 600,
            view_complete: true,
            ledger: &ledger,
            draw: std::time::Duration::ZERO,
        };
        let mut host = FakeHost::default();
        assert!(!host.after_present(&facts));
        host.exit_after_present = true;
        assert!(host.after_present(&facts));
        let presented = HostCall::Presented {
            width: 900,
            height: 600,
        };
        assert_eq!(host.take_calls(), vec![presented; 2]);
    }
}
