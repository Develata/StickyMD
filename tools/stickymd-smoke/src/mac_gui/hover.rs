//! Bracketed hover/leave observations; no late sample can establish timely motion. From the first
//! move onto the strip the frontmost app is read at most 20 ms apart (and with every window
//! observation), and the top band is watched for the whole dwell against the settled position, so
//! focus taken and given back, or a collapse or retraction undone in time, is caught; a measured
//! gap over `MAX_GAP_MS` leaves the watch NOT_TESTED.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    model::*,
    observation::{Bracket, LATE_MS, REST_MS, completion_verdict, rest_from, transition},
    scenarios::{Backend, Driver, Op},
};
/// The longest gap between two samples of a continuous watch; a longer one leaves it unobserved.
const MAX_GAP_MS: u64 = 60;

/// A bound on samples in one watch, so a backend whose clock stops cannot loop forever.
const MAX_SAMPLES: usize = 1000;

/// Whether the app became frontmost while watched: each sample is dated when the query returns,
/// in ms from `start`; `first` is the first sample with the app in front, `max_gap` the largest
/// measured distance between samples.
struct Focus {
    app: u32,
    start: u64,
    first: Option<u64>,
    last: u64,
    max_gap: u64,
}

impl Focus {
    fn new(app: u32, start: u64) -> Self {
        Self {
            app,
            start,
            first: None,
            last: 0,
            max_gap: 0,
        }
    }
    fn sample<B: Backend>(&mut self, d: &mut Driver<'_, B>) -> Result<(), String> {
        let front = d.front()?;
        let at = d.backend.now() - self.start;
        if self.first.is_none() && front == self.app {
            self.first = Some(at);
        }
        self.max_gap = self.max_gap.max(at - self.last);
        self.last = at;
        Ok(())
    }
    /// Reads the frontmost app for `ms`, waiting at most 20 ms between reads.
    fn watch<B: Backend>(&mut self, d: &mut Driver<'_, B>, ms: u64) -> Result<(), String> {
        let until = d.backend.now() + ms;
        for _ in 0..MAX_SAMPLES {
            self.sample(d)?;
            let now = d.backend.now();
            if now >= until {
                return Ok(());
            }
            d.wait(20.min(until - now))?;
        }
        self.max_gap = u64::MAX;
        Ok(())
    }
    fn verdict(&self) -> Verdict {
        if self.first.is_some() {
            Verdict::Fail
        } else if self.max_gap > MAX_GAP_MS {
            Verdict::NotTested
        } else {
            Verdict::Pass
        }
    }
    fn detail(&self) -> String {
        format!(
            "StickyMD frontmost from {:?}ms; front read until {}ms, largest gap {}ms",
            self.first, self.last, self.max_gap
        )
    }
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
    let front = d.front()?;
    d.row.check(
        "Finder frontmost before hover",
        front != d.pid,
        format!("front={front}, app={}", d.pid),
    );
    let early_start = d.backend.now();
    let mut focus = Focus::new(d.pid, early_start);
    d.op(Op::Move(strip))?;
    focus.watch(d, 35)?;
    d.op(Op::Move(away))?;
    let early_dwell = d.backend.now() - early_start;
    if early_dwell >= 100 {
        d.row.skip(
            "early-leave timing",
            "scheduler delay prevented a sub-100ms dwell",
        );
    }
    focus.watch(d, 350)?;
    let w = d.window()?;
    d.row.check(
        "early leave cancels",
        collapsed(&w, s, Edge::Top),
        format!("{w:?}"),
    );
    let start = d.backend.now();
    d.op(Op::Move(strip))?;
    focus.watch(d, 70)?;
    let early = d.window()?;
    let early_elapsed = d.backend.now() - start;
    if early_elapsed < 100 {
        d.row.check(
            "no expansion before 100ms",
            collapsed(&early, s, Edge::Top),
            format!("{early_elapsed}ms; {early:?}"),
        );
    } else {
        d.row.skip(
            "pre-100ms observation",
            "scheduler delay missed the observation window",
        );
    }
    let onset = observe_transition(
        d,
        start,
        100,
        |w| collapsed(w, s, Edge::Top),
        "hover movement onset",
        Some(&mut focus),
    )?;
    let w = completion(
        d,
        start,
        onset,
        |w| expanded(w, s),
        "hover expansion completion",
        Some(&mut focus),
    )?;
    let after = d.front()?;
    let verdict = if !expanded(&w, s) || after != front {
        Verdict::Fail
    } else {
        focus.verdict()
    };
    record(
        d,
        "hover expands without focus",
        verdict,
        format!("front={front}->{after}; {}; {w:?}", focus.detail()),
    );
    if band {
        let p = Point {
            x: w.bounds.center().x,
            y: s.origin.y,
        };
        d.row.check(
            "pointer in top band",
            top_band(p, w.bounds, s),
            format!("{p:?}"),
        );
        d.op(Op::Move(p))?;
        band_dwell(d, w.bounds)?;
    }
    let leave = d.backend.now();
    d.op(Op::Move(away))?;
    let onset = observe_transition(
        d,
        leave,
        500,
        |w| expanded(w, s),
        "leave movement onset",
        None,
    )?;
    completion(
        d,
        leave,
        onset,
        |w| collapsed(w, s, Edge::Top),
        "leave collapse completion",
        None,
    )?;
    Ok(())
}

/// W04: with the pointer in the top band the window stays at its settled position `rest`, and
/// never takes focus, for 850 ms; samples are taken at most 20 ms apart and judged on their
/// measured gaps. A collapse or a partial retraction undone before the end still fails.
fn band_dwell<B: Backend>(d: &mut Driver<'_, B>, rest: Rect) -> Result<(), String> {
    const DWELL_MS: u64 = 850;
    const TOLERANCE: f64 = 0.5;
    let start = d.backend.now();
    let mut focus = Focus::new(d.pid, start);
    let at_rest = |w: &Window| {
        (w.bounds.origin.x - rest.origin.x).abs() <= TOLERANCE
            && (w.bounds.origin.y - rest.origin.y).abs() <= TOLERANCE
            && (w.bounds.size.width - rest.size.width).abs() <= TOLERANCE
            && (w.bounds.size.height - rest.size.height).abs() <= TOLERANCE
    };
    let (mut moved_at, mut gap, mut last, mut samples, mut ended) = (None, 0, 0, 0, false);
    let mut seen = None;
    for _ in 0..MAX_SAMPLES {
        let w = d.window()?;
        let at = d.backend.now() - start;
        focus.sample(d)?;
        samples += 1;
        if moved_at.is_none() && !at_rest(&w) {
            moved_at = Some(at);
            seen = Some(w.bounds);
        }
        gap = gap.max(at - last);
        last = at;
        if at >= DWELL_MS {
            ended = true;
            break;
        }
        d.wait(20.min(DWELL_MS - at))?;
    }
    let verdict = if moved_at.is_some() {
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
            "{samples} samples over {last}ms, largest gap {gap}ms, watch ended {ended}; left the settled {rest:?} at {moved_at:?}ms ({seen:?})"
        ),
    );
    let verdict = focus.verdict();
    record(
        d,
        "top band dwell keeps focus away",
        verdict,
        focus.detail(),
    );
    Ok(())
}

fn record<B: Backend>(d: &mut Driver<'_, B>, name: &str, verdict: Verdict, detail: String) {
    if verdict == Verdict::NotTested {
        d.row.skip(name, &detail);
    } else {
        d.row.check(name, verdict == Verdict::Pass, detail);
    }
}

fn observe_transition<B: Backend>(
    d: &mut Driver<'_, B>,
    start: u64,
    target: u64,
    old_state: impl Fn(&Window) -> bool,
    name: &str,
    mut focus: Option<&mut Focus>,
) -> Result<Option<Bracket>, String> {
    let mut old = None;
    loop {
        // Using the request start for the old sample and return time for the new
        // sample includes time spent inside the observation API in the bracket.
        let requested = d.backend.now() - start;
        let w = d.window()?;
        let observed = d.backend.now() - start;
        if let Some(focus) = focus.as_deref_mut() {
            focus.sample(d)?;
        }
        if !old_state(&w) {
            let bracket = old.map(|old_ms| Bracket {
                old_ms,
                new_ms: observed,
            });
            let verdict = bracket.map_or(Verdict::NotTested, |b| transition(b, target));
            record(
                d,
                name,
                verdict,
                format!("target={target}ms; bracket={bracket:?}; {w:?}"),
            );
            return Ok(bracket);
        }
        if observed >= target + LATE_MS {
            let verdict = if observed - requested > 60 {
                Verdict::NotTested
            } else {
                Verdict::Fail
            };
            record(
                d,
                name,
                verdict,
                format!(
                    "still in old state at {requested}..{observed}ms; target={target}ms; {w:?}"
                ),
            );
            return Ok(None);
        }
        old = Some(requested);
        // Sample at the lower acceptance bound too; otherwise a perfect 500ms
        // transition sampled at 480/500ms would unnecessarily straddle 490ms.
        let lower = target - 10;
        d.wait(if observed < lower {
            20.min(lower - observed)
        } else {
            20
        })?;
    }
}

fn completion<B: Backend>(
    d: &mut Driver<'_, B>,
    start: u64,
    onset: Option<Bracket>,
    settled: impl Fn(&Window) -> bool,
    name: &str,
    mut focus: Option<&mut Focus>,
) -> Result<Window, String> {
    let Some(onset) = onset else {
        d.row.skip(name, "no observed onset bracket");
        return d.window();
    };
    // Earliest possible onset bounds the completion deadline conservatively.
    let deadline = onset.old_ms + 140 + LATE_MS;
    let end = deadline + REST_MS;
    let mut samples = Vec::new();
    let rest = loop {
        let w = d.window()?;
        let observed = d.backend.now() - start;
        if let Some(focus) = focus.as_deref_mut() {
            focus.sample(d)?;
        }
        samples.push((observed, w.bounds));
        if observed >= end {
            break w;
        }
        d.wait(20.min(end - observed))?;
    };
    let (first_at, before) = rest_from(&samples).ok_or("no completion samples")?;
    // A PASS needs the completion watched without a long gap between window samples.
    let gap = samples
        .windows(2)
        .map(|pair| pair[1].0 - pair[0].0)
        .max()
        .unwrap_or(0);
    let verdict = match completion_verdict(settled(&rest), first_at, before, deadline) {
        Verdict::Pass if gap > MAX_GAP_MS => Verdict::NotTested,
        verdict => verdict,
    };
    record(
        d,
        name,
        verdict,
        format!(
            "onset={onset:?}; at rest from {first_at}ms (sample before {before:?}ms); \
             deadline={deadline}ms; {} samples, largest gap {gap}ms; rest {rest:?}",
            samples.len()
        ),
    );
    Ok(rest)
}
