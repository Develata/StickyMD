//! A startup's failures through the real shell on `FakeHost`: what each does and in which order.
//! The two winit calls between the phases need a window and are not reached here (plan 11: a test
//! double proves the interface, not the implementation).
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-failure-paths

use std::cell::Cell;

use crate::shell::headless_tests::{HeadlessApp, fixture, recovering_at_start};
use crate::testing::HostCall;
use stickymd_app::flow::startup::StartupStop;
use stickymd_app::flow::window::geometry::{MonitorGeometry, MonitorIdentity, PhysicalRect};
use stickymd_app::instruction::WindowPreferenceIntent;
use stickymd_app::ports::{FailureKind, HostFailure, MonitorSnapshot};

fn failure(detail: &str) -> HostFailure {
    HostFailure::new(FailureKind::Failed, detail)
}

/// A shell on a desktop with one display, with nothing recorded yet.
fn on_a_display(mut app: HeadlessApp) -> HeadlessApp {
    app.host.monitors = Ok(MonitorSnapshot {
        displays: vec![MonitorGeometry::new(
            MonitorIdentity::new("headless"),
            PhysicalRect::new(0, 0, 1920, 1080),
            1.0,
            true,
        )],
        current: None,
    });
    app.host.take_calls();
    app
}

fn milestones(calls: &[HostCall]) -> Vec<&'static str> {
    calls
        .iter()
        .filter_map(|call| match call {
            HostCall::Milestone(name) => Some(*name),
            _ => None,
        })
        .collect()
}

fn dialogs(calls: &[HostCall]) -> Vec<&str> {
    calls
        .iter()
        .filter_map(|call| match call {
            HostCall::ShowError(body) => Some(body.as_str()),
            _ => None,
        })
        .collect()
}

/// Runs both phases, which must finish, and returns the host's calls in order.
fn run_both_phases(app: &mut HeadlessApp) -> Vec<HostCall> {
    app.prepare_window_shell().unwrap();
    app.settle_shown_window().unwrap();
    app.host.take_calls()
}

#[test]
fn a_healthy_startup_keeps_its_order_and_reports_nothing() {
    let mut app = on_a_display(fixture("start"));
    let calls = run_both_phases(&mut app);

    assert_eq!(
        milestones(&calls),
        [
            "monitor_ready",
            "tray_ready",
            "window_visible",
            "opacity_ready",
            "topmost_ready"
        ]
    );
    let native: Vec<_> = calls
        .iter()
        .filter(|call| {
            matches!(
                call,
                HostCall::TrayCreated(_)
                    | HostCall::ReassertIdentity
                    | HostCall::ApplyFrame(_)
                    | HostCall::SetOpacity(_)
                    | HostCall::SetTopmost(_)
            )
        })
        .collect();
    assert!(
        matches!(
            native.as_slice(),
            [
                HostCall::TrayCreated(_),
                HostCall::ReassertIdentity,
                HostCall::ApplyFrame(_),
                HostCall::ReassertIdentity,
                HostCall::SetOpacity(_),
                HostCall::SetTopmost(false),
            ]
        ),
        "{native:?}"
    );
    assert!(dialogs(&calls).is_empty());
    assert_eq!(app.diagnostic, None);
    assert!(!app.startup_failed());
}

#[test]
fn without_a_display_the_startup_stops_before_it_creates_anything() {
    let mut app = fixture("start");
    app.host.take_calls();

    assert_eq!(app.prepare_window_shell(), Err(StartupStop::NoDisplay));
    assert_eq!(app.host.take_calls(), []);
}

#[test]
fn an_identity_refused_before_the_show_stops_the_startup_before_the_window_is_placed() {
    let mut app = on_a_display(fixture("start"));
    app.host.identity_result = Err(failure("style refused"));

    assert_eq!(
        app.prepare_window_shell(),
        Err(StartupStop::ToolIdentity {
            shown: false,
            cause: failure("style refused"),
        })
    );
    let calls = app.host.take_calls();
    assert!(!calls.iter().any(|c| matches!(c, HostCall::ApplyFrame(_))));
}

#[test]
fn an_identity_lost_after_the_show_stops_the_startup_before_the_window_is_styled() {
    let mut app = on_a_display(fixture("start"));
    app.host.identity_answers = [Ok(()), Err(failure("style lost"))].into();

    app.prepare_window_shell().unwrap();
    assert_eq!(
        app.settle_shown_window(),
        Err(StartupStop::ToolIdentity {
            shown: true,
            cause: failure("style lost"),
        })
    );
    let calls = app.host.take_calls();
    assert!(!calls.iter().any(|c| matches!(c, HostCall::SetOpacity(_))));
    assert!(!milestones(&calls).contains(&"window_visible"));
}

/// The one way a startup ends early, for every reason it can end.
#[test]
fn a_stopped_startup_removes_the_tray_then_explains_then_exits_and_is_recorded() {
    for stop in [
        StartupStop::Window("no window".into()),
        StartupStop::Surface("no surface".into()),
        StartupStop::NoDisplay,
        StartupStop::ToolIdentity {
            shown: false,
            cause: failure("before"),
        },
        StartupStop::ToolIdentity {
            shown: true,
            cause: failure("after"),
        },
    ] {
        let mut app = fixture("start");
        app.host.take_calls();
        let exits = Cell::new(0);

        app.abort_startup(&stop, || exits.set(exits.get() + 1));

        assert_eq!(
            app.host.take_calls(),
            [
                HostCall::TrayDestroyed,
                HostCall::ShowError(stop.to_string())
            ]
        );
        assert_eq!(exits.get(), 1);
        assert!(app.startup_failed());
    }
}

/// The opacity is how the window looks: the note opens anyway, and nothing interrupts the user.
#[test]
fn a_refused_opacity_is_reported_and_the_startup_goes_on() {
    let mut app = on_a_display(fixture("start"));
    app.host.opacity_result = Err(failure("alpha refused"));

    let calls = run_both_phases(&mut app);

    assert!(milestones(&calls).contains(&"topmost_ready"));
    assert!(calls.iter().any(|c| matches!(c, HostCall::SetTopmost(_))));
    assert!(dialogs(&calls).is_empty());
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("无法应用窗口透明度：alpha refused")
    );
    assert!(!app.startup_failed());
}

/// A desktop may have no way to keep a window on top at all. The configured value stays, and
/// asking for it again reaches the host, as after a failure at runtime.
#[test]
fn a_refused_topmost_is_reported_and_can_be_retried() {
    let mut app = on_a_display(fixture("start"));
    app.host.topmost_result = Some(Err(failure("z-order refused")));

    let calls = run_both_phases(&mut app);

    assert!(milestones(&calls).contains(&"topmost_ready"));
    assert!(dialogs(&calls).is_empty());
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("无法应用窗口置顶状态：z-order refused")
    );
    assert!(!app.startup_failed());

    app.host.topmost_result = None;
    app.dispatch_window_preference_intent(WindowPreferenceIntent::SetAlwaysOnTop(false));
    assert!(app.host.take_calls().contains(&HostCall::SetTopmost(false)));
}

/// Without a tray the user is told at once, because closing the window now ends the program; the
/// taskbar entry is the way back and no tool-window identity is asserted.
#[test]
fn a_missing_tray_interrupts_once_and_the_startup_goes_on_with_a_taskbar_entry() {
    let mut app = on_a_display(fixture("start"));
    app.host.tray_result = Err(failure("no tray"));

    app.prepare_window_shell().unwrap();
    let calls = app.host.take_calls();

    let message =
        "无法创建系统托盘图标；StickyMD 将保持窗口可见，关闭窗口会安全保存并退出。\n\nno tray";
    assert_eq!(dialogs(&calls), [message]);
    assert!(calls.contains(&HostCall::RestoreOrdinaryEntry));
    assert!(!calls.contains(&HostCall::ReassertIdentity));
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("无法创建系统托盘图标；StickyMD 将保持窗口可见，关闭窗口会安全保存并退出。 no tray")
    );
    assert!(!app.startup_failed());
}

#[test]
fn every_survived_failure_of_a_startup_has_its_place_on_the_one_status_line() {
    let mut app = on_a_display(fixture("start"));
    app.host.tray_result = Err(failure("no tray"));
    app.host.opacity_result = Err(failure("alpha refused"));
    app.host.topmost_result = Some(Err(failure("z-order refused")));

    run_both_phases(&mut app);

    assert_eq!(
        app.diagnostic.as_deref(),
        Some(
            "无法创建系统托盘图标；StickyMD 将保持窗口可见，关闭窗口会安全保存并退出。 no tray；\
             无法应用窗口透明度：alpha refused；无法应用窗口置顶状态：z-order refused"
        )
    );
}

/// The event loop still delivers what it buffered after `exit`, and `ApplicationHandler` needs a
/// real event loop, so what each callback does first is pinned in the source.
#[test]
fn every_callback_returns_at_once_after_a_failed_startup() {
    let source = include_str!("handler.rs");
    for callback in ["fn window_event(", "fn about_to_wait(", "fn user_event("] {
        let mut lines = source.lines().skip_while(|line| !line.contains(callback));
        lines
            .find(|line| line.trim_end().ends_with('{'))
            .unwrap_or_else(|| panic!("no body for {callback}"));
        assert_eq!(
            lines.next().map(str::trim),
            Some("if self.startup_failed {"),
            "{callback}"
        );
    }
}

/// What loading the note found is worth more than how the window looks, and the recovery prompt
/// lays out its actions by position, so nothing is added to it.
#[test]
fn a_degraded_startup_does_not_replace_what_loading_the_note_found() {
    let mut app = on_a_display(recovering_at_start());
    let found = app.diagnostic.clone();
    assert!(found.is_some());
    app.host.opacity_result = Err(failure("alpha refused"));

    run_both_phases(&mut app);

    assert_eq!(app.diagnostic, found);
}

#[test]
fn a_measuring_hosts_exit_removes_the_tray_first() {
    let mut app = fixture("start");
    app.host.take_calls();
    let exited = Cell::new(false);

    app.finish_host_probe(|| exited.set(true));

    assert_eq!(app.host.take_calls(), [HostCall::TrayDestroyed]);
    assert!(exited.get());
    assert!(!app.startup_failed());
}
