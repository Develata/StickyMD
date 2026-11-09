//! The shell's side of every host port, through the real shell on `FakeHost`: what reaches the host
//! and in which order, and what each answer does. The native side has no test here (plan 11:
//! a test double proves the interface, not the implementation).
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

use crate::host::HostSignal;
use crate::input::WindowKey;
use crate::testing::{FakeHost, HostCall};
use stickymd_app::execution::IoCompletion;
use stickymd_app::flow::window::geometry::{MonitorGeometry, MonitorIdentity, PhysicalRect};
use stickymd_app::flow::window::state::{
    DockEdge, PLACEMENT_CONFIRM_TIMEOUT_MS, ShowReason, StableVisibility, WindowGuardSnapshot,
    WindowIntent,
};
use stickymd_app::instruction::{PreviewIntent, WindowPlatformIntent, WindowPreferenceIntent};
use stickymd_app::ports::{
    FailureKind, FrameFact, HostFailure, MonitorSnapshot, TopmostFact, TrayCommand, TrayView,
    Visibility,
};
use stickymd_render::preview::{LinkKind, SpanAction};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

use crate::shell::headless_tests::{
    HeadlessApp, clocked_fixture, fixture, fixture_on, started_on, window_flow,
};
use crate::shell::input::ShellKey;
use crate::shell::window_geometry_runtime::{GeometryEvent, geometry_event};
use stickymd_app::config::{DockEdge as DockEdgeConfig, ViewMode};

fn failure(detail: &str) -> HostFailure {
    HostFailure::new(FailureKind::Failed, detail)
}

fn close(app: &mut HeadlessApp) {
    let guards = app.window_guards();
    app.dispatch_window_intent(WindowIntent::CloseRequested { now_ms: 1, guards });
}

fn show(app: &mut HeadlessApp) {
    app.dispatch_window_intent(WindowIntent::ShowRequested {
        reason: ShowReason::Tray,
        now_ms: 2,
    });
}

const HIDDEN: TrayView = TrayView {
    window_visible: false,
    always_on_top: false,
};
const SHOWN: TrayView = TrayView {
    window_visible: true,
    always_on_top: false,
};

/// The identity comes back after each window change, and only a confirmed hide reaches the tray.
#[test]
fn phase15_hiding_and_showing_reach_the_host_in_order() {
    let mut app = fixture("start");
    close(&mut app);
    assert_eq!(
        app.host.take_calls(),
        [
            HostCall::Hide,
            HostCall::ReassertIdentity,
            HostCall::TrayUpdated(HIDDEN),
        ]
    );
    show(&mut app);
    let calls = app.host.take_calls();
    assert!(matches!(calls[0], HostCall::ApplyFrame(_)));
    assert_eq!(
        calls[1..],
        [
            HostCall::ObserveFrame,
            HostCall::Show,
            HostCall::ReassertIdentity,
            HostCall::TrayUpdated(SHOWN),
            HostCall::Focus,
            HostCall::ReassertIdentity,
        ]
    );
}

fn visibility(visible: Option<bool>, minimized: Option<bool>) -> Visibility {
    Visibility { visible, minimized }
}

/// The identity comes back after the show either way, but a show that the read-back does not
/// confirm tells the tray nothing, asks for no focus, and leaves input off and the Preview unasked.
#[test]
fn phase15_a_rejected_show_changes_nothing_but_the_status_line() {
    let cases = [
        (Ok(visibility(Some(false), Some(false))), "Failed"),
        (Ok(visibility(Some(true), Some(true))), "Failed"),
        (Ok(visibility(None, Some(false))), "Unsupported"),
        (Err(FailureKind::Unavailable), "Unavailable"),
    ];
    for (answer, kind) in cases {
        let mut app = fixture("start");
        close(&mut app);
        app.host.take_calls();
        app.applied_window_effects.clear();
        app.host.show_answers.push_back(answer);
        show(&mut app);
        let calls = app.host.take_calls();
        assert!(matches!(calls[0], HostCall::ApplyFrame(_)), "{calls:?}");
        assert_eq!(
            calls[1..],
            [
                HostCall::ObserveFrame,
                HostCall::Show,
                HostCall::ReassertIdentity
            ]
        );
        assert!(app.app.hidden_to_tray(), "{kind}");
        assert!(!app.shell_input_enabled, "{kind}");
        assert!(
            !app.applied_window_effects
                .iter()
                .any(|effect| effect == "WindowShown"),
            "{kind}"
        );
        assert_eq!(
            app.diagnostic.as_deref(),
            Some(format!("窗口未能显示（{kind}）。").as_str())
        );
    }
}

/// A failed show can be asked for again, and the second answer decides.
#[test]
fn phase15_a_show_can_be_retried_after_a_rejection() {
    let mut app = fixture("start");
    close(&mut app);
    app.host.show_answers.push_back(Err(FailureKind::Failed));
    show(&mut app);
    assert!(app.app.hidden_to_tray());
    app.host.take_calls();
    show(&mut app);
    assert!(!app.app.hidden_to_tray());
    assert!(app.shell_input_enabled);
    let calls = app.host.take_calls();
    assert!(calls.contains(&HostCall::TrayUpdated(SHOWN)), "{calls:?}");
    assert!(calls.contains(&HostCall::Focus), "{calls:?}");
}

/// A visible window only gets the focus request; a read-back that disagrees is reported and
/// nothing else moves.
#[test]
fn phase15_a_focus_request_on_a_visible_window_reports_a_bad_read_back() {
    let mut app = fixture("start");
    app.host
        .focus_answers
        .push_back(Ok(visibility(Some(true), Some(true))));
    app.handle_host_signal(HostSignal::ShowRequested);
    assert_eq!(
        app.host.take_calls(),
        [HostCall::Focus, HostCall::ReassertIdentity]
    );
    assert_eq!(app.diagnostic.as_deref(), Some("窗口未能显示（Failed）。"));
    assert!(app.shell_input_enabled);
}

/// A dock's animation reads the window only for its final frame, however many it draws: only
/// that frame has a verdict, and reading means enumerating the displays.
#[test]
fn phase15_an_animation_observes_the_window_only_at_its_last_frame() {
    let mut app = fixture("start");
    docked(&mut app);
    app.dispatch_window_intent(WindowIntent::ManualCollapse { now_ms: 0 });
    for now_ms in 1..140 {
        app.dispatch_window_intent(WindowIntent::Tick { now_ms });
        app.observe_pending_placement();
    }
    app.dispatch_window_intent(WindowIntent::Tick { now_ms: 140 });
    let observations = app
        .host
        .take_calls()
        .iter()
        .filter(|call| **call == HostCall::ObserveFrame)
        .count();
    assert_eq!(observations, 1);
}

fn dock_drag(app: &mut HeadlessApp) {
    app.dispatch_window_intent(WindowIntent::DragEnded {
        frame: PhysicalRect::new(10, 100, 780, 900),
        monitor: MonitorGeometry::new(
            MonitorIdentity::new("headless"),
            PhysicalRect::new(0, 0, 1920, 1080),
            1.0,
            true,
        ),
        now_ms: 50,
    });
}

/// A drag's end reaches the configuration through the frame the host reports.
#[test]
fn phase15_a_drag_is_saved_once_the_window_is_observed_there() {
    let mut app = fixture("start");
    app.host.take_calls();
    dock_drag(&mut app);
    let calls = app.host.take_calls();
    assert!(matches!(calls[0], HostCall::ApplyFrame(_)), "{calls:?}");
    assert_eq!(calls[1], HostCall::ObserveFrame, "{calls:?}");
    assert_eq!(
        app.app.config.current().window.dock_edge,
        stickymd_app::config::DockEdge::Left
    );
    assert_eq!(app.diagnostic, None);
}

fn wrong_frame() -> Result<FrameFact, FailureKind> {
    Ok(FrameFact {
        outer: PhysicalRect::new(9, 9, 700, 700),
    })
}

fn observations(app: &mut HeadlessApp) -> usize {
    app.host
        .take_calls()
        .iter()
        .filter(|call| **call == HostCall::ObserveFrame)
        .count()
}

fn pending_placement(app: &HeadlessApp) -> Option<u64> {
    let flow = app.app.window_flow.as_ref().unwrap();
    flow.state().pending_placement_id()
}

fn config_dock(app: &HeadlessApp) -> stickymd_app::config::DockEdge {
    app.app.config.current().window.dock_edge
}

/// A window that never reaches the target ends in a diagnostic and no save, not a hang: with a
/// controlled clock the deadline asks the window exactly once more, and that answer ends it.
#[test]
fn phase15_a_timed_out_placement_is_reported_and_writes_nothing() {
    let (mut app, clock) = clocked_fixture();
    clock.set(50);
    app.host.observed_frame = Some(wrong_frame());
    dock_drag(&mut app);
    assert_eq!(observations(&mut app), 1, "the read after the request");
    assert!(pending_placement(&app).is_some());
    assert_eq!(config_dock(&app), DockEdgeConfig::None);
    assert_eq!(app.diagnostic, None);

    let deadline = 50 + PLACEMENT_CONFIRM_TIMEOUT_MS;
    clock.set(deadline - 1);
    app.settle_event_pass(deadline - 1);
    assert_eq!(observations(&mut app), 0, "not before the deadline");
    assert!(pending_placement(&app).is_some());

    app.applied_window_effects.clear();
    clock.set(deadline);
    app.settle_event_pass(deadline);
    assert_eq!(observations(&mut app), 1, "exactly one at the deadline");
    assert_eq!(pending_placement(&app), None);
    assert_eq!(config_dock(&app), DockEdgeConfig::None);
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("窗口未能到达目标位置（Failed）。")
    );
    let order: Vec<_> = app
        .applied_window_effects
        .iter()
        .filter(|effect| effect.starts_with("Observe") || effect.starts_with("PlacementFailed"))
        .cloned()
        .collect();
    assert_eq!(order.len(), 2, "{order:?}");
    assert!(order[0].starts_with("ObservePlacement"), "{order:?}");
    assert_eq!(order[1], "PlacementFailed(Failed)");
}

/// Moved and Resized through the shared geometry event entry the event loop uses (the native loop
/// itself is not driven here). A read that disagrees keeps waiting; the first one that matches
/// confirms, and only then does the placement commit. Every observation here is answered
/// explicitly, so one the test did not plan fails it.
#[test]
fn phase15_moved_and_resized_events_can_confirm_after_a_mismatch() {
    let mut app = fixture("start");
    app.host.strict_observations = true;
    {
        let mut answers = app.host.observe_answers.borrow_mut();
        answers.push_back(wrong_frame()); // the read right after the request
        answers.push_back(wrong_frame()); // Moved
    }
    dock_drag(&mut app);
    assert_eq!(observations(&mut app), 1);
    assert!(pending_placement(&app).is_some());
    assert_eq!(config_dock(&app), DockEdgeConfig::None);
    let target = app.app.window_flow.as_ref().unwrap().state().frame();
    app.host
        .observe_answers
        .borrow_mut()
        .push_back(Ok(FrameFact { outer: target })); // Resized

    let moved = WindowEvent::Moved(PhysicalPosition::new(1, 2));
    app.on_window_geometry(geometry_event(&moved).unwrap());
    assert_eq!(observations(&mut app), 1);
    assert!(pending_placement(&app).is_some());
    assert_eq!(config_dock(&app), DockEdgeConfig::None);
    assert_eq!(app.diagnostic, None);

    let resized = WindowEvent::Resized(PhysicalSize::new(780, 900));
    app.on_window_geometry(geometry_event(&resized).unwrap());
    assert_eq!(observations(&mut app), 1);
    assert_eq!(pending_placement(&app), None);
    assert_eq!(config_dock(&app), DockEdgeConfig::Left);
    assert_eq!(app.diagnostic, None);

    app.on_window_geometry(geometry_event(&moved).unwrap());
    assert_eq!(
        observations(&mut app),
        0,
        "nothing to read once it is settled"
    );
}

/// winit applies its own SetWindowPos after the scale-change callback returns, so nothing is
/// observed or committed until the turn after it.
#[test]
fn phase15_a_scale_change_is_recovered_only_on_the_next_event_turn() {
    let mut app = fixture("start");
    app.host.monitors = Ok(MonitorSnapshot {
        displays: vec![MonitorGeometry::new(
            MonitorIdentity::new("elsewhere"),
            PhysicalRect::new(4000, 0, 1000, 800),
            1.0,
            true,
        )],
        current: None,
    });
    let before = app.app.config.current().window.monitor_id.clone();
    app.host.take_calls();
    app.on_window_geometry(GeometryEvent::ScaleChanged);
    assert!(app.host.take_calls().is_empty());
    assert_eq!(pending_placement(&app), None);
    assert_eq!(app.app.config.current().window.monitor_id, before);

    app.run_deferred_display_recovery();
    let calls = app.host.take_calls();
    assert!(matches!(calls[0], HostCall::ApplyFrame(_)), "{calls:?}");
    assert_eq!(calls[1], HostCall::ObserveFrame, "{calls:?}");
    assert_eq!(
        app.app.config.current().window.monitor_id,
        "elsewhere",
        "only after the deferred turn"
    );

    app.run_deferred_display_recovery();
    assert!(app.host.take_calls().is_empty(), "the turn runs once");
}

fn scale_changes_to_a_new_display(app: &mut HeadlessApp) {
    app.host.monitors = Ok(MonitorSnapshot {
        displays: vec![MonitorGeometry::new(
            MonitorIdentity::new("elsewhere"),
            PhysicalRect::new(4000, 0, 1000, 800),
            1.0,
            true,
        )],
        current: None,
    });
    app.on_window_geometry(GeometryEvent::ScaleChanged);
}

/// While a display change is due, no observation may settle a placement from before it: the
/// recovery replaces it, and its own new placement is what gets confirmed.
#[test]
fn phase15_no_observation_settles_a_placement_while_recovery_is_due() {
    let mut app = fixture("start");
    app.host
        .observe_answers
        .borrow_mut()
        .push_back(wrong_frame());
    dock_drag(&mut app);
    let waiting = pending_placement(&app).expect("the drag waits");
    assert_eq!(observations(&mut app), 1);

    scale_changes_to_a_new_display(&mut app);
    let moved = WindowEvent::Moved(PhysicalPosition::new(1, 2));
    let resized = WindowEvent::Resized(PhysicalSize::new(780, 900));
    app.on_window_geometry(geometry_event(&moved).unwrap());
    app.on_window_geometry(geometry_event(&resized).unwrap());
    assert_eq!(observations(&mut app), 0);
    assert_eq!(pending_placement(&app), Some(waiting));
    assert_eq!(config_dock(&app), DockEdgeConfig::None);

    app.run_deferred_display_recovery();
    assert_eq!(pending_placement(&app), None, "the recovery's own frame");
    assert_eq!(
        app.app.config.current().window.monitor_id,
        "elsewhere",
        "the old drag is not what got written"
    );
}

/// A recovery that creates no placement still lets the waiting one be judged afterwards.
#[test]
fn phase15_a_recovery_with_no_placement_observes_the_waiting_one_after() {
    let mut app = fixture("start");
    app.host
        .observe_answers
        .borrow_mut()
        .push_back(wrong_frame());
    dock_drag(&mut app);
    assert!(pending_placement(&app).is_some());
    app.host.take_calls();

    app.host.monitors = Err(FailureKind::Unavailable);
    app.on_window_geometry(GeometryEvent::ScaleChanged);
    app.on_window_geometry(GeometryEvent::Moved);
    assert_eq!(observations(&mut app), 0, "not while recovery is due");
    assert!(pending_placement(&app).is_some());

    app.run_deferred_display_recovery();
    assert_eq!(observations(&mut app), 1);
    assert_eq!(pending_placement(&app), None);
    assert_eq!(config_dock(&app), DockEdgeConfig::Left);
}

/// The deferred turn really runs in the event loop's own pass, before the session's ticks.
#[test]
fn phase15_the_deferred_display_recovery_runs_in_about_to_wait() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/shell/handler.rs");
    let text = std::fs::read_to_string(path).unwrap();
    let pass = text.split("fn about_to_wait(").nth(1).unwrap();
    let recovery = pass.find("self.run_deferred_display_recovery();").unwrap();
    let settle = pass.find("self.settle_event_pass(now_ms);").unwrap();
    assert!(recovery < settle);
}

/// A tick that finds the placement deadline and an auto-collapse due together replaces the
/// placement first, so the window is not read for a placement that is gone.
#[test]
fn phase15_a_collapse_due_at_the_placement_deadline_reads_nothing() {
    let (mut app, clock) = clocked_fixture();
    clock.set(50);
    app.host.observed_frame = Some(wrong_frame());
    dock_drag(&mut app);
    assert_eq!(observations(&mut app), 1);
    let both_due = 50 + 700;
    clock.set(both_due);
    app.settle_event_pass(both_due);
    assert_eq!(observations(&mut app), 0);
    assert_eq!(pending_placement(&app), None);
    assert_eq!(app.diagnostic, None);
}

/// The startup applies the configured value and stops only on a host error, as it always has; a
/// read-back of any kind is not a startup failure. At runtime the same answer is `Unsupported`.
#[test]
fn phase15_the_startup_topmost_ignores_the_read_back() {
    let mut app = fixture("start");
    app.host.topmost_result = Some(Ok(TopmostFact { topmost: None }));
    assert!(app.apply_startup_topmost());
    assert_eq!(app.diagnostic, None);

    app.host.topmost_result = Some(Err(failure("z-order refused")));
    assert!(!app.apply_startup_topmost());
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("无法应用窗口置顶状态：z-order refused")
    );

    let mut app = fixture("start");
    app.host.topmost_result = Some(Ok(TopmostFact { topmost: None }));
    app.dispatch_window_preference_intent(WindowPreferenceIntent::SetAlwaysOnTop(true));
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("无法应用窗口置顶状态（Unsupported）。")
    );
    assert!(!app.app.config.current().always_on_top);
}

#[test]
fn phase15_a_window_event_with_no_pending_placement_reads_nothing() {
    let mut app = fixture("start");
    app.host.take_calls();
    app.observe_pending_placement();
    assert!(app.host.take_calls().is_empty());
}

fn start_native_drag(app: &mut HeadlessApp) {
    assert!(app.dispatch_window_platform_intent(WindowPlatformIntent::RequestDrag));
    assert!(app.move_resize_active);
}

fn topology_change(app: &mut HeadlessApp, now_ms: u64) {
    let display = MonitorGeometry::new(
        MonitorIdentity::new("elsewhere"),
        PhysicalRect::new(4000, 0, 1000, 800),
        1.0,
        true,
    );
    app.dispatch_window_intent(WindowIntent::DisplayTopologyChanged {
        monitors: vec![display],
        now_ms,
    });
}

/// A display change under a native drag starts no wait: it has no verdict and no diagnostic,
/// even after the timeout. The drag's end is judged and written; what follows is judged as usual.
#[test]
fn phase15_a_topology_change_during_a_drag_reports_nothing() {
    let mut app = fixture("start");
    start_native_drag(&mut app);
    topology_change(&mut app, 10);
    let flow = app.app.window_flow.as_ref().unwrap();
    assert_eq!(flow.state().pending_placement_id(), None);
    let calls = app.host.take_calls();
    assert!(!calls.contains(&HostCall::ObserveFrame), "{calls:?}");
    for now_ms in [10_000, 10_001] {
        app.dispatch_window_intent(WindowIntent::Tick { now_ms });
    }
    assert_eq!(app.diagnostic, None);
    assert_eq!(
        app.app.config.current().window.dock_edge,
        stickymd_app::config::DockEdge::None
    );

    app.complete_window_drag();
    assert!(!app.move_resize_active);
    dock_drag(&mut app);
    assert_eq!(app.diagnostic, None);
    assert_eq!(
        app.app.config.current().window.dock_edge,
        stickymd_app::config::DockEdge::Left
    );

    app.host.observed_frame = Some(Ok(FrameFact {
        outer: PhysicalRect::new(9, 9, 700, 700),
    }));
    // A focused window keeps its dock, so no collapse replaces this placement while it waits.
    app.dispatch_window_intent(WindowIntent::GuardsChanged {
        guards: WindowGuardSnapshot {
            window_focused: true,
            ..Default::default()
        },
        now_ms: 19_999,
    });
    topology_change(&mut app, 20_000);
    let flow = app.app.window_flow.as_ref().unwrap();
    assert!(flow.state().pending_placement_id().is_some());
    for now_ms in [20_500, 20_501] {
        app.dispatch_window_intent(WindowIntent::Tick { now_ms });
    }
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("窗口未能到达目标位置（Failed）。")
    );
}

/// The user's own drag is no confirmation of an earlier request, and the window is not read while
/// it lasts. The failure of a later placement is retried.
#[test]
fn phase15_a_native_drag_reads_nothing_and_a_failed_placement_can_be_retried() {
    let mut app = fixture("start");
    app.host.frame_result = Err(FailureKind::Failed);
    dock_drag(&mut app);
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("窗口未能到达目标位置（Failed）。")
    );
    let flow = app.app.window_flow.as_ref().unwrap();
    assert_eq!(flow.state().pending_placement_id(), None);
    app.host.frame_result = Ok(());
    app.diagnostic = None;
    dock_drag(&mut app);
    assert_eq!(app.diagnostic, None);
    assert_eq!(
        app.app.config.current().window.dock_edge,
        stickymd_app::config::DockEdge::Left
    );

    app.host
        .observe_answers
        .borrow_mut()
        .push_back(Err(FailureKind::Unavailable));
    dock_drag(&mut app);
    let flow = app.app.window_flow.as_ref().unwrap();
    assert!(flow.state().pending_placement_id().is_some());
    start_native_drag(&mut app);
    app.host.take_calls();
    app.observe_pending_placement();
    assert!(app.host.take_calls().is_empty(), "no read during a drag");
}

#[test]
fn phase15_a_rejected_hide_tells_the_tray_nothing() {
    let mut app = fixture("start");
    app.host.hide_result = Err(FailureKind::Failed);
    close(&mut app);
    assert_eq!(
        app.host.take_calls(),
        [HostCall::Hide, HostCall::ReassertIdentity]
    );
}

/// A window that lost its identity gets its taskbar entry back, and the hide still counts.
#[test]
fn phase15_a_lost_identity_restores_the_taskbar_entry_and_says_so() {
    let mut app = fixture("start");
    app.host.identity_result = Err(failure("style refused"));
    close(&mut app);
    assert_eq!(
        app.host.take_calls(),
        [
            HostCall::Hide,
            HostCall::ReassertIdentity,
            HostCall::RestoreOrdinaryEntry,
            HostCall::TrayUpdated(HIDDEN),
        ]
    );
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("无法保持工具窗口身份；已恢复任务栏入口以避免窗口不可达：style refused")
    );
}

#[test]
fn phase15_topmost_and_its_failure_reach_the_host_and_the_tray() {
    let mut app = fixture("start");
    app.dispatch_window_preference_intent(WindowPreferenceIntent::SetAlwaysOnTop(true));
    assert_eq!(
        app.host.take_calls(),
        [
            HostCall::SetTopmost(true),
            HostCall::TrayUpdated(TrayView {
                always_on_top: true,
                ..SHOWN
            }),
        ]
    );
    assert_eq!(app.diagnostic, None);

    app.host.topmost_result = Some(Err(failure("z-order refused")));
    app.dispatch_window_preference_intent(WindowPreferenceIntent::SetAlwaysOnTop(false));
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("无法应用窗口置顶状态：z-order refused")
    );
}

/// The user's value reaches the configuration and the tray only when the read-back confirms it;
/// the same value can be asked for again after a failure.
#[test]
fn phase15_a_rejected_topmost_writes_nothing_and_can_be_retried() {
    let cases = [
        (
            Ok(TopmostFact {
                topmost: Some(false),
            }),
            "无法应用窗口置顶状态（Failed）。",
        ),
        (
            Ok(TopmostFact { topmost: None }),
            "无法应用窗口置顶状态（Unsupported）。",
        ),
        (
            Err(failure("z-order refused")),
            "无法应用窗口置顶状态：z-order refused",
        ),
    ];
    for (answer, diagnostic) in cases {
        let mut app = fixture("start");
        app.host.topmost_answers.push_back(answer);
        app.dispatch_window_preference_intent(WindowPreferenceIntent::SetAlwaysOnTop(true));
        assert!(!app.app.config.current().always_on_top, "{diagnostic}");
        assert_eq!(
            app.host.take_calls(),
            [HostCall::SetTopmost(true), HostCall::SetTopmost(false)],
            "a failure asks for the confirmed bit again, once"
        );
        assert_eq!(app.diagnostic.as_deref(), Some(diagnostic));

        app.dispatch_window_preference_intent(WindowPreferenceIntent::SetAlwaysOnTop(true));
        assert!(app.app.config.current().always_on_top, "{diagnostic}");
        assert_eq!(
            app.host.take_calls(),
            [
                HostCall::SetTopmost(true),
                HostCall::TrayUpdated(TrayView {
                    always_on_top: true,
                    ..SHOWN
                }),
            ]
        );
    }
}

/// The sensor's temporary request reaches the window, not the configuration or the tray, and a
/// rejection of it only reaches the status line.
#[test]
fn phase15_a_sensor_topmost_stays_out_of_the_configuration() {
    let mut app = fixture("start");
    docked(&mut app);
    app.dispatch_window_intent(WindowIntent::ManualCollapse { now_ms: 10 });
    assert_eq!(app.host.take_calls(), [HostCall::SetTopmost(true)]);
    assert!(!app.app.config.current().always_on_top);

    let mut app = fixture("start");
    docked(&mut app);
    app.host.topmost_result = Some(Ok(TopmostFact {
        topmost: Some(false),
    }));
    app.dispatch_window_intent(WindowIntent::ManualCollapse { now_ms: 10 });
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("无法应用窗口置顶状态（Failed）。")
    );
}

fn docked(app: &mut HeadlessApp) {
    let edge = StableVisibility::DockedExpanded(DockEdge::Left);
    app.app.window_flow = Some(window_flow(edge));
}

#[test]
fn phase15_opacity_and_its_failure_reach_the_host() {
    let mut app = fixture("start");
    app.dispatch_window_preference_intent(WindowPreferenceIntent::PreviewOpacity(60));
    assert_eq!(app.host.take_calls(), [HostCall::SetOpacity(60)]);

    app.host.opacity_result = Err(failure("alpha refused"));
    app.dispatch_window_preference_intent(WindowPreferenceIntent::PreviewOpacity(70));
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("无法预览透明度：alpha refused")
    );
}

/// The quit starts from the tray; the tray stays until the barriers are through.
#[test]
fn phase15_a_tray_quit_starts_the_quit_and_keeps_the_tray_for_now() {
    let mut app = fixture("start");
    app.handle_host_signal(HostSignal::Tray(TrayCommand::Quit));
    let flow = app.app.window_flow.as_ref().unwrap();
    assert!(flow.state().lifecycle().is_quitting());
    assert!(!app.host.take_calls().contains(&HostCall::TrayDestroyed));
    assert!(!app.exit_pending);
}

#[test]
fn phase15_the_tray_toggle_hides_a_shown_window_and_shows_a_hidden_one() {
    let mut app = fixture("start");
    app.handle_host_signal(HostSignal::Tray(TrayCommand::ToggleVisibility));
    assert!(app.host.take_calls().contains(&HostCall::Hide));
    app.handle_host_signal(HostSignal::Tray(TrayCommand::ToggleVisibility));
    assert!(app.host.take_calls().contains(&HostCall::Show));
}

#[test]
fn phase15_a_second_start_shows_the_window() {
    let mut app = fixture("start");
    close(&mut app);
    app.host.take_calls();
    app.handle_host_signal(HostSignal::ShowRequested);
    assert!(app.host.take_calls().contains(&HostCall::Show));
}

/// Without an answer nothing moves; a window left off every display comes back onto one.
#[test]
fn phase15_a_topology_change_reads_the_displays_when_there_are_any() {
    let mut app = fixture("start");
    app.handle_host_signal(HostSignal::DisplayTopologyChanged);
    assert!(app.host.take_calls().is_empty());

    let display = MonitorGeometry::new(
        MonitorIdentity::new("elsewhere"),
        PhysicalRect::new(4000, 0, 1000, 800),
        1.0,
        true,
    );
    app.host.monitors = Ok(MonitorSnapshot {
        displays: vec![display],
        current: None,
    });
    app.handle_host_signal(HostSignal::DisplayTopologyChanged);
    let calls = app.host.take_calls();
    assert!(
        calls
            .iter()
            .any(|call| matches!(call, HostCall::ApplyFrame(frame) if frame.x >= 4000)),
        "{calls:?}"
    );
}

/// A note hint is acknowledged so the host may send the next one; a failed watch ends it.
#[test]
fn phase15_watch_signals_reach_the_host() {
    let mut app = fixture("start");
    app.handle_host_signal(HostSignal::NoteDirectoryChanged);
    assert_eq!(app.host.take_calls(), [HostCall::AcknowledgeWatch]);

    app.host.watching = true;
    app.handle_host_signal(HostSignal::WatchFailed("cause".into()));
    assert_eq!(app.host.take_calls(), [HostCall::StopWatch]);
    assert!(!app.host.watching);
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("外部文件自动检测已停止；安全保存仍受指纹校验保护：cause")
    );
}

#[test]
fn phase15_a_watch_that_cannot_start_is_reported_and_the_note_stays_open() {
    let host = FakeHost {
        watch_result: Err(failure("no watcher")),
        ..FakeHost::default()
    };
    let app = fixture_on(host, "start");
    assert!(!app.started_watcher);
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("外部文件自动检测不可用；安全保存仍受指纹校验保护：no watcher")
    );
}

#[test]
fn phase15_an_export_goes_where_the_picker_says() {
    let mut app = fixture("start");
    app.request_export();
    assert_eq!(app.host.take_calls(), [HostCall::ChooseExport]);
    assert!(
        !app.app.export_in_flight,
        "a cancelled dialog exports nothing"
    );
    assert_eq!(app.diagnostic, None);

    app.host.export_result = Err(failure("no dialog"));
    app.request_export();
    assert!(!app.app.export_in_flight);
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("无法打开导出位置选择器：no dialog")
    );

    app.host.export_result = Ok(Some(app.paths.note_dir.join("out.md")));
    app.request_export();
    assert!(app.app.export_in_flight);
}

#[test]
fn phase15_a_link_goes_to_the_launcher_and_its_failure_to_the_status_line() {
    let link = || {
        PreviewIntent::Activate(SpanAction::OpenLink {
            destination: "https://example.com".into(),
            kind: LinkKind::Https,
        })
    };
    let mut app = fixture("start");
    app.dispatch_preview_intent(link());
    assert_eq!(
        app.host.take_calls(),
        [HostCall::OpenTarget(
            "https://example.com".into(),
            LinkKind::Https
        )]
    );
    assert_eq!(app.diagnostic, None);

    app.host.open_result = Err(failure("refused"));
    app.dispatch_preview_intent(link());
    assert_eq!(app.diagnostic.as_deref(), Some("refused"));
}

const CARET: PhysicalRect = PhysicalRect::new(10, 20, 2, 18);

/// With nothing to draw any more, the overlay takes back what it inverted.
#[test]
fn phase15_the_overlay_inverts_back_what_it_inverted() {
    let mut app = fixture("start");
    app.native_caret_overlay = Some(CARET);
    app.advance_caret_blink();
    assert_eq!(app.host.take_calls(), [HostCall::CaretToggled(CARET)]);
    assert_eq!(app.native_caret_overlay, None);
    assert!(!app.native_caret_failed);
}

/// A failed overlay falls back to the frame for good, and the status line says why.
#[test]
fn phase15_a_failed_overlay_falls_back_to_the_frame() {
    let mut app = fixture("start");
    app.native_caret_overlay = Some(CARET);
    app.host.caret_result = Err(failure("no device context"));
    app.advance_caret_blink();
    assert_eq!(
        app.host.take_calls(),
        [HostCall::CaretToggled(CARET), HostCall::CaretReleased]
    );
    assert!(app.native_caret_failed);
    assert_eq!(app.native_caret_overlay, None);
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("caret overlay unavailable: no device context")
    );

    app.advance_caret_blink();
    assert!(app.host.take_calls().is_empty(), "the frame draws it now");
}

/// A host with no overlay is no failure: the frame draws the caret from the start, in silence.
#[test]
fn phase15_a_host_without_an_overlay_draws_the_caret_in_the_frame() {
    let host = FakeHost {
        caret_overlay: false,
        ..FakeHost::default()
    };
    let mut app = fixture_on(host, "start");
    assert!(app.native_caret_failed);
    app.advance_caret_blink();
    assert!(app.host.take_calls().is_empty());
    assert_eq!(app.diagnostic, None);
}

/// Sees, when the host drops, whether the worker's thread had already ended and let go of its end.
struct WorkerProbe {
    completions: mpsc::Receiver<IoCompletion>,
    worker_stopped: Arc<AtomicBool>,
}

impl Drop for WorkerProbe {
    fn drop(&mut self) {
        while self.completions.try_recv().is_ok() {}
        let ended = matches!(
            self.completions.try_recv(),
            Err(mpsc::TryRecvError::Disconnected)
        );
        self.worker_stopped.store(ended, Ordering::SeqCst);
    }
}

/// The worker stops before anything of the host drops, the single-instance guard among it: the
/// shell declares `app` before `host`.
#[test]
fn phase15_the_worker_stops_before_the_host_drops() {
    let (sender, completions) = mpsc::channel();
    let worker_stopped = Arc::new(AtomicBool::new(false));
    let host = FakeHost {
        dropped_with_host: Some(Box::new(WorkerProbe {
            completions,
            worker_stopped: Arc::clone(&worker_stopped),
        })),
        ..FakeHost::default()
    };
    let app = started_on(host, "start", None, move |completion| {
        let _ = sender.send(completion);
    });
    drop(app);
    assert!(worker_stopped.load(Ordering::SeqCst));
}

/// The shell asks the host's table: here a plain letter switches the view.
#[test]
fn phase15_a_key_means_what_the_hosts_table_says() {
    let mut host = FakeHost::default();
    host.keymap.window_key = |_, _, _| Some(WindowKey::View(ViewMode::Split));
    let mut app = fixture_on(host, "start");
    let letter = ShellKey {
        physical_key: PhysicalKey::Code(KeyCode::KeyA),
        state: ElementState::Pressed,
        repeat: false,
        ime_owned: false,
        text: Some("a"),
    };
    app.apply_key(letter, false);
    assert_eq!(app.app.config.current().view_mode, ViewMode::Split);
    assert_eq!(app.app.coordinator.view().text, "start");
}
