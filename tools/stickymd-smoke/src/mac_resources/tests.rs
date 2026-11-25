//! Pure measurement and CLI regression tests; no GUI is started.
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

use super::{model::*, report::Evidence, rows::Conditions};

fn sample(seconds: f64, ticks: u64, wakeups: u64) -> Sample {
    Sample {
        elapsed_seconds: seconds,
        footprint_bytes: 1024,
        user_ticks: ticks,
        system_ticks: ticks,
        pkg_idle_wakeups: wakeups,
        interrupt_wakeups: wakeups,
    }
}

/// CPU ticks (timebase 125/3) for 0.001% over 60 s, and for Split's 0.103% of the row's history.
pub(super) const IDLE_TICKS: u64 = 14_400;
pub(super) const BUSY_TICKS: u64 = 1_483_200;

/// A valid run as the tool takes it: `view` reported, ready at 0.5 s, the first sample after the
/// 30 s warm-up, the middle one 59.9 s later and the last 60 s later, at `footprint_bytes`,
/// `cpu_ticks` of CPU between the first and the last spread evenly, 100 interrupt and 25 idle
/// wake-ups a second, readings computed from them.
pub(super) fn measured(view: &str, repetition: usize, footprint_bytes: u64, cpu_ticks: u64) -> Run {
    measured_over(view, repetition, footprint_bytes, cpu_ticks, 60.0)
}

/// [`measured`] with the second sample `window` seconds after the first.
pub(super) fn measured_over(
    view: &str,
    repetition: usize,
    footprint_bytes: u64,
    cpu_ticks: u64,
    window: f64,
) -> Run {
    let mut before = sample(31.5, 1_000, 2_000);
    before.footprint_bytes = footprint_bytes;
    let mut after = sample(31.5 + window, 1_000 + cpu_ticks / 2, 2_000 + 6_000);
    after.system_ticks = 1_000 + cpu_ticks - cpu_ticks / 2;
    after.pkg_idle_wakeups = 2_000 + 1_500;
    after.footprint_bytes = footprint_bytes;
    let spread = (u128::from(cpu_ticks) * 599 / (window * 10.0).round() as u128) as u64;
    let mut lower = sample(
        31.5 + LOWER_SAMPLE_SECONDS,
        1_000 + spread / 2,
        2_000 + 5_990,
    );
    lower.system_ticks = 1_000 + spread - spread / 2;
    lower.pkg_idle_wakeups = 2_000 + 1_497;
    lower.footprint_bytes = footprint_bytes;
    let mut run = Run::new(repetition);
    run.pid = Some(4242);
    run.ready_seconds = Some(0.5);
    run.metrics = Some(metrics(&before, &after, 125, 3).unwrap());
    run.samples = vec![before, lower, after];
    run.status = "VALID";
    run.view = Some(view.to_owned());
    run
}

fn valid_run(repetition: usize, footprint_bytes: u64) -> Run {
    measured("source", repetition, footprint_bytes, IDLE_TICKS)
}

#[test]
fn fixture_has_exact_size_twenty_formulas_and_stable_mixed_text() {
    let note = fixture();
    assert_eq!(note.len(), 20 * 1024);
    assert_eq!(note.lines().filter(|line| *line == "$$").count(), 40);
    assert_eq!(note.matches('$').count(), 80);
    assert!(note.contains("中文") && note.contains("Latin text"));
    assert_eq!(note, fixture());
    assert!(note.ends_with('\n'));
    for scenario in SCENARIOS {
        let mode = scenario.mode;
        assert_eq!(
            config(mode),
            format!("version = 1\nview_mode = \"{mode}\"\n")
        );
    }
}

#[test]
fn scenarios_cover_the_four_macos_gates_and_hide_only_hidden() {
    // `10` macOS gates: Source 72, Preview 96, Split 128, Hidden to tray 48 MiB.
    let gates = SCENARIOS
        .iter()
        .map(|s| (s.name, s.mode, s.limit / 1024 / 1024, s.hidden))
        .collect::<Vec<_>>();
    assert_eq!(
        gates,
        [
            ("source", "source", 72, false),
            ("preview", "preview", 96, false),
            ("split", "split", 128, false),
            ("hidden", "source", 48, true),
        ]
    );
    assert_eq!(HIDE_ITEM, "隐藏");
}

#[test]
fn aggregation_sorts_and_averages_middle_pair_for_even_cohorts() {
    assert_eq!(
        summarize([4.0, 1.0, 8.0, 2.0].into_iter()),
        Some(Summary {
            median: 3.0,
            max: 8.0
        })
    );
    assert_eq!(
        summarize([4.0, 1.0, 8.0].into_iter()),
        Some(Summary {
            median: 4.0,
            max: 8.0
        })
    );
    assert_eq!(summarize([].into_iter()), None);
    assert_eq!(summarize([f64::NAN].into_iter()), None);
}

#[test]
fn memory_gate_includes_boundary_but_never_ignores_failed_or_missing_runs() {
    for limit in SCENARIOS.map(|scenario| scenario.limit) {
        let mut runs = (1..=6).map(|i| valid_run(i, limit)).collect::<Vec<_>>();
        assert!(passed(&runs, 6, limit));
        assert!(!passed(&runs, 5, limit));
        runs[0].metrics.as_mut().unwrap().footprint_bytes = limit + 1;
        assert!(!passed(&runs, 6, limit));
        runs[0] = valid_run(1, limit);
        runs[0].status = "ERROR";
        assert!(!passed(&runs, 6, limit));
        runs[0] = valid_run(1, limit);
        runs[0].cleanup_error = Some("not reaped".into());
        assert!(!passed(&runs, 6, limit));
        assert!(!passed(&runs[1..4], 3, limit));
    }
}

#[test]
fn cpu_uses_actual_elapsed_time_and_mach_conversion_without_overflow() {
    let before = sample(30.0, 9_000, 10);
    let mut after = sample(90.0, 750_009_000, 70);
    after.pkg_idle_wakeups = 40;
    let values = metrics(&before, &after, 125, 3).unwrap();
    assert!((values.idle_cpu_percent - 104.16666666666667).abs() < 1e-9);
    assert_eq!(values.interrupt_wakeups_per_second, 1.0);
    assert_eq!(values.idle_wakeups_per_second, 0.5);
    assert_eq!(values.footprint_bytes, 1024);
    assert!(metrics(&before, &sample(90.0, 0, 130), 1, 1).is_err());
    assert!(metrics(&before, &sample(89.0, 10_000, 130), 1, 1).is_err());
    // `10`'s window is 60 s: a late second sample would dilute the average. 0.5 s of scheduling
    // is allowed (VM runs overshot by at most 20 ms); 80 ms of CPU in the window and 60 s idle
    // after it read 0.0667% over 120 s where the window held 0.1333%.
    let late = |seconds: f64| {
        let mut late = after.clone();
        late.elapsed_seconds = seconds;
        metrics(&before, &late, 125, 3)
    };
    assert!(late(90.5).is_ok());
    assert!(late(90.51).is_err());
    assert!(late(150.0).is_err());
    assert!(metrics(&before, &after, 1, 0).is_err());
    let mut empty = before.clone();
    empty.footprint_bytes = 0;
    assert!(metrics(&empty, &after, 1, 1).is_err());
    assert!(
        metrics(
            &sample(0.0, 0, 0),
            &sample(60.0, u64::MAX, u64::MAX),
            u32::MAX,
            1
        )
        .is_ok()
    );
}

#[test]
fn early_exit_keeps_raw_sample_but_has_no_derived_values() {
    for raw_count in 0..=2 {
        let mut run = valid_run(1, 1024);
        run.samples = (0..raw_count)
            .map(|i| sample(i as f64 * 60.0, 10, 20))
            .collect();
        run.exited(45.125, "exit status: 7".into());
        assert_eq!(run.status, "EXITED_EARLY");
        assert_eq!(run.exit_seconds, Some(45.125));
        assert_eq!(run.samples.len(), raw_count);
        assert!(run.metrics.is_none());
    }
}

#[test]
fn cli_parses_paths_defaults_equals_help_and_rejects_incomplete_or_ambiguous_options() {
    let parse =
        |args: &[&str]| crate::cli::CommandLine::parse(args.iter().map(|s| (*s).to_owned()));
    let crate::cli::CommandLine::MacResources(options) =
        parse(&["mac-resources", "--app", "中文 path/StickyMD.app"]).unwrap()
    else {
        panic!("wrong route")
    };
    assert_eq!(options.repetitions, 5);
    assert!(!options.wrong_view);
    // The calibration bad sample is a flag (review F7); once only.
    let crate::cli::CommandLine::MacResources(calibration) =
        parse(&["mac-resources", "--app", "x.app", "--wrong-view"]).unwrap()
    else {
        panic!("wrong route")
    };
    assert!(calibration.wrong_view);
    assert!(
        super::parse(&[
            "--app=x".into(),
            "--wrong-view".into(),
            "--wrong-view".into()
        ])
        .is_err()
    );
    assert_eq!(options.app.to_str().unwrap(), "中文 path/StickyMD.app");
    assert!(
        parse(&[
            "mac-resources",
            "--app=x.app",
            "--out=a b",
            "--repetitions=6"
        ])
        .is_ok()
    );
    let crate::cli::CommandLine::MacResources(help) = parse(&["mac-resources", "--help"]).unwrap()
    else {
        panic!("wrong route")
    };
    assert!(help.help);
    for args in [
        vec![],
        vec!["--app"],
        vec!["--app="],
        vec!["--app=x", "--repetitions=4"],
        vec!["--app=x", "--repetitions=-5"],
        vec!["--app=x", "--repetitions=no"],
        vec!["--app=x", "--app=y"],
        vec!["--app=x", "--unknown=x"],
        vec!["--help", "--app=x"],
        vec!["--app", "--out", "x"],
    ] {
        assert!(
            super::parse(&args.iter().map(|s| (*s).into()).collect::<Vec<_>>()).is_err(),
            "{args:?}"
        );
    }
}

/// A finished campaign in the VM: every scenario's runs valid and within the gates.
pub(super) fn evidence() -> Evidence {
    Evidence {
        environment: "{\"os\":\"ProductName:\\tmacOS\\nProductVersion:\\t27.2\\nBuildVersion:\\t26B123\\n\",\"hv_vmm_present\":\"1\",\"display_count\":1,\"macos_version\":\"27.2\",\"os_build\":\"26B123\",\"unix_seconds\":7,\"main_display\":{\"pixel_width\":2048,\"pixel_height\":1536,\"point_width\":1024,\"point_height\":768,\"scale_x\":2,\"scale_y\":2}}".into(),
        conditions: Conditions {
            macos_version: "27.2".into(),
            scale: [2.0, 2.0],
            displays: 1,
        },
        app_hash: "a".repeat(64),
        bundle_hash: "c".repeat(64),
        inputs: Some("e".repeat(64)),
        driver: "d".repeat(64),
        calibration: false,
        finished: true,
        fixture_hash: crate::integrity::sha256_bytes(fixture().as_bytes()).unwrap(),
        repetitions: 5,
        timebase: (125, 3),
        scenarios: SCENARIOS
            .iter()
            .map(|scenario| {
                (1..=5)
                    .map(|i| {
                        let mut run = measured(scenario.mode, i, scenario.limit, IDLE_TICKS);
                        if scenario.hidden {
                            hide(&mut run);
                        }
                        run
                    })
                    .collect()
            })
            .collect(),
    }
}

/// A run of the Hidden scenario: hidden through the menu 0.5 s after it was ready, which the
/// first sample's warm-up (31.5 s) still covers.
pub(super) fn hide(run: &mut Run) {
    run.status_item_wait_seconds = Some(0.004);
    run.hidden_seconds = Some(1.0);
}

/// A good campaign's JSON for the ledger's tests; `busy` gives Split a median idle CPU of 0.103%
/// (an R02 FAIL), with rows that agree.
pub(crate) fn evidence_json(busy: bool) -> String {
    let mut evidence = evidence();
    if busy {
        let limit = SCENARIOS[2].limit;
        for (index, run) in evidence.scenarios[2].iter_mut().enumerate() {
            *run = measured("split", index + 1, limit, BUSY_TICKS);
        }
    }
    evidence.json()
}

/// A `--wrong-view` calibration as the tool writes it: Source's configuration in every scenario,
/// views checked, nothing measured.
pub(super) fn calibration() -> Evidence {
    let mut evidence = evidence();
    evidence.calibration = true;
    for (runs, scenario) in evidence.scenarios.iter_mut().zip(SCENARIOS) {
        for run in runs {
            run.samples.clear();
            run.metrics = None;
            run.status_item_wait_seconds = None;
            run.hidden_seconds = None;
            run.view = Some("source".into());
            run.status = if scenario.mode == "source" {
                "VIEW_CHECKED"
            } else {
                "WRONG_VIEW"
            };
        }
    }
    evidence
}

/// The calibration's JSON as the tool writes it, for the ledger's tests.
pub(crate) fn calibration_json() -> String {
    calibration().json()
}

#[test]
fn json_preserves_failure_details_and_reports_hidden_like_the_rest() {
    let mut evidence = evidence();
    assert!(evidence.passed());
    let text = evidence.json();
    assert!(!text.contains("\"wakeups_per_second\""));
    assert!(text.contains("\"interrupt_wakeups_per_second\":100"));
    assert!(text.contains("\"idle_wakeups_per_second\":25"));
    assert!(text.contains("\"interrupt_wakeups_per_second\":{\"median\":100,\"max\":100}"));
    assert!(text.contains("\"idle_wakeups_per_second\":{\"median\":25,\"max\":25}"));
    let json = crate::release::json::parse(&evidence.json()).unwrap();
    assert_eq!(
        json.field("acceptance_status").unwrap().string().unwrap(),
        "INCOMPLETE"
    );
    assert_eq!(
        json.field("status").unwrap().string().unwrap(),
        "SCENARIOS_PASS"
    );
    let hidden = &json.field("scenarios").unwrap().array().unwrap()[3];
    assert_eq!(
        hidden.field("scenario").unwrap().string().unwrap(),
        "hidden"
    );
    assert_eq!(
        hidden.field("memory_status").unwrap().string().unwrap(),
        "PASS"
    );
    assert_eq!(
        hidden.field("config").unwrap().string().unwrap(),
        "version = 1\nview_mode = \"source\"\n"
    );
    assert_eq!(
        hidden.field("hidden_through_menu_bar").unwrap(),
        &crate::release::json::Value::Bool(true)
    );
    // A hidden run over its own 48 MiB gate fails, though it is within Source's 72 MiB.
    evidence.scenarios[3][0] = valid_run(1, 48 * 1024 * 1024 + 1);
    assert!(!evidence.passed());
    evidence.scenarios[3][0] = valid_run(1, 48 * 1024 * 1024);
    assert!(evidence.passed());
    evidence.scenarios[0][0].exited(0.1, "signal: 9\n\"test\"".into());
    assert!(!evidence.passed());
    let json = crate::release::json::parse(&evidence.json()).unwrap();
    let run = &json.field("scenarios").unwrap().array().unwrap()[0]
        .field("runs")
        .unwrap()
        .array()
        .unwrap()[0];
    assert_eq!(
        run.field("metrics").unwrap(),
        &crate::release::json::Value::Null
    );
    assert_eq!(
        run.field("exit_status").unwrap().string().unwrap(),
        "signal: 9\n\"test\""
    );
    evidence.print_summary();
}

#[cfg(not(target_os = "macos"))]
#[test]
fn measurement_off_macos_never_succeeds() {
    let options = super::parse(&["--app=x.app".into()]).unwrap();
    assert!(super::execute(&options).unwrap_err().contains("NOT_TESTED"));
}
