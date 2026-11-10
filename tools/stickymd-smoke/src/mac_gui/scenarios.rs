//! Scenario orchestration depends only on external observations supplied by Backend.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::input::CMD;
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
    pub fn until(
        &mut self,
        max_ms: u64,
        predicate: impl Fn(&Window) -> bool,
    ) -> Result<(Window, u64), String> {
        let start = self.backend.now();
        loop {
            let w = self.window()?;
            let elapsed = self.backend.now().saturating_sub(start);
            if predicate(&w) || elapsed >= max_ms {
                return Ok((w, elapsed));
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
        "W06" => Some("S7 manual-collapse/focus-cycle scenario deferred by scope"),
        "W10" => Some("requires caret visual and compositor frame instrumentation"),
        "W12" => {
            Some("requires disposable logout session, modal termination and save-failure injection")
        }
        "R03" => Some("requires quarantined downloaded signed artifact / App Translocation"),
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
            let (w, ms) = d.until(MENU_EFFECT_MS, |w| !w.onscreen)?;
            d.row
                .check("hidden", !w.onscreen, format!("after {ms}ms: {w:?}"));
            d.op(Op::Reopen)?;
            let (w, _) = d.until(3000, |w| w.onscreen)?;
            let front = d.front()?;
            let alive = matches!(d.op(Op::Alive)?, Value::Bool(true));
            d.row.check(
                "reopen retains PID and focuses",
                alive && w.onscreen && front == d.pid,
                format!("owned pid={}, front={front}, alive={alive}, {w:?}", d.pid),
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
            let (w, ms) = d.until(MENU_EFFECT_MS, |w| !w.onscreen)?;
            d.row
                .check("隐藏", !w.onscreen, format!("after {ms}ms: {w:?}"));
            d.op(Op::Menu("显示".into()))?;
            let (w, ms) = d.until(MENU_EFFECT_MS, settled)?;
            d.row.check(
                "显示",
                settled(&w),
                format!("after {ms}ms: {w:?}; before {before:?}"),
            );
            let normal = w;
            d.op(Op::Menu("置顶".into()))?;
            let (top, ms) = d.until(MENU_EFFECT_MS, |w| w.layer > normal.layer)?;
            d.row.check(
                "topmost layer rises",
                top.layer > normal.layer,
                format!("after {ms}ms: normal={}, top={}", normal.layer, top.layer),
            );
            d.config("", "always_on_top", "true")?;
            d.op(Op::Menu("置顶".into()))?;
            let (restored, ms) = d.until(MENU_EFFECT_MS, |w| w.layer == normal.layer)?;
            d.row.check(
                "topmost layer restored",
                restored.layer == normal.layer,
                format!("after {ms}ms: {restored:?}"),
            );
            d.op(Op::Activate)?;
            d.type_text(" final")?;
            d.op(Op::Menu("退出".into()))?;
            quit(d)?;
            d.note("quit barrier evidence final")?;
        }
        "W09" => super::editing::opacity(d)?,
        "W11" => {
            d.op(Op::Activate)?;
            d.chord(&[CMD, 12])?;
            quit(d)?;
            d.note("")?;
            d.row.skip(
                "save failure cancels quit",
                "requires approved save fault injection; no runtime note/config writes",
            );
        }
        "K02" => super::editing::keys(d)?,
        "E01" => super::editing::export(d)?,
        "F01" => {
            let log = d.text(Op::Fonts)?;
            // Plan 02's macOS defaults; the log names the CJK and Latin faces, not the code face.
            d.row.check(
                "font selection log",
                log.contains("CJK=PingFang SC found=true Latin=Times New Roman found=true"),
                log,
            );
            d.row.skip(
                "code face Menlo",
                "the font log does not name the code face; Menlo needs the screenshot's visual review",
            );
            let screenshot = d.text(Op::Screenshot)?;
            d.row.trace.push(Trace {
                ms: d.backend.now(),
                event: screenshot,
            });
            d.row.skip("glyph visual review","font log and optional screenshot need visual review; they do not prove per-glyph font selection");
        }
        _ => return Err(format!("unknown executable scenario {id}")),
    }
    Ok(())
}
fn quit<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    let start = d.backend.now();
    loop {
        let alive = matches!(d.op(Op::Alive)?, Value::Bool(true));
        if !alive || d.backend.now() - start >= 5000 {
            d.row
                .check("process exited", !alive, format!("alive={alive}"));
            return Ok(());
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
    let (w, elapsed) = d.until(1800, |w| collapsed(w, s, edge))?;
    d.row.check(
        &format!("{} 2 DIP collapse", edge.name()),
        collapsed(&w, s, edge),
        format!("elapsed={elapsed}ms; {w:?}; screen={s:?}"),
    );
    d.config("window", "dock_edge", &format!("\"{}\"", edge.name()))?;
    Ok((s, w))
}
