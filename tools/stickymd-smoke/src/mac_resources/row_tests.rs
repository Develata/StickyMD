//! P16-R01 and P16-R02 from a campaign's runs, and the reader the ledger uses, against bad
//! samples: each changes one fact of a good campaign.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use super::model::SCENARIOS;
use super::report::Evidence;
use super::rows::{RowVerdict, VIEW_UNCONFIRMED, from_evidence};
use super::tests::{BUSY_TICKS, evidence, measured};
use crate::release::json;

/// Each row's verdict, with `MEASURED` for complete runs within the line: NOT_TESTED until the
/// views are confirmed (review F7).
fn verdicts(evidence: &Evidence) -> [(&'static str, &'static str); 2] {
    evidence.rows().map(|row| {
        if row.measured {
            assert_eq!(row.verdict, "NOT_TESTED", "{row:?}");
            assert!(row.detail.contains(VIEW_UNCONFIRMED), "{row:?}");
            (row.row, "MEASURED")
        } else {
            (row.row, row.verdict)
        }
    })
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
    assert_eq!(verdicts(&good), [("R01", "MEASURED"), ("R02", "MEASURED")]);
    // Fail closed (review F7): complete runs within both lines are not a PASS while no run
    // confirms its view.
    let rows = read(&good.json()).unwrap();
    assert_eq!(
        rows.clone().map(|row| row.verdict),
        ["NOT_TESTED", "NOT_TESTED"]
    );
    assert!(
        rows.iter()
            .all(|row| row.measured && row.detail.contains(VIEW_UNCONFIRMED))
    );
    assert!(good.measured());
    assert!(rows[0].detail.contains("hidden max"), "{:?}", rows[0]);
}

#[test]
fn one_run_over_its_memory_gate_fails_r01_alone_and_names_the_bytes() {
    let mut over = evidence();
    let limit = SCENARIOS[2].limit;
    over.scenarios[2][4] = measured(5, limit + 1, super::tests::IDLE_TICKS);
    assert_eq!(verdicts(&over), [("R01", "FAIL"), ("R02", "MEASURED")]);
    // One byte over is visible in the detail, not rounded away.
    assert!(
        over.rows()[0]
            .detail
            .contains("split 134217729 B (128.000 MiB) > 134217728 B"),
        "{:?}",
        over.rows()[0]
    );
    assert!(!over.measured());
    // The evidence written for it reads back the same.
    assert_eq!(read(&over.json()).unwrap()[0].verdict, "FAIL");
    // At the gate is within it.
    over.scenarios[2][4] = measured(5, limit, super::tests::IDLE_TICKS);
    assert_eq!(verdicts(&over), [("R01", "MEASURED"), ("R02", "MEASURED")]);
}

/// The history of the row: periodic caret-layer toggling gave Split a median of 0.103%. Each case
/// changes the runs' raw samples, which the rule reads, and is read back from its JSON.
#[test]
fn a_median_idle_cpu_above_the_hard_line_fails_r02_and_a_single_spike_does_not() {
    let with_ticks = |scenario: usize, ticks: &dyn Fn(usize) -> u64, window: f64| {
        let mut evidence = evidence();
        let limit = SCENARIOS[scenario].limit;
        for (index, run) in evidence.scenarios[scenario].iter_mut().enumerate() {
            *run = super::tests::measured_over(index + 1, limit, ticks(index), window);
        }
        let verdict = verdicts(&evidence)[1].1;
        assert_eq!(
            read(&evidence.json()).unwrap()[1].verdict,
            if verdict == "MEASURED" {
                "NOT_TESTED"
            } else {
                verdict
            },
            "read back"
        );
        (verdict, evidence.rows()[1].detail.clone())
    };
    let (verdict, detail) = with_ticks(2, &|_| BUSY_TICKS, 60.0);
    assert_eq!(verdict, "FAIL");
    assert!(
        detail.contains("split median 0.103000% in 60 s windows"),
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
    assert_eq!(with_ticks(2, &spike, 60.0).0, "MEASURED");
    // Exactly 60 ms of CPU (1,440,000 ticks at 125/3) is at the line, and within it; one tick
    // more is over it. Compared in raw ticks, the review's late sample (the second at
    // 91.500012 s) whose percent of the budget reads 0.10000000000000002 is still at the line.
    const AT_THE_LINE: u64 = 1_440_000;
    assert_eq!(with_ticks(1, &|_| AT_THE_LINE, 60.0).0, "MEASURED");
    assert_eq!(with_ticks(1, &|_| AT_THE_LINE + 1, 60.0).0, "FAIL");
    assert_eq!(with_ticks(1, &|_| AT_THE_LINE, 60.000012).0, "MEASURED");
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
    short.scenarios[0][0] = measured(1, u64::MAX / 2, super::tests::IDLE_TICKS);
    assert_eq!(verdicts(&short), [("R01", "FAIL"), ("R02", "NOT_TESTED")]);
}

/// `10`: memory is measured at scale 2, and every measurement on macOS 27.
#[test]
fn outside_plan_10s_environment_the_rows_are_not_tested() {
    let mut scale_1 = evidence();
    scale_1.conditions.scale = [1.0, 1.0];
    assert_eq!(
        verdicts(&scale_1),
        [("R01", "NOT_TESTED"), ("R02", "MEASURED")]
    );
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
    scale_1.scenarios[0][0] = measured(1, u64::MAX / 2, super::tests::IDLE_TICKS);
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
    late.scenarios[1][2].samples[1].elapsed_seconds = 150.0;
    refused(&late.json(), "give no reading");
    // A run's status changed: the rows no longer hold.
    let errored = good.replacen("\"status\":\"VALID\"", "\"status\":\"ERROR\"", 1);
    refused(&errored, "its runs give");
    // Rows claimed by a status field are not taken: a PASS stated where the runs give NOT_TESTED
    // (views unconfirmed, review F7) is refused, and so is another reason for the same verdict.
    let claimed = good.replacen(
        "\"id\":\"P16-R02\",\"verdict\":\"NOT_TESTED\"",
        "\"id\":\"P16-R02\",\"verdict\":\"PASS\"",
        1,
    );
    assert_ne!(claimed, good);
    refused(&claimed, "its runs give");
    let reason = good.replacen(VIEW_UNCONFIRMED, "views confirmed", 1);
    assert_ne!(reason, good);
    refused(&reason, "its runs give");
}

#[test]
fn the_reader_refuses_runs_that_are_not_the_repetitions_in_order() {
    let mut repeated = evidence();
    repeated.scenarios[1][1].repetition = 1;
    refused(&repeated.json(), "repetitions 1..=5 in order");
    let mut extra = evidence();
    extra.scenarios[0].push(measured(6, 1024, super::tests::IDLE_TICKS));
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
    assert_eq!(verdicts(&two), [("R01", "NOT_TESTED"), ("R02", "MEASURED")]);
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
            *run = measured_over(index + 1, limit, ticks(cpu_seconds), window);
        }
        evidence
    };
    let undecided = with(0.0603, 60.5);
    assert_eq!(
        verdicts(&undecided),
        [("R01", "MEASURED"), ("R02", "NOT_TESTED")]
    );
    assert!(
        undecided.rows()[1]
            .detail
            .contains("does not show a 60 s window above it"),
        "{:?}",
        undecided.rows()[1]
    );
    assert_eq!(read(&undecided.json()).unwrap()[1].verdict, "NOT_TESTED");
    assert_eq!(verdicts(&with(0.062, 60.5))[1].1, "NOT_TESTED");
    assert_eq!(verdicts(&with(0.0599, 60.5))[1].1, "MEASURED");
    assert_eq!(verdicts(&with(0.0603, 60.0))[1].1, "FAIL");
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
            *run = measured(index + 1, limit, cpu);
        }
        evidence
    };
    assert_eq!(verdicts(&mixed(3))[1].1, "FAIL");
    assert_eq!(verdicts(&mixed(2))[1].1, "MEASURED");
}
