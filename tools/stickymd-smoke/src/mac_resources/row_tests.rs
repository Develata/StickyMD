//! P16-R01 and P16-R02 from a campaign's runs, and the reader the ledger uses, against bad
//! samples: each changes one fact of a good campaign.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use super::model::SCENARIOS;
use super::report::Evidence;
use super::rows::{CPU_HARD_FAILURE_PERCENT, RowVerdict, VIEW_UNCONFIRMED, from_evidence};
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

/// The history of the row: periodic caret-layer toggling gave Split a median of 0.103%.
#[test]
fn a_median_idle_cpu_above_the_hard_line_fails_r02_and_a_single_spike_does_not() {
    let mut busy = evidence();
    let limit = SCENARIOS[2].limit;
    for (index, run) in busy.scenarios[2].iter_mut().enumerate() {
        *run = measured(index + 1, limit, BUSY_TICKS);
    }
    assert_eq!(verdicts(&busy), [("R01", "MEASURED"), ("R02", "FAIL")]);
    assert!(busy.rows()[1].detail.contains("split median 0.103000%"));
    assert_eq!(read(&busy.json()).unwrap()[1].verdict, "FAIL");
    let mut spike = evidence();
    spike.scenarios[0][0]
        .metrics
        .as_mut()
        .unwrap()
        .idle_cpu_percent = 5.0;
    assert_eq!(verdicts(&spike), [("R01", "MEASURED"), ("R02", "MEASURED")]);
    // At the line is within it.
    let mut edge = evidence();
    for run in &mut edge.scenarios[3] {
        run.metrics.as_mut().unwrap().idle_cpu_percent = CPU_HARD_FAILURE_PERCENT;
    }
    assert_eq!(verdicts(&edge), [("R01", "MEASURED"), ("R02", "MEASURED")]);
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
    refused(&text, "its runs give");
    let text = good.replacen(
        "\"macos_version\":\"27.2\"",
        "\"macos_version\":\"26.4\"",
        1,
    );
    assert_ne!(text, good);
    refused(&text, "its runs give");
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
