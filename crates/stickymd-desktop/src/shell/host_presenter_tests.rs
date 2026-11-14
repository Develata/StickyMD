//! A host that presents frames itself (macOS, plan 06 frame presentation): the shell hands it the
//! finished canvas, counts only `Presented` as a frame that reached the window, draws a deferred
//! frame again through one bounded wake, and never presents while the frames are released.
//!
//! plan_ref: docs/plan/06_markdown_math_rendering.md#frame-presentation

use std::time::{Duration, Instant};

use stickymd_app::flow::window::state::{ShowReason, WindowIntent};
use stickymd_render::source::SourceProjection;
use winit::dpi::PhysicalSize;

use super::presentation::PendingRedraw;
use crate::frame::SoftwareSurface;
use crate::host::{PRESENT_RETRY, PresentOutcome};
use crate::shell::headless_tests::{HeadlessApp, fixture};
use crate::testing::HostCall;

const VIEW: PhysicalSize<u32> = PhysicalSize::new(900, 600);

/// A shown Source view whose host presents frames, with a canvas and a pane frame.
fn presenting() -> HeadlessApp {
    let mut app = fixture("start\n");
    app.host.presents_frames = true;
    app.host.releases_hidden_frames = true;
    app.headless_view = Some((VIEW, 1.0));
    app.surface = Some(SoftwareSurface::for_host(VIEW.width, VIEW.height).unwrap());
    let snapshot = app.app.coordinator.snapshot();
    app.projection = Some(SourceProjection::new(
        &snapshot,
        VIEW.width,
        VIEW.height,
        1.0,
    ));
    app.configure_viewports();
    app.host.calls.borrow_mut().clear();
    app
}

fn frames(app: &HeadlessApp) -> Vec<HostCall> {
    app.host
        .calls
        .borrow()
        .iter()
        .filter(|call| {
            matches!(
                call,
                HostCall::PresentedFrame { .. } | HostCall::Presented { .. }
            )
        })
        .cloned()
        .collect()
}

const FULL: HostCall = HostCall::PresentedFrame {
    width: VIEW.width,
    height: VIEW.height,
};
const REACHED: HostCall = HostCall::Presented {
    width: VIEW.width,
    height: VIEW.height,
};

#[test]
fn the_host_gets_the_whole_canvas_and_a_presented_frame_counts() {
    let mut app = presenting();
    app.pending_redraw = PendingRedraw::Full;
    assert!(app.draw_frame(None));
    assert_eq!(frames(&app), [FULL, REACHED]);
    assert_eq!(app.present_retry_at, None);
}

#[test]
fn a_deferred_frame_is_drawn_again_through_one_bounded_wake() {
    let mut app = presenting();
    app.host.present_answers.push_back(PresentOutcome::Deferred);
    let before = Instant::now();
    app.pending_redraw = PendingRedraw::Full;
    assert!(
        !app.draw_frame(None),
        "a deferred frame did not reach the window"
    );
    assert_eq!(frames(&app), [FULL], "no after_present for it");
    assert_eq!(app.pending_redraw, PendingRedraw::Full);
    let retry = app.present_retry_at.expect("one retry");
    assert!(retry >= before + PRESENT_RETRY);

    // Before it is due the retry is the next wake, and nothing is redrawn yet.
    app.pending_redraw = PendingRedraw::None;
    let wake = app.advance_and_next_wake(before).expect("a wake");
    assert!(wake <= retry);
    assert_eq!(app.pending_redraw, PendingRedraw::None);
    // Once due it asks for a full redraw and leaves no wake of its own.
    app.advance_and_next_wake(retry + Duration::from_millis(1));
    assert_eq!(app.pending_redraw, PendingRedraw::Full);
    assert_eq!(app.present_retry_at, None);
    // The next frame is presented and counts.
    assert!(app.draw_frame(None));
    assert_eq!(frames(&app), [FULL, FULL, REACHED]);
}

#[test]
fn a_failed_frame_is_reported_and_not_retried() {
    let mut app = presenting();
    app.host
        .present_answers
        .push_back(PresentOutcome::Failed("no surface".into()));
    app.pending_redraw = PendingRedraw::Full;
    assert!(!app.draw_frame(None));
    assert_eq!(frames(&app), [FULL]);
    assert_eq!(app.diagnostic.as_deref(), Some("no surface"));
    assert_eq!(app.present_retry_at, None);
}

/// Defers one frame, then answers the next draw with `then` before the retry is due.
fn deferred_then(then: PresentOutcome) -> (HeadlessApp, Instant) {
    let mut app = presenting();
    app.host.present_answers.push_back(PresentOutcome::Deferred);
    app.host.present_answers.push_back(then);
    app.pending_redraw = PendingRedraw::Full;
    assert!(!app.draw_frame(None));
    let retry = app.present_retry_at.expect("one retry");
    // Another event draws before the retry is due.
    app.pending_redraw = PendingRedraw::Full;
    (app, retry)
}

/// What a retry that is still pending would do once due: ask for a redraw.
fn retry_fires(app: &mut HeadlessApp, retry: Instant) -> bool {
    app.pending_redraw = PendingRedraw::None;
    app.advance_and_next_wake(retry + Duration::from_millis(1));
    app.pending_redraw != PendingRedraw::None
}

#[test]
fn a_failure_after_a_deferral_ends_the_wait() {
    let (mut app, retry) = deferred_then(PresentOutcome::Failed("lost".into()));
    assert!(!app.draw_frame(None));
    assert_eq!(app.diagnostic.as_deref(), Some("lost"));
    assert_eq!(app.present_retry_at, None, "a failed frame is not retried");
    assert!(!retry_fires(&mut app, retry));
}

#[test]
fn a_frame_presented_before_the_retry_is_due_ends_the_wait() {
    let (mut app, retry) = deferred_then(PresentOutcome::Presented);
    assert!(app.draw_frame(None));
    assert_eq!(app.present_retry_at, None);
    assert!(!retry_fires(&mut app, retry));
}

#[test]
fn a_resize_while_deferred_presents_the_new_size_once() {
    let mut app = presenting();
    app.host.present_answers.push_back(PresentOutcome::Deferred);
    app.pending_redraw = PendingRedraw::Full;
    assert!(!app.draw_frame(None));
    let wider = PhysicalSize::new(1000, 600);
    app.headless_view = Some((wider, 1.0));
    app.resize(wider);
    assert!(app.draw_frame(None));
    assert_eq!(
        frames(&app),
        [
            FULL,
            HostCall::PresentedFrame {
                width: 1000,
                height: 600
            },
            HostCall::Presented {
                width: 1000,
                height: 600
            }
        ]
    );
    assert_eq!(app.present_retry_at, None);
}

#[test]
fn only_a_presented_frame_touches_the_caret_overlay() {
    let caret_calls = |app: &HeadlessApp| {
        app.host
            .calls
            .borrow()
            .iter()
            .filter(|call| matches!(call, HostCall::CaretReleased | HostCall::CaretToggled(_)))
            .count()
    };
    let mut app = presenting();
    app.host.present_answers.push_back(PresentOutcome::Deferred);
    app.host
        .present_answers
        .push_back(PresentOutcome::Failed("lost".into()));
    for _ in 0..2 {
        app.pending_redraw = PendingRedraw::Full;
        assert!(!app.draw_frame(None));
    }
    assert_eq!(caret_calls(&app), 0, "the old frame and its caret stay");
    app.pending_redraw = PendingRedraw::Full;
    assert!(app.draw_frame(None));
    assert!(
        caret_calls(&app) > 0,
        "new pixels replace the overlay's caret"
    );
}

#[test]
fn a_show_whose_frame_fails_reports_it_and_waits_for_no_retry() {
    let mut app = presenting();
    let guards = app.window_guards();
    app.dispatch_window_intent(WindowIntent::CloseRequested { now_ms: 1, guards });
    assert!(app.frames_released);
    app.host
        .present_answers
        .push_back(PresentOutcome::Failed("no surface".into()));
    app.dispatch_window_intent(WindowIntent::ShowRequested {
        reason: ShowReason::Tray,
        now_ms: 2,
    });
    assert!(
        !app.frames_released,
        "the storage is back for the next draw"
    );
    assert_eq!(app.diagnostic.as_deref(), Some("no surface"));
    assert_eq!(app.present_retry_at, None);
}

#[test]
fn a_caret_only_redraw_goes_to_the_host_as_a_whole_frame() {
    let mut app = presenting();
    app.native_caret_failed = true;
    app.pending_redraw = PendingRedraw::CaretOnly;
    assert!(app.draw_frame(None));
    assert_eq!(frames(&app), [FULL, REACHED]);
    assert_eq!(
        app.diagnostic, None,
        "no softbuffer damage present was tried"
    );
}

#[test]
fn hidden_frames_present_nothing_and_the_show_presents_the_full_size_again() {
    let mut app = presenting();
    app.host.present_answers.push_back(PresentOutcome::Deferred);
    app.pending_redraw = PendingRedraw::Full;
    app.draw_frame(None);
    assert!(app.present_retry_at.is_some());
    app.host.calls.borrow_mut().clear();

    let guards = app.window_guards();
    app.dispatch_window_intent(WindowIntent::CloseRequested { now_ms: 1, guards });
    assert!(app.frames_released);
    assert_eq!(app.present_retry_at, None, "a hide cancels the retry");
    let canvas = app.surface.as_ref().unwrap().pixmap();
    assert_eq!((canvas.width(), canvas.height()), (1, 1));
    assert_eq!(frames(&app), [], "a released canvas is not presented");

    app.dispatch_window_intent(WindowIntent::ShowRequested {
        reason: ShowReason::Tray,
        now_ms: 2,
    });
    assert!(!app.frames_released);
    assert!(
        frames(&app).contains(&FULL),
        "the show presents a full frame"
    );
}

#[test]
fn a_canvas_needs_a_size_and_reports_its_host() {
    assert!(SoftwareSurface::for_host(0, 10).is_err());
    let canvas = SoftwareSurface::for_host(4, 3).unwrap();
    assert!(canvas.presented_by_host());
    assert_eq!((canvas.pixmap().width(), canvas.pixmap().height()), (4, 3));
}
