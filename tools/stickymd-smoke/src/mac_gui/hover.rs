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

/// The scenario's one stream of samples. `cut` is set when it stopped early: the sample bound
/// ran out, or the frontmost app could not be read (`unavailable`); what it no longer covers is
/// NOT_TESTED, never a product FAIL.
#[derive(Default)]
struct Watch {
    samples: Vec<Sample>,
    cut: bool,
    unavailable: Option<String>,
}

/// A pointer move: `zero` is its request; `before` indexes the sample read just before it.
#[derive(Clone, Copy)]
struct Phase {
    zero: u64,
    before: usize,
}

impl Watch {
    /// Reads one sample; `None` once the watch is cut (sample bound, or the frontmost app could
    /// not be read: an Accessibility message failure is no observation of the product).
    fn sample<B: Backend>(&mut self, d: &mut Driver<'_, B>) -> Result<Option<usize>, String> {
        if self.cut {
            return Ok(None);
        }
        if self.samples.len() >= MAX_SAMPLES {
            self.cut = true;
            return Ok(None);
        }
        let req = d.backend.now();
        let window = d.window()?;
        let front = match d.front() {
            Ok(front) => front,
            Err(error) => {
                self.cut = true;
                self.unavailable = Some(error);
                return Ok(None);
            }
        };
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
    /// Reads the window just before the move, then moves the pointer; `None` once cut.
    fn move_to<B: Backend>(
        &mut self,
        d: &mut Driver<'_, B>,
        to: Point,
    ) -> Result<Option<Phase>, String> {
        let Some(before) = self.sample(d)? else {
            return Ok(None);
        };
        let zero = d.backend.now();
        d.op(Op::Move(to))?;
        Ok(Some(Phase { zero, before }))
    }
    /// Why the watch stopped early, for a NOT_TESTED detail.
    fn why(&self) -> String {
        match (&self.unavailable, self.cut) {
            (Some(error), _) => format!("frontmost app unreadable: {error}"),
            (None, true) => format!("the watch reached its bound of {MAX_SAMPLES} samples"),
            (None, false) => "watch running".into(),
        }
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
    // 100 ms or more (a late move) may legitimately expand, so it cannot test the cancellation; a
    // trial the sampling could not resolve is repeated (at most `EARLY_TRIALS`), while motion
    // observed in any trial fails at once.
    const EARLY_TRIALS: usize = 3;
    let mut first = None;
    let mut last = 0;
    let mut trials = Vec::new();
    let mut strip_rest = None;
    let mut verdict = Verdict::NotTested;
    for trial in 1..=EARLY_TRIALS {
        let Some(early) = watch.move_to(d, strip)? else {
            break;
        };
        if first.is_none() {
            first = Some(early.before);
            // The Finder PID the backend verified in front when docking; another app in front
            // since then changed the scene (NOT_TESTED), StickyMD in front is the product's doing.
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
        }
        last = early.before;
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
        // Motion already seen before 90 ms is early whatever the dwell became: a long dwell
        // only excuses motion that may have followed 100 ms on the strip.
        let early_motion = moved.is_some_and(|(at, ..)| at < 90);
        verdict = if !is_collapsed(&reference) {
            Verdict::NotTested
        } else if early_motion || (moved.is_some() && dwell < 100) {
            Verdict::Fail
        } else if dwell >= 100 {
            Verdict::NotTested
        } else if !ended || gap > MAX_GAP_MS {
            Verdict::NotTested
        } else {
            Verdict::Pass
        };
        trials.push(format!(
            "trial {trial}: dwell {dwell}ms (100ms or more cannot test it); collapsed before {};              {} samples, largest gap {gap}ms; moved at {moved:?} (ms, bounds, on screen); {}",
            is_collapsed(&reference),
            watch.last() - early.before,
            watch.why()
        ));
        strip_rest = settle(d, &mut watch, is_collapsed)?;
        if verdict != Verdict::NotTested || strip_rest.is_none() || watch.cut {
            break;
        }
    }
    record(d, "early leave cancels", verdict, trials.join("; "));
    let (Some(start), Some(strip_rest)) = (first, strip_rest) else {
        return skip_all(
            d,
            band,
            &format!(
                "the window did not rest on the strip after the early leave; {}",
                watch.why()
            ),
        );
    };
    let Some(hover) = watch.move_to(d, strip)? else {
        return skip_all(d, band, &watch.why());
    };
    let onset = observe_onset(
        d,
        &mut watch,
        hover,
        100,
        is_collapsed,
        &strip_rest,
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
    let hover_end = watch.last();
    let Some(rest) = rest else {
        // No proven rest: only an observed StickyMD in front fails the focus check.
        let (focus, detail) = hover_focus(&watch, d.pid, finder, start, last, hover_end);
        record(d, "hover expands without focus", focus, detail);
        return skip_rest(d, band, "the hover expansion was not established");
    };
    let band_before = if band {
        let p = Point {
            x: rest.bounds.center().x,
            y: s.origin.y,
        };
        d.row.check(
            "pointer in top band",
            top_band(p, rest.bounds, s),
            format!("{p:?}"),
        );
        band_dwell(d, &mut watch, p, &rest)?
    } else {
        None
    };
    // The read just before the leave closes the hover (or the band): its gap, its front app and
    // its place belong to the checks before it, and it is the leave's start.
    let leave = watch.move_to(d, away)?;
    let end = leave.map_or(watch.last(), |l| l.before);
    let hover_to = band_before.unwrap_or(end);
    let (focus, detail) = hover_focus(&watch, d.pid, finder, start, last, hover_to);
    record(d, "hover expands without focus", focus, detail);
    if let Some(from) = band_before {
        let (focus, detail) = watch.focus(d.pid, finder, from, end);
        record(d, "top band dwell keeps focus away", focus, detail);
    }
    let Some(leave) = leave else {
        for name in ["leave movement onset", "leave collapse completion"] {
            d.row.skip(name, &watch.why());
        }
        return Ok(());
    };
    let onset = observe_onset(
        d,
        &mut watch,
        leave,
        500,
        |w| expanded(w, s),
        &rest,
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

/// The hover's focus check: judged over the last early-leave trial and the hover (`last..=to`);
/// a trial repeated because its sampling could not resolve it adds only what it observed, so
/// StickyMD seen in front there still fails, but its gaps are not part of the claim.
fn hover_focus(
    watch: &Watch,
    app: u32,
    finder: u32,
    start: usize,
    last: usize,
    to: usize,
) -> (Verdict, String) {
    let (verdict, detail) = watch.focus(app, finder, last, to);
    match watch.samples[start..last].iter().find(|s| s.front == app) {
        Some(s) => (
            Verdict::Fail,
            format!(
                "StickyMD frontmost at {} in a repeated early-leave trial; {detail}",
                s.ret
            ),
        ),
        None => (verdict, detail),
    }
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

/// Samples until the window has been seen in `state`, at one place, for at least 100 ms (from
/// the return of the first such sample to the request of the latest, every sample in between
/// in that state; the run stops at a gap over `MAX_GAP_MS`, so an old slow read does not keep a
/// later clean run from counting), at most 1.5 s. Returns that settled window.
fn settle<B: Backend>(
    d: &mut Driver<'_, B>,
    watch: &mut Watch,
    state: impl Fn(&Window) -> bool,
) -> Result<Option<Window>, String> {
    let limit = d.backend.now() + 1500;
    loop {
        let Some(last) = watch.sample(d)? else {
            return Ok(None);
        };
        let now = watch.samples[last].window.clone();
        let mut first = last;
        while first > 0 && {
            let w = &watch.samples[first - 1].window;
            state(w)
                && same_place(w, &now)
                && w.layer == now.layer
                && watch.gap(first, first) <= MAX_GAP_MS
        } {
            first -= 1;
        }
        let steady = watch.samples[last]
            .req
            .saturating_sub(watch.samples[first].ret);
        if state(&now) && steady >= 100 && watch.gap(first + 1, last) <= MAX_GAP_MS {
            return Ok(Some(now));
        }
        if watch.samples[last].ret >= limit {
            return Ok(None);
        }
        d.wait(STEP_MS)?;
    }
}

/// W04: with the pointer in the top band the window stays visible at its settled position `rest`
/// for 850 ms from the move. A collapse, a partial retraction or a hidden sample undone before
/// the end still fails. Returns the index of the read before the move, which starts the band's
/// focus range (judged once the read before the leave closes it), if the band was watched.
fn band_dwell<B: Backend>(
    d: &mut Driver<'_, B>,
    watch: &mut Watch,
    p: Point,
    rest: &Window,
) -> Result<Option<usize>, String> {
    const DWELL_MS: u64 = 850;
    let Some(band) = watch.move_to(d, p)? else {
        d.row.skip("top band holds past 500ms", &watch.why());
        d.row.skip("top band dwell keeps focus away", &watch.why());
        return Ok(None);
    };
    if !same_place(&watch.samples[band.before].window, rest) {
        let reason = format!(
            "not at the settled position before the move: {:?}",
            watch.samples[band.before].window
        );
        d.row.skip("top band holds past 500ms", &reason);
        d.row.skip("top band dwell keeps focus away", &reason);
        return Ok(None);
    }
    let ended = watch.until(d, band.zero + DWELL_MS)?;
    let left = watch.samples[band.before + 1..]
        .iter()
        .find(|x| !same_place(&x.window, rest))
        .map(|x| (x.ret - band.zero, x.window.bounds, x.window.onscreen));
    let gap = watch.gap(band.before, watch.last());
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
             {left:?} (ms, bounds, on screen); {}",
            watch.last() - band.before,
            watch.samples[watch.last()].ret - band.zero,
            rest.bounds,
            watch.why()
        ),
    );
    Ok(Some(band.before))
}

fn record<B: Backend>(d: &mut Driver<'_, B>, name: &str, verdict: Verdict, detail: String) {
    if verdict == Verdict::NotTested {
        d.row.skip(name, &detail);
    } else {
        d.row.check(name, verdict == Verdict::Pass, detail);
    }
}

/// Samples until the window, on screen, leaves the place it had before the move, which the
/// contract allows from `target` - 10 ms and requires by `target` + `LATE_MS` after the move.
/// The read before the move must be in the start state (`precondition`) and at `expected`, the
/// place the previous phase established; otherwise the phase cannot be judged. A hidden sample
/// is not motion (it may be the scene, not the product), so a PASS after one cannot be given.
/// The motion is bracketed by the request of the sample before (the move, for the first sample
/// after it) and the return of the first moved sample, so time spent inside the queries counts
/// against the bracket; the gap into the read before the move counts too. Returns that bracket
/// and the moved sample's index.
fn observe_onset<B: Backend>(
    d: &mut Driver<'_, B>,
    watch: &mut Watch,
    phase: Phase,
    target: u64,
    precondition: impl Fn(&Window) -> bool,
    expected: &Window,
    name: &str,
) -> Result<Option<(Bracket, usize)>, String> {
    let reference = watch.samples[phase.before].window.clone();
    if !precondition(&reference) || !same_place(&reference, expected) {
        d.row.skip(
            name,
            &format!(
                "not at the established start before the move; {reference:?}, expected {:?}",
                expected.bounds
            ),
        );
        return Ok(None);
    }
    let lower = target - 10;
    let mut hidden = None;
    loop {
        let Some(i) = watch.sample(d)? else {
            d.row.skip(name, &watch.why());
            return Ok(None);
        };
        let s = &watch.samples[i];
        let (req, ret) = (
            s.req.saturating_sub(phase.zero),
            s.ret.saturating_sub(phase.zero),
        );
        if !s.window.onscreen {
            hidden.get_or_insert(ret);
        } else if !same_place(&s.window, &reference) {
            let old_ms = if i - 1 == phase.before {
                0
            } else {
                watch.samples[i - 1].req - phase.zero
            };
            let bracket = Bracket {
                old_ms,
                new_ms: ret,
            };
            let gap = watch.gap(phase.before, i);
            let verdict = match transition(bracket, target) {
                // A gap before the bracket could hide an earlier motion undone in time, and a
                // hidden sample could hide one too.
                Verdict::Pass if gap > MAX_GAP_MS || hidden.is_some() => Verdict::NotTested,
                verdict => verdict,
            };
            let detail = format!(
                "target={target}ms; bracket={bracket:?}; {} samples, largest gap {gap}ms; hidden \
                 at {hidden:?}ms; {:?}",
                i - phase.before,
                s.window
            );
            record(d, name, verdict, detail);
            return Ok(Some((bracket, i)));
        }
        if req >= target + LATE_MS {
            let detail = format!(
                "still in place when requested at {req}ms (returned {ret}ms); target={target}ms; \
                 hidden at {hidden:?}ms; {:?}",
                s.window
            );
            // Seen on screen in place after the latest allowed onset: late. Hidden: cannot tell.
            let verdict = if s.window.onscreen {
                Verdict::Fail
            } else {
                Verdict::NotTested
            };
            record(d, name, verdict, detail);
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
/// stretch to the next one is covered. The rest is the first sample from which every later one
/// has the last sample's place, visibility and level, so a level reached late is late. Returns
/// the rest only when the completion passed: a later phase starts from a proven end state.
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
        .map(|s| (s.window.bounds, s.window.onscreen, s.window.layer))
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
             {rest:?}; {}",
            tail.len(),
            watch.why()
        ),
    );
    Ok((verdict == Verdict::Pass).then_some(rest))
}
