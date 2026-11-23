//! P16-R01 and P16-R02 from a campaign's runs, and the reader the ledger uses, against bad
//! samples: each changes one fact of a good campaign.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use super::model::SCENARIOS;
use super::report::Evidence;
use super::rows::{CPU_HARD_FAILURE_PERCENT, RowVerdict, from_evidence};
use super::tests::evidence;
use crate::release::json;

fn verdicts(evidence: &Evidence) -> [(&'static str, &'static str); 2] {
    evidence.rows().map(|row| (row.row, row.verdict))
}

fn read(text: &str) -> Result<[RowVerdict; 2], String> {
    from_evidence(&json::parse(text).unwrap())
}

#[test]
fn a_complete_campaign_within_both_lines_passes_both_rows() {
    let good = evidence();
    assert_eq!(verdicts(&good), [("R01", "PASS"), ("R02", "PASS")]);
    let rows = read(&good.json()).unwrap();
    assert_eq!(rows.clone().map(|row| row.verdict), ["PASS", "PASS"]);
    assert!(rows[0].detail.contains("hidden max"), "{:?}", rows[0]);
}

#[test]
fn one_run_over_its_memory_gate_fails_r01_alone() {
    let mut over = evidence();
    let limit = SCENARIOS[2].limit;
    over.scenarios[2][4]
        .metrics
        .as_mut()
        .unwrap()
        .footprint_bytes = limit + 1;
    assert_eq!(verdicts(&over), [("R01", "FAIL"), ("R02", "PASS")]);
    assert!(over.rows()[0].detail.contains("split"));
    assert!(!over.passed());
    // At the gate is within it.
    over.scenarios[2][4]
        .metrics
        .as_mut()
        .unwrap()
        .footprint_bytes = limit;
    assert_eq!(verdicts(&over), [("R01", "PASS"), ("R02", "PASS")]);
}

/// The history of the row: periodic caret-layer toggling gave Split a median of 0.103%.
#[test]
fn a_median_idle_cpu_above_the_hard_line_fails_r02_and_a_single_spike_does_not() {
    let mut busy = evidence();
    for run in &mut busy.scenarios[2] {
        run.metrics.as_mut().unwrap().idle_cpu_percent = 0.103;
    }
    assert_eq!(verdicts(&busy), [("R01", "PASS"), ("R02", "FAIL")]);
    assert!(busy.rows()[1].detail.contains("split median 0.1030%"));
    let mut spike = evidence();
    spike.scenarios[0][0]
        .metrics
        .as_mut()
        .unwrap()
        .idle_cpu_percent = 5.0;
    assert_eq!(verdicts(&spike), [("R01", "PASS"), ("R02", "PASS")]);
    // At the line is within it.
    let mut edge = evidence();
    for run in &mut edge.scenarios[3] {
        run.metrics.as_mut().unwrap().idle_cpu_percent = CPU_HARD_FAILURE_PERCENT;
    }
    assert_eq!(verdicts(&edge), [("R01", "PASS"), ("R02", "PASS")]);
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
    short.scenarios[0][0]
        .metrics
        .as_mut()
        .unwrap()
        .footprint_bytes = u64::MAX;
    assert_eq!(verdicts(&short), [("R01", "FAIL"), ("R02", "NOT_TESTED")]);
}

#[test]
fn the_reader_recomputes_from_runs_and_refuses_stated_rows_that_disagree() {
    let good = evidence().json();
    // A run over its gate, with the rows still saying PASS.
    let over = good.replacen(
        &format!("\"footprint_bytes\":{}", SCENARIOS[0].limit),
        &format!("\"footprint_bytes\":{}", SCENARIOS[0].limit + 1),
        1,
    );
    assert_ne!(over, good);
    let error = read(&over).unwrap_err();
    assert!(error.contains("its runs give"), "{error}");
    // A run's status changed: the rows no longer hold.
    let errored = good.replacen("\"status\":\"VALID\"", "\"status\":\"ERROR\"", 1);
    assert!(read(&errored).unwrap_err().contains("its runs give"));
    // Rows claimed by a status field are not taken: a FAIL stated as PASS is refused.
    let claimed = good.replacen(
        "\"id\":\"P16-R02\",\"verdict\":\"PASS\"",
        "\"id\":\"P16-R02\",\"verdict\":\"FAIL\"",
        1,
    );
    assert!(read(&claimed).unwrap_err().contains("its runs give"));
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
        let error = read(&good.replacen(&from, &to, 1)).unwrap_err();
        assert!(error.contains(words), "{from}: {error}");
    }
}
