//! Compiled development commands keep planning and observations separate from acceptance.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::support::{TemporaryDirectory, repository_root};
use std::{fs, process::Command};

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"));
    command
        .current_dir(repository_root())
        .env("GIT_OPTIONAL_LOCKS", "0");
    command
}

#[test]
fn local_plan_is_read_only_and_malformed_requests_fail_before_checks() {
    let output = cli().args(["dev-check", "--plan"]).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\"NOT_RUN\""), "{text}");
    assert!(!text.contains("LOCAL_CHECKS=PASS"), "{text}");
    for args in [
        vec!["dev-check", "--mode=unknown", "--plan"],
        vec!["dev-check", "--plan", "--plan"],
        vec!["dev-check", "--evidence-file=candidate.json"],
        vec!["timings"],
        vec!["timings", "--input", "missing-log-file"],
    ] {
        let output = cli().args(&args).output().unwrap();
        assert!(!output.status.success(), "{args:?}: {output:?}");
        assert!(output.stdout.is_empty(), "{args:?}: {output:?}");
    }
}

#[test]
fn timing_cli_reads_unicode_inputs_outside_a_repository_and_never_emits_partial_results() {
    let directory = TemporaryDirectory::new("timing-cli", "中文 space");
    let input = directory.path().join("原始日志 space.log");
    let content = "TASK_TIMING task=\"workspace tests\" status=FAILED elapsed_seconds=1.250000\n";
    fs::write(&input, content).unwrap();
    let output = cli()
        .current_dir(directory.path())
        .args(["timings", "--input"])
        .arg(&input)
        .arg("--json")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    assert!(text.contains("\"status\":\"OBSERVATION_ONLY\""), "{text}");
    assert!(text.contains("\"agent_work_seconds\":null"), "{text}");
    assert!(text.contains("\"status\":\"FAILED\""), "{text}");
    assert_eq!(fs::read_to_string(&input).unwrap(), content);

    let output = cli()
        .args(["timings", "--input"])
        .arg(&input)
        .args(["--input", "missing-log-file", "--json"])
        .output()
        .unwrap();
    assert!(!output.status.success(), "{output:?}");
    assert!(output.stdout.is_empty(), "{output:?}");
}
