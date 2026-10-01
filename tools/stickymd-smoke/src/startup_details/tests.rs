use super::*;

const DETAILS: &str = include_str!("../../tests/fixtures/startup-details-v1.trace");

#[test]
fn substeps_account_for_the_parent_interval_without_claiming_qualification() {
    let trace = crate::startup_trace::fixture();
    let analysis = analysis::analyze(&trace, DETAILS).unwrap();
    assert_eq!(
        analysis,
        analysis::analyze(&trace.replace('\n', "\r\n"), &DETAILS.replace('\n', "\r\n")).unwrap()
    );
    assert_eq!(analysis.pid, 4242);
    assert_eq!(analysis.tray_to_visible_us, 100);
    assert_eq!(analysis.segments_us, [3, 70, 10]);
    assert_eq!(analysis.other_us, 17);
    let json = analysis.render(true);
    assert!(json.contains("\"status\":\"OBSERVATION_ONLY\""));
    assert!(json.contains("\"qualification_receipt\":false"));
    assert!(!json.contains("PASSED"));
    assert!(analysis.render(false).contains("window_show=70 us"));
}

#[test]
fn mismatched_trace_missing_duplicate_reordered_and_out_of_bounds_details_fail() {
    let trace = crate::startup_trace::fixture();
    for invalid in [
        DETAILS.replace("_details_v1", "_details_v2"),
        DETAILS.replace("pid=4242", "pid=0"),
        DETAILS.replace("pid=4242", "pid=4294967296"),
        DETAILS.replace("pid=4242\n", ""),
        DETAILS.replace("split_mode_end=1805\n", ""),
        DETAILS.replace(
            "split_mode_end=1805",
            "split_mode_end=1805\nsplit_mode_end=1805",
        ),
        DETAILS.replace("split_mode_end=1805", "split_mode_end=1800"),
        DETAILS.replace("split_mode_end=1805", "split_mode_end=NaN"),
        DETAILS.replace("split_mode_begin=1802", "split_mode_begin=1799"),
        DETAILS.replace("tool_identity_end=1895", "tool_identity_end=1901"),
        DETAILS.replace("legacy_trace_begin\n", "unknown=0\nlegacy_trace_begin\n"),
        DETAILS.replace("editor_ready=2500", "editor_ready=2501"),
        DETAILS.replace("legacy_trace_begin", "legacy_trace_missing"),
        format!("{DETAILS}\n"),
    ] {
        assert!(analysis::analyze(&trace, &invalid).is_err(), "{invalid}");
    }
    assert!(analysis::analyze(&trace.replace("_v2", "_v1"), DETAILS).is_err());
}

#[test]
fn cli_rejects_incomplete_duplicate_and_evidence_writing_options() {
    let valid = [
        "--trace",
        "中文 trace",
        "--details",
        "中文 details",
        "--json",
    ]
    .map(str::to_owned);
    let command = crate::cli::CommandLine::parse(
        std::iter::once("startup-details".into()).chain(valid.clone()),
    )
    .unwrap();
    assert!(matches!(
        command,
        crate::cli::CommandLine::StartupDetails(_)
    ));
    for args in [
        vec![],
        vec!["--trace"],
        vec!["--trace", ""],
        vec!["--trace", "x"],
        vec!["--details", "x"],
        vec!["--trace", "x", "--trace", "y", "--details", "z"],
        vec!["--trace", "x", "--details", "y", "--json", "--json"],
        vec!["--trace", "x", "--details", "y", "--evidence-file=z"],
    ] {
        assert!(parse(&args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>()).is_err());
    }
}
