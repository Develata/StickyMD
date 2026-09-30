//! Historical format fixtures validate parsing only, never current qualification.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::*;

// Unmodified JSON and selected complete lines from the existing local logs:
// target/acceptance-profiling/governance.json
// target/acceptance-profiling/resource-round2-live-20260930/resumed.log
// target/cli-continuation-20260930/final-smoke-tests.log
// They are intentionally historical observations, not evidence for this worktree.
const GOVERNANCE: &str = include_str!("fixtures/governance.json");
const RESUMED: &str = include_str!("fixtures/resource-resumed.log");
const CARGO: &str = include_str!("fixtures/cargo-tests.log");
const RESUMED_JSON: &str = include_str!("fixtures/resource-resumed.json");

#[test]
fn historical_governance_json_retains_missing_timing_and_recorded_identity() {
    let summary = summarize(GOVERNANCE).unwrap();
    assert_eq!(summary.missing_task_timings, ["acceptance readiness"]);
    assert_eq!(summary.suite.as_deref(), Some("phase-00"));
    assert_eq!(
        summary.source.as_deref(),
        Some("559d94408f2edc3f83fb70319c968aa5f81c968a")
    );
    assert_eq!(summary.observations[0].seconds, Some(2.937108));
    let inputs = [(PathBuf::from("中文 空格.json"), summary)];
    let json = report::render(&inputs, true);
    let value = crate::release::json::parse(&json).unwrap();
    assert_eq!(
        *value.field("wall_clock_seconds").unwrap(),
        crate::release::json::Value::Null
    );
    assert_eq!(
        *value.field("agent_work_seconds").unwrap(),
        crate::release::json::Value::Null
    );
    assert!(json.contains("\"source_verified\":false"));
    assert!(report::render(&inputs, false).contains("unknown, not zero"));
}

#[test]
fn existing_resume_log_separates_nested_identity_lookup_and_wait_budgets() {
    let summary = summarize(RESUMED).unwrap();
    let rows = &summary.observations;
    let current: Vec<_> = rows
        .iter()
        .filter(|row| row.kind == "current_task")
        .collect();
    assert_eq!(current.len(), 4);
    assert_eq!(current.last().unwrap().seconds, Some(7.580151));
    assert!(rows.iter().any(|row| row.kind == "current_group"
        && row.seconds == Some(7.557)
        && row.status.as_deref() == Some("DIAGNOSTIC_REUSED")));
    assert!(
        rows.iter()
            .any(|row| row.kind == "identity_total" && row.seconds == Some(0.786374))
    );
    assert!(rows.iter().any(|row| row.kind == "fixed_wait_budget"
        && row.metric == "avoided_fixed_wait_seconds"
        && row.seconds == Some(75.0)));
    let rendered = report::render(&[(PathBuf::from("historical resumed.log"), summary)], true);
    assert!(rendered.contains("NONE_OVERLAPPING_SCOPES"));
    assert!(!rendered.contains("total_elapsed_seconds"));
}

#[test]
fn actual_resource_group_planning_messages_are_ignored_in_mixed_timing_logs() {
    // resource_modules prints these planning/reuse shapes without an elapsed
    // field. The private evidence path is replaced with a portable fixture path.
    let planning = "RESOURCE_GROUP=zoom STATUS=RUN_REQUIRED REASON=NO_LAST_SUCCESS\nRESOURCE_GROUP=window REUSED_PASS origin_source=old-source origin_exe=old-exe origin_zip=old-zip evidence=ignored/evidence.json\n";
    let expected = summarize(RESUMED).unwrap();
    let mixed = summarize(&format!("{planning}{RESUMED}")).unwrap();
    assert_eq!(mixed.observations.len(), expected.observations.len());
    assert_eq!(
        mixed
            .observations
            .iter()
            .filter(|row| row.kind == "current_group")
            .count(),
        1
    );
    assert!(summarize(planning).is_err());
}

#[test]
fn existing_resumed_receipt_keeps_fresh_probe_and_lookup_separate_from_origin() {
    let summary = summarize(RESUMED_JSON).unwrap();
    let row = |name: &str| {
        summary
            .observations
            .iter()
            .find(|row| row.metric == name)
            .unwrap()
    };
    assert_eq!(
        row("desktop_probe.execution_seconds").kind,
        "current_nested"
    );
    assert_eq!(
        row("group-zoom.origin_execution_seconds").kind,
        "historical_origin"
    );
    assert!(row("group-zoom.origin_execution_seconds").seconds.unwrap() > 93.0);
    assert!(row("group-zoom.origin_execution_seconds").origin.is_some());
    assert_eq!(row("group.execution_seconds").kind, "current_group");
    assert!(row("group.execution_seconds").seconds.unwrap() < 8.0);
    assert!(
        summary
            .missing_task_timings
            .contains(&"acceptance readiness".into())
    );
}

#[test]
fn cargo_reported_build_and_binary_times_remain_independent() {
    let summary = summarize(CARGO).unwrap();
    assert_eq!(summary.observations.len(), 3);
    assert_eq!(
        summary
            .observations
            .iter()
            .map(|row| row.seconds.unwrap())
            .collect::<Vec<_>>(),
        [9.53, 29.45, 48.81]
    );
    assert_eq!(summary.observations[0].kind, "cargo_build");
    assert_eq!(summary.observations[1].kind, "cargo_test_binary");
    assert_eq!(summary.status, "COMPLETENESS_UNKNOWN");
    let summary = summarize("Finished `test` profile [unoptimized] target(s) in 1m 02.5s\ntest result: FAILED. 0 passed; 1 failed; finished in 250ms\n").unwrap();
    assert_eq!(summary.observations[0].seconds, Some(62.5));
    assert_eq!(summary.observations[1].seconds, Some(0.25));
    assert_eq!(summary.observations[1].status.as_deref(), Some("FAILED"));
    for invalid in [
        "Finished `test` profile target(s)",
        "Finished `test` profile target(s) in 1s 1m",
        "test result: ok. 1 passed; finished in NaNs",
        "test result: ok. 1 passed",
        "test result: UNKNOWN. finished in 1s",
    ] {
        assert!(summarize(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn colored_ci_logs_retain_build_test_and_task_times_without_weakening_validation() {
    // CI uses CARGO_TERM_COLOR=always; Cargo colors the Finished label even
    // when stderr is redirected to a file. Libtest can also color its status.
    let colored = concat!(
        "\x1b[1m\x1b[92m    Finished\x1b[0m `dev` profile [unoptimized + debuginfo] target(s) in 0.26s\n",
        "test result: \x1b[32mok\x1b[m. 1 passed; 0 failed; finished in 1.50s\n",
        "TASK_TIMING task=\"中文 空格\" status=\x1b[38;2;0;255;0mPASSED\x1b[0m elapsed_seconds=2.0\n",
    );
    let plain = concat!(
        "    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.26s\n",
        "test result: ok. 1 passed; 0 failed; finished in 1.50s\n",
        "TASK_TIMING task=\"中文 空格\" status=PASSED elapsed_seconds=2.0\n",
    );
    assert_eq!(
        summarize(colored).unwrap().observations,
        summarize(plain).unwrap().observations
    );
    assert!(summarize(&colored.replace("0.26s", "NaNs")).is_err());
    assert!(summarize(&colored.replace("PASSED", "UNKNOWN")).is_err());
}

#[test]
fn json_truncation_rejects_missing_closure_with_any_checkout_line_ending() {
    let receipt = GOVERNANCE.trim_end();
    let truncated = receipt.strip_suffix('}').expect("fixture is a JSON object");
    for ending in ["", "\n", "\r\n"] {
        assert!(summarize(&format!("{receipt}{ending}")).is_ok());
        assert!(summarize(&format!("{truncated}{ending}")).is_err());
    }
}

#[test]
fn json_rejects_missing_malformed_negative_nonfinite_or_duplicate_timing() {
    for invalid in [
        GOVERNANCE.replace("\"schema_version\":2", "\"schema_version\":1"),
        GOVERNANCE.replace("\"results\":[", "\"wrong\":["),
        GOVERNANCE.replace("\"measurements\":", "\"wrong\":"),
        GOVERNANCE.replace("\"unit\":\"seconds\"", "\"unit\":\"ms\""),
        GOVERNANCE.replace("2.937108", "-1"),
        GOVERNANCE.replace("2.937108", "1e999"),
        GOVERNANCE.replace("2.937108", "\"2.937108\""),
        GOVERNANCE.replace("2.937108", "null"),
        GOVERNANCE.replace("\"PASSED\"", "\"UNKNOWN\""),
        GOVERNANCE.replace("\"acceptance readiness\"", "\"governance contracts\""),
        GOVERNANCE.replace("\"value\":2.937108", "\"value\":2.937108,\"value\":3"),
        GOVERNANCE.replace("\"unit\":\"seconds\",\"value\":2.937108}", "\"unit\":\"seconds\",\"value\":2.937108},{\"name\":\"task.execution_seconds\",\"unit\":\"seconds\",\"value\":3}"),
    ] {
        assert!(summarize(&invalid).is_err(), "{invalid}");
    }
}

#[test]
fn origin_timing_is_historical_and_never_added_to_current_lookup() {
    let text = r#"{"schema_version":2,"suite":"diagnostic","commit":"old-source","results":[{"id":"Zoom resources","status":"PASSED","detail":"DIAGNOSTIC_REUSED diagnostic-cache:old:123:group-zoom; origin_unit=group-zoom; origin_source=old-source; origin_execution_seconds=93.5418989","measurements":[{"name":"group-zoom.origin_execution_seconds","unit":"seconds","value":93.5418989},{"name":"group.execution_seconds","unit":"seconds","value":7.557},{"name":"task.execution_seconds","unit":"seconds","value":7.580151}],"samples":[{"measurements":[{"name":"task.execution_seconds","unit":"seconds","value":999}]}]}]}"#;
    let summary = summarize(text).unwrap();
    assert_eq!(summary.observations.len(), 3);
    assert_eq!(summary.observations[0].kind, "historical_origin");
    assert_eq!(
        summary.observations[0].origin.as_deref(),
        Some("old-source")
    );
    assert_eq!(summary.observations[1].kind, "current_group");
    assert_eq!(summary.observations[2].kind, "current_task");
    // Sample measurements are nested observations and must never become tasks.
    assert!(
        !summary
            .observations
            .iter()
            .any(|row| row.seconds == Some(999.0))
    );
}

#[test]
fn workspace_and_resource_case_timings_keep_their_actual_scope() {
    let text = r#"{"schema_version":2,"suite":"diagnostic","commit":"old-source","results":[{"id":"workspace tests","status":"PASSED","detail":"REUSED_PASS SOURCE_BOUND origin_source=old-workspace input_fingerprint=abc receipt=dist/evidence/source-success/workspace-tests.json","measurements":[{"name":"workspace.origin_run_seconds","unit":"seconds","value":30},{"name":"workspace.identity_before_seconds","unit":"seconds","value":1}]},{"id":"Zoom resources","status":"PASSED","detail":"DIAGNOSTIC_REUSED diagnostic-cache:old:123:group-zoom; origin_source=old-resources","measurements":[{"name":"split-zoom-50.execution_seconds","unit":"seconds","value":25},{"name":"cache_batch.execution_seconds","unit":"seconds","value":2}]},{"id":"fresh resource group","status":"FAILED","detail":null,"measurements":[{"name":"workspace.run_seconds","unit":"seconds","value":10},{"name":"desktop_probe.execution_seconds","unit":"seconds","value":0.5},{"name":"cache_batch.execution_seconds","unit":"seconds","value":0.3}]}]}"#;
    let summary = summarize(text).unwrap();
    assert_eq!(summary.observations[0].kind, "historical_origin");
    assert_eq!(
        summary.observations[0].origin.as_deref(),
        Some("old-workspace")
    );
    assert_eq!(summary.observations[1].kind, "identity_component");
    assert_eq!(summary.observations[2].kind, "current_nested");
    assert_eq!(summary.observations[2].origin, None);
    assert_eq!(summary.observations[3].kind, "current_nested");
    assert!(
        summary.observations[4..]
            .iter()
            .all(|row| row.kind == "current_nested")
    );
    let source = r#"{"schema_version":1,"status":"PASSED","evidence_class":"SOURCE_BOUND","scope":"FULL_WORKSPACE","command":"cargo test --workspace --locked","source_commit":"old-source","input_fingerprint":"abc","run_seconds":45.5}"#;
    let summary = summarize(source).unwrap();
    assert_eq!(summary.format, "workspace_source_receipt_json");
    assert_eq!(summary.source.as_deref(), Some("old-source"));
    assert_eq!(summary.observations[0].seconds, Some(45.5));
}

#[test]
fn incomplete_progress_and_diagnostic_plan_budgets_are_not_observed_waits() {
    for (text, expected) in [
        (
            r#"{"schema_version":1,"status":"INCOMPLETE","stage":"resource-preflight","remaining_fixed_wait_seconds":null}"#,
            None,
        ),
        (
            r#"{"schema_version":1,"status":"INCOMPLETE","stage":"warmup","phase_fixed_seconds":5,"remaining_fixed_wait_seconds":420}"#,
            Some(420.0),
        ),
        (
            r#"{"schema_version":1,"kind":"DIAGNOSTIC_PLAN","status":"NOT_RUN","planned_fixed_wait_seconds":0,"avoided_fixed_wait_seconds":75,"units":[]}"#,
            Some(0.0),
        ),
    ] {
        let summary = summarize(text).unwrap();
        assert_eq!(summary.observations[0].seconds, expected);
        assert!(
            summary
                .observations
                .iter()
                .all(|row| row.kind == "fixed_wait_budget")
        );
        assert!(matches!(summary.status.as_str(), "INCOMPLETE" | "NOT_RUN"));
    }
    for invalid in [
        r#"{"schema_version":1,"status":"INCOMPLETE","stage":"warmup"}"#,
        r#"{"schema_version":1,"status":"INCOMPLETE","stage":"warmup","remaining_fixed_wait_seconds":-1}"#,
        r#"{"schema_version":1,"kind":"DIAGNOSTIC_PLAN","status":"PASSED","planned_fixed_wait_seconds":0,"avoided_fixed_wait_seconds":75}"#,
    ] {
        assert!(summarize(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn logs_reject_duplicates_incomplete_and_invalid_values_but_ignore_noise() {
    for invalid in [
        "noise\n",
        "TASK_TIMING task=\"x\" status=PASSED",
        "TASK_TIMING task=\"x\" status=PASSED elapsed_seconds=-1",
        "TASK_TIMING task=\"x\" status=PASSED elapsed_seconds=NaN",
        "TASK_TIMING task=\"x\" status=PASSED elapsed_seconds=inf",
        "TASK_TIMING task=\"x\" status=PASSED elapsed_seconds=1e999",
        "TASK_TIMING task=\"x\" status=PASSED elapsed_seconds=1 elapsed_seconds=2",
        "TASK_TIMING task=\"x\" status=UNKNOWN elapsed_seconds=1",
        "TASK_TIMING task=\"x\"status=PASSED elapsed_seconds=1",
        "TASK_TIMING task=\"x\" status=PASSED elapsed_seconds=1\nTASK_TIMING task=\"x\" status=PASSED elapsed_seconds=1",
        "RESOURCE_IDENTITY git_seconds=1",
    ] {
        assert!(summarize(invalid).is_err(), "{invalid}");
    }
    let summary = summarize("noise\nTASK_TIMING task=\"中文 空格 \\\"quoted\\\"\" status=FAILED elapsed_seconds=0.125\nnoise").unwrap();
    assert_eq!(summary.observations[0].label, "中文 空格 \"quoted\"");
    assert_eq!(summary.observations[0].line, Some(2));
}

#[test]
fn inputs_handle_unicode_spaces_aliases_multiple_files_and_failures_without_writes() {
    let directory = std::env::temp_dir().join(format!(
        "stickymd-timing-中文 空格-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let receipt = directory.join("收据 空格.json");
    let log = directory.join("历史 日志.log");
    fs::write(&receipt, GOVERNANCE).unwrap();
    fs::write(&log, RESUMED).unwrap();
    let summaries = read_inputs(&[receipt.clone(), log.clone()]).unwrap();
    assert_eq!(summaries.len(), 2);
    let rendered = report::render(&summaries, true);
    assert!(crate::release::json::parse(&rendered).is_ok());
    assert!(rendered.contains("收据 空格.json"));
    assert!(read_inputs(&[receipt.clone(), directory.join("./收据 空格.json")]).is_err());
    assert!(read_inputs(std::slice::from_ref(&directory)).is_err());
    assert!(read_inputs(&[directory.join("missing.json")]).is_err());
    fs::write(&log, [0xff, 0xfe, 0xff]).unwrap();
    assert!(read_inputs(&[receipt.clone(), log.clone()]).is_err());
    assert_eq!(fs::read_to_string(&receipt).unwrap(), GOVERNANCE);
    assert_eq!(fs::read_dir(&directory).unwrap().count(), 2);
    fs::remove_file(receipt).unwrap();
    fs::remove_file(log).unwrap();
    fs::remove_dir(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn fifo_input_is_rejected_before_a_blocking_open() {
    let directory = std::env::temp_dir().join(format!(
        "stickymd-timing-fifo-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&directory).unwrap();
    let fifo = directory.join("input.fifo");
    assert!(
        std::process::Command::new("mkfifo")
            .arg(&fifo)
            .status()
            .unwrap()
            .success()
    );
    let (sender, receiver) = std::sync::mpsc::channel();
    let input = fifo.clone();
    std::thread::spawn(move || {
        let _ = sender.send(read_inputs(&[input]).is_err());
    });
    let outcome = receiver.recv_timeout(std::time::Duration::from_secs(2));
    fs::remove_file(fifo).unwrap();
    fs::remove_dir(directory).unwrap();
    assert_eq!(outcome, Ok(true));
}

#[test]
fn cli_accepts_multiple_inputs_and_rejects_missing_or_repeated_flags() {
    let arguments =
        ["--input", "中文 空格.json", "--input", "test.log", "--json"].map(str::to_owned);
    let options = parse(&arguments).unwrap();
    assert_eq!(
        options.inputs,
        [PathBuf::from("中文 空格.json"), PathBuf::from("test.log")]
    );
    assert!(options.json);
    for invalid in [
        vec![],
        vec!["--json"],
        vec!["--input"],
        vec!["--input", "--json"],
        vec!["--input", "test.log", "--json", "--json"],
        vec!["--input", "test.log", "--unknown"],
    ] {
        assert!(parse(&invalid.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
    }
}
