//! Bracketed hover/leave observations; no late sample can establish timely motion. The whole
//! scenario is one continuous watch: from the sample read just before the first move onto the
//! strip until the leave has settled, the window and the frontmost app are read together about
//! `STEP_MS` apart, and every check reads that one stream. A phase is a pointer move (its
//! request is the phase's zero) plus the sample read just before it. Motion is a sample that
//! differs from that sample, so movement inside the screen counts; `collapsed`/`expanded` only
//! judge preconditions and end states. A measured gap over `MAX_GAP_MS` in what a check covers,
//! or a changed scene (another app than Finder in front), leaves it NOT_TESTED; a FAIL needs an
//! observed violation.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    model::*,
    observation::{Bracket, LATE_MS, REST_MS, completion_verdict, rest_index, transition},
    scenarios::{Backend, Driver, Op},
};

/// The longest unobserved stretch a check may cover; a longer one leaves it NOT_TESTED.
const MAX_GAP_MS: u64 = 60;

/// The pause between two samples; the time spent in the queries comes on top.
const STEP_MS: u64 = 10;

/// A bound on samples in one watch, so a backend whose clock stops cannot loop forever.
const MAX_SAMPLES: usize = 4000;

/// Bounds within this distance (pt) of a reference are the same position.
const TOLERANCE: f64 = 0.5;

/// One observation on the backend clock: the window and then the frontmost app, requested at
/// `req`, both returned by `ret`.
struct Sample {
    req: u64,
    ret: u64,
    window: Window,
    front: u32,
}

/// The scenario's one stream of samples. `cut` is set when the sample bound ran out.
#[derive(Default)]
struct Watch {
    samples: Vec<Sample>,
    cut: bool,
}

/// A pointer move: `zero` is its request; `before` indexes the sample read just before it.
#[derive(Clone, Copy)]
struct Phase {
    zero: u64,
    before: usize,
}

impl Watch {
    /// Reads one sample; `None` once the sample bound is reached (the watch is then cut).
    fn sample<B: Backend>(&mut self, d: &mut Driver<'_, B>) -> Result<Option<usize>, String> {
        if self.samples.len() >= MAX_SAMPLES {
            self.cut = true;
            return Ok(None);
        }
        let req = d.backend.now();
        let window = d.window()?;
        let front = d.front()?;
        let ret = d.backend.now();
        self.samples.push(Sample {
            req,
            ret,
            window,
            front,
        });
        Ok(Some(self.samples.len() - 1))
    }
    /// Samples until one returns at or after `until` on the backend clock; false when cut.
    fn until<B: Backend>(&mut self, d: &mut Driver<'_, B>, until: u64) -> Result<bool, String> {
        loop {
            let Some(i) = self.sample(d)? else {
                return Ok(false);
            };
            let ret = self.samples[i].ret;
            if ret >= until {
                return Ok(true);
            }
            d.wait(STEP_MS.min(until - ret))?;
        }
    }
    /// Reads the window just before the move, then moves the pointer.
    fn move_to<B: Backend>(&mut self, d: &mut Driver<'_, B>, to: Point) -> Result<Phase, String> {
        let before = self
            .sample(d)?
            .ok_or("the watch reached its sample bound")?;
        let zero = d.backend.now();
        d.op(Op::Move(to))?;
        Ok(Phase { zero, before })
    }
    /// The longest unobserved stretch over samples `from..=to`: from one sample's request to the
    /// next one's return (the first sample of the watch: its own duration).
    fn gap(&self, from: usize, to: usize) -> u64 {
        (from..=to.min(self.samples.len().saturating_sub(1)))
            .map(|i| {
                let s = &self.samples[i];
                s.ret - i.checked_sub(1).map_or(s.req, |p| self.samples[p].req)
            })
            .max()
            .unwrap_or(u64::MAX)
    }
    fn last(&self) -> usize {
        self.samples.len() - 1
    }
    /// Whether the product took focus over samples `from..=to`: FAIL when it was frontmost in
    /// any; NOT_TESTED when another app than `finder` was (the scene changed under the check), a
    /// gap exceeds `MAX_GAP_MS`, or the watch was cut.
    fn focus(&self, app: u32, finder: u32, from: usize, to: usize) -> (Verdict, String) {
        let range = &self.samples[from..=to];
        let taken = range.iter().find(|s| s.front == app).map(|s| s.ret);
        let other = range
            .iter()
            .find(|s| s.front != app && s.front != finder)
            .map(|s| (s.ret, s.front));
        let gap = self.gap(from, to);
        let verdict = if taken.is_some() {
            Verdict::Fail
        } else if other.is_some() || gap > MAX_GAP_MS || self.cut {
            Verdict::NotTested
        } else {
            Verdict::Pass
        };
        (
            verdict,
            format!(
                "{} front reads; StickyMD frontmost at {taken:?}, another app at {other:?} \
                 (ms, PID); Finder {finder}; largest gap {gap}ms; cut {}",
                range.len(),
                self.cut
            ),
        )
    }
}

/// Whether `w` is at `reference`: on screen alike, every bound within `TOLERANCE`.
fn same_place(w: &Window, reference: &Window) -> bool {
    w.onscreen == reference.onscreen
        && (w.bounds.origin.x - reference.bounds.origin.x).abs() <= TOLERANCE
        && (w.bounds.origin.y - reference.bounds.origin.y).abs() <= TOLERANCE
        && (w.bounds.size.width - reference.bounds.size.width).abs() <= TOLERANCE
        && (w.bounds.size.height - reference.bounds.size.height).abs() <= TOLERANCE
}

pub(super) fn run<B: Backend>(d: &mut Driver<'_, B>, band: bool) -> Result<(), String> {
    let (s, w, finder) = super::scenarios::dock(d, Edge::Top)?;
    let strip = Point {
        x: w.bounds.center().x,
        y: s.origin.y + 1.0,
    };
    let away = Point {
        x: s.right() - 20.0,
        y: s.bottom() - 100.0,
    };
    let is_collapsed = |w: &Window| collapsed(w, s, Edge::Top);
    let mut watch = Watch::default();
    // Early leave: the pointer leaves the strip after about 35 ms and nothing may move. A dwell of
    // 100 ms or more (a late move) may legitimately expand, so it cannot test the cancellation.
    let early = watch.move_to(d, strip)?;
    // The Finder PID the backend verified in front when docking; another app in front since then
    // changed the scene (NOT_TESTED), StickyMD in front is the product's doing.
    let front = watch.samples[early.before].front;
    let verdict = if front == d.pid {
        Verdict::Fail
    } else if front != finder {
        Verdict::NotTested
    } else {
        Verdict::Pass
    };
    record(
        d,
        "Finder frontmost before hover",
        verdict,
        format!("front={front}, Finder={finder}, app={}", d.pid),
    );
    if verdict != Verdict::Pass {
        return skip_all(d, band, "Finder was not frontmost when the hover began");
    }
    watch.until(d, early.zero + 35)?;
    d.op(Op::Move(away))?;
    let dwell = d.backend.now() - early.zero;
    let ended = watch.until(d, early.zero + dwell + 350)?;
    let reference = watch.samples[early.before].window.clone();
    let moved = watch.samples[early.before + 1..]
        .iter()
        .find(|x| !same_place(&x.window, &reference))
        .map(|x| (x.ret - early.zero, x.window.bounds, x.window.onscreen));
    let gap = watch.gap(early.before + 1, watch.last());
    let verdict = if !is_collapsed(&reference) || dwell >= 100 {
        Verdict::NotTested
    } else if moved.is_some() {
        Verdict::Fail
    } else if !ended || gap > MAX_GAP_MS {
        Verdict::NotTested
    } else {
        Verdict::Pass
    };
    record(
        d,
        "early leave cancels",
        verdict,
        format!(
            "dwell {dwell}ms (100ms or more cannot test it); collapsed before {}; {} samples, \
             largest gap {gap}ms; moved at {moved:?} (ms, bounds, on screen)",
            is_collapsed(&reference),
            watch.last() - early.before
        ),
    );
    if !settle(d, &mut watch, is_collapsed)? {
        return skip_all(
            d,
            band,
            "the window did not rest on the strip after the early leave",
        );
    }
    let hover = watch.move_to(d, strip)?;
    let onset = observe_onset(
        d,
        &mut watch,
        hover,
        100,
        is_collapsed,
        "hover movement onset",
    )?;
    let rest = completion(
        d,
        &mut watch,
        hover,
        onset,
        |w| expanded(w, s),
        "hover expansion completion",
    )?;
    let (focus, detail) = watch.focus(d.pid, finder, early.before, watch.last());
    let verdict = match &rest {
        Some(rest) if !expanded(rest, s) => Verdict::Fail,
        Some(_) => focus,
        None if focus == Verdict::Fail => Verdict::Fail,
        None => Verdict::NotTested,
    };
    record(
        d,
        "hover expands without focus",
        verdict,
        format!("{detail}; rest {rest:?}"),
    );
    let Some(rest) = rest.filter(|rest| expanded(rest, s)) else {
        return skip_rest(d, band, "the hover did not settle expanded");
    };
    if band {
        let p = Point {
            x: rest.bounds.center().x,
            y: s.origin.y,
        };
        d.row.check(
            "pointer in top band",
            top_band(p, rest.bounds, s),
            format!("{p:?}"),
        );
        band_dwell(d, &mut watch, p, &rest, finder)?;
    }
    let leave = watch.move_to(d, away)?;
    let onset = observe_onset(
        d,
        &mut watch,
        leave,
        500,
        |w| expanded(w, s),
        "leave movement onset",
    )?;
    completion(
        d,
        &mut watch,
        leave,
        onset,
        is_collapsed,
        "leave collapse completion",
    )?;
    Ok(())
}

/// The checks a hover scenario makes after the early leave, all NOT_TESTED for `reason`.
fn skip_all<B: Backend>(d: &mut Driver<'_, B>, band: bool, reason: &str) -> Result<(), String> {
    for name in [
        "early leave cancels",
        "hover movement onset",
        "hover expansion completion",
        "hover expands without focus",
    ] {
        if !d.row.checks.iter().any(|c| c.name == name) {
            d.row.skip(name, reason);
        }
    }
    skip_rest(d, band, reason)
}

/// The band and leave checks, NOT_TESTED for `reason`.
fn skip_rest<B: Backend>(d: &mut Driver<'_, B>, band: bool, reason: &str) -> Result<(), String> {
    let band_checks = [
        "pointer in top band",
        "top band holds past 500ms",
        "top band dwell keeps focus away",
    ];
    let leave_checks = ["leave movement onset", "leave collapse completion"];
    for name in band_checks
        .iter()
        .filter(|_| band)
        .chain(leave_checks.iter())
    {
        d.row.skip(name, reason);
    }
    Ok(())
}

/// Samples until the window has rested in `state` for 100 ms (at most 1.5 s), so the next phase
/// starts from a window that is not still moving through it.
fn settle<B: Backend>(
    d: &mut Driver<'_, B>,
    watch: &mut Watch,
    state: impl Fn(&Window) -> bool,
) -> Result<bool, String> {
    let limit = d.backend.now() + 1500;
    loop {
        let Some(last) = watch.sample(d)? else {
            return Ok(false);
        };
        let now = watch.samples[last].window.clone();
        let mut first = last;
        while first > 0 && same_place(&watch.samples[first - 1].window, &now) {
            first -= 1;
        }
        let steady = watch.samples[last].ret - watch.samples[first].req;
        if state(&now) && steady >= 100 && watch.gap(first + 1, last) <= MAX_GAP_MS {
            return Ok(true);
        }
        if watch.samples[last].ret >= limit {
            return Ok(false);
        }
        d.wait(STEP_MS)?;
    }
}

/// W04: with the pointer in the top band the window stays visible at its settled position `rest`
/// and never takes focus for 850 ms from the move. A collapse, a partial retraction or a hidden
/// sample undone before the end still fails.
fn band_dwell<B: Backend>(
    d: &mut Driver<'_, B>,
    watch: &mut Watch,
    p: Point,
    rest: &Window,
    finder: u32,
) -> Result<(), String> {
    const DWELL_MS: u64 = 850;
    let band = watch.move_to(d, p)?;
    if !same_place(&watch.samples[band.before].window, rest) {
        let reason = format!(
            "not at the settled position before the move: {:?}",
            watch.samples[band.before].window
        );
        d.row.skip("top band holds past 500ms", &reason);
        d.row.skip("top band dwell keeps focus away", &reason);
        return Ok(());
    }
    let ended = watch.until(d, band.zero + DWELL_MS)?;
    let left = watch.samples[band.before + 1..]
        .iter()
        .find(|x| !same_place(&x.window, rest))
        .map(|x| (x.ret - band.zero, x.window.bounds, x.window.onscreen));
    let gap = watch.gap(band.before + 1, watch.last());
    let verdict = if left.is_some() {
        Verdict::Fail
    } else if !ended || gap > MAX_GAP_MS {
        Verdict::NotTested
    } else {
        Verdict::Pass
    };
    record(
        d,
        "top band holds past 500ms",
        verdict,
        format!(
            "{} samples over {}ms, largest gap {gap}ms, ended {ended}; left the settled {:?} at \
             {left:?} (ms, bounds, on screen)",
            watch.last() - band.before,
            watch.samples[watch.last()].ret - band.zero,
            rest.bounds
        ),
    );
    let (verdict, detail) = watch.focus(d.pid, finder, band.before, watch.last());
    record(d, "top band dwell keeps focus away", verdict, detail);
    Ok(())
}

fn record<B: Backend>(d: &mut Driver<'_, B>, name: &str, verdict: Verdict, detail: String) {
    if verdict == Verdict::NotTested {
        d.row.skip(name, &detail);
    } else {
        d.row.check(name, verdict == Verdict::Pass, detail);
    }
}

/// Samples until the window leaves the place it had before the move (`precondition` must hold
/// there), which the contract allows from `target` - 10 ms and requires by `target` + `LATE_MS`
/// after the move. The motion is bracketed by the request of the sample before (the move, for
/// the first sample after it) and the return of the first moved sample, so time spent inside the
/// queries counts against the bracket. Returns that bracket and the moved sample's index.
fn observe_onset<B: Backend>(
    d: &mut Driver<'_, B>,
    watch: &mut Watch,
    phase: Phase,
    target: u64,
    precondition: impl Fn(&Window) -> bool,
    name: &str,
) -> Result<Option<(Bracket, usize)>, String> {
    let reference = watch.samples[phase.before].window.clone();
    if !precondition(&reference) {
        d.row.skip(
            name,
            &format!("not in the start state before the move; {reference:?}"),
        );
        return Ok(None);
    }
    let lower = target - 10;
    loop {
        let Some(i) = watch.sample(d)? else {
            d.row.skip(name, "the watch reached its sample bound");
            return Ok(None);
        };
        let s = &watch.samples[i];
        let (req, ret) = (
            s.req.saturating_sub(phase.zero),
            s.ret.saturating_sub(phase.zero),
        );
        if !same_place(&s.window, &reference) {
            let old_ms = if i - 1 == phase.before {
                0
            } else {
                watch.samples[i - 1].req - phase.zero
            };
            let bracket = Bracket {
                old_ms,
                new_ms: ret,
            };
            let gap = watch.gap(phase.before + 1, i);
            let verdict = match transition(bracket, target) {
                // A gap before the bracket could hide an earlier motion undone in time.
                Verdict::Pass if gap > MAX_GAP_MS => Verdict::NotTested,
                verdict => verdict,
            };
            let detail = format!(
                "target={target}ms; bracket={bracket:?}; {} samples, largest gap {gap}ms; {:?}",
                i - phase.before,
                s.window
            );
            record(d, name, verdict, detail);
            return Ok(Some((bracket, i)));
        }
        if req >= target + LATE_MS {
            let detail = format!(
                "still in place when requested at {req}ms (returned {ret}ms); target={target}ms; \
                 {:?}",
                s.window
            );
            record(d, name, Verdict::Fail, detail);
            return Ok(None);
        }
        // Sample at the lower acceptance bound too; otherwise a perfect transition sampled
        // just before and after it would straddle the bound.
        d.wait(if ret < lower {
            STEP_MS.min(lower - ret)
        } else {
            STEP_MS
        })?;
    }
}

/// Continues the watch from the moved sample until `REST_MS` past the latest completion
/// deadline. The earliest possible onset bounds the deadline a PASS must meet, the latest one
/// the deadline a FAIL must exceed; the moved sample is the first completion sample, so the
/// stretch to the next one is covered. Returns the rest position, if one was established.
fn completion<B: Backend>(
    d: &mut Driver<'_, B>,
    watch: &mut Watch,
    phase: Phase,
    onset: Option<(Bracket, usize)>,
    settled: impl Fn(&Window) -> bool,
    name: &str,
) -> Result<Option<Window>, String> {
    let Some((onset, first)) = onset else {
        d.row.skip(name, "no observed onset bracket");
        return Ok(None);
    };
    let early = onset.old_ms + 140 + LATE_MS;
    let late = onset.new_ms + 140 + LATE_MS;
    let ended = watch.until(d, phase.zero + late + REST_MS)?;
    let tail = &watch.samples[first..];
    let states: Vec<_> = tail
        .iter()
        .map(|s| (s.window.bounds, s.window.onscreen))
        .collect();
    let k = rest_index(&states).ok_or("no completion samples")?;
    let rest = tail[tail.len() - 1].window.clone();
    let at = |ms: u64| ms - phase.zero;
    let first_rest = at(tail[k].ret);
    let moving = k.checked_sub(1).map(|j| at(tail[j].req));
    let last_req = at(tail[tail.len() - 1].req);
    let gap = watch.gap(first + 1, watch.last());
    let verdict =
        match completion_verdict(settled(&rest), last_req, first_rest, moving, early, late) {
            _ if !ended => Verdict::NotTested,
            Verdict::Pass if gap > MAX_GAP_MS => Verdict::NotTested,
            verdict => verdict,
        };
    record(
        d,
        name,
        verdict,
        format!(
            "onset={onset:?}; at rest from {first_rest}ms (moving sample requested at \
             {moving:?}ms); deadlines {early}/{late}ms; {} samples, largest gap {gap}ms; rest \
             {rest:?}",
            tail.len()
        ),
    );
    Ok(ended.then_some(rest))
}
