//! Real CLI boundary checks for isolated, content-free startup observations.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::support::TemporaryDirectory;
use std::{
    fs,
    process::{Command, Output},
};

const TRACE: &str = include_str!("fixtures/startup-v2.trace");
const DETAILS: &str = include_str!("fixtures/startup-details-v1.trace");

#[test]
fn startup_details_cli_is_read_only_outside_a_checkout_and_fails_without_partial_output() {
    let directory = TemporaryDirectory::new("startup-details", "中文 space");
    let trace = directory.path().join("启动 trace.txt");
    let details = directory.path().join("细分 details.txt");
    fs::write(&trace, TRACE).unwrap();
    fs::write(&details, DETAILS).unwrap();
    let run = || -> Output {
        Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
            .current_dir(directory.path())
            .args(["startup-details", "--trace"])
            .arg(&trace)
            .arg("--details")
            .arg(&details)
            .arg("--json")
            .output()
            .unwrap()
    };
    let output = run();
    assert!(output.status.success(), "{output:?}");
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(json.contains("\"window_show\":70"), "{json}");
    assert!(json.contains("\"qualification_receipt\":false"), "{json}");
    assert_eq!(fs::read_to_string(&trace).unwrap(), TRACE);
    assert_eq!(fs::read_to_string(&details).unwrap(), DETAILS);
    assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 2);

    for invalid in [
        DETAILS
            .replace("editor_ready=2500", "editor_ready=2501")
            .into_bytes(),
        vec![b'x'; 8193],
        vec![0xff],
    ] {
        fs::write(&details, &invalid).unwrap();
        let output = run();
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert_eq!(fs::read(&details).unwrap(), invalid);
    }
    fs::remove_file(&details).unwrap();
    assert_eq!(run().status.code(), Some(1));
    fs::create_dir(&details).unwrap();
    assert_eq!(run().status.code(), Some(1));
}
