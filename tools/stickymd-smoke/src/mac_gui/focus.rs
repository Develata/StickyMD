//! W06 (S7): after a manual collapse, focus leaving and returning neither expands the window nor
//! drops the collapsed strip from its raised layer, and hovering still expands it. Focus returns
//! through AX only: reopening the bundle is a show request, a different product path. The Space
//! switch in the row's wording is not driven; it stays a manual observation.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    model::*,
    scenarios::{Backend, Driver, Op, dock},
};

/// Esc collapse and hover expansion: a 140 ms animation (plus 100 ms dwell for hover) with slack.
const MOTION_MS: u64 = 1000;
/// Long enough after focus returns for a reveal (100 ms) and its animation to have shown.
const SETTLE_MS: u64 = 800;
/// A gap between samples (or between the action and the first sample) above this could hide a
/// brief expansion; the same bound the hover brackets use.
const MAX_GAP_MS: u64 = 60;
const ESCAPE: u16 = 53;

pub(super) fn run<B: Backend>(d: &mut Driver<'_, B>) -> Result<(), String> {
    let edge = Edge::Right;
    let (s, _) = dock(d, edge)?;
    d.op(Op::Activate)?;
    let (shown, ms, met) = d.until(MOTION_MS, |w| expanded(w, s))?;
    let front = d.front()?;
    d.row.check(
        "reopen shows the docked window focused",
        met && front == d.pid,
        format!("front={front} after {ms}ms: {shown:?}"),
    );
    d.chord(&[ESCAPE])?;
    let (strip, ms, met) = d.until(MOTION_MS, |w| collapsed(w, s, edge))?;
    d.row.check(
        "Esc collapses (manual collapse)",
        met,
        format!("after {ms}ms: {strip:?}"),
    );
    let raised = strip.layer;
    d.row.check(
        "collapsed strip above the focused window's layer",
        raised > shown.layer,
        format!("focused={}, collapsed={raised}", shown.layer),
    );
    let held = |w: &Window| collapsed(w, s, edge) && w.layer == raised;
    // Opening Finder waits for it to be frontmost, so sampling starts when that returns: the check
    // covers the state after the switch, not the switch itself.
    d.op(Op::Finder)?;
    let since = d.backend.now();
    let away = throughout(d, since, SETTLE_MS, held)?;
    away.record(
        d,
        "after switching away the strip stays collapsed and raised",
    );
    let since = d.backend.now();
    d.op(Op::Focus)?;
    let back = throughout(d, since, SETTLE_MS, held)?;
    back.record(d, "focus return neither expands nor lowers the strip");
    let front = d.front()?;
    d.row.check(
        "focus returns to the product",
        front == d.pid,
        format!("front={front}, app={}", d.pid),
    );
    d.op(Op::Move(Point {
        x: s.right() - 1.0,
        y: back.last.bounds.center().y,
    }))?;
    let (w, ms, met) = d.until(MOTION_MS, |w| expanded(w, s))?;
    d.row.check(
        "hover expands after focus returned",
        met,
        format!("after {ms}ms: {w:?}"),
    );
    d.op(Op::Move(Point {
        x: s.origin.x + 20.0,
        y: s.bottom() - 100.0,
    }))?;
    d.row.skip(
        "Space switch",
        "switching Spaces needs Mission Control orchestration; it stays a manual observation",
    );
    Ok(())
}

struct Coverage {
    failing: Option<Window>,
    last: Window,
    samples: u32,
    max_gap: u64,
}

impl Coverage {
    /// A failing sample fails; otherwise sampling too coarse to rule out a brief change is not
    /// tested; otherwise it passes.
    fn record<B: Backend>(&self, d: &mut Driver<'_, B>, name: &str) {
        let detail = format!(
            "{} samples, largest gap {}ms; {:?}",
            self.samples,
            self.max_gap,
            self.failing.as_ref().unwrap_or(&self.last)
        );
        if self.failing.is_some() {
            d.row.check(name, false, detail);
        } else if self.max_gap > MAX_GAP_MS {
            d.row.skip(name, &format!("sampling too coarse: {detail}"));
        } else {
            d.row.check(name, true, detail);
        }
    }
}

/// Samples every 20 ms from `since` until `ms` have passed, keeping the largest gap between `since`
/// and the first sample or between two samples; stops at the first sample that breaks `holds`.
fn throughout<B: Backend>(
    d: &mut Driver<'_, B>,
    since: u64,
    ms: u64,
    holds: impl Fn(&Window) -> bool,
) -> Result<Coverage, String> {
    let mut previous = since;
    let mut max_gap = 0;
    let mut samples = 0;
    loop {
        let w = d.window()?;
        let now = d.backend.now();
        max_gap = max_gap.max(now.saturating_sub(previous));
        previous = now;
        samples += 1;
        if !holds(&w) {
            return Ok(Coverage {
                failing: Some(w.clone()),
                last: w,
                samples,
                max_gap,
            });
        }
        if now.saturating_sub(since) >= ms {
            return Ok(Coverage {
                failing: None,
                last: w,
                samples,
                max_gap,
            });
        }
        d.wait(20)?;
    }
}
