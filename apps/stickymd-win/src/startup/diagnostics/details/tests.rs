use super::*;

fn complete(path: PathBuf) -> Details {
    let mut details = Details::new(path);
    for (index, milestone) in [
        StartupDetail::SplitModeBegin,
        StartupDetail::SplitModeEnd,
        StartupDetail::WindowShowBegin,
        StartupDetail::WindowShowEnd,
        StartupDetail::ToolIdentityBegin,
        StartupDetail::ToolIdentityEnd,
    ]
    .into_iter()
    .enumerate()
    {
        details.record(milestone, index as u128);
    }
    details
}

#[test]
fn details_need_a_legacy_trace_and_complete_ordered_substeps() {
    assert!(Details::from_environment(false).is_none());
    let mut missing = Details::new(PathBuf::new());
    assert!(missing.encode("legacy").is_err());
    missing.record(StartupDetail::WindowShowBegin, 0);
    assert!(missing.encode("legacy").is_err());
    let mut duplicate = complete(PathBuf::new());
    duplicate.record(StartupDetail::ToolIdentityEnd, 7);
    assert!(duplicate.encode("legacy").is_err());
    let trace = "stickymd_startup_trace_v2\nprocess_start=0\neditor_ready=42\n";
    let output = complete(PathBuf::new()).encode(trace).unwrap();
    assert_eq!(
        output
            .split_once("legacy_trace_begin\n")
            .unwrap()
            .1
            .as_bytes(),
        trace.as_bytes()
    );
    assert!(output.contains("window_show_begin=2\nwindow_show_end=3\n"));
}

#[test]
fn detail_file_is_atomic_and_never_replaces_an_existing_input() {
    let path = crate::test_support::unique_temp_path("启动细分 space");
    let details = complete(path.clone());
    details.write("first trace\n").unwrap();
    let original = std::fs::read(&path).unwrap();
    assert!(details.write("second trace\n").is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    std::fs::remove_file(path).unwrap();
}
