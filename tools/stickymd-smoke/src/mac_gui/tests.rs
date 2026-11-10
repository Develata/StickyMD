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
fn timing_has_closed_bounds_and_rejects_late_observations() {
    assert!(timing(100, 90, 700));
    assert!(timing(700, 90, 700));
    assert!(!timing(89, 90, 700));
    assert!(!timing(701, 90, 700));
    assert!(timing(640, 490, 1100));
    assert!(!timing(350, 490, 1100));
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
    assert!(permission_error(&["F01".into(), "W05".into()], Permissions::default()).is_ok());
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
    for args in [
        vec!["--app"],
        vec!["--app", "x", "--only", "W99"],
        vec!["--app", "x", "--only", "W01,W01"],
        vec!["--allow-host", "--allow-host"],
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
}
impl HoverBackend {
    fn new() -> Self {
        Self {
            now: 0,
            entered: None,
            left: None,
            focused: 42,
            steals: false,
            expand_at: 240,
        }
    }
    fn expanded(&self) -> bool {
        self.entered.is_some_and(|t| self.now - t >= self.expand_at)
            || self.left.is_some_and(|t| self.now - t < 640)
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
            Op::Window => Value::Window(if self.expanded() {
                window(rect(460.0, 28.0, 520.0, 480.0), 0)
            } else {
                window(rect(460.0, -478.0, 520.0, 480.0), 27)
            }),
            Op::Screen => Value::Rect(rect(0.0, 0.0, 1440.0, 900.0)),
            Op::Front => Value::Pid(if self.steals && self.expanded() {
                42
            } else {
                self.focused
            }),
            Op::Finder => {
                self.focused = 99;
                Value::Pid(99)
            }
            Op::Activate | Op::Drag(..) => Value::Unit,
            Op::Move(p) => {
                if p.y <= 1.0 {
                    if self.entered.is_none() {
                        self.entered = Some(self.now);
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
