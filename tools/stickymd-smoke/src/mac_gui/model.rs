//! Pure observations, geometry, interlocks and evidence. Coordinates are global CG DIP.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Point {
    pub x: f64,
    pub y: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Size {
    pub width: f64,
    pub height: f64,
}
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Rect {
    pub origin: Point,
    pub size: Size,
}
impl Rect {
    pub fn right(self) -> f64 {
        self.origin.x + self.size.width
    }
    pub fn bottom(self) -> f64 {
        self.origin.y + self.size.height
    }
    pub fn center(self) -> Point {
        Point {
            x: self.origin.x + self.size.width / 2.0,
            y: self.origin.y + self.size.height / 2.0,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Edge {
    Left,
    Right,
    Top,
}
impl Edge {
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Top => "top",
        }
    }
}
#[derive(Clone, Debug)]
pub(super) struct Window {
    pub id: u32,
    pub bounds: Rect,
    pub alpha: f64,
    pub layer: i32,
    pub onscreen: bool,
}
/// One on-screen window, as CoreGraphics lists them: front to back.
#[derive(Clone, Debug)]
pub(super) struct Hit {
    pub pid: u32,
    pub owner: String,
    pub layer: i32,
    pub bounds: Rect,
}
/// `kCGCursorWindowLevel`: the pointer itself, drawn above everything and never hit.
pub(super) const CURSOR_LAYER: i32 = 2_147_483_630;
/// The front window under `p`, which a press there reaches. The Dock's full-screen window lies
/// above normal windows but passes clicks through outside the Dock, so it is skipped, and so is
/// the pointer.
pub(super) fn front_hit(front_to_back: &[Hit], p: Point) -> Option<&Hit> {
    front_to_back.iter().find(|w| {
        w.owner != "Dock"
            && w.layer != CURSOR_LAYER
            && p.x >= w.bounds.origin.x
            && p.x < w.bounds.right()
            && p.y >= w.bounds.origin.y
            && p.y < w.bounds.bottom()
    })
}
/// Picks the product window among the PID's content-sized windows. A window number must match
/// exactly one of them, hidden or not (W07/W08 hide it). Without a number only on-screen windows
/// count: AppKit attaches windows that are never ordered in to the PID (macOS 27.2 shows one
/// 500×500 off-screen window), and at launch the product's own window is the one on screen.
/// `Err` carries the number of matching windows.
pub(super) fn select_window(candidates: Vec<Window>, id: Option<u32>) -> Result<Window, usize> {
    let mut matching: Vec<Window> = candidates
        .into_iter()
        .filter(|w| id.map_or(w.onscreen, |id| w.id == id))
        .collect();
    match matching.len() {
        1 => Ok(matching.remove(0)),
        n => Err(n),
    }
}
pub(super) fn collapsed(w: &Window, screen: Rect, edge: Edge) -> bool {
    let r = w.bounds;
    let strip = match edge {
        Edge::Left => r.right() - screen.origin.x,
        Edge::Right => screen.right() - r.origin.x,
        Edge::Top => r.bottom() - screen.origin.y,
    };
    let offscreen = match edge {
        Edge::Left => r.origin.x < screen.origin.x,
        Edge::Right => r.right() > screen.right(),
        Edge::Top => r.origin.y < screen.origin.y,
    };
    let overlap = match edge {
        Edge::Top => r.right() > screen.origin.x && r.origin.x < screen.right(),
        _ => r.bottom() > screen.origin.y && r.origin.y < screen.bottom(),
    };
    w.onscreen
        && offscreen
        && overlap
        && (strip - 2.0).abs() <= 0.75
        && (edge != Edge::Top || w.layer > 24)
}
pub(super) fn expanded(w: &Window, screen: Rect) -> bool {
    w.onscreen
        && w.bounds.origin.x >= screen.origin.x - 1.0
        && w.bounds.origin.y >= screen.origin.y - 1.0
        && w.bounds.right() <= screen.right() + 1.0
        && w.bounds.bottom() <= screen.bottom() + 1.0
}
pub(super) fn top_band(p: Point, window: Rect, screen: Rect) -> bool {
    p.x >= window.origin.x
        && p.x < window.right()
        && p.y >= screen.origin.y
        && p.y < window.origin.y
}
pub(super) fn vm_value(output: &str) -> bool {
    output.trim() == "1"
}
pub(super) fn require_vm(output: &str, allow_host: bool) -> Result<(), String> {
    if allow_host || vm_value(output) {
        Ok(())
    } else {
        Err(format!(
            "mac-gui refused: kern.hv_vmm_present={} (requires 1 / dedicated VM); no app path was accessed and no input was posted. --allow-host is an explicit override, not authorized on the owner's desktop.",
            output.trim()
        ))
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub(super) struct Permissions {
    pub ax: bool,
    pub post: bool,
    pub screen: bool,
}
pub(super) fn required(id: &str) -> (bool, bool) {
    match id {
        "W01" => (true, false),
        "W02" | "W03" | "W04" | "W07" | "W08" | "W09" | "W11" | "K02" | "E01" => (true, true),
        _ => (false, false),
    }
}
pub(super) fn permission_error(ids: &[String], p: Permissions) -> Result<(), String> {
    let missing = ids
        .iter()
        .filter_map(|id| {
            let (ax, post) = required(id);
            let mut needs = Vec::new();
            if ax && !p.ax {
                needs.push("Accessibility (AXIsProcessTrusted)");
            }
            if post && !p.post {
                needs.push("PostEvent (CGPreflightPostEventAccess)");
            }
            (!needs.is_empty()).then(|| format!("{id}: {}", needs.join(" + ")))
        })
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "mac-gui refused before launch; missing permissions: {}. Grant to the driver in the VM and rerun. Screen Recording is optional for F01.",
            missing.join("; ")
        ))
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Verdict {
    Pass,
    Fail,
    NotTested,
}
impl Verdict {
    pub fn name(self) -> &'static str {
        match self {
            Self::Pass => "PASS",
            Self::Fail => "FAIL",
            Self::NotTested => "NOT_TESTED",
        }
    }
}
pub(super) fn aggregate(checks: &[Check]) -> Verdict {
    if checks.iter().any(|c| c.verdict == Verdict::Fail) {
        Verdict::Fail
    } else if checks.is_empty() || checks.iter().any(|c| c.verdict == Verdict::NotTested) {
        Verdict::NotTested
    } else {
        Verdict::Pass
    }
}
pub(super) struct Check {
    pub name: String,
    pub verdict: Verdict,
    pub observation: String,
}
#[derive(Debug)]
pub(super) struct Trace {
    pub ms: u64,
    pub event: String,
}
pub(super) struct Row {
    pub id: String,
    pub checks: Vec<Check>,
    pub trace: Vec<Trace>,
}
impl Row {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.into(),
            checks: Vec::new(),
            trace: Vec::new(),
        }
    }
    pub fn check(&mut self, name: &str, pass: bool, observation: impl Into<String>) {
        self.checks.push(Check {
            name: name.into(),
            verdict: if pass { Verdict::Pass } else { Verdict::Fail },
            observation: observation.into(),
        });
    }
    pub fn skip(&mut self, name: &str, reason: &str) {
        self.checks.push(Check {
            name: name.into(),
            verdict: Verdict::NotTested,
            observation: reason.into(),
        });
    }
    pub fn json(&self) -> String {
        let checks = self
            .checks
            .iter()
            .map(|c| {
                format!(
                    "{{\"name\":{},\"verdict\":{},\"observation\":{}}}",
                    quote(&c.name),
                    quote(c.verdict.name()),
                    quote(&c.observation)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        let trace = self
            .trace
            .iter()
            .map(|t| format!("{{\"ms\":{},\"event\":{}}}", t.ms, quote(&t.event)))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"id\":{},\"verdict\":{},\"checks\":[{}],\"timeline\":[{}]}}",
            quote(&format!("P16-{}", self.id)),
            quote(aggregate(&self.checks).name()),
            checks,
            trace
        )
    }
}
pub(super) fn quote(s: &str) -> String {
    format!("\"{}\"", crate::evidence::escape_json(s))
}
// Narrow reader for known scalar keys from the product serializer, not a TOML parser.
pub(super) fn config_value<'a>(s: &'a str, section: &str, key: &str) -> Option<&'a str> {
    let mut current = "";
    for line in s.lines().map(str::trim) {
        if line.starts_with('[') {
            current = line.trim_matches(['[', ']']);
            continue;
        }
        if current == section
            && let Some((k, v)) = line.split_once('=')
            && k.trim() == key
        {
            return Some(v.trim());
        }
    }
    None
}
