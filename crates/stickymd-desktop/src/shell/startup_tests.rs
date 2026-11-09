//! A startup's failures through the real shell on `FakeHost`: what each does and in which order.
//! The event loop's callbacks need a real loop and are not reached here (plan 11: a test double
//! proves the interface, not the implementation).
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-failure-paths

use std::rc::Rc;

use crate::shell::headless_tests::{HeadlessApp, fixture, fixture_on, recovering_at_start};
use crate::testing::{FakeHost, HostCall};
use stickymd_app::flow::startup::StartupStop;
use stickymd_app::flow::window::geometry::{MonitorGeometry, MonitorIdentity, PhysicalRect};
use stickymd_app::instruction::WindowPreferenceIntent;
use stickymd_app::ports::{FailureKind, HostFailure, MonitorSnapshot};

const TRAY_LINE: &str =
    "无法创建系统托盘图标；StickyMD 将保持窗口可见，关闭窗口会安全保存并退出。 no tray";

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

/// Runs the whole startup, with the steps only a window can take recorded in their place.
fn start(app: &mut HeadlessApp) -> (Result<(), StartupStop>, Vec<HostCall>) {
    let reveal = Rc::clone(&app.host.calls);
    let focus = Rc::clone(&app.host.calls);
    let outcome = app.initialize_window_shell(
        || reveal.borrow_mut().push(HostCall::WindowRevealed),
        || focus.borrow_mut().push(HostCall::WindowFocused),
    );
    (outcome, app.host.take_calls())
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

fn count(calls: &[HostCall], wanted: &HostCall) -> usize {
    calls.iter().filter(|call| *call == wanted).count()
}

fn position(calls: &[HostCall], wanted: &HostCall) -> usize {
    calls
        .iter()
        .position(|call| call == wanted)
        .unwrap_or_else(|| panic!("no {wanted:?} in {calls:?}"))
}

const ALL_MILESTONES: [&str; 8] = [
    "monitor_ready",
    "tray_ready",
    "window_visible",
    "opacity_ready",
    "topmost_ready",
    "focus_ready",
    "guards_ready",
    "shell_ready",
];

#[test]
fn a_healthy_startup_keeps_its_order_and_reports_nothing() {
    let mut app = on_a_display(fixture("start"));
    let (outcome, calls) = start(&mut app);

    assert_eq!(outcome, Ok(()));
    assert!(
        matches!(
            calls.as_slice(),
            [
                HostCall::Milestone("monitor_ready"),
                HostCall::TrayCreated(_),
                HostCall::ReassertIdentity,
                HostCall::ApplyFrame(_),
                HostCall::Milestone("tray_ready"),
                HostCall::WindowRevealed,
                HostCall::ReassertIdentity,
                HostCall::Milestone("window_visible"),
                HostCall::SetOpacity(_),
                HostCall::Milestone("opacity_ready"),
                HostCall::SetTopmost(false),
                HostCall::Milestone("topmost_ready"),
                HostCall::WindowFocused,
                HostCall::Milestone("focus_ready"),
                HostCall::Milestone("guards_ready"),
                HostCall::Milestone("shell_ready"),
            ]
        ),
        "{calls:?}"
    );
    assert_eq!(app.diagnostic, None);
    assert!(!app.startup_failed());
}

#[test]
fn without_a_display_the_startup_stops_before_it_creates_anything() {
    let mut app = fixture("start");
    app.host.take_calls();

    let (outcome, calls) = start(&mut app);

    assert_eq!(outcome, Err(StartupStop::NoDisplay));
    assert_eq!(calls, []);
}

#[test]
fn an_identity_refused_before_the_show_stops_the_startup_before_the_window_is_placed() {
    let mut app = on_a_display(fixture("start"));
    app.host.identity_result = Err(failure("style refused"));

    let (outcome, calls) = start(&mut app);

    assert_eq!(
        outcome,
        Err(StartupStop::ToolIdentity {
            shown: false,
            cause: failure("style refused"),
        })
    );
    assert!(!calls.iter().any(|c| matches!(c, HostCall::ApplyFrame(_))));
    assert_eq!(count(&calls, &HostCall::WindowRevealed), 0);
}

#[test]
fn an_identity_lost_after_the_show_stops_the_startup_before_the_window_is_styled() {
    let mut app = on_a_display(fixture("start"));
    app.host.identity_answers = [Ok(()), Err(failure("style lost"))].into();

    let (outcome, calls) = start(&mut app);

    assert_eq!(
        outcome,
        Err(StartupStop::ToolIdentity {
            shown: true,
            cause: failure("style lost"),
        })
    );
    assert_eq!(count(&calls, &HostCall::WindowRevealed), 1);
    assert!(!calls.iter().any(|c| matches!(c, HostCall::SetOpacity(_))));
    assert_eq!(count(&calls, &HostCall::WindowFocused), 0);
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
        let record = Rc::clone(&app.host.calls);

        app.abort_startup(&stop, || {
            record.borrow_mut().push(HostCall::EventLoopExited)
        });

        assert_eq!(
            app.host.take_calls(),
            [
                HostCall::TrayDestroyed,
                HostCall::ShowError(stop.to_string()),
                HostCall::EventLoopExited,
            ]
        );
        assert!(app.startup_failed());
    }
}

/// The opacity is how the window looks: the note opens anyway, and nothing interrupts the user.
#[test]
fn a_refused_opacity_is_reported_and_the_startup_goes_on() {
    let mut app = on_a_display(fixture("start"));
    app.host.opacity_result = Err(failure("alpha refused"));

    let (outcome, calls) = start(&mut app);

    assert_eq!(outcome, Ok(()));
    assert_eq!(milestones(&calls), ALL_MILESTONES);
    assert_eq!(count(&calls, &HostCall::WindowFocused), 1);
    assert!(dialogs(&calls).is_empty());
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("无法应用窗口透明度：alpha refused")
    );
    assert!(!app.startup_failed());
}

/// A desktop may have no way to keep a window on top at all. The configured value and the tray
/// stay, and asking for the value again reaches the host once; after that it is proved.
#[test]
fn a_refused_topmost_is_reported_and_can_be_retried_once() {
    let mut app = on_a_display(fixture("start"));
    app.host.topmost_result = Some(Err(failure("z-order refused")));

    let (outcome, calls) = start(&mut app);

    assert_eq!(outcome, Ok(()));
    assert_eq!(milestones(&calls), ALL_MILESTONES);
    assert_eq!(count(&calls, &HostCall::SetTopmost(false)), 1);
    assert!(dialogs(&calls).is_empty());
    assert_eq!(
        app.diagnostic.as_deref(),
        Some("无法应用窗口置顶状态：z-order refused")
    );
    assert!(!app.startup_failed());

    app.host.topmost_result = None;
    app.dispatch_window_preference_intent(WindowPreferenceIntent::SetAlwaysOnTop(false));
    let calls = app.host.take_calls();
    assert_eq!(count(&calls, &HostCall::SetTopmost(false)), 1, "{calls:?}");
    assert!(!calls.iter().any(|c| matches!(c, HostCall::TrayUpdated(_))));
    assert!(!app.app.config().current().always_on_top);

    app.dispatch_window_preference_intent(WindowPreferenceIntent::SetAlwaysOnTop(false));
    assert_eq!(
        count(&app.host.take_calls(), &HostCall::SetTopmost(false)),
        0
    );
}

/// Without a tray the user is told at once, because closing the window now ends the program; the
/// taskbar entry is the way back, in place before the window shows.
#[test]
fn a_missing_tray_interrupts_once_and_the_startup_goes_on_with_a_taskbar_entry() {
    let mut app = on_a_display(fixture("start"));
    app.host.tray_result = Err(failure("no tray"));

    let (outcome, calls) = start(&mut app);

    assert_eq!(outcome, Ok(()));
    assert_eq!(
        dialogs(&calls),
        ["无法创建系统托盘图标；StickyMD 将保持窗口可见，关闭窗口会安全保存并退出。\n\nno tray"]
    );
    let shown = position(&calls, &HostCall::WindowRevealed);
    assert!(position(&calls, &HostCall::RestoreOrdinaryEntry) < shown);
    assert_eq!(count(&calls, &HostCall::ReassertIdentity), 1);
    assert!(position(&calls, &HostCall::ReassertIdentity) > shown);
    assert_eq!(app.diagnostic.as_deref(), Some(TRAY_LINE));
    assert!(!app.startup_failed());
}

#[test]
fn every_survived_failure_of_a_startup_has_its_place_on_the_one_status_line() {
    let mut app = on_a_display(fixture("start"));
    app.host.tray_result = Err(failure("no tray"));
    app.host.opacity_result = Err(failure("alpha refused"));
    app.host.topmost_result = Some(Err(failure("z-order refused")));

    assert_eq!(start(&mut app).0, Ok(()));

    assert_eq!(
        app.diagnostic,
        Some(format!(
            "{TRAY_LINE}；无法应用窗口透明度：alpha refused；无法应用窗口置顶状态：z-order refused"
        ))
    );
}

/// A diagnostic from before the window existed is kept, with the failure after it.
#[test]
fn a_survived_failure_joins_what_the_status_line_already_says() {
    let host = FakeHost {
        watch_result: Err(failure("no watch")),
        ..FakeHost::default()
    };
    let mut app = on_a_display(fixture_on(host, "start"));
    let earlier = app.diagnostic.clone().expect("the watcher's diagnostic");
    app.host.opacity_result = Err(failure("alpha refused"));

    assert_eq!(start(&mut app).0, Ok(()));

    assert_eq!(
        app.diagnostic,
        Some(format!("{earlier}；无法应用窗口透明度：alpha refused"))
    );
}

/// The recovery prompt lays out its actions by position, so it takes nothing more; the reason is
/// told in a dialog and not lost.
#[test]
fn a_survived_failure_goes_to_a_dialog_while_the_recovery_prompt_holds_the_status_line() {
    let mut app = on_a_display(recovering_at_start());
    let prompt = app.diagnostic.clone();
    assert!(prompt.as_deref().is_some_and(|text| text.contains("[F7")));
    app.host.opacity_result = Err(failure("alpha refused"));

    let (outcome, calls) = start(&mut app);

    assert_eq!(outcome, Ok(()));
    assert_eq!(dialogs(&calls), ["无法应用窗口透明度：alpha refused"]);
    assert_eq!(app.diagnostic, prompt);
}

#[test]
fn a_missing_tray_is_told_once_while_the_recovery_prompt_holds_the_status_line() {
    let mut app = on_a_display(recovering_at_start());
    let prompt = app.diagnostic.clone();
    app.host.tray_result = Err(failure("no tray"));

    let (outcome, calls) = start(&mut app);

    assert_eq!(outcome, Ok(()));
    assert_eq!(
        dialogs(&calls),
        ["无法创建系统托盘图标；StickyMD 将保持窗口可见，关闭窗口会安全保存并退出。\n\nno tray"]
    );
    assert_eq!(milestones(&calls), ALL_MILESTONES);
    assert_eq!(count(&calls, &HostCall::WindowFocused), 1);
    assert_eq!(app.diagnostic, prompt);
}

/// The event loop still delivers what it buffered after `exit`, and `ApplicationHandler` needs a
/// real event loop, so what each callback does first is pinned in the source. That the guard
/// keeps buffered input out of the note is P15-S06.
#[test]
fn every_callback_starts_by_returning_after_a_failed_startup() {
    let source = include_str!("handler.rs");
    for callback in ["fn window_event(", "fn about_to_wait(", "fn user_event("] {
        let mut lines = source.lines().skip_while(|line| !line.contains(callback));
        lines
            .find(|line| line.trim_end().ends_with('{'))
            .unwrap_or_else(|| panic!("no body for {callback}"));
        let first: Vec<_> = lines.take(3).map(str::trim).collect();
        assert_eq!(
            first,
            ["if self.startup_failed {", "return;", "}"],
            "{callback}"
        );
    }
}

#[test]
fn a_measuring_hosts_exit_removes_the_tray_first() {
    let mut app = fixture("start");
    app.host.take_calls();
    let record = Rc::clone(&app.host.calls);

    app.finish_host_probe(|| record.borrow_mut().push(HostCall::EventLoopExited));

    assert_eq!(
        app.host.take_calls(),
        [HostCall::TrayDestroyed, HostCall::EventLoopExited]
    );
    assert!(!app.startup_failed());
}
