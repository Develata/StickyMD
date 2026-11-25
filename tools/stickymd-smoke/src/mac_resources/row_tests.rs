//! P16-R01 and P16-R02 from a campaign's runs, and the reader the ledger uses, against bad
//! samples: each changes one fact of a good campaign.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use super::model::SCENARIOS;
use super::report::Evidence;
use super::rows::{RowVerdict, from_evidence};
use super::tests::{BUSY_TICKS, calibration, evidence, measured};
use crate::release::json;

fn verdicts(evidence: &Evidence) -> [(&'static str, &'static str); 2] {
    evidence.rows().map(|row| (row.row, row.verdict))
}

fn read(text: &str) -> Result<[RowVerdict; 2], String> {
    from_evidence(&json::parse(text).unwrap())
}

fn refused(text: &str, words: &str) {
    let error = read(text).unwrap_err();
    assert!(error.contains(words), "{words}: {error}");
}

#[test]
fn a_complete_campaign_within_both_lines_passes_both_rows() {
    let good = evidence();
    assert_eq!(verdicts(&good), [("R01", "PASS"), ("R02", "PASS")]);
    // Every run reported its scenario's view (review F7, USER 2026-11-24): a PASS.
    let rows = read(&good.json()).unwrap();
    assert_eq!(rows.clone().map(|row| row.verdict), ["PASS", "PASS"]);
    assert!(good.passed());
    assert!(rows[0].detail.contains("hidden max"), "{:?}", rows[0]);
}

#[test]
fn one_run_over_its_memory_gate_fails_r01_alone_and_names_the_bytes() {
    let mut over = evidence();
    let limit = SCENARIOS[2].limit;
    over.scenarios[2][4] = measured("split", 5, limit + 1, super::tests::IDLE_TICKS);
    assert_eq!(verdicts(&over), [("R01", "FAIL"), ("R02", "PASS")]);
    // One byte over is visible in the detail, not rounded away.
    assert!(
        over.rows()[0]
            .detail
            .contains("split 134217729 B (128.000 MiB) > 134217728 B"),
        "{:?}",
        over.rows()[0]
    );
    assert!(!over.passed());
    // The evidence written for it reads back the same.
    assert_eq!(read(&over.json()).unwrap()[0].verdict, "FAIL");
    // At the gate is within it.
    over.scenarios[2][4] = measured("split", 5, limit, super::tests::IDLE_TICKS);
    assert_eq!(verdicts(&over), [("R01", "PASS"), ("R02", "PASS")]);
}

/// The history of the row: periodic caret-layer toggling gave Split a median of 0.103%. Each case
/// changes the runs' raw samples, which the rule reads, and is read back from its JSON.
#[test]
fn a_median_idle_cpu_above_the_hard_line_fails_r02_and_a_single_spike_does_not() {
    let with_ticks = |scenario: usize, ticks: &dyn Fn(usize) -> u64, window: f64| {
        let mut evidence = evidence();
        let limit = SCENARIOS[scenario].limit;
        for (index, run) in evidence.scenarios[scenario].iter_mut().enumerate() {
            *run = super::tests::measured_over(
                SCENARIOS[scenario].mode,
                index + 1,
                limit,
                ticks(index),
                window,
            );
        }
        let verdict = verdicts(&evidence)[1].1;
        assert_eq!(
            read(&evidence.json()).unwrap()[1].verdict,
            verdict,
            "read back"
        );
        (verdict, evidence.rows()[1].detail.clone())
    };
    let (verdict, detail) = with_ticks(2, &|_| BUSY_TICKS, 60.0);
    assert_eq!(verdict, "FAIL");
    assert!(
        detail.contains("split median at least 0.103000% in the first 60 s window"),
        "{detail}"
    );
    // One run fifty times busier does not move the median.
    let spike = |index: usize| {
        if index == 0 {
            50 * BUSY_TICKS
        } else {
            super::tests::IDLE_TICKS
        }
    };
    assert_eq!(with_ticks(2, &spike, 60.0).0, "PASS");
    // Exactly 60 ms of CPU (1,440,000 ticks at 125/3) is at the line, and within it; one tick
    // more is over it. Compared in raw ticks, the review's late sample (the second at
    // 91.500012 s) whose percent of the budget reads 0.10000000000000002 is still at the line.
    const AT_THE_LINE: u64 = 1_440_000;
    assert_eq!(with_ticks(1, &|_| AT_THE_LINE, 60.0).0, "PASS");
    assert_eq!(with_ticks(1, &|_| AT_THE_LINE + 1, 60.0).0, "FAIL");
    assert_eq!(with_ticks(1, &|_| AT_THE_LINE, 60.000012).0, "PASS");
}

/// The exact median against the 60 ms budget (in nanoseconds, timebase denominator 1): the
/// middle value of an odd count, the two middle values' sum against twice the budget for an
/// even one, as a campaign of six repetitions has.
#[test]
fn the_median_is_compared_with_the_budget_exactly() {
    use super::rows::median_over;
    let ms = |n: u128| n * 1_000_000;
    assert!(!median_over(vec![ms(1), ms(60), ms(90)], 1));
    assert!(median_over(vec![ms(1), ms(60) + 1, ms(90)], 1));
    // Six runs: 59.9 and 60.1 ms in the middle average to the line, within it; 60 and 60.2 ms
    // average above it.
    let six = |a: u128, b: u128| vec![ms(1), ms(2), a, b, ms(90), ms(95)];
    assert!(!median_over(six(59_900_000, 60_100_000), 1));
    assert!(median_over(six(ms(60), 60_200_000), 1));
    // The denominator scales the budget, not the values.
    assert!(!median_over(vec![ms(60) * 3], 3));
    assert!(median_over(vec![ms(60) * 3 + 1], 3));
}

#[test]
fn a_missing_invalid_or_uncleaned_run_leaves_both_rows_not_tested() {
    let mut short = evidence();
    short.scenarios[1].pop();
    assert_eq!(
        verdicts(&short),
        [("R01", "NOT_TESTED"), ("R02", "NOT_TESTED")]
    );
    assert!(
        short.rows()[0]
            .detail
            .contains("preview (4 of 5 runs valid)")
    );
    let mut exited = evidence();
    exited.scenarios[0][1].exited(3.0, "signal: 9".into());
    assert_eq!(
        verdicts(&exited),
        [("R01", "NOT_TESTED"), ("R02", "NOT_TESTED")]
    );
    // A run that measured but was not cleaned up is not valid, whatever it read.
    let mut uncleaned = evidence();
    uncleaned.scenarios[3][0].cleanup_error = Some("not reaped".into());
    uncleaned.scenarios[3][0]
        .metrics
        .as_mut()
        .unwrap()
        .footprint_bytes = u64::MAX;
    assert_eq!(
        verdicts(&uncleaned),
        [("R01", "NOT_TESTED"), ("R02", "NOT_TESTED")]
    );
    let mut partial = evidence();
    partial.scenarios.truncate(3);
    assert_eq!(
        verdicts(&partial),
        [("R01", "NOT_TESTED"), ("R02", "NOT_TESTED")]
    );
    assert!(partial.rows()[1].detail.contains("3 of 4 scenarios"));
    // Fewer repetitions than the plan's five are not a campaign.
    let mut few = evidence();
    few.repetitions = 4;
    for runs in &mut few.scenarios {
        runs.pop();
    }
    assert_eq!(
        verdicts(&few),
        [("R01", "NOT_TESTED"), ("R02", "NOT_TESTED")]
    );
    // A violation stays a FAIL in an incomplete campaign.
    short.scenarios[0][0] = measured("source", 1, u64::MAX / 2, super::tests::IDLE_TICKS);
    assert_eq!(verdicts(&short), [("R01", "FAIL"), ("R02", "NOT_TESTED")]);
}

/// Review S3: evidence marked finished whose bytes list three scenarios is refused by the reader,
/// even with the rows it states for them. `Evidence::json` writes every scenario of the tool, so
/// the fourth is cut from the text (pre-freeze, producers item 5 (third review)).
#[test]
fn a_finished_campaign_whose_evidence_lists_three_scenarios_is_refused() {
    let mut partial = evidence();
    partial.scenarios.truncate(3);
    assert!(partial.finished);
    let written = partial.json();
    let fourth = written.rfind(",{\"scenario\":\"hidden\"").unwrap();
    let text = format!("{}]}}\n", &written[..fourth]);
    assert_eq!(text.matches("\"scenario\":\"").count(), 3);
    assert!(text.contains("3 of 4 scenarios measured"), "{text}");
    refused(&text, "a finished campaign lists 3 of 4 scenarios");
}

/// `10`: memory is measured at scale 2, and every measurement on macOS 27.
#[test]
fn outside_plan_10s_environment_the_rows_are_not_tested() {
    let mut scale_1 = evidence();
    scale_1.conditions.scale = [1.0, 1.0];
    assert_eq!(verdicts(&scale_1), [("R01", "NOT_TESTED"), ("R02", "PASS")]);
    assert!(scale_1.rows()[0].detail.contains("at scale 1x1"));
    let mut other_system = evidence();
    other_system.conditions.macos_version = "26.4".into();
    assert_eq!(
        verdicts(&other_system),
        [("R01", "NOT_TESTED"), ("R02", "NOT_TESTED")]
    );
    // "270.1" is not macOS 27.
    other_system.conditions.macos_version = "270.1".into();
    assert_eq!(verdicts(&other_system)[1].1, "NOT_TESTED");
    // Even over its gate: a FAIL measured elsewhere does not stand for macOS 27 at scale 2.
    scale_1.scenarios[0][0] = measured("source", 1, u64::MAX / 2, super::tests::IDLE_TICKS);
    assert_eq!(verdicts(&scale_1)[0].1, "NOT_TESTED");
    // The reader takes the environment from the evidence, not from the rows written beside it.
    let good = evidence().json();
    let text = good.replacen("\"scale_x\":2", "\"scale_x\":1", 1);
    assert_ne!(text, good);
    refused(&text, "pixels over its points");
    let text = good.replacen(
        "\"macos_version\":\"27.2\"",
        "\"macos_version\":\"26.4\"",
        1,
    );
    assert_ne!(text, good);
    refused(&text, "disagrees with itself");
}

/// Every reading is recomputed from the run's two raw samples and the Mach timebase.
#[test]
fn the_reader_recomputes_readings_from_raw_samples() {
    let good = evidence().json();
    // No samples behind a VALID run: the readings beside it prove nothing.
    let mut empty = evidence();
    empty.scenarios[0][0].samples.clear();
    refused(&empty.json(), "has 0 samples");
    // A reading written over its gate's line beside samples that give another: refused.
    let limit = SCENARIOS[0].limit;
    let mut forged = evidence();
    forged.scenarios[0][0]
        .metrics
        .as_mut()
        .unwrap()
        .footprint_bytes = limit - 1;
    refused(&forged.json(), "differ from those its samples give");
    let mut forged = evidence();
    forged.scenarios[2][0]
        .metrics
        .as_mut()
        .unwrap()
        .idle_cpu_percent = 0.0;
    refused(&forged.json(), "differ from those its samples give");
    // A raw counter edited without its reading.
    let text = good.replacen("\"user_ticks\":8200", "\"user_ticks\":9200", 1);
    assert_ne!(text, good);
    refused(&text, "differ from those its samples give");
    // Another timebase gives other readings.
    let text = good.replacen("\"numer\":125", "\"numer\":250", 1);
    assert_ne!(text, good);
    refused(&text, "differ from those its samples give");
    // A window past 60.5 s is no `10` reading, however the readings were written.
    let mut late = evidence();
    late.scenarios[1][2].samples[2].elapsed_seconds = 150.0;
    refused(&late.json(), "give no reading");
    // The middle sample as the driver takes it: not before 59.9 s, before the last one, its
    // counters between the others; two samples are not a run of this tool.
    let mut early = evidence();
    early.scenarios[1][2].samples[1].elapsed_seconds -= 1.0;
    refused(&early.json(), "middle sample is not one the driver takes");
    let mut after = evidence();
    after.scenarios[1][2].samples[1].elapsed_seconds = 31.5 + 60.25;
    refused(&after.json(), "middle sample is not one the driver takes");
    let mut counters = evidence();
    counters.scenarios[1][2].samples[1].user_ticks += 1_000_000;
    refused(&counters.json(), "counters do not sit between the others");
    let mut two = evidence();
    two.scenarios[1][2].samples.remove(1);
    refused(&two.json(), "the tool takes exactly three");
    // A run's status changed: the rows no longer hold.
    let errored = good.replacen("\"status\":\"VALID\"", "\"status\":\"ERROR\"", 1);
    refused(&errored, "its runs give");
    // Rows claimed by a status field are not taken: a FAIL stated where the runs give PASS is
    // refused, and so is another reason for the same verdict.
    let claimed = good.replacen(
        "\"id\":\"P16-R02\",\"verdict\":\"PASS\"",
        "\"id\":\"P16-R02\",\"verdict\":\"FAIL\"",
        1,
    );
    assert_ne!(claimed, good);
    refused(&claimed, "its runs give");
    let reason = good.replacen("all runs within the memory gates", "all runs fine", 1);
    assert_ne!(reason, good);
    refused(&reason, "its runs give");
}

#[test]
fn the_reader_refuses_runs_that_are_not_the_repetitions_in_order() {
    let mut repeated = evidence();
    repeated.scenarios[1][1].repetition = 1;
    refused(&repeated.json(), "repetitions 1..=5 in order");
    let mut extra = evidence();
    extra.scenarios[0].push(measured("source", 6, 1024, super::tests::IDLE_TICKS));
    // A finished campaign ran each scenario exactly its repetitions (review S3); a checkpoint
    // cannot list more runs than repetitions either.
    refused(&extra.json(), "ran other than 5 times");
    extra.finished = false;
    refused(&extra.json(), "6 runs for 5 repetitions");
}

/// The tool stops at the first failed cleanup, before it marks the campaign finished.
#[test]
fn a_finished_campaign_with_a_cleanup_error_is_refused() {
    let mut uncleaned = evidence();
    uncleaned.scenarios[3][4].cleanup_error = Some("not reaped".into());
    uncleaned.scenarios[3][4].status = "ERROR";
    refused(&uncleaned.json(), "cleanup error");
    // The same run in a checkpoint is what the tool writes when it stops.
    uncleaned.finished = false;
    assert_eq!(read(&uncleaned.json()).unwrap()[0].verdict, "NOT_TESTED");
    // A VALID run cannot carry a cleanup error.
    let mut valid = evidence();
    valid.finished = false;
    valid.scenarios[0][0].cleanup_error = Some("not reaped".into());
    refused(&valid.json(), "VALID run with a cleanup error");
}

#[test]
fn the_reader_refuses_other_gates_scenarios_or_timings() {
    let good = evidence().json();
    let samples = [
        (
            format!("\"memory_limit_bytes\":{}", SCENARIOS[3].limit),
            format!("\"memory_limit_bytes\":{}", SCENARIOS[3].limit * 2),
            "not this tool's",
        ),
        (
            "\"scenario\":\"split\"".into(),
            "\"scenario\":\"wide\"".into(),
            "not this tool's",
        ),
        (
            "\"hidden_through_menu_bar\":true".into(),
            "\"hidden_through_menu_bar\":false".into(),
            "not this tool's",
        ),
        (
            "\"idle_seconds\":60".into(),
            "\"idle_seconds\":10".into(),
            "warm-up or idle",
        ),
        (
            "\"warmup_seconds\":30".into(),
            "\"warmup_seconds\":5".into(),
            "warm-up or idle",
        ),
    ];
    for (from, to, words) in samples {
        assert!(good.contains(&from), "{from}");
        refused(&good.replacen(&from, &to, 1), words);
    }
}
/// Review B1: a VALID run must have lived as the driver drives it; S3: a finished campaign ran
/// everything, and only this tool's statuses; and the note measured is the tool's fixture.
#[test]
fn the_reader_refuses_runs_the_driver_cannot_produce() {
    type Change = fn(&mut Evidence);
    let samples: [(&str, Change); 9] = [
        ("warm-up", |e| e.scenarios[0][0].ready_seconds = Some(2.0)),
        ("names no process", |e| e.scenarios[1][0].pid = None),
        ("records an exit", |e| {
            e.scenarios[2][0].exit_status = Some("exit status: 0".into())
        }),
        ("readiness wait", |e| {
            e.scenarios[0][1].ready_seconds = Some(31.0)
        }),
        ("hiding", |e| e.scenarios[3][0].hidden_seconds = None),
        ("hiding", |e| super::tests::hide(&mut e.scenarios[0][2])),
        ("warm-up", |e| e.scenarios[3][1].hidden_seconds = Some(5.0)),
        ("ran other than 5 times", |e| e.scenarios.truncate(3)),
        ("another note", |e| e.fixture_hash = "b".repeat(64)),
    ];
    for (words, change) in samples {
        let mut evidence = evidence();
        change(&mut evidence);
        refused(&evidence.json(), words);
    }
    let good = evidence().json();
    refused(
        &good.replacen("\"status\":\"VALID\"", "\"status\":\"FINE\"", 1),
        "not one this tool writes",
    );
}

/// Review B3: the system and the scale are judged from one environment record whose parts agree.
#[test]
fn the_environment_must_agree_with_itself() {
    let good = evidence().json();
    for (from, to, words) in [
        (
            "\"os_build\":\"26B123\"",
            "\"os_build\":\"26C1\"",
            "disagrees with itself",
        ),
        (
            "BuildVersion:\\t26B123",
            "BuildVersion:\\t26C1",
            "disagrees with itself",
        ),
        (
            "ProductVersion:\\t27.2",
            "ProductVersion:\\t26.4",
            "disagrees with itself",
        ),
        (
            "\"pixel_width\":2048",
            "\"pixel_width\":1024",
            "pixels over its points",
        ),
    ] {
        assert!(good.contains(from), "{from}");
        refused(&good.replacen(from, to, 1), words);
    }
    let both = good
        .replacen("ProductVersion:\\t27.2", "ProductVersion:\\t27.2b", 1)
        .replacen(
            "\"macos_version\":\"27.2\"",
            "\"macos_version\":\"27.2b\"",
            1,
        );
    refused(&both, "not a macOS version");
    // Two displays: the recorded scale need not be the window's, so R01 is not judged.
    let mut two = evidence();
    two.conditions.displays = 2;
    assert_eq!(verdicts(&two), [("R01", "NOT_TESTED"), ("R02", "PASS")]);
    refused(
        &good.replacen("\"display_count\":1", "\"display_count\":2", 1),
        "its runs give",
    );
}

/// Review B8 and round 3 B3: a second sample up to 0.5 s late dilutes the average; holding needs
/// the CPU time within the 60 s budget, and a FAIL a 60 s window shown above the line, which only
/// a window of exactly 60 s shows. A later sample above the budget is NOT_TESTED: the review's
/// 31 ms at each end of a 60.5 s window averages 0.1025% yet no 60 s window exceeds 0.052%. The
/// median is of five runs: three high and two low fail, two and three hold; with fewer than five
/// valid runs nothing fails.
#[test]
fn a_late_sample_cannot_turn_a_hard_failure_into_a_pass() {
    use super::tests::measured_over;
    // CPU seconds over a 60.5 s window: 0.0603 reads 0.09967% of the window, 0.1005% of 60 s.
    let ticks = |cpu_seconds: f64| (cpu_seconds * 1e9 * 3.0 / 125.0).round() as u64;
    let with = |cpu_seconds: f64, window: f64| {
        let mut evidence = evidence();
        let limit = SCENARIOS[1].limit;
        for (index, run) in evidence.scenarios[1].iter_mut().enumerate() {
            *run = measured_over("preview", index + 1, limit, ticks(cpu_seconds), window);
        }
        evidence
    };
    let undecided = with(0.0603, 60.5);
    assert_eq!(
        verdicts(&undecided),
        [("R01", "PASS"), ("R02", "NOT_TESTED")]
    );
    assert!(
        undecided.rows()[1].detail.contains("cannot be told"),
        "{:?}",
        undecided.rows()[1]
    );
    assert_eq!(read(&undecided.json()).unwrap()[1].verdict, "NOT_TESTED");
    // Spread evenly, 62 ms over 60.5 s holds 61.4 ms before the middle sample at 59.9 s: the
    // first 60 s window is shown above the line, a FAIL even though the window ran late
    // (USER 2026-11-24, option A of review round 3 B3).
    let over = with(0.062, 60.5);
    assert_eq!(verdicts(&over)[1].1, "FAIL");
    assert_eq!(read(&over.json()).unwrap()[1].verdict, "FAIL");
    assert_eq!(verdicts(&with(0.0599, 60.5))[1].1, "PASS");
    assert_eq!(verdicts(&with(0.0603, 60.0))[1].1, "FAIL");
    // The review's counterexample: 31 ms at the start and 31 ms in the last 31 ms of a 60.5 s
    // window. No 60 s window exceeds the line, and none is shown to: the middle sample has 31 ms,
    // the last 62 ms, so the scenario is NOT_TESTED, never a FAIL.
    let mut ends = with(0.062, 60.5);
    let half = ticks(0.031);
    for run in &mut ends.scenarios[1] {
        let start = run.samples[0].clone();
        run.samples[1].user_ticks = start.user_ticks + half / 2;
        run.samples[1].system_ticks = start.system_ticks + half - half / 2;
    }
    assert_eq!(verdicts(&ends)[1].1, "NOT_TESTED");
    assert_eq!(read(&ends.json()).unwrap()[1].verdict, "NOT_TESTED");
    // A FAIL names the lower bound it rests on, not the window's whole CPU: 100 ms over each
    // 60.5 s window, 61 ms of it before the middle sample (review fix round, 7).
    let mut burst = with(0.1, 60.5);
    let part = ticks(0.061);
    for run in &mut burst.scenarios[1] {
        let start = run.samples[0].clone();
        run.samples[1].user_ticks = start.user_ticks + part / 2;
        run.samples[1].system_ticks = start.system_ticks + part - part / 2;
    }
    let row = &burst.rows()[1];
    assert_eq!(row.verdict, "FAIL", "{row:?}");
    assert!(row.detail.contains("at least 0.101667%"), "{row:?}");
    assert!(!row.detail.contains("0.166667"), "{row:?}");
    // A middle sample that itself landed after 60 s bounds nothing from below.
    let mut slept = with(0.062, 60.5);
    for run in &mut slept.scenarios[1] {
        run.samples[1].elapsed_seconds = run.samples[0].elapsed_seconds + 60.2;
    }
    assert_eq!(verdicts(&slept)[1].1, "NOT_TESTED");
    assert_eq!(read(&slept.json()).unwrap()[1].verdict, "NOT_TESTED");
    // One valid run of five, far over the line: incomplete, not a FAIL.
    let mut one = with(0.5, 60.0);
    for run in &mut one.scenarios[1][1..] {
        run.exited(3.0, "signal: 9".into());
    }
    assert_eq!(verdicts(&one)[1].1, "NOT_TESTED");
    let mixed = |high: usize| {
        let mut evidence = evidence();
        let limit = SCENARIOS[2].limit;
        for (index, run) in evidence.scenarios[2].iter_mut().enumerate() {
            let cpu = if index < high {
                BUSY_TICKS
            } else {
                super::tests::IDLE_TICKS
            };
            *run = measured("split", index + 1, limit, cpu);
        }
        evidence
    };
    assert_eq!(verdicts(&mixed(3))[1].1, "FAIL");
    assert_eq!(verdicts(&mixed(2))[1].1, "PASS");
}
/// Review F7 (USER 2026-11-24): a run counts only when the product reported its scenario's view.
#[test]
fn only_runs_that_showed_their_scenarios_view_count() {
    // The product showed Source where Preview was configured: the run is not valid, and the
    // evidence the tool writes for it reads back the same.
    let mut wrong = evidence();
    let run = &mut wrong.scenarios[1][2];
    run.status = "WRONG_VIEW";
    run.view = Some("source".into());
    run.samples.clear();
    run.metrics = None;
    assert_eq!(
        verdicts(&wrong),
        [("R01", "NOT_TESTED"), ("R02", "NOT_TESTED")]
    );
    assert!(
        wrong.rows()[0]
            .detail
            .contains("preview (4 of 5 runs valid)")
    );
    assert_eq!(
        read(&wrong.json()).unwrap().map(|row| row.verdict),
        ["NOT_TESTED", "NOT_TESTED"]
    );
    // No report, or one the tool could not read: the same.
    let mut unreported = evidence();
    let run = &mut unreported.scenarios[2][0];
    run.status = "VIEW_UNREPORTED";
    run.view = None;
    run.samples.clear();
    run.metrics = None;
    assert_eq!(
        read(&unreported.json()).unwrap().map(|row| row.verdict),
        ["NOT_TESTED", "NOT_TESTED"]
    );
    // A VALID run cannot report another view or none: the tool marks it WRONG_VIEW. Neither the
    // tool's own rows nor the reader count it.
    for view in [Some("source"), None] {
        let mut forged = evidence();
        forged.scenarios[2][0].view = view.map(str::to_owned);
        assert!(!forged.passed(), "{view:?}");
        refused(&forged.json(), "WRONG_VIEW");
    }
    // Hidden opens in Source: its runs report Source.
    let good = evidence();
    assert_eq!(good.scenarios[3][0].view.as_deref(), Some("source"));
    assert!(good.passed());
}

/// Review F7: the calibration holds only when the tool told every configured-as-Source run apart,
/// and its evidence is never finished, so even a reader that ignored `calibration` would refuse
/// it (as for mac-gui's calibration, review fix round B1).
#[test]
fn the_wrong_view_calibration_holds_only_when_every_run_was_told_apart() {
    let good = calibration();
    assert!(good.calibration_held());
    assert!(!good.passed());
    let text = good.json();
    assert!(text.contains("\"calibration\":\"wrong-view\""), "{text}");
    assert!(text.contains("\"finished\":false"), "{text}");
    // Its runs are this tool's: the reader takes them, and neither row is judged.
    assert_eq!(
        read(&text).unwrap().map(|row| row.verdict),
        ["NOT_TESTED", "NOT_TESTED"]
    );
    // An ordinary campaign is finished and names no calibration.
    let ordinary = evidence().json();
    assert!(ordinary.contains("\"finished\":true") && !ordinary.contains("calibration"));
    // The product followed the configuration but the tool took it for the scenario's view; a run
    // that reported nothing; a Source run taken for a wrong one; a missing run.
    let mut accepted = calibration();
    accepted.scenarios[2][1].status = "VIEW_CHECKED";
    let mut silent = calibration();
    silent.scenarios[1][4].status = "VIEW_UNREPORTED";
    silent.scenarios[1][4].view = None;
    let mut source = calibration();
    source.scenarios[0][0].status = "WRONG_VIEW";
    let mut short = calibration();
    short.scenarios[3].pop();
    let mut uncleaned = calibration();
    uncleaned.scenarios[3][0].cleanup_error = Some("left running".into());
    for (bad, what) in [
        (accepted, "accepted"),
        (silent, "silent"),
        (source, "source"),
        (short, "short"),
        (uncleaned, "uncleaned"),
    ] {
        assert!(!bad.calibration_held(), "{what}");
    }
}
