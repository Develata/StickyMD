use std::process::Command;

#[test]
fn invalid_request_returns_a_nonzero_process_exit_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
        .arg("not-a-command")
        .output()
        .expect("start stickymd-smoke subprocess");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("stickymd-smoke:"));
}

#[test]
fn successful_json_request_returns_zero_and_writes_one_json_document() {
    let repository = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("automation crate is under the repository root");
    let output = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
        .args(["phase", "00", "--json"])
        .current_dir(repository)
        .output()
        .expect("start stickymd-smoke JSON subprocess");
    assert!(
        output.status.success(),
        "stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("JSON stdout is UTF-8");
    assert_eq!(stdout.lines().count(), 1);
    assert!(stdout.starts_with("{\"schema_version\":1,"));
    assert!(stdout.trim_end().ends_with('}'));
}
