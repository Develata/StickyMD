//! Scenario orchestration depends only on external observations supplied by Backend.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::model::*;
#[derive(Clone, Debug)]
pub(super) enum Op {
    Wait(u64),
    Window,
    Screen,
    Front,
    Finder,
    Activate,
    Move(Point),
    Drag(Point, Point),
    Click(Point),
    Chord(Vec<u16>),
    Text(String),
    Menu(String),
    Config,
    Note,
    NoteNow,
    Alive,
    Clipboard,
    Reopen,
    AppType,
    Dock,
    Panel(bool),
    PanelOpen,
    Export,
    Fonts,
    Screenshot,
    /// Remove (`true`) or restore (`false`) write permission on the fixture's note directory.
    Fault(bool),
    /// The product windows' AX titles, one per line.
    Title,
    /// The system's termination request (`NSRunningApplication.terminate`) to the owned PID.
    Terminate,
    /// Make the product frontmost through AX only, without reopening the bundle.
    Focus,
    /// Files the product has memory-mapped (`lsof -d txt`).
    Mapped,
    /// Point the open save dialog at the session's `export.md` without answering it.
    PanelTarget,
    /// Start the product again in the same fixture after it quit; answers its new PID.
    Relaunch,
    /// SIGTERM to the owned product, as launchd sends it at a logout.
    Sigterm,
}
#[derive(Clone, Debug)]
pub(super) enum Value {
    Unit,
    Time(u64),
    Text(String),
    Window(Window),
    Rect(Rect),
    Pid(u32),
    Bool(bool),
}
pub(super) trait Backend {
    fn now(&self) -> u64;
    fn perform(&mut self, op: Op) -> Result<Value, String>;
}
pub(super) struct Driver<'a, B> {
    pub backend: &'a mut B,
    pub row: &'a mut Row,
    pub pid: u32,
}
impl<B: Backend> Driver<'_, B> {
    pub fn op(&mut self, op: Op) -> Result<Value, String> {
        self.row.trace.push(Trace {
            ms: self.backend.now(),
            event: format!("request {op:?}"),
        });
        let result = self.backend.perform(op);
        self.row.trace.push(Trace {
            ms: self.backend.now(),
            event: format!("observed {result:?}"),
        });
        result
    }
    pub fn wait(&mut self, ms: u64) -> Result<(), String> {
        self.op(Op::Wait(ms)).map(|_| ())
    }
    pub fn window(&mut self) -> Result<Window, String> {
        match self.op(Op::Window)? {
            Value::Window(w) => Ok(w),
            v => Err(format!("expected window, got {v:?}")),
        }
    }
    pub fn text(&mut self, op: Op) -> Result<String, String> {
        match self.op(op)? {
            Value::Text(s) => Ok(s),
            v => Err(format!("expected text, got {v:?}")),
        }
    }
    pub fn front(&mut self) -> Result<u32, String> {
        match self.op(Op::Front)? {
            Value::Pid(p) => Ok(p),
            v => Err(format!("expected PID, got {v:?}")),
        }
    }
    pub fn screen(&mut self) -> Result<Rect, String> {
        match self.op(Op::Screen)? {
            Value::Rect(r) => Ok(r),
            v => Err(format!("expected screen, got {v:?}")),
        }
    }
    pub fn chord(&mut self, keys: &[u16]) -> Result<(), String> {
        self.op(Op::Chord(keys.to_vec())).map(|_| ())
    }
    pub fn type_text(&mut self, s: &str) -> Result<(), String> {
        self.op(Op::Text(s.into())).map(|_| ())
    }
    pub fn config(&mut self, section: &str, key: &str, expected: &str) -> Result<(), String> {
        self.wait(1400)?;
        let s = self.text(Op::Config)?;
        self.row.check(
            &format!("config {section}.{key}"),
            config_value(&s, section, key) == Some(expected),
            s,
        );
        Ok(())
    }
    pub fn note(&mut self, expected: &str) -> Result<(), String> {
        let s = self.text(Op::Note)?;
        self.row.check(
            "saved note bytes",
            s == expected,
            format!("expected={expected:?}; actual={s:?}"),
        );
        Ok(())
    }
    /// Polls every 20 ms until `predicate` holds or `max_ms` passes. Returns the last sample,
    /// the time it returned, and whether it was met: only a sample that holds and returned
    /// within `max_ms` counts, so a success first seen after the deadline is late.
    pub fn until(
        &mut self,
        max_ms: u64,
        predicate: impl Fn(&Window) -> bool,
    ) -> Result<(Window, u64, bool), String> {
        let start = self.backend.now();
        loop {
            let w = self.window()?;
            let elapsed = self.backend.now().saturating_sub(start);
            let holds = predicate(&w);
            if holds || elapsed >= max_ms {
                return Ok((w, elapsed, holds && elapsed <= max_ms));
            }
            self.wait(20)?;
        }
    }
}
/// Bound for a status-menu command's effect: the menu blinks and fades before AppKit sends the
/// action, and hiding or showing the window animates. The elapsed time is kept as evidence.
const MENU_EFFECT_MS: u64 = 2000;
pub(super) fn skipped(id: &str) -> Option<&'static str> {
    match id {
        "I01" | "I02" => {
            Some("requires real configured IME, candidate UI and 13 composition cases")
        }
        "I03" => {
            Some("requires controlled system repeat settings and sustained physical key semantics")
        }
        "W05" => Some("requires Spaces/full-screen desktop orchestration"),
        "W10" => Some("requires caret visual and compositor frame instrumentation"),
        "R03" => Some(
            "requires a quarantined download opened through LaunchServices (App Translocation); \
             probed by script, not by this driver",
        ),
        "W15" => Some("requires Windows regression machine"),
        _ => None,
    }
}
pub(super) fn run<B: Backend>(d: &mut Driver<'_, B>, id: &str) -> Result<(), String> {
    match id {
        "W01" => {
            let kind = d.text(Op::AppType)?;
            d.row.check(
                "UIElement",
                super::observation::lsappinfo_type(&kind) == Some("UIElement"),
                kind,
            );
            let dock = d.text(Op::Dock)?;
            d.row
                .check("no Dock tile", !dock.lines().any(|s| s == "StickyMD"), dock);
            d.row.skip(
                "Cmd+Tab visual",
                "UIElement implies switcher exclusion; switcher pixels are not inspected",
            );
        }
        "W02" => {
            for edge in [Edge::Left, Edge::Right, Edge::Top] {
                dock(d, edge)?;
            }
        }
        "W03" | "W04" => super::hover::run(d, id == "W04")?,
        "W07" => {
            d.op(Op::Menu("隐藏".into()))?;
            let (w, ms, met) = d.until(MENU_EFFECT_MS, |w| !w.onscreen)?;
            d.row.check("hidden", met, format!("after {ms}ms: {w:?}"));
            d.op(Op::Reopen)?;
            let (w, ms, shown) = d.until(3000, |w| w.onscreen)?;
            let front = d.front()?;
            let alive = matches!(d.op(Op::Alive)?, Value::Bool(true));
            d.row.check(
                "reopen retains PID and focuses",
                alive && shown && front == d.pid,
                format!(
                    "owned pid={}, front={front}, alive={alive}, after {ms}ms: {w:?}",
                    d.pid
                ),
            );
        }
        "W08" => {
            d.op(Op::Activate)?;
            d.type_text("quit barrier evidence")?;
            let before = d.window()?;
            // Shown means back where it was at its opacity, after the show animation.
            let settled =
                |w: &Window| w.onscreen && w.alpha == before.alpha && w.bounds == before.bounds;
            d.op(Op::Menu("隐藏".into()))?;
            let (w, ms, met) = d.until(MENU_EFFECT_MS, |w| !w.onscreen)?;
            d.row.check("隐藏", met, format!("after {ms}ms: {w:?}"));
            d.op(Op::Menu("显示".into()))?;
            let (w, ms, met) = d.until(MENU_EFFECT_MS, settled)?;
            d.row.check(
                "显示",
                met,
                format!("after {ms}ms: {w:?}; before {before:?}"),
            );
            let normal = w;
            d.op(Op::Menu("置顶".into()))?;
            let (top, ms, met) = d.until(MENU_EFFECT_MS, |w| w.layer > normal.layer)?;
            d.row.check(
                "topmost layer rises",
                met,
                format!("after {ms}ms: normal={}, top={}", normal.layer, top.layer),
            );
            d.config("", "always_on_top", "true")?;
            d.op(Op::Menu("置顶".into()))?;
            let (restored, ms, met) = d.until(MENU_EFFECT_MS, |w| w.layer == normal.layer)?;
            d.row.check(
                "topmost layer restored",
                met,
                format!("after {ms}ms: {restored:?}"),
            );
            d.op(Op::Activate)?;
            d.type_text(" final")?;
            d.op(Op::Menu("退出".into()))?;
            quit(d)?;
            d.note("quit barrier evidence final")?;
        }
        "W09" => super::editing::opacity(d)?,
        "W06" => super::focus::run(d)?,
        "W11" => super::lifecycle::command_quit(d)?,
        "W12" => super::lifecycle::system_termination(d)?,
        "K02" => super::editing::keys(d)?,
        "E01" => super::editing::export(d)?,
        "F01" => super::editing::fonts(d)?,
        _ => return Err(format!("unknown executable scenario {id}")),
    }
    Ok(())
}
pub(super) fn quit<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    let (exited, ms) = exits_within(d, 5000)?;
    d.row.check(
        "process exited",
        exited,
        format!("alive={} after {ms}ms", !exited),
    );
    Ok(())
}
/// Polls every 50 ms; an exit first seen after `max_ms` is late.
pub(super) fn exits_within<B: Backend>(
    d: &mut Driver<'_, B>,
    max_ms: u64,
) -> Result<(bool, u64), String> {
    let start = d.backend.now();
    loop {
        let alive = matches!(d.op(Op::Alive)?, Value::Bool(true));
        let elapsed = d.backend.now().saturating_sub(start);
        if !alive || elapsed >= max_ms {
            return Ok((!alive && elapsed <= max_ms, elapsed));
        }
        d.wait(50)?;
    }
}
pub(super) fn dock<B: Backend>(
    d: &mut Driver<'_, B>,
    edge: Edge,
) -> Result<(Rect, Window), String> {
    d.op(Op::Activate)?;
    d.wait(350)?;
    let w = d.window()?;
    let s = d.screen()?;
    let r = w.bounds;
    // Toolbar center is the unoccupied drag strip for the fixed 520 DIP fixture.
    let from = Point {
        x: r.center().x,
        y: r.origin.y + 18.0,
    };
    let to = match edge {
        Edge::Left => Point {
            x: s.origin.x + r.size.width / 2.0 + 4.0,
            y: s.center().y - r.size.height / 2.0 + 18.0,
        },
        Edge::Right => Point {
            x: s.right() - r.size.width / 2.0 - 4.0,
            y: s.center().y - r.size.height / 2.0 + 18.0,
        },
        Edge::Top => Point {
            x: s.center().x,
            y: s.origin.y + 46.0,
        },
    };
    d.op(Op::Drag(from, to))?;
    d.op(Op::Move(Point {
        x: s.right() - 20.0,
        y: s.bottom() - 100.0,
    }))?;
    d.op(Op::Finder)?;
    let (w, elapsed, met) = d.until(1800, |w| collapsed(w, s, edge))?;
    d.row.check(
        &format!("{} 2 DIP collapse", edge.name()),
        met,
        format!("elapsed={elapsed}ms; {w:?}; screen={s:?}"),
    );
    d.config("window", "dock_edge", &format!("\"{}\"", edge.name()))?;
    Ok((s, w))
}
