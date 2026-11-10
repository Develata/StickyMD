//! Pure measurement and CLI regression tests; no GUI is started.
//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates

use super::{model::*, report::Evidence};

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

fn valid_run(repetition: usize, footprint_bytes: u64) -> Run {
    let mut run = Run::new(repetition);
    run.status = "VALID";
    run.metrics = Some(Metrics {
        footprint_bytes,
        idle_cpu_percent: 100.0,
        wakeups_per_second: 100.0,
    });
    run
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
    for (mode, _) in SCENARIOS {
        assert_eq!(
            config(mode),
            format!("version = 1\nview_mode = \"{mode}\"\n")
        );
    }
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
    for (_, limit) in SCENARIOS {
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
    let after = sample(150.0, 1_500_009_000, 130);
    let values = metrics(&before, &after, 125, 3).unwrap();
    assert!((values.idle_cpu_percent - 104.16666666666667).abs() < 1e-9);
    assert_eq!(values.wakeups_per_second, 2.0);
    assert_eq!(values.footprint_bytes, 1024);
    assert!(metrics(&before, &sample(90.0, 0, 130), 1, 1).is_err());
    assert!(metrics(&before, &sample(89.0, 10_000, 130), 1, 1).is_err());
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

#[test]
fn json_preserves_failure_details_and_hidden_never_becomes_a_pass() {
    let mut evidence = Evidence {
        environment: "{}".into(),
        app_hash: "a".repeat(64),
        fixture_hash: "b".repeat(64),
        repetitions: 5,
        timebase: (125, 3),
        scenarios: SCENARIOS
            .iter()
            .map(|(_, limit)| (1..=5).map(|i| valid_run(i, *limit)).collect())
            .collect(),
    };
    assert!(evidence.passed());
    let json = crate::release::json::parse(&evidence.json()).unwrap();
    assert_eq!(
        json.field("acceptance_status").unwrap().string().unwrap(),
        "INCOMPLETE"
    );
    let hidden = &json.field("scenarios").unwrap().array().unwrap()[3];
    assert_eq!(
        hidden.field("status").unwrap().string().unwrap(),
        "NOT_TESTED"
    );
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
