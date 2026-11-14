//! A host that asks for it (macOS) gets the frame storage released while the window is hidden and
//! rebuilt before the window shows again; a host that does not (Windows) keeps what it had.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use stickymd_app::config::ViewMode;
use stickymd_app::flow::window::state::{ShowReason, WindowIntent};
use stickymd_app::ports::{FailureKind, Visibility};
use stickymd_render::source::SourceProjection;
use winit::dpi::PhysicalSize;

use crate::shell::headless_tests::{HeadlessApp, fixture};
use crate::testing::HostCall;

const VIEW: PhysicalSize<u32> = PhysicalSize::new(900, 600);

/// A shown Source view with its pane frame, on a host that releases hidden frames or not.
fn shown(releases: bool) -> HeadlessApp {
    shown_in(ViewMode::Source, releases)
}

fn shown_in(mode: ViewMode, releases: bool) -> HeadlessApp {
    let mut app = fixture("start\n");
    app.app
        .config
        .update(|config| config.view_mode = mode)
        .unwrap();
    app.host.releases_hidden_frames = releases;
    app.headless_view = Some((VIEW, 1.0));
    let snapshot = app.app.coordinator.snapshot();
    app.projection = Some(SourceProjection::new(
        &snapshot,
        VIEW.width,
        VIEW.height,
        1.0,
    ));
    app.configure_viewports();
    if mode != ViewMode::Preview {
        assert!(app.source_frame.is_some());
    }
    app
}

fn preview_jobs(app: &HeadlessApp) -> u64 {
    app.app
        .preview_worker
        .as_ref()
        .map_or(0, |worker| worker.metrics().unwrap().submitted)
}

fn hide(app: &mut HeadlessApp) {
    let guards = app.window_guards();
    app.dispatch_window_intent(WindowIntent::CloseRequested { now_ms: 1, guards });
}

fn show(app: &mut HeadlessApp) {
    app.dispatch_window_intent(WindowIntent::ShowRequested {
        reason: ShowReason::Tray,
        now_ms: 2,
    });
}

/// The calls that order the release against the window: hide, show and the release notices.
fn release_calls(app: &HeadlessApp) -> Vec<HostCall> {
    app.host
        .calls
        .borrow()
        .iter()
        .filter(|call| {
            matches!(
                call,
                HostCall::Hide | HostCall::Show | HostCall::HiddenFramesReleased(_)
            )
        })
        .cloned()
        .collect()
}

#[test]
fn a_releasing_host_gives_up_frames_after_the_hide_and_gets_them_back_before_the_show() {
    let mut app = shown(true);
    hide(&mut app);
    assert!(app.frames_released);
    assert!(app.source_frame.is_none());
    assert_eq!(
        release_calls(&app),
        [HostCall::Hide, HostCall::HiddenFramesReleased(true)]
    );

    // Nothing window-sized comes back while hidden: not for a new layout, not for a resize.
    app.configure_viewports();
    app.headless_view = Some((PhysicalSize::new(800, 500), 1.0));
    app.resize(PhysicalSize::new(800, 500));
    assert!(app.source_frame.is_none());
    assert!(app.frames_released);

    show(&mut app);
    assert!(!app.frames_released);
    assert!(app.source_frame.is_some(), "the pane frame is rebuilt");
    assert_eq!(
        release_calls(&app),
        [
            HostCall::Hide,
            HostCall::HiddenFramesReleased(true),
            HostCall::HiddenFramesReleased(false),
            HostCall::Show,
        ]
    );
}

#[test]
fn a_keeping_host_releases_nothing_more_than_before() {
    let mut app = shown(false);
    hide(&mut app);
    assert!(!app.frames_released);
    assert!(app.source_frame.is_none(), "the hidden caches go as before");
    // As before, a layout while hidden rebuilds the pane frame.
    app.configure_viewports();
    assert!(app.source_frame.is_some());
    show(&mut app);
    assert_eq!(release_calls(&app), [HostCall::Hide, HostCall::Show]);
}

#[test]
fn a_rejected_hide_releases_no_frames() {
    let mut app = shown(true);
    app.host.hide_result = Err(FailureKind::Failed);
    hide(&mut app);
    assert!(!app.frames_released);
    assert!(app.source_frame.is_some());
    assert_eq!(release_calls(&app), [HostCall::Hide]);
}

/// A hidden Split or Preview asks the worker for nothing, whatever changes; the show builds once.
#[test]
fn no_preview_job_runs_while_the_frames_are_released() {
    for mode in [ViewMode::Split, ViewMode::Preview] {
        let mut app = shown_in(mode, true);
        hide(&mut app);
        assert!(app.frames_released);
        let before = preview_jobs(&app);
        app.headless_view = Some((PhysicalSize::new(800, 500), 1.0));
        app.resize(PhysicalSize::new(800, 500));
        app.request_preview_relayout();
        app.settle_event_pass(u64::MAX / 2);
        assert_eq!(preview_jobs(&app), before, "{mode:?}: no job while hidden");
        show(&mut app);
        assert_eq!(preview_jobs(&app), before + 1, "{mode:?}: the show builds");
    }
}

/// A show that fails leaves the window hidden, so what the show rebuilt is released again.
#[test]
fn a_rejected_show_releases_the_frames_again() {
    let mut app = shown(true);
    hide(&mut app);
    app.host.show_answers.push_back(Ok(Visibility {
        visible: Some(false),
        minimized: Some(false),
    }));
    show(&mut app);
    assert!(app.frames_released);
    assert!(app.source_frame.is_none());
    assert_eq!(
        release_calls(&app),
        [
            HostCall::Hide,
            HostCall::HiddenFramesReleased(true),
            HostCall::HiddenFramesReleased(false),
            HostCall::Show,
            HostCall::HiddenFramesReleased(true),
        ]
    );
    // The next show works as the first.
    show(&mut app);
    assert!(!app.frames_released);
    assert!(app.source_frame.is_some());
}

/// A failed focus is reported as a rejected show too, but the window stays shown: its frames stay,
/// whether the window was already shown or the show itself worked.
#[test]
fn a_failed_focus_keeps_the_frames_of_a_shown_window() {
    let mut shown_window = shown(true);
    shown_window
        .host
        .focus_answers
        .push_back(Err(FailureKind::Failed));
    show(&mut shown_window);
    assert!(!shown_window.frames_released);
    assert!(shown_window.source_frame.is_some());

    let mut was_hidden = shown(true);
    hide(&mut was_hidden);
    was_hidden
        .host
        .focus_answers
        .push_back(Err(FailureKind::Failed));
    show(&mut was_hidden);
    assert!(!was_hidden.frames_released);
    assert!(was_hidden.source_frame.is_some());
    assert_eq!(
        release_calls(&was_hidden),
        [
            HostCall::Hide,
            HostCall::HiddenFramesReleased(true),
            HostCall::HiddenFramesReleased(false),
            HostCall::Show,
        ]
    );
}
