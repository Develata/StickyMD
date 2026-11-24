//! Headless pure tests; never construct a native event or launch a GUI process.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    input::{CMD, Event, Injection, Sender},
    model::*,
    options,
    scenarios::{self, Backend, Driver, Op, Value},
};
fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
    Rect {
        origin: Point { x, y },
        size: Size {
            width: w,
            height: h,
        },
    }
}
fn window(bounds: Rect, layer: i32) -> Window {
    Window {
        id: 7,
        bounds,
        alpha: 1.0,
        layer,
        onscreen: true,
    }
}
#[test]
fn two_dip_requires_offscreen_geometry_overlap_visibility_and_top_layer() {
    let s = rect(100.0, 50.0, 1440.0, 900.0);
    for (edge, bounds) in [
        (Edge::Left, rect(-418.0, 80.0, 520.0, 480.0)),
        (Edge::Right, rect(1538.0, 80.0, 520.0, 480.0)),
        (Edge::Top, rect(300.0, -428.0, 520.0, 480.0)),
    ] {
        let w = window(bounds, 27);
        assert!(collapsed(&w, s, edge));
        let mut hidden = w.clone();
        hidden.onscreen = false;
        assert!(!collapsed(&hidden, s, edge));
        let mut wide = w.clone();
        match edge {
            Edge::Left => wide.bounds.origin.x += 2.0,
            Edge::Right => wide.bounds.origin.x -= 2.0,
            Edge::Top => wide.bounds.origin.y += 2.0,
        };
        assert!(!collapsed(&wide, s, edge));
    }
    assert!(!collapsed(
        &window(rect(300.0, -428.0, 520.0, 480.0), 24),
        s,
        Edge::Top
    ));
    assert!(!collapsed(
        &window(rect(3000.0, -428.0, 520.0, 480.0), 27),
        s,
        Edge::Top
    ));
    assert!(!expanded(&window(rect(100.0, 50.0, 1500.0, 480.0), 0), s));
}
#[test]
fn band_includes_screen_top_but_not_outside_the_window_width() {
    let s = rect(0.0, 0.0, 1440.0, 900.0);
    let w = rect(400.0, 28.0, 520.0, 480.0);
    assert!(top_band(Point { x: 400.0, y: 0.0 }, w, s));
    for p in [
        Point { x: 399.0, y: 0.0 },
        Point { x: 920.0, y: 0.0 },
        Point { x: 500.0, y: -1.0 },
        Point { x: 500.0, y: 28.0 },
    ] {
        assert!(!top_band(p, w, s));
    }
}
#[test]
fn vm_interlock_is_strict_and_permissions_are_per_scenario() {
    for output in [
        "0",
        "",
        "true",
        "01",
        "1 extra",
        "kern.hv_vmm_present: 1",
        "unavailable",
    ] {
        assert!(!vm_value(output));
        assert!(require_vm(output, false).is_err());
    }
    assert!(require_vm("1\n", false).is_ok());
    assert!(require_vm("0", true).is_ok());
    let ids = vec!["W01".into(), "K02".into()];
    let e = permission_error(&ids, Permissions::default()).unwrap_err();
    assert!(e.contains("W01: Accessibility") && e.contains("K02:") && e.contains("PostEvent"));
    assert!(
        permission_error(
            &ids,
            Permissions {
                ax: true,
                post: true,
                screen: false
            }
        )
        .is_ok()
    );
    assert!(permission_error(&["W05".into()], Permissions::default()).is_ok());
    // R03 answers the prompt with a Return posted to the product alone; it reads no AX.
    assert!(permission_error(&["R03".into()], Permissions::default()).is_err());
    let post_only = Permissions {
        ax: false,
        post: true,
        screen: false,
    };
    assert!(permission_error(&["R03".into()], post_only).is_ok());
    // F01 now switches to Preview with a real key press before reading the mapped fonts.
    assert!(permission_error(&["F01".into()], Permissions::default()).is_err());
}
#[test]
fn failure_dominates_skips_and_json_escapes_observations() {
    let mut row = Row::new("W11");
    assert_eq!(aggregate(&row.checks), Verdict::NotTested);
    row.check("quit", true, "exited\n\"0\"");
    assert_eq!(aggregate(&row.checks), Verdict::Pass);
    row.skip("fault", "unavailable");
    assert_eq!(aggregate(&row.checks), Verdict::NotTested);
    row.check("save", false, "wrong bytes");
    assert_eq!(aggregate(&row.checks), Verdict::Fail);
    let json = row.json();
    assert!(json.contains("exited\\n\\\"0\\\""));
    assert!(json.contains("P16-W11"));
}
#[test]
fn standalone_parser_never_needs_an_existing_app() {
    let parse =
        |args: &[&str]| options::parse(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>());
    assert!(parse(&["--help"]).unwrap().help);
    let o = parse(&["--app", "/nonexistent.app", "--only", "P16-W01,K02"]).unwrap();
    assert_eq!(o.only, ["W01", "K02"]);
    assert!(!o.allow_host);
    assert_eq!(o.batch, None);
    let o = parse(&["--app", "x", "--batch", "rel-0.3.0"]).unwrap();
    assert_eq!(o.batch.as_deref(), Some("rel-0.3.0"));
    assert_eq!(o.zip, None);
    // R03 takes the release ZIP; with every row or with R03 named, --zip is accepted.
    let o = parse(&["--app", "x", "--zip", "c.zip"]).unwrap();
    assert_eq!(o.zip.as_deref(), Some(std::path::Path::new("c.zip")));
    assert!(parse(&["--app", "x", "--only", "W02,R03", "--zip", "c.zip"]).is_ok());
    let error = parse(&["--app", "x", "--only", "W02", "--zip", "c.zip"]).unwrap_err();
    assert!(error.contains("only for R03"), "{error}");
    // The write watch calibration goes with R03 alone, and is off unless asked for.
    assert!(!o.r03_write_restore);
    let o = parse(&["--app", "x", "--only", "R03", "--r03-write-restore"]).unwrap();
    assert!(o.r03_write_restore);
    let error = parse(&["--app", "x", "--only", "W02", "--r03-write-restore"]).unwrap_err();
    assert!(
        error.contains("--r03-write-restore is only for R03"),
        "{error}"
    );
    for args in [
        vec!["--app", "x", "--zip"],
        vec!["--app", "x", "--zip", "a.zip", "--zip", "b.zip"],
        vec!["--app"],
        vec!["--app", "x", "--only", "W99"],
        vec!["--app", "x", "--only", "W01,W01"],
        vec!["--allow-host", "--allow-host"],
        vec!["--app", "x", "--batch", "no spaces"],
        vec!["--app", "x", "--batch"],
    ] {
        assert!(parse(&args).is_err());
    }
    let args = ["mac-gui", "--help"].map(str::to_owned);
    assert!(matches!(
        crate::cli::CommandLine::parse(args),
        Ok(crate::cli::CommandLine::MacGui(_))
    ));
}
#[test]
fn config_reader_does_not_confuse_sections() {
    let s = "opacity = 40\n[window]\ndock_edge = \"top\"\nopacity = 99\n";
    assert_eq!(config_value(s, "", "opacity"), Some("40"));
    assert_eq!(config_value(s, "window", "dock_edge"), Some("\"top\""));
    assert_eq!(config_value(s, "", "dock_edge"), None);
}
#[derive(Default)]
struct FakeInput {
    events: Vec<Event>,
    fail: Option<usize>,
    panic: Option<usize>,
}
impl Sender for FakeInput {
    fn send(&mut self, event: Event) -> Result<(), String> {
        self.events.push(event);
        let n = self.events.len();
        if self.panic == Some(n) {
            panic!("injected backend panic");
        }
        if self.fail == Some(n) {
            Err("injected send failure".into())
        } else {
            Ok(())
        }
    }
    fn pause(&mut self, _: u64) {}
}
#[test]
fn successful_chord_and_unicode_text_release_every_press() {
    let mut input = Injection::new(FakeInput::default());
    input.chord(&[CMD, super::input::SHIFT, 6]).unwrap();
    let released = input
        .backend
        .events
        .iter()
        .filter_map(|event| match event {
            Event::Key {
                code, down: false, ..
            } => Some(*code),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(released, [6, super::input::SHIFT, CMD]);
    input.backend.events.clear();
    input.text("a\n中").unwrap();
    assert_eq!(input.backend.events.len(), 6);
    for pair in input.backend.events.chunks_exact(2) {
        assert!(matches!(pair[0], Event::Key { down: true, .. }));
        assert!(matches!(pair[1], Event::Key { down: false, .. }));
    }
    assert!(matches!(
        input.backend.events[2],
        Event::Key {
            code: 36,
            text: None,
            ..
        }
    ));
}
#[test]
fn duplicate_press_is_rejected_and_the_existing_press_released() {
    let mut input = Injection::new(FakeInput::default());
    assert!(input.chord(&[CMD, CMD]).is_err());
    assert_eq!(input.backend.events.len(), 2);
    assert!(matches!(
        input.backend.events[1],
        Event::Key {
            code: CMD,
            down: false,
            ..
        }
    ));
}
#[test]
fn every_registered_key_is_released_after_partial_press_failure() {
    let mut input = Injection::new(FakeInput {
        fail: Some(2),
        ..Default::default()
    });
    assert!(input.chord(&[CMD, 0]).is_err());
    assert!(matches!(
        input.backend.events[2],
        Event::Key {
            code: 0,
            down: false,
            ..
        }
    ));
    assert!(matches!(
        input.backend.events[3],
        Event::Key {
            code: CMD,
            down: false,
            flags: 0,
            ..
        }
    ));
    let n = input.backend.events.len();
    assert!(input.chord(&[1]).is_err());
    assert_eq!(input.backend.events.len(), n);
}
#[test]
fn release_failure_stops_later_scenarios_and_still_releases_modifiers() {
    let mut input = Injection::new(FakeInput {
        fail: Some(3),
        ..Default::default()
    });
    assert!(input.chord(&[CMD, 1]).is_err());
    assert!(matches!(
        input.backend.events.last(),
        Some(Event::Key {
            code: CMD,
            down: false,
            ..
        })
    ));
    assert!(input.move_to(Point::default()).is_err());
}
#[test]
fn unwind_and_button_motion_failure_release_owned_input() {
    let mut input = Injection::new(FakeInput {
        panic: Some(2),
        ..Default::default()
    });
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| input.chord(&[CMD, 1]))).is_err()
    );
    assert!(matches!(
        input.backend.events.last(),
        Some(Event::Key {
            code: CMD,
            down: false,
            ..
        })
    ));
    let mut input = Injection::new(FakeInput {
        fail: Some(3),
        ..Default::default()
    });
    assert!(
        input
            .drag(Point::default(), Point { x: 90.0, y: 40.0 })
            .is_err()
    );
    assert!(matches!(
        input.backend.events.last(),
        Some(Event::Button { down: false, .. })
    ));
}
#[test]
fn a_panicking_release_does_not_skip_remaining_modifiers() {
    let mut input = Injection::new(FakeInput {
        panic: Some(3),
        ..Default::default()
    });
    assert!(input.chord(&[CMD, 1]).is_err());
    assert!(matches!(
        input.backend.events.last(),
        Some(Event::Key {
            code: CMD,
            down: false,
            ..
        })
    ));
    assert!(input.stopped.is_some());
}
struct HoverBackend {
    now: u64,
    entered: Option<u64>,
    left: Option<u64>,
    focused: u32,
    steals: bool,
    expand_at: u64,
    leave_at: u64,
    /// How long the window stays on screen but short of its rest after the 140 ms animation.
    short_of_rest: u64,
    /// Since the pointer entered: when StickyMD is frontmost, then gives focus back.
    flash_focus: Option<(u64, u64)>,
    /// Since the pointer entered: when the window collapses, then expands again.
    dip: Option<(u64, u64)>,
    /// Where the window sits during the dip: fully collapsed, or only partly retracted.
    dip_y: f64,
    /// Since the pointer entered: when the window keeps its bounds but is not on screen.
    hidden: Option<(u64, u64)>,
    /// Since the pointer entered: when another app (PID 77) is frontmost.
    other_front: Option<(u64, u64)>,
    /// Once: the first window query from this time on, since the pointer entered (or left, when
    /// the flag is set), takes this many ms and returns the state at its end.
    slow_window: Option<(bool, u64, u64)>,
    /// Once: the same for a frontmost-app query, since the pointer entered.
    slow_front: Option<(u64, u64)>,
    /// Since the pointer left: when the window moves inside the screen (to y = 9).
    leave_dip: Option<(u64, u64)>,
    /// The n-th window query of the scenario (from 0) takes this many ms.
    slow_nth: Option<(usize, u64)>,
    /// Window queries so far.
    windows: usize,
    /// On the fake clock: when StickyMD is frontmost, when another app (PID 77) is, and when
    /// the window is not on screen.
    flash_abs: Option<(u64, u64)>,
    other_abs: Option<(u64, u64)>,
    hidden_abs: Option<(u64, u64)>,
    /// Since the pointer entered: the expanded window keeps the moving layer until this time.
    layer_late: Option<u64>,
    /// On the fake clock: when the expanded window sits partly retracted (y = 9).
    retract_abs: Option<(u64, u64)>,
    /// Since the pointer entered: from this time on, reading the frontmost app fails.
    front_error_at: Option<u64>,
    /// Since the pointer left: when the window is hidden in place.
    leave_hidden: Option<(u64, u64)>,
    /// In every entry onto the strip: the first window query from this time on takes this long.
    slow_each_entry: Option<(u64, u64)>,
    slowed_this_entry: bool,
}
impl HoverBackend {
    fn new() -> Self {
        Self {
            now: 0,
            entered: None,
            left: None,
            focused: 42,
            steals: false,
            expand_at: 100,
            leave_at: 500,
            short_of_rest: 0,
            flash_focus: None,
            dip: None,
            dip_y: -478.0,
            hidden: None,
            other_front: None,
            slow_window: None,
            slow_front: None,
            leave_dip: None,
            slow_nth: None,
            windows: 0,
            flash_abs: None,
            other_abs: None,
            hidden_abs: None,
            layer_late: None,
            retract_abs: None,
            front_error_at: None,
            leave_hidden: None,
            slow_each_entry: None,
            slowed_this_entry: false,
        }
    }
    fn at(&self, range: Option<(u64, u64)>) -> bool {
        range.is_some_and(|(from, to)| (from..to).contains(&self.now))
    }
    /// Spends a slow query's time when this query is the one it applies to.
    fn slow(&mut self, window: bool) {
        let (mark, slow) = match (window, self.slow_window, self.slow_front) {
            (true, Some((true, at, cost)), _) => (self.left, Some((at, cost))),
            (true, Some((false, at, cost)), _) => (self.entered, Some((at, cost))),
            (false, _, slow) => (self.entered, slow),
            (true, None, _) => (None, None),
        };
        let Some((at, cost)) = slow else { return };
        if mark.is_some_and(|t| self.now - t >= at) {
            if window {
                self.slow_window = None;
            } else {
                self.slow_front = None;
            }
            self.now += cost;
        }
    }
    fn window_now(&self) -> Window {
        let mut w = if self.within(self.dip) {
            window(rect(460.0, self.dip_y, 520.0, 480.0), 27)
        } else if self.leave_dip.is_some_and(|(from, to)| {
            self.left
                .is_some_and(|t| (from..to).contains(&(self.now - t)))
        }) {
            window(rect(460.0, 9.0, 520.0, 480.0), 0)
        } else if self
            .entered
            .is_some_and(|t| (self.expand_at..self.expand_at + 140).contains(&(self.now - t)))
            || self
                .left
                .is_some_and(|t| (self.leave_at..self.leave_at + 140).contains(&(self.now - t)))
        {
            window(rect(460.0, -200.0, 520.0, 480.0), 27)
        } else if self.entered.is_some_and(|t| {
            let rest = self.expand_at + 140;
            (rest..rest + self.short_of_rest).contains(&(self.now - t))
        }) {
            window(rect(460.0, 12.0, 520.0, 480.0), 27)
        } else if self.at(self.retract_abs) {
            window(rect(460.0, 9.0, 520.0, 480.0), 0)
        } else if self.expanded() {
            let level_late = self
                .layer_late
                .is_some_and(|until| self.entered.is_some_and(|t| self.now - t < until));
            window(
                rect(460.0, 28.0, 520.0, 480.0),
                if level_late { 27 } else { 0 },
            )
        } else {
            window(rect(460.0, -478.0, 520.0, 480.0), 27)
        };
        let leave_hidden = self.leave_hidden.is_some_and(|(from, to)| {
            self.left
                .is_some_and(|t| (from..to).contains(&(self.now - t)))
        });
        w.onscreen = !self.within(self.hidden) && !self.at(self.hidden_abs) && !leave_hidden;
        w
    }
    fn within(&self, range: Option<(u64, u64)>) -> bool {
        range.is_some_and(|(from, to)| {
            self.entered
                .is_some_and(|t| (from..to).contains(&(self.now - t)))
        })
    }
    fn expanded(&self) -> bool {
        self.entered
            .is_some_and(|t| self.now - t >= self.expand_at + 140)
            || self.left.is_some_and(|t| self.now - t < self.leave_at)
    }
}
impl Backend for HoverBackend {
    fn now(&self) -> u64 {
        self.now
    }
    fn perform(&mut self, op: Op) -> Result<Value, String> {
        Ok(match op {
            Op::Wait(ms) => {
                self.now += ms;
                Value::Unit
            }
            Op::Window => {
                if self.slow_nth.is_some_and(|(n, _)| n == self.windows) {
                    self.now += self.slow_nth.map_or(0, |(_, cost)| cost);
                }
                self.windows += 1;
                if let Some((at, cost)) = self.slow_each_entry
                    && !self.slowed_this_entry
                    && self.entered.is_some_and(|t| self.now - t >= at)
                {
                    self.slowed_this_entry = true;
                    self.now += cost;
                }
                self.slow(true);
                Value::Window(self.window_now())
            }
            Op::Screen => Value::Rect(rect(0.0, 0.0, 1440.0, 900.0)),
            Op::Front => {
                self.slow(false);
                if self
                    .front_error_at
                    .is_some_and(|at| self.entered.is_some_and(|t| self.now - t >= at))
                {
                    return Err("AX error -25204 (kAXErrorCannotComplete)".into());
                }
                Value::Pid(
                    if (self.steals && self.expanded())
                        || self.within(self.flash_focus)
                        || self.at(self.flash_abs)
                    {
                        42
                    } else if self.within(self.other_front) || self.at(self.other_abs) {
                        77
                    } else {
                        self.focused
                    },
                )
            }
            Op::Finder => {
                self.focused = 99;
                Value::Pid(99)
            }
            Op::Activate | Op::Drag(..) => Value::Unit,
            Op::Move(p) => {
                if p.y <= 1.0 {
                    if self.entered.is_none() {
                        self.entered = Some(self.now);
                        self.slowed_this_entry = false;
                    }
                    self.left = None;
                } else {
                    if self.expanded() {
                        self.left = Some(self.now);
                    }
                    self.entered = None;
                }
                Value::Unit
            }
            Op::Config => Value::Text("[window]\ndock_edge = \"top\"\n".into()),
            _ => return Err(format!("unexpected fake operation {op:?}")),
        })
    }
}
fn hover_row(fake: &mut HoverBackend, id: &str) -> Row {
    let mut row = Row::new(id);
    scenarios::run(
        &mut Driver {
            backend: fake,
            row: &mut row,
            pid: 42,
        },
        id,
    )
    .unwrap();
    row
}
#[test]
fn hover_orchestration_checks_band_cancellation_timing_and_focus() {
    let row = hover_row(&mut HoverBackend::new(), "W04");
    assert_eq!(aggregate(&row.checks), Verdict::Pass, "{}", row.json());
    let mut stealing = HoverBackend::new();
    stealing.steals = true;
    assert_eq!(
        aggregate(&hover_row(&mut stealing, "W03").checks),
        Verdict::Fail
    );
    let mut late = HoverBackend::new();
    late.expand_at = 800;
    assert_eq!(
        aggregate(&hover_row(&mut late, "W03").checks),
        Verdict::Fail
    );
    let mut early = HoverBackend::new();
    early.expand_at = 0;
    assert_eq!(
        aggregate(&hover_row(&mut early, "W03").checks),
        Verdict::Fail
    );
}

/// Focus taken during the expansion and given back before it settles, and a collapse in the top
/// band undone before the dwell ends: both happen between the old single checks and must fail.
#[test]
fn hover_catches_focus_taken_back_and_a_collapse_undone_in_time() {
    let mut flash = HoverBackend::new();
    flash.flash_focus = Some((100, 160));
    let row = hover_row(&mut flash, "W03");
    assert_eq!(aggregate(&row.checks), Verdict::Fail, "{}", row.json());
    assert!(row.json().contains("StickyMD frontmost at Some("));
    let mut dip = HoverBackend::new();
    dip.dip = Some((900, 960));
    let row = hover_row(&mut dip, "W04");
    assert_eq!(aggregate(&row.checks), Verdict::Fail, "{}", row.json());
    assert!(row.json().contains("left the settled"));
    // Focus taken during the early-leave dwell, before any window observation of the hover.
    let mut early_flash = HoverBackend::new();
    early_flash.flash_focus = Some((5, 30));
    let row = hover_row(&mut early_flash, "W03");
    assert_eq!(aggregate(&row.checks), Verdict::Fail, "{}", row.json());
    // A partial retraction that never leaves the screen still leaves the settled position.
    let mut partial = HoverBackend::new();
    partial.dip = Some((900, 960));
    partial.dip_y = 12.0;
    let row = hover_row(&mut partial, "W04");
    assert_eq!(aggregate(&row.checks), Verdict::Fail, "{}", row.json());
    // The same collapse outside the dwell, in W03 which has none, changes nothing.
    let mut outside = HoverBackend::new();
    outside.dip = Some((5000, 5060));
    assert_eq!(
        aggregate(&hover_row(&mut outside, "W03").checks),
        Verdict::Pass
    );
}

fn check(row: &Row, name: &str) -> Verdict {
    row.checks
        .iter()
        .find(|c| c.name == name)
        .unwrap_or_else(|| panic!("no check {name}: {}", row.json()))
        .verdict
}

/// Round 4 of the codex review: the stretches the old checks left unobserved.
#[test]
fn hover_tracks_leave_no_unobserved_stretch() {
    // Expanded at 40-60 ms after the formal move and back before the old first window read.
    let mut flash = HoverBackend::new();
    flash.dip = Some((40, 60));
    flash.dip_y = 28.0;
    let row = hover_row(&mut flash, "W03");
    assert_eq!(
        check(&row, "hover movement onset"),
        Verdict::Fail,
        "{}",
        row.json()
    );
    // Hidden in the top band at its settled bounds, then shown again.
    let mut hidden = HoverBackend::new();
    hidden.hidden = Some((900, 960));
    let row = hover_row(&mut hidden, "W04");
    assert_eq!(
        check(&row, "top band holds past 500ms"),
        Verdict::Fail,
        "{}",
        row.json()
    );
    // Another app in front: the scene changed under the check, which cannot pass or fail.
    let mut other = HoverBackend::new();
    other.other_front = Some((150, 200));
    let row = hover_row(&mut other, "W03");
    assert_eq!(
        check(&row, "hover expands without focus"),
        Verdict::NotTested
    );
    assert_eq!(aggregate(&row.checks), Verdict::NotTested, "{}", row.json());
    // The window query after the onset returns after the deadline with the window at rest: the
    // onset sample dates it, so the sampling cannot tell, rather than a late completion.
    let mut late_sample = HoverBackend::new();
    late_sample.slow_window = Some((false, 105, 200));
    let row = hover_row(&mut late_sample, "W03");
    assert_eq!(
        check(&row, "hover expansion completion"),
        Verdict::NotTested,
        "{}",
        row.json()
    );
    assert_ne!(aggregate(&row.checks), Verdict::Fail);
    // The leave onset at 500 ms, the next window read 150 ms later already at rest: the stretch
    // from the onset sample counts.
    let mut leave_gap = HoverBackend::new();
    leave_gap.slow_window = Some((true, 505, 150));
    let row = hover_row(&mut leave_gap, "W03");
    assert_eq!(
        check(&row, "leave collapse completion"),
        Verdict::NotTested,
        "{}",
        row.json()
    );
    // A slow front read while focus is taken and given back: the gap counts.
    let mut slow_front = HoverBackend::new();
    slow_front.slow_front = Some((450, 80));
    slow_front.flash_focus = Some((455, 520));
    let row = hover_row(&mut slow_front, "W03");
    assert_eq!(
        check(&row, "hover expands without focus"),
        Verdict::NotTested,
        "{}",
        row.json()
    );
    // An early dwell of 100 ms or more may expand and cannot test the cancellation: nothing
    // fails, the trial is repeated once the window rests on the strip, and the next one passes.
    let mut long_dwell = HoverBackend::new();
    long_dwell.slow_window = Some((false, 20, 90));
    let row = hover_row(&mut long_dwell, "W03");
    assert_eq!(
        check(&row, "early leave cancels"),
        Verdict::Pass,
        "{}",
        row.json()
    );
    assert!(row.json().contains("trial 2:"), "{}", row.json());
    // The unresolved first trial is not part of the focus claim: the row passes.
    assert_eq!(aggregate(&row.checks), Verdict::Pass, "{}", row.json());
    // StickyMD in front during that repeated trial still fails.
    // Before the slowness the clock matches an undisturbed run, so this lands in trial 1 only.
    let (t0, _) = query_before_move("W03", 1.0, 0);
    let mut taken = HoverBackend::new();
    taken.slow_window = Some((false, 20, 90));
    taken.flash_abs = Some((t0 + 1, t0 + 15));
    let row = hover_row(&mut taken, "W03");
    assert_eq!(
        check(&row, "hover expands without focus"),
        Verdict::Fail,
        "{}",
        row.json()
    );
    assert!(
        row.json().contains("in a repeated early-leave trial"),
        "{}",
        row.json()
    );
}

/// The fake-clock time and the index (from 0) of the window query read just before the move
/// to `y` with the given ordinal (0 = first such move), from an undisturbed run of `id`.
fn query_before_move(id: &str, y: f64, ordinal: usize) -> (u64, usize) {
    let row = hover_row(&mut HoverBackend::new(), id);
    let mut windows = 0;
    let mut last = None;
    let mut seen = 0;
    for t in &row.trace {
        if t.event == "request Window" {
            last = Some((t.ms, windows));
            windows += 1;
        } else if t.event.starts_with("request Move(") && t.event.contains(&format!("y: {y:?} }}"))
        {
            if seen == ordinal {
                return last.expect("a window read before the move");
            }
            seen += 1;
        }
    }
    panic!("no move to y={y} #{ordinal} in {}", row.json());
}

/// Round 5 of the codex review: motion inside the screen, the seams between phases, the deadline
/// boundary, a moving window passing through the strip, and the Finder precondition.
#[test]
fn hover_watch_is_one_stream_and_motion_is_any_change_of_place() {
    // The leave starts inside the screen at 400 ms and crosses the edge on time: early motion.
    let mut early_leave = HoverBackend::new();
    early_leave.leave_dip = Some((400, 500));
    let row = hover_row(&mut early_leave, "W03");
    assert_eq!(
        check(&row, "leave movement onset"),
        Verdict::Fail,
        "{}",
        row.json()
    );
    // The read before the band move takes 120 ms while StickyMD takes focus and gives it back:
    // the seam between hover and band is inside the watch.
    let (t, n) = query_before_move("W04", 0.0, 0);
    let mut seam = HoverBackend::new();
    seam.slow_nth = Some((n, 120));
    seam.flash_abs = Some((t + 20, t + 100));
    let row = hover_row(&mut seam, "W04");
    assert_eq!(
        check(&row, "top band dwell keeps focus away"),
        Verdict::NotTested,
        "{}",
        row.json()
    );
    // Hidden in the read just before the band move: the band cannot start from the rest.
    let mut hidden_before = HoverBackend::new();
    hidden_before.hidden_abs = Some((t, t + 1));
    let row = hover_row(&mut hidden_before, "W04");
    assert_eq!(
        check(&row, "top band holds past 500ms"),
        Verdict::NotTested,
        "{}",
        row.json()
    );
    // An early dwell long enough to expand fully: the formal hover starts only once the window
    // has rested on the strip again, and then passes.
    let mut expanded_early = HoverBackend::new();
    expanded_early.slow_window = Some((false, 20, 250));
    let row = hover_row(&mut expanded_early, "W03");
    assert_eq!(
        check(&row, "early leave cancels"),
        Verdict::Pass,
        "{}",
        row.json()
    );
    for name in [
        "hover movement onset",
        "hover expansion completion",
        "leave movement onset",
    ] {
        assert_eq!(check(&row, name), Verdict::Pass, "{name}: {}", row.json());
    }
    // Another app in front since docking: the scene changed, which cannot pass or fail.
    let mut other = HoverBackend::new();
    other.other_abs = Some((0, u64::MAX));
    let row = hover_row(&mut other, "W03");
    assert_eq!(
        check(&row, "Finder frontmost before hover"),
        Verdict::NotTested
    );
    assert_eq!(aggregate(&row.checks), Verdict::NotTested, "{}", row.json());
}

/// Round 6 of the codex review: a level reached late, a retraction just before the leave, an
/// unproven expansion feeding the focus check, an unreadable front app, a hidden flicker.
#[test]
fn hover_judges_level_seams_and_unobservable_reads() {
    // In place on time but still at the moving level until 400 ms: the expansion is late.
    let mut level = HoverBackend::new();
    level.layer_late = Some(400);
    let row = hover_row(&mut level, "W03");
    assert_eq!(
        check(&row, "hover expansion completion"),
        Verdict::Fail,
        "{}",
        row.json()
    );
    // Retracted to y = 9 in the read just before the leave: the leave cannot start from it.
    // Moves to y = 800: docking, the early leave, then the leave itself.
    let (t, _) = query_before_move("W03", 800.0, 2);
    let mut retract = HoverBackend::new();
    retract.retract_abs = Some((t, t + 1));
    let row = hover_row(&mut retract, "W03");
    assert_eq!(
        check(&row, "leave movement onset"),
        Verdict::NotTested,
        "{}",
        row.json()
    );
    // Moving at 220 ms, then the front read blocks to 530 ms: completion cannot tell, and the
    // focus check must not fail on the unproven rest.
    let mut blocked = HoverBackend::new();
    blocked.slow_front = Some((220, 310));
    let row = hover_row(&mut blocked, "W03");
    assert_eq!(
        check(&row, "hover expansion completion"),
        Verdict::NotTested,
        "{}",
        row.json()
    );
    assert_eq!(
        check(&row, "hover expands without focus"),
        Verdict::NotTested
    );
    assert_ne!(aggregate(&row.checks), Verdict::Fail, "{}", row.json());
    // The front app unreadable from 150 ms: what follows is not observed, not failed.
    let mut unreadable = HoverBackend::new();
    unreadable.front_error_at = Some(150);
    let row = hover_row(&mut unreadable, "W04");
    assert_eq!(aggregate(&row.checks), Verdict::NotTested, "{}", row.json());
    // Every early trial too slow to resolve: three trials, then NOT_TESTED, and nothing fails.
    let mut always_slow = HoverBackend::new();
    always_slow.slow_each_entry = Some((20, 90));
    let row = hover_row(&mut always_slow, "W03");
    assert_eq!(check(&row, "early leave cancels"), Verdict::NotTested);
    assert!(row.json().contains("trial 3:"), "{}", row.json());
    assert_ne!(aggregate(&row.checks), Verdict::Fail, "{}", row.json());
    // Hidden in place at 530-540 ms after leaving, the collapse starting only at 580 ms: the
    // flicker is no onset, and the window still in place at 570 ms is late.
    let mut flicker = HoverBackend::new();
    flicker.leave_hidden = Some((530, 540));
    flicker.leave_at = 580;
    let row = hover_row(&mut flicker, "W03");
    assert_eq!(
        check(&row, "leave movement onset"),
        Verdict::Fail,
        "{}",
        row.json()
    );
}

#[test]
fn ax_only_unsupported_and_no_value_mean_absent() {
    use super::observation::ax_attribute_present;
    assert_eq!(ax_attribute_present(0), Ok(true));
    for status in [-25205, -25212] {
        assert_eq!(ax_attribute_present(status), Ok(false));
    }
    for status in [-25204, -25202, -25208, -25200, 1, -1] {
        assert!(
            ax_attribute_present(status)
                .unwrap_err()
                .contains(&status.to_string())
        );
    }
}

#[test]
fn transition_brackets_reject_early_late_and_coarse_samples() {
    use super::observation::{Bracket, transition};
    for target in [100, 500] {
        let judge = |old_ms, new_ms| transition(Bracket { old_ms, new_ms }, target);
        assert_eq!(judge(target - 10, target + 10), Verdict::Pass);
        assert_eq!(judge(target + 50, target + 70), Verdict::Pass);
        assert_eq!(judge(target - 40, target - 20), Verdict::Fail);
        assert_eq!(judge(target + 80, target + 100), Verdict::Fail);
        assert_eq!(judge(target - 10, target + 51), Verdict::NotTested);
        assert_eq!(judge(target - 20, target), Verdict::NotTested);
        assert_eq!(judge(target + 60, target + 80), Verdict::NotTested);
        assert_eq!(judge(target + 10, target), Verdict::NotTested);
    }
}

#[test]
fn three_hundred_ms_dwell_and_four_hundred_ms_leave_cannot_pass() {
    let mut late = HoverBackend::new();
    late.expand_at = 300;
    assert_ne!(
        aggregate(&hover_row(&mut late, "W03").checks),
        Verdict::Pass
    );
    let mut early = HoverBackend::new();
    early.leave_at = 400;
    assert_eq!(
        aggregate(&hover_row(&mut early, "W04").checks),
        Verdict::Fail
    );
}

struct SaveBackend {
    now: u64,
    saved_at: u64,
    read_delay: u64,
    command_delay: u64,
}
impl Backend for SaveBackend {
    fn now(&self) -> u64 {
        self.now
    }
    fn perform(&mut self, op: Op) -> Result<Value, String> {
        Ok(match op {
            Op::Text(_) => {
                self.now = 80;
                Value::Time(0)
            }
            Op::Chord(keys) => {
                assert_eq!(keys, [CMD, 1]);
                self.now += self.command_delay;
                Value::Unit
            }
            Op::NoteNow => {
                self.now += self.read_delay;
                Value::Text(
                    if self.now >= self.saved_at {
                        "alpha beta"
                    } else {
                        "old"
                    }
                    .into(),
                )
            }
            Op::Wait(ms) => {
                self.now += ms;
                Value::Unit
            }
            _ => panic!("manual save must not use quiet_note: {op:?}"),
        })
    }
}
#[test]
fn manual_save_pass_requires_direct_bytes_before_550ms_not_autosave() {
    for (saved_at, read_delay, command_delay, expected) in [
        (150, 0, 60, Verdict::Pass),
        (550, 0, 60, Verdict::Pass),
        (551, 0, 60, Verdict::NotTested),
        (650, 0, 60, Verdict::NotTested),
        (150, 600, 60, Verdict::NotTested),
        (150, 0, 600, Verdict::NotTested),
    ] {
        let mut fake = SaveBackend {
            now: 0,
            saved_at,
            read_delay,
            command_delay,
        };
        let mut row = Row::new("K02");
        super::editing::manual_save(
            &mut Driver {
                backend: &mut fake,
                row: &mut row,
                pid: 42,
            },
            "alpha beta",
        )
        .unwrap();
        assert_eq!(aggregate(&row.checks), expected, "{}", row.json());
        assert!(row.json().contains("last_key=0ms"));
        assert!(row.json().contains("deadline=550ms"));
    }
}

/// The dialog is seen open once; every later inspection is incomplete.
struct PanelFailure {
    seen_open: bool,
}
impl Backend for PanelFailure {
    fn now(&self) -> u64 {
        0
    }
    fn perform(&mut self, op: Op) -> Result<Value, String> {
        match op {
            Op::PanelOpen if !self.seen_open => {
                self.seen_open = true;
                Ok(Value::Bool(true))
            }
            Op::PanelOpen => Err("AX attribute error -25202; inspection incomplete".into()),
            Op::Note => Ok(Value::Text("# Export\nplain bytes".into())),
            _ => Ok(Value::Unit),
        }
    }
}
#[test]
fn incomplete_cancel_tree_never_records_panel_closed_pass() {
    let mut row = Row::new("E01");
    let error = super::editing::export(&mut Driver {
        backend: &mut PanelFailure { seen_open: false },
        row: &mut row,
        pid: 42,
    })
    .unwrap_err();
    assert!(error.contains("-25202"));
    assert!(
        !row.checks
            .iter()
            .any(|c| c.name == "cancel dismisses save panel" && c.verdict == Verdict::Pass)
    );
}
/// A dialog whose presentation holds the main thread: for `busy_ms` after each Cmd+Shift+S every
/// inspection and button press ends as CannotComplete, as seen in the VM; then it is open until
/// a button ends it.
struct SlowPanel {
    clock: u64,
    busy_ms: u64,
    busy_until: u64,
    open: bool,
    exported: bool,
}
impl Backend for SlowPanel {
    fn now(&self) -> u64 {
        self.clock
    }
    fn perform(&mut self, op: Op) -> Result<Value, String> {
        let busy = self.clock < self.busy_until;
        let cannot = || Err("AX attribute error -25204; inspection incomplete".into());
        match op {
            Op::Wait(ms) => {
                self.clock += ms;
                Ok(Value::Unit)
            }
            Op::Chord(keys) if keys == [CMD, super::input::SHIFT, 1] => {
                self.open = true;
                self.busy_until = self.clock + self.busy_ms;
                Ok(Value::Unit)
            }
            Op::PanelOpen if busy => cannot(),
            Op::PanelOpen => Ok(Value::Bool(self.open)),
            Op::Panel(_) if busy => cannot(),
            Op::Panel(confirm) => {
                self.open = false;
                self.exported |= confirm;
                Ok(Value::Text(String::new()))
            }
            Op::Export if self.exported => Ok(Value::Text("# Export\nplain bytes".into())),
            Op::Export => Ok(Value::Text("<absent>".into())),
            Op::Note => Ok(Value::Text("# Export\nplain bytes".into())),
            _ => Ok(Value::Unit),
        }
    }
}
fn slow_export(busy_ms: u64) -> Row {
    let mut row = Row::new("E01");
    let mut backend = SlowPanel {
        clock: 0,
        busy_ms,
        busy_until: 0,
        open: false,
        exported: false,
    };
    super::editing::export(&mut Driver {
        backend: &mut backend,
        row: &mut row,
        pid: 42,
    })
    .unwrap();
    row
}
#[test]
fn export_waits_for_a_slow_dialog_before_pressing_its_buttons() {
    let row = slow_export(1300);
    assert_eq!(aggregate(&row.checks), Verdict::Pass, "{}", row.json());
    for name in [
        "save panel opens",
        "save panel opens again",
        "export exact bytes",
    ] {
        assert!(
            row.checks
                .iter()
                .any(|c| c.name == name && c.verdict == Verdict::Pass),
            "{name}: {}",
            row.json()
        );
    }
}
#[test]
fn export_fails_when_the_dialog_is_never_seen_open() {
    let row = slow_export(10_000);
    assert_eq!(aggregate(&row.checks), Verdict::Fail, "{}", row.json());
    assert!(
        !row.checks
            .iter()
            .any(|c| c.name == "cancel dismisses save panel")
    );
}
#[test]
fn launch_discovery_ignores_off_screen_windows_but_lookup_by_number_does_not() {
    let shown = window(rect(252.0, 30.0, 520.0, 680.0), 0);
    let mut never_shown = window(rect(0.0, 268.0, 500.0, 500.0), 0);
    never_shown.id = 8;
    never_shown.onscreen = false;
    let both = || vec![never_shown.clone(), shown.clone()];
    assert_eq!(select_window(both(), None).map(|w| w.id), Ok(7));
    assert_eq!(select_window(both(), Some(8)).map(|w| w.id), Ok(8));
    assert_eq!(select_window(both(), Some(9)).map(|w| w.id), Err(0));
    let mut hidden = shown.clone();
    hidden.onscreen = false;
    assert_eq!(
        select_window(vec![hidden.clone()], None).map(|w| w.id),
        Err(0)
    );
    assert_eq!(select_window(vec![hidden], Some(7)).map(|w| w.id), Ok(7));
    let mut twin = shown.clone();
    twin.id = 9;
    assert_eq!(select_window(vec![shown, twin], None).map(|w| w.id), Err(2));
}

#[test]
fn ax_actions_tolerate_only_cannot_complete_as_unconfirmed() {
    use super::observation::ax_action_confirmed;
    assert_eq!(ax_action_confirmed("AXPress", 0), Ok(true));
    assert_eq!(ax_action_confirmed("AXShowMenu", -25204), Ok(false));
    for status in [-25205, -25202, -25200, 1] {
        let err = ax_action_confirmed("AXPress", status).unwrap_err();
        assert!(err.contains("AXPress") && err.contains(&status.to_string()));
    }
}

#[test]
fn lsappinfo_asn_and_type_parse_the_macos_27_output() {
    use super::observation::{lsappinfo_asn, lsappinfo_type};
    assert_eq!(
        lsappinfo_asn("ASN:0x0-0x9e09e-\"StickyMD\":\n").as_deref(),
        Ok("ASN:0x0-0x9e09e")
    );
    for bad in [
        "",
        "ASN:0x0-0x9e09e-\"a\":\nASN:0x0-0x1-\"b\":",
        "ASN:0x0",
        "0x0-0x1",
        "ASN:0xg-0x1-\"a\"",
    ] {
        assert!(lsappinfo_asn(bad).is_err(), "{bad:?}");
    }
    let info = "[ NULL ]  [ NULL ]  \n    bundleID=[ NULL ] \n !cgsConnection !signalled type=\"UIElement\" flavor=[ NULL ]  Version=[ NULL ]\n";
    assert_eq!(lsappinfo_type(info), Some("UIElement"));
    assert_eq!(
        lsappinfo_type("filetype=\"APPL\" type=\"Foreground\""),
        Some("Foreground")
    );
    assert_eq!(lsappinfo_type(""), None);
}

#[test]
fn hover_completion_is_dated_by_the_rest_position_not_by_entering_the_screen() {
    // On screen 16 DIP short of rest for 200 ms after the animation: not finished in time.
    let mut lagging = HoverBackend::new();
    lagging.short_of_rest = 200;
    let row = hover_row(&mut lagging, "W03");
    let completion = row
        .checks
        .iter()
        .find(|c| c.name == "hover expansion completion")
        .unwrap();
    assert_eq!(completion.verdict, Verdict::Fail, "{}", row.json());
    // A brief settle within the deadline still passes.
    let mut settling = HoverBackend::new();
    settling.short_of_rest = 20;
    assert_eq!(
        aggregate(&hover_row(&mut settling, "W03").checks),
        Verdict::Pass
    );
}

#[test]
fn rest_dating_and_completion_verdicts() {
    use super::observation::{completion_verdict, rest_index};
    assert_eq!(rest_index(&[-200, 12, 28, 12, 28, 28]), Some(4));
    assert_eq!(rest_index(&[1, 1]), Some(0));
    assert_eq!(rest_index::<i32>(&[]), None);
    // (end state, last request, first rest return, moving request, early, late deadline)
    let judge = |end, first, moving| completion_verdict(end, 500, first, moving, 304, 320);
    assert_eq!(judge(true, 300, Some(280)), Verdict::Pass);
    assert_eq!(judge(true, 304, Some(284)), Verdict::Pass);
    // Moving at 300, at rest by 320: arriving at 303 fits both, so it cannot tell.
    assert_eq!(judge(true, 320, Some(300)), Verdict::NotTested);
    assert_eq!(judge(true, 380, Some(290)), Verdict::NotTested);
    // Still moving when requested at the latest deadline or after: late.
    assert_eq!(judge(true, 380, Some(320)), Verdict::Fail);
    assert_eq!(judge(true, 340, None), Verdict::NotTested);
    // A wrong rest fails only when observed at or after the latest deadline.
    assert_eq!(judge(false, 200, Some(180)), Verdict::Fail);
    assert_eq!(
        completion_verdict(false, 310, 200, Some(180), 304, 320),
        Verdict::NotTested
    );
}

/// A window whose observation takes 30 ms and which goes off screen at `hidden_at`.
struct SlowWindow {
    now: u64,
    hidden_at: u64,
}
impl Backend for SlowWindow {
    fn now(&self) -> u64 {
        self.now
    }
    fn perform(&mut self, op: Op) -> Result<Value, String> {
        Ok(match op {
            Op::Wait(ms) => {
                self.now += ms;
                Value::Unit
            }
            Op::Window => {
                self.now += 30;
                let mut w = window(rect(0.0, 0.0, 520.0, 480.0), 0);
                w.onscreen = self.now < self.hidden_at;
                Value::Window(w)
            }
            _ => return Err(format!("unexpected fake operation {op:?}")),
        })
    }
}

#[test]
fn a_success_first_seen_after_the_deadline_is_late() {
    for (hidden_at, met) in [(100, true), (1_950, true), (1_990, false), (5_000, false)] {
        let mut fake = SlowWindow { now: 0, hidden_at };
        let mut row = Row::new("W07");
        let mut d = Driver {
            backend: &mut fake,
            row: &mut row,
            pid: 42,
        };
        let (_, elapsed, observed) = d.until(2_000, |w| !w.onscreen).unwrap();
        assert_eq!(
            observed, met,
            "hidden at {hidden_at}, returned at {elapsed}"
        );
    }
}

/// R03 from observations: the bad samples a check must refuse, and what it cannot tell.
#[test]
fn translocation_checks_refuse_bad_samples() {
    use super::translocation::{
        EXIT_ONE, GENERIC_ADVICE, NOTE_ATTRIB, NOTE_EXTEND, NOTE_REVOKE, NOTE_WRITE, Observed,
        TRANSLOCATED_ADVICE, Watched, judge,
    };
    let plist = "StickyMD.app/Contents/Info.plist";
    let tree = vec![
        "dir . mtime=1".to_owned(),
        "dir StickyMD.app mtime=2".to_owned(),
        "dir StickyMD.app/Contents mtime=3".to_owned(),
        format!("file {plist} bytes=10 sha256=ab mtime=4"),
    ];
    let entries = [".", "StickyMD.app", "StickyMD.app/Contents", plist].map(str::to_owned);
    let watched = |events: Vec<(String, u32)>| {
        Some(Watched {
            entries: entries.to_vec(),
            events,
        })
    };
    let good = || Observed {
        zip: None,
        quarantine: Some("00c3;6afec4a5;Safari;".into()),
        path: Some(
            "/private/var/folders/x/T/AppTranslocation/U/d/StickyMD.app/Contents/MacOS/StickyMD"
                .into(),
        ),
        mount: Some(
            "/tmp/download/StickyMD.app on /private/var/folders/x/T/AppTranslocation/U \
             (nullfs, local, nodev, nosuid, read-only, nobrowse, mounted by admin)"
                .into(),
        ),
        exited_early: false,
        window: Some("Window { id: 64, layer: 8, onscreen: true }".into()),
        stderr: "当前目录不可写，请先用访达把 StickyMD.app 移到别处。\n\ncannot create a file: \
                 Read-only file system (os error 30)\n"
            .into(),
        stderr_unreadable: None,
        unanswered: None,
        exited: Some(true),
        termination: Some(EXIT_ONE.into()),
        before: tree.clone(),
        after: tree.clone(),
        // Gatekeeper's own quarantine attribute change is not a write.
        watch: watched(vec![("StickyMD.app".into(), NOTE_ATTRIB)]),
    };
    let verdict = |o: Observed| {
        let mut row = Row::new("R03");
        judge(&o, &mut row);
        (aggregate(&row.checks), row)
    };
    // Every observation holds: equal snapshots and a watch with no write (USER 2026-11-24, A).
    let (v, row) = verdict(good());
    assert_eq!(v, Verdict::Pass, "{}", row.json());
    assert_eq!(row.checks.len(), 6);
    let unwritten = row
        .checks
        .iter()
        .find(|c| c.name == super::translocation::UNWRITTEN)
        .unwrap();
    assert!(
        unwritten
            .observation
            .contains("attributes changed: [\"StickyMD.app: ATTRIB\"]"),
        "{}",
        unwritten.observation
    );
    // Bad samples: gone before a prompt, a prompt about something else, the advice for any other
    // unwritable folder, still running after Return, another exit, a folder written beside the
    // package, and writes the snapshots cannot see: a file rewritten in place (its bytes and
    // time restored), a file grown and shrunk back, a file created and removed in the folder.
    for bad in [
        Observed {
            window: None,
            exited_early: true,
            exited: None,
            ..good()
        },
        Observed {
            stderr: "事件循环异常结束".into(),
            exited: None,
            ..good()
        },
        Observed {
            stderr: format!(
                "当前目录不可写，{GENERIC_ADVICE}。\n\ncannot create a file: Read-only file system \
                 (os error 30)\n"
            ),
            ..good()
        },
        Observed {
            termination: Some("(2, 9, 9)".into()),
            ..good()
        },
        Observed {
            after: [tree.clone(), vec!["dir note mtime=9".to_owned()]].concat(),
            ..good()
        },
        Observed {
            watch: watched(vec![(plist.into(), NOTE_WRITE | NOTE_ATTRIB)]),
            ..good()
        },
        Observed {
            watch: watched(vec![(plist.into(), NOTE_EXTEND)]),
            ..good()
        },
        Observed {
            watch: watched(vec![(".".into(), NOTE_WRITE | NOTE_ATTRIB)]),
            ..good()
        },
        // A write and a revoked entry: the write is still observed.
        Observed {
            watch: watched(vec![(".".into(), NOTE_REVOKE), (plist.into(), NOTE_WRITE)]),
            ..good()
        },
        // Written through the watch while stuck, snapshots equal: the driver ended it.
        Observed {
            exited: Some(false),
            termination: None,
            watch: watched(vec![(plist.into(), NOTE_WRITE)]),
            ..good()
        },
        // Written beside the package and then stuck: the driver ended it (review round 3).
        Observed {
            exited: Some(false),
            termination: None,
            after: vec!["StickyMD.app".into(), "StickyMD.zip".into(), "note".into()],
            ..good()
        },
    ] {
        let (v, row) = verdict(bad);
        assert_eq!(v, Verdict::Fail, "{}", row.json());
    }
    // Cannot tell: no preset attribute, no translocation, no prompt window while running. Each
    // sample leaves the check it targets untested, which a good run passes (review round 3: the
    // whole row is untested anyway while "nothing written" is).
    use super::translocation::{EXITS, PRESET, PROMPTED, STATUS, TRANSLOCATED};
    let (_, good_row) = verdict(good());
    for target in [PRESET, TRANSLOCATED, PROMPTED, EXITS, STATUS] {
        assert!(
            good_row
                .checks
                .iter()
                .any(|c| c.name == target && c.verdict == Verdict::Pass),
            "{target}"
        );
    }
    for (unknown, target) in [
        (
            Observed {
                quarantine: Some("0083;6afec4a5;Safari;".into()),
                ..good()
            },
            PRESET,
        ),
        (
            Observed {
                path: Some("/tmp/download/StickyMD.app/Contents/MacOS/StickyMD".into()),
                ..good()
            },
            TRANSLOCATED,
        ),
        (
            Observed {
                mount: None,
                ..good()
            },
            TRANSLOCATED,
        ),
        (
            Observed {
                window: None,
                exited: None,
                ..good()
            },
            PROMPTED,
        ),
        // The prompt in words this driver does not know.
        (
            Observed {
                stderr: "当前目录不可写，请换个地方。\n\ncannot create a file: Read-only file system \
                         (os error 30)\n"
                    .into(),
                ..good()
            },
            PROMPTED,
        ),
        // Seen but not answered: the process went, or the window did, before Return.
        (
            Observed {
                unanswered: Some("the process ended before Return was posted".into()),
                exited: None,
                termination: None,
                ..good()
            },
            EXITS,
        ),
        // Still running after Return: with no receipt of which window took the key, not a
        // refusal to exit.
        (
            Observed {
                exited: Some(false),
                termination: None,
                ..good()
            },
            EXITS,
        ),
        // A process outside this copy's translocation mount is not the one started.
        (
            Observed {
                path: Some(
                    "/private/var/folders/x/T/AppTranslocation/OTHER/d/StickyMD.app/Contents/MacOS/StickyMD"
                        .into(),
                ),
                ..good()
            },
            TRANSLOCATED,
        ),
        // The prompt window seen but its stderr unreadable: its words are unknown.
        (
            Observed {
                stderr: String::new(),
                stderr_unreadable: Some("stderr.log: permission denied".into()),
                ..good()
            },
            PROMPTED,
        ),
        // Answered and gone, but no termination record in the log.
        (
            Observed {
                termination: None,
                ..good()
            },
            STATUS,
        ),
        // No write seen, but the product did not end by itself.
        (
            Observed {
                exited: Some(false),
                termination: None,
                ..good()
            },
            super::translocation::UNWRITTEN,
        ),
        // No watch read; a watch that did not cover what the snapshot lists; an entry revoked.
        (
            Observed {
                watch: None,
                ..good()
            },
            super::translocation::UNWRITTEN,
        ),
        (
            Observed {
                watch: Some(Watched {
                    entries: entries[..3].to_vec(),
                    events: Vec::new(),
                }),
                ..good()
            },
            super::translocation::UNWRITTEN,
        ),
        (
            Observed {
                watch: watched(vec![(plist.into(), NOTE_REVOKE)]),
                ..good()
            },
            super::translocation::UNWRITTEN,
        ),
    ] {
        let (v, row) = verdict(unknown);
        assert_eq!(v, Verdict::NotTested, "{}", row.json());
        let check = row.checks.iter().find(|c| c.name == target).unwrap();
        assert_eq!(check.verdict, Verdict::NotTested, "{target}: {}", row.json());
    }
    // The advices are the product's own words (main.rs now, paths.rs after Phase 18), and the
    // generic one never reads as the move advice.
    let product = [
        include_str!("../../../../apps/stickymd-mac/src/main.rs"),
        include_str!("../../../../apps/stickymd-mac/src/paths.rs"),
    ]
    .concat();
    assert!(
        TRANSLOCATED_ADVICE
            .iter()
            .any(|a| product.contains("当前目录不可写，") && product.contains(a)),
        "the product no longer gives a known move advice; update TRANSLOCATED_ADVICE"
    );
    assert!(product.contains(&format!("当前目录不可写，{GENERIC_ADVICE}。")));
    assert!(
        TRANSLOCATED_ADVICE
            .iter()
            .all(|a| !GENERIC_ADVICE.contains(a))
    );
    // Phase 18's wording passes too.
    let (v, row) = verdict(Observed {
        stderr:
            "当前目录不可写，请先把 StickyMD.app 拖进\u{201c}应用程序\u{201d}，或把整个 StickyMD \
                 文件夹移到别处。\n\ncannot create a file: Read-only file system (os error 30)\n"
                .into(),
        ..good()
    });
    assert_eq!(v, Verdict::Pass, "{}", row.json());
    assert!(
        row.checks
            .iter()
            .any(|c| c.name == super::translocation::PROMPTED && c.verdict == Verdict::Pass)
    );
}

/// R03's facts are what its rule reads, written and read back unchanged; read strictly; and
/// judged again to the same checks.
#[test]
fn r03_facts_round_trip_and_rejudge_alike() {
    use super::translocation::{NOTE_ATTRIB, NOTE_WRITE, facts, judge};
    use crate::release::json;
    for events in [
        vec![],
        vec![("StickyMD.app", NOTE_ATTRIB)],
        vec![(".", NOTE_WRITE)],
    ] {
        for zip in [None, Some("f".repeat(64))] {
            let text = facts::sample(zip.as_deref(), &events);
            let observed = facts::parse(&json::parse(&text).unwrap()).unwrap();
            assert_eq!(facts::json(&observed), text);
            let mut row = Row::new("R03");
            judge(&observed, &mut row);
            let rejudged = facts::rejudge(&json::parse(&text).unwrap()).unwrap();
            assert_eq!(rejudged.zip, zip);
            assert_eq!(
                rejudged.checks,
                row.checks
                    .iter()
                    .map(|c| (
                        c.name.clone(),
                        c.verdict.name().to_owned(),
                        c.observation.clone()
                    ))
                    .collect::<Vec<_>>()
            );
        }
    }
    // A watch without its events, an event without its flags, a flag beyond 32 bits.
    let text = facts::sample(None, &[(".", NOTE_WRITE)]);
    for (from, to) in [
        (",\"events\":[{\"entry\":\".\",\"flags\":2}]", ""),
        (",\"flags\":2", ""),
        ("\"flags\":2", "\"flags\":4294967296"),
    ] {
        assert!(text.contains(from), "{from}");
        let bad = text.replace(from, to);
        assert!(facts::parse(&json::parse(&bad).unwrap()).is_err(), "{bad}");
    }
}

fn image(width: usize, height: usize, fill: [u8; 3]) -> super::pixels::Image {
    super::pixels::Image {
        width,
        height,
        rgba: std::iter::repeat_n([fill[0], fill[1], fill[2], 255], width * height)
            .flatten()
            .collect(),
    }
}
fn paint(
    mut i: super::pixels::Image,
    xs: std::ops::Range<usize>,
    ys: std::ops::Range<usize>,
    c: [u8; 3],
) -> super::pixels::Image {
    for y in ys {
        for x in xs.clone() {
            let k = (y * i.width + x) * 4;
            i.rgba[k..k + 3].copy_from_slice(&c);
        }
    }
    i
}

/// W02: a strip that nothing covers and that changes the screen passes; a covered strip cannot
/// be judged; a strip that changes nothing fails.
#[test]
fn strip_visibility_needs_an_uncovered_strip_that_changes_the_screen() {
    use super::pixels::strip_visible;
    let strip = image(1040, 4, [248, 246, 239]);
    let desktop = image(1040, 4, [40, 70, 120]);
    assert_eq!(strip_visible(&strip, &strip, &desktop).0, Verdict::Pass);
    let menu_over = paint(strip.clone(), 0..1040, 0..4, [230, 230, 230]);
    assert_eq!(
        strip_visible(&menu_over, &strip, &desktop).0,
        Verdict::NotTested
    );
    assert_eq!(strip_visible(&desktop, &desktop, &desktop).0, Verdict::Fail);
    assert_eq!(
        strip_visible(&strip, &image(10, 4, [0, 0, 0]), &desktop).0,
        Verdict::NotTested
    );
}

/// Frames every `every` ms over 3.6 s: `shown(t)` decides which image a frame at `t` is.
fn frames_of(
    every: u64,
    shown: impl Fn(u64) -> super::pixels::Image,
) -> Vec<(u64, super::pixels::Image)> {
    (0..3600 / every)
        .map(|k| (k * every, shown(k * every)))
        .collect()
}

/// W10: an inverse caret alternating every ~550 ms passes; a steady caret, no caret, a caret in a
/// fixed colour, or one blinking far too fast fails; other changes or long frame gaps cannot tell.
#[test]
fn caret_blink_judge_refuses_bad_samples() {
    use super::pixels::caret_blinks;
    let off = image(40, 60, [248, 246, 239]);
    let on = paint(off.clone(), 4..6, 3..43, [7, 9, 16]);
    let fixed = paint(off.clone(), 4..6, 3..43, [32, 79, 143]);
    // A whole layer flipping to its inverse: alternating, inverse, but not a caret.
    let flipped = paint(off.clone(), 0..40, 0..60, [7, 9, 16]);
    let alternate = |half: u64, a: &super::pixels::Image, b: &super::pixels::Image| {
        let (a, b) = (a.clone(), b.clone());
        move |t: u64| {
            if (t / half).is_multiple_of(2) {
                a.clone()
            } else {
                b.clone()
            }
        }
    };
    assert_eq!(
        caret_blinks(&frames_of(60, alternate(550, &on, &off))).0,
        Verdict::Pass
    );
    // Steady, absent, fixed-colour, far too fast.
    assert_eq!(
        caret_blinks(&frames_of(60, |_| on.clone())).0,
        Verdict::Fail
    );
    assert_eq!(
        caret_blinks(&frames_of(60, |_| off.clone())).0,
        Verdict::Fail
    );
    assert_eq!(
        caret_blinks(&frames_of(60, alternate(550, &fixed, &off))).0,
        Verdict::Fail
    );
    assert_eq!(
        caret_blinks(&frames_of(60, alternate(550, &flipped, &off))).0,
        Verdict::Fail
    );
    assert_eq!(
        caret_blinks(&frames_of(40, alternate(120, &on, &off))).0,
        Verdict::Fail
    );
    // Something else changing (three states in one pixel), or frames too far apart.
    let blink = alternate(550, &on, &off);
    let noisy = |t: u64| paint(blink(t), 15..16, 15..16, [((t / 60) % 3 * 100) as u8, 0, 0]);
    assert_eq!(caret_blinks(&frames_of(60, noisy)).0, Verdict::NotTested);
    assert_eq!(
        caret_blinks(&frames_of(400, alternate(550, &on, &off))).0,
        Verdict::NotTested
    );
}

/// W10: the counters must be live (an edit presents) and a snapshot must present nothing itself
/// before an idle stretch is judged.
#[test]
fn idle_counters_need_live_counters_and_quiet_snapshots() {
    use super::pixels::idle_presents_nothing;
    let a = [10, 2, 1, 0];
    let b = [11, 2, 1, 0];
    assert_eq!(
        idle_presents_nothing((a, b), (b, b), (b, b)).0,
        Verdict::Pass
    );
    assert_eq!(
        idle_presents_nothing((a, b), (b, b), (b, [12, 2, 1, 0])).0,
        Verdict::Fail
    );
    assert_eq!(
        idle_presents_nothing((a, b), (b, b), (b, [11, 3, 1, 0])).0,
        Verdict::Fail
    );
    assert_eq!(
        idle_presents_nothing((a, b), (b, b), (b, [11, 2, 1, 1])).0,
        Verdict::Fail
    );
    assert_eq!(
        idle_presents_nothing((a, a), (a, a), (a, a)).0,
        Verdict::NotTested
    );
    assert_eq!(
        idle_presents_nothing((a, b), (b, [12, 2, 1, 0]), (b, b)).0,
        Verdict::NotTested
    );
}

/// I03: the control must show the accent menu (a new window of TextEdit's own process); then a
/// new window of StickyMD's own process, a stray or missing character, or no repeat fails, and a
/// foreign window cannot be judged.
#[test]
fn press_and_hold_judge_refuses_bad_samples() {
    use super::hold::{appeared, judge};
    let (textedit, sticky) = (801, 42);
    let panel = vec![(67, textedit, "TextEdit".to_string())];
    let verdicts = |control: &[(u32, u32, String)], product: &[(u32, u32, String)], added: &str| {
        let mut row = Row::new("I03");
        judge(
            (control, textedit, "á"),
            (Ok(product), sticky),
            added,
            &mut row,
        );
        row.checks.iter().map(|c| c.verdict).collect::<Vec<_>>()
    };
    let aaa = "aaaaaaaaaaaaaaa2";
    assert_eq!(verdicts(&panel, &[], aaa), [Verdict::Pass, Verdict::Pass]);
    // The control opened no menu (none, or only another process's window): nothing is judged.
    assert_eq!(
        verdicts(&[], &[], aaa),
        [Verdict::NotTested, Verdict::NotTested]
    );
    let elsewhere = vec![(68, 330, "NotificationCenter".to_string())];
    assert_eq!(verdicts(&elsewhere, &[], aaa)[0], Verdict::NotTested);
    // A new window of StickyMD's own process is its accent menu; another process's cannot tell.
    let menu = vec![(902, sticky, "StickyMD".to_string())];
    assert_eq!(verdicts(&panel, &menu, aaa)[0], Verdict::Fail);
    let other = vec![(903, 77, "NotificationCenter".to_string())];
    assert_eq!(verdicts(&panel, &other, aaa)[0], Verdict::NotTested);
    // The menu seen but no accent picked by the digit: the control does not hold.
    let mut row = Row::new("I03");
    judge((&panel, textedit, "a2"), (Ok(&[]), sticky), aaa, &mut row);
    assert!(row.checks.iter().all(|c| c.verdict == Verdict::NotTested));
    // No window list while held: the menu check cannot tell.
    let mut row = Row::new("I03");
    judge(
        (&panel, textedit, "á"),
        (Err("unavailable"), sticky),
        aaa,
        &mut row,
    );
    assert_eq!(row.checks[0].verdict, Verdict::NotTested);
    // Too few or too many held letters (one per key-down: 15 or 16 here).
    for bad in [
        "aaaá2",
        "aaaa",
        "a2",
        "aab2",
        "2",
        "aaa22",
        "aaaaaaaaaaaaa2",
        &"a".repeat(17),
    ] {
        assert_eq!(verdicts(&panel, &[], bad)[1], Verdict::Fail, "{bad}");
    }
    let before = vec![(1, 1, "Finder".to_string())];
    let after = vec![(1, 1, "Finder".to_string()), (5, 9, "X".to_string())];
    assert_eq!(appeared(&before, &after), vec![(5, 9, "X".to_string())]);
}

/// W12's logout cases: the facts the system log gives (real lines of the p4 VM run), and each
/// case's bad samples.
#[test]
fn logout_cases_judge_the_log_and_the_note() {
    use super::logout::check::{
        Facts, LOST, Logged, NOTE, Note, SAVED, TYPED, UNSAVED, from_log, judge, log_time,
    };
    assert_eq!(
        log_time("2026-11-19 12:51:39.066088+0000  localhost x"),
        Some(1_795_092_699_066_088)
    );
    assert_eq!(log_time("1970-01-01 00:00:00+0000 x"), Some(0));
    assert_eq!(
        log_time("2000-03-01 08:00:00.5+0800 x"),
        Some(951_868_800_500_000)
    );
    assert_eq!(log_time("1969-12-31 23:59:59.999999+0000 x"), Some(-1));
    for bad in [
        "08:55:11.718 localhost",
        "2026-13-01 00:00:00+0000 x",
        "2026-11-19 24:00:00+0000 x",
        "2026-11-19 12:51:39.066088 x",
        "2026-11-19 12:51:39.1234567890+0000 x",
        "2026-11-19-1 12:51:39+0000 x",
        "",
    ] {
        assert_eq!(log_time(bad), None, "{bad}");
    }
    let quit = "2026-11-19 12:51:39.066088+0000  localhost StickyMD[991]: (AppKit) [com.apple.AppKit:Application] Handling Quit AppleEvent";
    let later = "2026-11-19 12:51:39.066255+0000  localhost StickyMD[991]: (AppKit) [com.apple.AppKit:Application] applicationShouldTerminate: NSTerminateLater";
    let yes = "2026-11-19 12:51:39.198758+0000  localhost StickyMD[991]: (AppKit) [com.apple.AppKit:Application] replyToApplicationShouldTerminate:YES";
    let ended = "2026-11-19 12:51:39.300215+0000  localhost runningboardd[175]: (RunningBoard) [com.apple.runningboard:ttl] [app<application.io.github.develata.stickymd.516736.516741.2DBA30AC-C083-4C8B-B538-31068C90C401(501)>:991] termination reported by launchd (0, 0, 0)";
    let other = "2026-11-19 12:51:39.100000+0000  localhost StickyMD[9991]: (AppKit) [com.apple.AppKit:Application] replyToApplicationShouldTerminate:NO";
    let early = "2026-11-19 12:51:38.000000+0000  localhost StickyMD[991]: (AppKit) [com.apple.AppKit:Application] applicationShouldTerminate: NSTerminateNow";
    // A quit request before this run asked for the logout is not this one.
    let old_quit = "2026-11-19 12:51:37.000000+0000  localhost StickyMD[991]: (AppKit) [com.apple.AppKit:Application] Handling Quit AppleEvent";
    let mut facts = Facts::default();
    from_log(
        &[old_quit, early, quit, later, other, yes, ended].join("\n"),
        991,
        log_time(quit).unwrap() - 1_000,
        &mut facts,
    );
    assert_eq!(
        facts.quit,
        Some(Logged {
            line: 2,
            at: log_time(quit),
            value: String::new()
        })
    );
    let seen = |l: &Option<Logged>| l.as_ref().map(|l| (l.line, l.value.clone()));
    assert_eq!(
        seen(&facts.decision),
        Some((3, "NSTerminateLater".into())),
        "a decision before the quit event does not answer it"
    );
    assert_eq!(
        seen(&facts.reply),
        Some((5, "YES".into())),
        "another PID's reply is not ours"
    );
    assert_eq!(seen(&facts.termination), Some((6, "(0, 0, 0)".into())));

    let run = |case: &str, f: Facts| {
        let mut row = Row::new("W12-logout");
        judge(case, &f, &mut row);
        aggregate(&row.checks)
    };
    let at = log_time(quit).unwrap();
    let note = |text: &str, modified: i64| -> Result<Note, String> {
        Ok(Note {
            text: text.into(),
            modified,
        })
    };
    let logged = |line: usize, value: &str| {
        Some(Logged {
            line,
            at: Some(at + line as i64 * 100_000),
            value: value.into(),
        })
    };
    let base = || Facts {
        logged_out: true,
        gone: true,
        quit: logged(1, ""),
        ..Facts::default()
    };
    let unread = || -> Result<Note, String> { Err("denied".into()) };

    // text: unsaved at the request, saved by it, later then yes, a normal exit.
    let text = || Facts {
        before: note(NOTE, at - 5_000_000),
        after: note(TYPED, at + 100_000),
        decision: logged(2, "NSTerminateLater"),
        reply: logged(3, "YES"),
        termination: logged(4, "(0, 0, 0)"),
        ..base()
    };
    assert_eq!(run("text", text()), Verdict::Pass);
    let samples: Vec<(Facts, Verdict, &str)> = vec![
        (
            Facts {
                after: note(TYPED, at - 1),
                ..text()
            },
            Verdict::NotTested,
            "saved before the request",
        ),
        (
            Facts {
                before: note(TYPED, at - 1),
                ..text()
            },
            Verdict::NotTested,
            "already on disk",
        ),
        (
            Facts {
                before: unread(),
                ..text()
            },
            Verdict::NotTested,
            "before unread",
        ),
        (
            Facts {
                after: unread(),
                ..text()
            },
            Verdict::NotTested,
            "after unread",
        ),
        (
            Facts {
                after: note(NOTE, at - 5_000_000),
                ..text()
            },
            Verdict::Fail,
            "not saved",
        ),
        (
            Facts {
                after: note(&format!("{TYPED}\n"), at + 1),
                ..text()
            },
            Verdict::Fail,
            "other bytes",
        ),
        (
            Facts {
                decision: logged(2, "NSTerminateNow"),
                ..text()
            },
            Verdict::Fail,
            "barrier skipped",
        ),
        (
            Facts {
                reply: logged(3, "NO"),
                ..text()
            },
            Verdict::Fail,
            "refused",
        ),
        (
            Facts {
                decision: logged(5, "NSTerminateLater"),
                ..text()
            },
            Verdict::Fail,
            "answered before the decision",
        ),
        (
            Facts {
                termination: logged(4, "(2, 9, 9)"),
                ..text()
            },
            Verdict::Fail,
            "killed",
        ),
        (
            Facts {
                termination: logged(4, "(2, 15, 15)"),
                ..text()
            },
            Verdict::Fail,
            "SIGTERM default death",
        ),
        (
            Facts {
                termination: None,
                ..text()
            },
            Verdict::NotTested,
            "no termination record",
        ),
        (
            Facts {
                quit: None,
                ..text()
            },
            Verdict::NotTested,
            "no quit event",
        ),
        (
            Facts {
                logged_out: false,
                ..text()
            },
            Verdict::NotTested,
            "no logout",
        ),
    ];
    for (facts, verdict, why) in samples {
        assert_eq!(run("text", facts), verdict, "text: {why}");
    }
    // Saved only after it answered YES; the barrier skipped with the answer missing; a product
    // still running when the cleanup began (its exit may be the cleanup's).
    let written_late = Facts {
        after: note(TYPED, at + 350_000),
        ..text()
    };
    assert_eq!(run("text", written_late), Verdict::Fail, "saved after YES");
    let skipped = Facts {
        decision: logged(2, "NSTerminateNow"),
        reply: None,
        ..text()
    };
    assert_eq!(
        run("text", skipped),
        Verdict::Fail,
        "a seen violation beats a missing fact"
    );
    let lingering = Facts {
        gone: false,
        ..text()
    };
    assert_eq!(
        run("text", lingering),
        Verdict::NotTested,
        "ended by the cleanup"
    );

    // dialog: the request meets the open dialog, the second line is saved by it, nothing lands at
    // the aimed destination, a normal exit.
    let dialog = || Facts {
        before: note(SAVED, at - 3_000_000),
        after: note(&format!("{SAVED}\n{UNSAVED}"), at + 200_000),
        exported: Ok(false),
        decision: logged(2, "NSTerminateCancel"),
        reply: logged(3, "NO"),
        termination: logged(4, "(0, 0, 0)"),
        ..base()
    };
    assert_eq!(run("dialog", dialog()), Verdict::Pass);
    let samples: Vec<(Facts, Verdict, &str)> = vec![
        (
            Facts {
                exported: Ok(true),
                ..dialog()
            },
            Verdict::Fail,
            "exported",
        ),
        (
            Facts {
                exported: Err("denied".into()),
                ..dialog()
            },
            Verdict::NotTested,
            "destination unreadable",
        ),
        (
            Facts {
                decision: logged(2, "NSTerminateLater"),
                ..dialog()
            },
            Verdict::Fail,
            "dialog not taken",
        ),
        (
            Facts {
                after: note(SAVED, at - 3_000_000),
                ..dialog()
            },
            Verdict::Fail,
            "second line lost",
        ),
        (
            Facts {
                before: note(&format!("{SAVED}\n{UNSAVED}"), at - 1),
                ..dialog()
            },
            Verdict::NotTested,
            "second line on disk before",
        ),
        (
            Facts {
                termination: logged(4, "(2, 9, 9)"),
                ..dialog()
            },
            Verdict::Fail,
            "killed",
        ),
        (
            Facts {
                termination: None,
                ..dialog()
            },
            Verdict::NotTested,
            "no termination record",
        ),
    ];
    for (facts, verdict, why) in samples {
        assert_eq!(run("dialog", facts), verdict, "dialog: {why}");
    }

    // fault: later then no, the old bytes untouched, ended by SIGKILL only.
    let fault = || Facts {
        before: note(SAVED, at - 4_000_000),
        after: note(SAVED, at - 4_000_000),
        decision: logged(2, "NSTerminateLater"),
        reply: logged(3, "NO"),
        termination: logged(4, "(2, 9, 9)"),
        ..base()
    };
    assert_eq!(run("fault", fault()), Verdict::Pass);
    let samples: Vec<(Facts, Verdict, &str)> = vec![
        (
            Facts {
                reply: logged(3, "YES"),
                ..fault()
            },
            Verdict::Fail,
            "accepted",
        ),
        (
            Facts {
                after: note(&format!("{SAVED}\n{LOST}"), at + 1),
                ..fault()
            },
            Verdict::Fail,
            "new bytes",
        ),
        (
            Facts {
                after: note(SAVED, at + 1),
                ..fault()
            },
            Verdict::Fail,
            "rewritten",
        ),
        (
            Facts {
                before: note(NOTE, at - 4_000_000),
                ..fault()
            },
            Verdict::NotTested,
            "saved line missing",
        ),
        (
            Facts {
                after: unread(),
                ..fault()
            },
            Verdict::NotTested,
            "after unread",
        ),
        (
            Facts {
                termination: logged(4, "(2, 15, 15)"),
                ..fault()
            },
            Verdict::Fail,
            "SIGTERM default death",
        ),
        (
            Facts {
                termination: logged(4, "(0, 0, 0)"),
                ..fault()
            },
            Verdict::Fail,
            "normal exit",
        ),
        (
            Facts {
                termination: None,
                ..fault()
            },
            Verdict::NotTested,
            "no termination record",
        ),
    ];
    for (facts, verdict, why) in samples {
        assert_eq!(run("fault", facts), verdict, "fault: {why}");
    }
    // The SIGKILL of a product the cleanup found still running may be the cleanup's own.
    let lingering = Facts {
        gone: false,
        ..fault()
    };
    assert_eq!(
        run("fault", lingering),
        Verdict::NotTested,
        "ended by the cleanup"
    );
}

/// The facts the ladder-p8 VM run recorded for the three logout cases (driver `d347b215…`),
/// judged again under the round-8 rules: each still passes.
#[test]
fn ladder_p8_logout_facts_pass_the_stricter_rules() {
    use super::logout::check::{Facts, Logged, Note, judge};
    let note = |text: &str, modified: i64| -> Result<Note, String> {
        Ok(Note {
            text: text.into(),
            modified,
        })
    };
    let logged = |line: usize, at: i64, value: &str| {
        Some(Logged {
            line,
            at: Some(at),
            value: value.into(),
        })
    };
    let run = |case: &str, f: Facts| {
        let mut row = Row::new("W12-logout");
        judge(case, &f, &mut row);
        (aggregate(&row.checks), row.json())
    };
    let text = Facts {
        logged_out: true,
        gone: true,
        before: note("before logout\n", 1795104349726449),
        after: note("typed just before logout", 1795104354804169),
        exported: Err("not read".into()),
        quit: logged(959, 1795104354645310, ""),
        decision: logged(964, 1795104354645429, "NSTerminateLater"),
        reply: logged(1346, 1795104354960946, "YES"),
        termination: logged(2578, 1795104355104607, "(0, 0, 0)"),
    };
    let dialog = Facts {
        logged_out: true,
        gone: true,
        before: note("saved line", 1795104427072172),
        after: note("saved line\nunsaved line", 1795104431870127),
        exported: Ok(false),
        quit: logged(1420, 1795104431611969, ""),
        decision: logged(1429, 1795104431617936, "NSTerminateCancel"),
        reply: logged(1430, 1795104431617937, "NO"),
        termination: logged(2172, 1795104431979765, "(0, 0, 0)"),
    };
    let fault = Facts {
        logged_out: true,
        gone: true,
        before: note("saved line", 1795104509590180),
        after: note("saved line", 1795104509590180),
        exported: Err("not read".into()),
        quit: logged(1150, 1795104513004346, ""),
        decision: logged(1155, 1795104513004483, "NSTerminateLater"),
        reply: logged(1169, 1795104513016993, "NO"),
        termination: logged(3152, 1795104516405512, "(2, 9, 9)"),
    };
    for (case, facts) in [("text", text), ("dialog", dialog), ("fault", fault)] {
        let (verdict, json) = run(case, facts);
        assert_eq!(verdict, Verdict::Pass, "{case}: {json}");
    }
}

#[test]
fn mac_logout_options() {
    use super::logout::{parse, rows};
    let args = |s: &str| s.split_whitespace().map(str::to_owned).collect::<Vec<_>>();
    let o = parse(&args("--app a.app --case dialog --batch b1")).unwrap();
    assert_eq!(
        (o.case.as_str(), o.batch.as_deref()),
        ("dialog", Some("b1"))
    );
    assert_eq!(o.out, None, "one directory of its own per run unless given");
    assert_eq!(
        parse(&args("--app a.app --case text --out d")).unwrap().out,
        Some("d".into())
    );
    assert!(parse(&args("--app a.app --case logout")).is_err());
    assert!(parse(&args("--app a.app")).is_err());
    assert!(parse(&args("--help")).unwrap().help);
    assert_eq!(
        rows(),
        ["W12-logout-text", "W12-logout-dialog", "W12-logout-fault"]
    );
}

/// W02: windows in front that may hide a strip; the Dock's screen-sized overlay does not count.
#[test]
fn only_windows_that_may_paint_over_a_strip_cover_it() {
    use super::visual::covers;
    let rect = |x, y, width, height| Rect {
        origin: Point { x, y },
        size: Size { width, height },
    };
    let screen = rect(0.0, 0.0, 1024.0, 768.0);
    let strip = rect(0.0, 144.0, 2.0, 480.0);
    assert!(!covers(&("Dock".into(), screen, 20), strip, screen));
    // Anything else that meets the strip counts: another owner, another level, a smaller Dock
    // window, a notification.
    assert!(covers(&("Finder".into(), screen, 20), strip, screen));
    assert!(covers(&("Dock".into(), screen, 21), strip, screen));
    assert!(covers(
        &("Dock".into(), rect(0.0, 100.0, 80.0, 600.0), 20),
        strip,
        screen
    ));
    assert!(covers(
        &(
            "Notification Center".into(),
            rect(0.0, 140.0, 360.0, 80.0),
            23
        ),
        strip,
        screen
    ));
    // A window that does not meet the strip does not.
    assert!(!covers(
        &("Finder".into(), rect(100.0, 100.0, 300.0, 300.0), 0),
        strip,
        screen
    ));
}

/// Round 8: a scene below that CoreGraphics returned transparent (unreadable) proves nothing;
/// the control text must hold exactly one accented `a`; a caret watch with a long gap judges a
/// missing blink as untested, but a non-inverse blink still fails.
#[test]
fn round_8_bad_samples_for_w02_i03_and_w10() {
    use super::hold::picked_accent;
    use super::pixels::{MAX_FRAME_GAP_MS, caret_blinks, strip_visible};
    let strip = image(1040, 4, [248, 246, 239]);
    let mut unread = image(1040, 4, [0, 0, 0]);
    for p in unread.rgba.chunks_exact_mut(4) {
        p[3] = 0;
    }
    assert_eq!(strip_visible(&strip, &strip, &unread).0, Verdict::NotTested);
    // Half the scene below read (and like the strip), half unreadable: the difference would come
    // only from the unread half.
    let mut half = image(1040, 4, [248, 246, 239]);
    for (k, p) in half.rgba.chunks_exact_mut(4).enumerate() {
        if k % 2 == 0 {
            p.copy_from_slice(&[0, 0, 0, 0]);
        }
    }
    assert_eq!(strip_visible(&strip, &strip, &half).0, Verdict::NotTested);
    // Barely opaque pixels are not read colours either.
    let mut faint = image(1040, 4, [40, 70, 120]);
    for p in faint.rgba.chunks_exact_mut(4) {
        p[3] = 1;
    }
    assert_eq!(strip_visible(&strip, &strip, &faint).0, Verdict::NotTested);
    assert_eq!(
        strip_visible(&strip, &strip, &image(1040, 4, [40, 70, 120])).0,
        Verdict::Pass
    );
    assert!(picked_accent("á") && picked_accent("à"));
    for bad in ["", "a", "A2", "a2", "áá", "é", "á2"] {
        assert!(!picked_accent(bad), "{bad:?}");
    }
    let on = image(8, 60, [7, 9, 16]);
    let off = image(8, 60, [248, 246, 239]);
    // Two frames far apart: nothing alternates, but the watch did not cover the period.
    let sparse = vec![(0, off.clone()), (MAX_FRAME_GAP_MS + 50, off.clone())];
    assert_eq!(caret_blinks(&sparse).0, Verdict::NotTested);
    let one = vec![(0, on.clone())];
    assert_eq!(caret_blinks(&one).0, Verdict::NotTested);
    // A non-inverse alternation fails whatever the gaps.
    let tinted = image(8, 60, [200, 30, 30]);
    let wrong = vec![(0, off.clone()), (500, tinted.clone()), (1000, off.clone())];
    assert_eq!(caret_blinks(&wrong).0, Verdict::Fail);
}
