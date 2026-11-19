//! Bracketed hover/leave observations; no late sample can establish timely motion. Each phase is
//! one continuous track whose zero is the request of the pointer move it observes: the window,
//! and while the pointer is on the strip also the frontmost app, are sampled about `STEP_MS`
//! apart, and every check of the phase reads that one stream, so a motion or a focus change
//! undone between two checks is still seen. A measured gap over `MAX_GAP_MS` in what a check
//! covers leaves it NOT_TESTED, and so does a changed scene (another app in front); a FAIL needs
//! an observed violation.
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

/// A bound on samples in one track, so a backend whose clock stops cannot loop forever.
const MAX_SAMPLES: usize = 1000;

/// One observation, in ms from its track's zero: requested at `req`, returned at `ret`.
struct Sample {
    req: u64,
    ret: u64,
    window: Window,
    front: Option<u32>,
}

/// A continuous watch of one phase. `before` is the window read just before the move, the zero
/// is the move's request, so the move itself lies inside the first gap. `cut` is set when the
/// sample bound ran out before the watch reached its end.
struct Track {
    start: u64,
    focus: bool,
    before: Window,
    samples: Vec<Sample>,
    cut: bool,
}

impl Track {
    fn begin<B: Backend>(d: &mut Driver<'_, B>, focus: bool, to: Point) -> Result<Self, String> {
        let before = d.window()?;
        let start = d.backend.now();
        d.op(Op::Move(to))?;
        Ok(Self {
            start,
            focus,
            before,
            samples: Vec::new(),
            cut: false,
        })
    }
    fn now<B: Backend>(&self, d: &Driver<'_, B>) -> u64 {
        d.backend.now() - self.start
    }
    fn sample<B: Backend>(&mut self, d: &mut Driver<'_, B>) -> Result<&Sample, String> {
        let req = self.now(d);
        let window = d.window()?;
        let front = if self.focus { Some(d.front()?) } else { None };
        let ret = self.now(d);
        self.samples.push(Sample {
            req,
            ret,
            window,
            front,
        });
        Ok(&self.samples[self.samples.len() - 1])
    }
    /// Samples until one returns at or after `ms` from the zero.
    fn watch_until<B: Backend>(&mut self, d: &mut Driver<'_, B>, ms: u64) -> Result<(), String> {
        for _ in 0..MAX_SAMPLES {
            let ret = self.sample(d)?.ret;
            if ret >= ms {
                return Ok(());
            }
            d.wait(STEP_MS.min(ms - ret))?;
        }
        self.cut = true;
        Ok(())
    }
    /// The longest unobserved stretch from sample `from` on: from the request of the sample
    /// before (the zero, for the first sample) to the return of the next. None is unobserved.
    fn gap(&self, from: usize) -> u64 {
        (from..self.samples.len())
            .map(|i| self.samples[i].ret - i.checked_sub(1).map_or(0, |p| self.samples[p].req))
            .max()
            .unwrap_or(u64::MAX)
    }
    fn last_ret(&self) -> u64 {
        self.samples.last().map_or(0, |s| s.ret)
    }
}

/// Whether the product took focus in the tracks (all watching focus): FAIL when it was frontmost
/// in any sample; NOT_TESTED when an app other than `front` was (the scene changed under the
/// check), when a gap exceeds `MAX_GAP_MS`, or when a watch was cut.
fn focus_verdict(app: u32, tracks: &[&Track], front: u32) -> (Verdict, String) {
    let (mut taken, mut other, mut gap, mut reads, mut cut) = (None, None, 0, 0, false);
    for (k, t) in tracks.iter().enumerate() {
        gap = gap.max(t.gap(0));
        cut |= t.cut;
        for s in &t.samples {
            let Some(f) = s.front else { continue };
            reads += 1;
            if f == app {
                taken.get_or_insert((k, s.ret));
            } else if f != front {
                other.get_or_insert((k, s.ret, f));
            }
        }
    }
    let verdict = if taken.is_some() {
        Verdict::Fail
    } else if other.is_some() || gap > MAX_GAP_MS || cut || reads == 0 {
        Verdict::NotTested
    } else {
        Verdict::Pass
    };
    (
        verdict,
        format!(
            "{reads} front reads; StickyMD frontmost at {taken:?}, another app at {other:?} \
             (track, ms[, PID]); largest gap {gap}ms; cut {cut}"
        ),
    )
}

pub(super) fn run<B: Backend>(d: &mut Driver<'_, B>, band: bool) -> Result<(), String> {
    let (s, w) = super::scenarios::dock(d, Edge::Top)?;
    let strip = Point {
        x: w.bounds.center().x,
        y: s.origin.y + 1.0,
    };
    let away = Point {
        x: s.right() - 20.0,
        y: s.bottom() - 100.0,
    };
    let is_collapsed = |w: &Window| collapsed(w, s, Edge::Top);
    let front = d.front()?;
    d.row.check(
        "Finder frontmost before hover",
        front != d.pid,
        format!("front={front}, app={}", d.pid),
    );
    // Early leave: the pointer leaves the strip after about 35 ms and nothing may move. A dwell of
    // 100 ms or more (a late move) may legitimately expand, so it cannot test the cancellation.
    let mut early = Track::begin(d, true, strip)?;
    early.watch_until(d, 35)?;
    d.op(Op::Move(away))?;
    let dwell = early.now(d);
    early.watch_until(d, dwell + 350)?;
    let moved = early
        .samples
        .iter()
        .find(|x| !is_collapsed(&x.window))
        .map(|x| (x.ret, x.window.bounds));
    let gap = early.gap(0);
    let verdict = if !is_collapsed(&early.before) || dwell >= 100 {
        Verdict::NotTested
    } else if moved.is_some() {
        Verdict::Fail
    } else if early.cut || gap > MAX_GAP_MS {
        Verdict::NotTested
    } else {
        Verdict::Pass
    };
    record(
        d,
        "early leave cancels",
        verdict,
        format!(
            "dwell {dwell}ms (100ms or more cannot test it); collapsed before {}; {} samples over \
             {}ms, largest gap {gap}ms; left the collapsed strip at {moved:?}",
            is_collapsed(&early.before),
            early.samples.len(),
            early.last_ret()
        ),
    );
    if !early
        .samples
        .last()
        .is_some_and(|x| is_collapsed(&x.window))
    {
        settle(d, is_collapsed)?;
    }
    let mut hover = Track::begin(d, true, strip)?;
    let onset = observe_onset(d, &mut hover, 100, is_collapsed, "hover movement onset")?;
    let rest = completion(
        d,
        &mut hover,
        onset,
        |w| expanded(w, s),
        "hover expansion completion",
    )?;
    let (verdict, detail) = focus_verdict(d.pid, &[&early, &hover], front);
    let verdict = if expanded(&rest, s) {
        verdict
    } else {
        Verdict::Fail
    };
    record(
        d,
        "hover expands without focus",
        verdict,
        format!("front={front}; {detail}; {rest:?}"),
    );
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
        band_dwell(d, p, rest.bounds, front)?;
    }
    let mut leave = Track::begin(d, false, away)?;
    let onset = observe_onset(
        d,
        &mut leave,
        500,
        |w| expanded(w, s),
        "leave movement onset",
    )?;
    completion(
        d,
        &mut leave,
        onset,
        is_collapsed,
        "leave collapse completion",
    )?;
    Ok(())
}

/// Waits up to 1.5 s for the window to return to `state`, so the next phase starts from it.
fn settle<B: Backend>(
    d: &mut Driver<'_, B>,
    state: impl Fn(&Window) -> bool,
) -> Result<(), String> {
    for _ in 0..30 {
        if state(&d.window()?) {
            return Ok(());
        }
        d.wait(50)?;
    }
    Err("the window did not return to the strip within 1.5s after the early leave".into())
}

/// W04: with the pointer in the top band the window stays visible at its settled position `rest`
/// and never takes focus for 850 ms from the move. A collapse, a partial retraction or a hidden
/// sample undone before the end still fails.
fn band_dwell<B: Backend>(
    d: &mut Driver<'_, B>,
    p: Point,
    rest: Rect,
    front: u32,
) -> Result<(), String> {
    const DWELL_MS: u64 = 850;
    const TOLERANCE: f64 = 0.5;
    let at_rest = |w: &Window| {
        w.onscreen
            && (w.bounds.origin.x - rest.origin.x).abs() <= TOLERANCE
            && (w.bounds.origin.y - rest.origin.y).abs() <= TOLERANCE
            && (w.bounds.size.width - rest.size.width).abs() <= TOLERANCE
            && (w.bounds.size.height - rest.size.height).abs() <= TOLERANCE
    };
    let mut t = Track::begin(d, true, p)?;
    t.watch_until(d, DWELL_MS)?;
    let left = t
        .samples
        .iter()
        .find(|x| !at_rest(&x.window))
        .map(|x| (x.ret, x.window.bounds, x.window.onscreen));
    let gap = t.gap(0);
    let verdict = if left.is_some() {
        Verdict::Fail
    } else if t.cut || gap > MAX_GAP_MS {
        Verdict::NotTested
    } else {
        Verdict::Pass
    };
    record(
        d,
        "top band holds past 500ms",
        verdict,
        format!(
            "{} samples over {}ms, largest gap {gap}ms, cut {}; left the settled {rest:?} \
             (or hid) at {left:?}",
            t.samples.len(),
            t.last_ret(),
            t.cut
        ),
    );
    let (verdict, detail) = focus_verdict(d.pid, &[&t], front);
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

/// Samples the track until the window leaves `old_state`, which the contract allows from
/// `target` - 10 ms and requires by `target` + `LATE_MS` after the move. The motion is bracketed
/// by the request of the sample before (the move, for the first) and the return of the first
/// sample out of the old state, so time spent inside the queries counts against the bracket.
/// Returns that bracket and the index of the first sample out of the old state.
fn observe_onset<B: Backend>(
    d: &mut Driver<'_, B>,
    t: &mut Track,
    target: u64,
    old_state: impl Fn(&Window) -> bool,
    name: &str,
) -> Result<Option<(Bracket, usize)>, String> {
    if !old_state(&t.before) {
        d.row.skip(
            name,
            &format!("not in the old state before the move; {:?}", t.before),
        );
        return Ok(None);
    }
    let lower = target - 10;
    for _ in 0..MAX_SAMPLES {
        let (req, ret, moved) = {
            let s = t.sample(d)?;
            (s.req, s.ret, !old_state(&s.window))
        };
        let i = t.samples.len() - 1;
        if moved {
            let bracket = Bracket {
                old_ms: i.checked_sub(1).map_or(0, |p| t.samples[p].req),
                new_ms: ret,
            };
            let gap = t.gap(0);
            let verdict = match transition(bracket, target) {
                // A gap before the bracket could hide an earlier motion undone in time.
                Verdict::Pass if gap > MAX_GAP_MS => Verdict::NotTested,
                verdict => verdict,
            };
            let detail = format!(
                "target={target}ms; bracket={bracket:?}; {} samples, largest gap {gap}ms; {:?}",
                i + 1,
                t.samples[i].window
            );
            record(d, name, verdict, detail);
            return Ok(Some((bracket, i)));
        }
        if req >= target + LATE_MS {
            let detail = format!(
                "still in the old state when requested at {req}ms (returned {ret}ms); \
                 target={target}ms; {:?}",
                t.samples[i].window
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
    t.cut = true;
    d.row
        .skip(name, &format!("watch cut after {MAX_SAMPLES} samples"));
    Ok(None)
}

/// Continues the track from the onset sample until `REST_MS` past the completion deadline, which
/// the earliest possible onset bounds. The onset sample is the first of the completion samples,
/// so the stretch to the next sample is covered and a first rest sample returned late is dated
/// against it rather than taken as late.
fn completion<B: Backend>(
    d: &mut Driver<'_, B>,
    t: &mut Track,
    onset: Option<(Bracket, usize)>,
    settled: impl Fn(&Window) -> bool,
    name: &str,
) -> Result<Window, String> {
    let Some((onset, first)) = onset else {
        d.row.skip(name, "no observed onset bracket");
        return d.window();
    };
    let deadline = onset.old_ms + 140 + LATE_MS;
    t.watch_until(d, deadline + REST_MS)?;
    let tail = &t.samples[first..];
    let states: Vec<_> = tail
        .iter()
        .map(|s| (s.window.bounds, s.window.onscreen))
        .collect();
    let k = rest_index(&states).ok_or("no completion samples")?;
    let rest = tail[tail.len() - 1].window.clone();
    let first_at = tail[k].ret;
    let before = k.checked_sub(1).map(|j| tail[j].req);
    let gap = t.gap(first + 1);
    let coarse = onset.new_ms - onset.old_ms > MAX_GAP_MS;
    let verdict = match completion_verdict(settled(&rest), first_at, before, deadline) {
        _ if t.cut => Verdict::NotTested,
        Verdict::Pass if gap > MAX_GAP_MS => Verdict::NotTested,
        // Late only against a deadline the coarse onset bracket cannot fix.
        Verdict::Fail if settled(&rest) && coarse => Verdict::NotTested,
        verdict => verdict,
    };
    record(
        d,
        name,
        verdict,
        format!(
            "onset={onset:?}; at rest from {first_at}ms (sample before requested at {before:?}ms); \
             deadline={deadline}ms; {} samples, largest gap {gap}ms; rest {rest:?}",
            tail.len()
        ),
    );
    Ok(rest)
}
