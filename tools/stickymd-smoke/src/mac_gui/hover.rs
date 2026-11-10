//! Bracketed hover/leave observations; no late sample can establish timely motion.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    model::*,
    observation::{Bracket, LATE_MS, REST_MS, completion_verdict, rest_from, transition},
    scenarios::{Backend, Driver, Op},
};
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
    d.op(Op::Move(strip))?;
    d.wait(35)?;
    d.op(Op::Move(away))?;
    let early_dwell = d.backend.now() - early_start;
    if early_dwell >= 100 {
        d.row.skip(
            "early-leave timing",
            "scheduler delay prevented a sub-100ms dwell",
        );
    }
    d.wait(350)?;
    let w = d.window()?;
    d.row.check(
        "early leave cancels",
        collapsed(&w, s, Edge::Top),
        format!("{w:?}"),
    );
    let start = d.backend.now();
    d.op(Op::Move(strip))?;
    d.wait(70)?;
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
    )?;
    let w = completion(
        d,
        start,
        onset,
        |w| expanded(w, s),
        "hover expansion completion",
    )?;
    let after = d.front()?;
    d.row.check(
        "hover expands without focus",
        expanded(&w, s) && after == front,
        format!("front={front}->{after}; {w:?}"),
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
        d.wait(850)?;
        let stay = d.window()?;
        d.row.check(
            "top band holds past 500ms",
            expanded(&stay, s),
            format!("{stay:?}"),
        );
    }
    let leave = d.backend.now();
    d.op(Op::Move(away))?;
    let onset = observe_transition(d, leave, 500, |w| expanded(w, s), "leave movement onset")?;
    completion(
        d,
        leave,
        onset,
        |w| collapsed(w, s, Edge::Top),
        "leave collapse completion",
    )?;
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
) -> Result<Option<Bracket>, String> {
    let mut old = None;
    loop {
        // Using the request start for the old sample and return time for the new
        // sample includes time spent inside the observation API in the bracket.
        let requested = d.backend.now() - start;
        let w = d.window()?;
        let observed = d.backend.now() - start;
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
        samples.push((observed, w.bounds));
        if observed >= end {
            break w;
        }
        d.wait(20.min(end - observed))?;
    };
    let (first_at, before) = rest_from(&samples).ok_or("no completion samples")?;
    let verdict = completion_verdict(settled(&rest), first_at, before, deadline);
    record(
        d,
        name,
        verdict,
        format!(
            "onset={onset:?}; at rest from {first_at}ms (sample before {before:?}ms); \
             deadline={deadline}ms; {} samples; rest {rest:?}",
            samples.len()
        ),
    );
    Ok(rest)
}
