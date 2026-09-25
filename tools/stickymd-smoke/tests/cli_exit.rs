use std::process::Command;

#[test]
fn package_checksum_roles_must_refer_to_distinct_artifacts() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "stickymd-checksum-alias-{}-{nonce}-中文 space",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let zip = directory.join("SBOM.spdx.json");
    std::fs::write(&zip, b"abc").unwrap();
    let hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    std::fs::write(
        directory.join("SHA256SUMS.txt"),
        format!("{hash} *SBOM.spdx.json\n{hash} *unexpected.bin\n"),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
        .args(["release", "verify-package", "--package-directory"])
        .arg(&directory)
        .arg("--zip")
        .arg(&zip)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output();
    std::fs::remove_dir_all(directory).unwrap();
    let output = output.unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("distinct names"),
        "roles must be rejected before ZIP/native checks: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn malformed_release_requests_never_emit_a_success_marker() {
    for args in [
        vec!["release", "notices"],
        vec![
            "release",
            "verify-promoted",
            "--artifact-directory",
            "missing",
        ],
        vec!["release", "verify-package", "--unknown", "value"],
        vec![
            "release",
            "notices",
            "--destination",
            "one",
            "--destination",
            "two",
        ],
        vec!["release", "workspace-version", "--runtime"],
        vec!["release", "verify-package", "--exact-candidate"],
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
            .args(args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
    }
}

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
    assert!(stdout.starts_with("{\"schema_version\":2,\"suite_version\":\"2\","));
    assert!(stdout.trim_end().ends_with('}'));
}

#[test]
fn module_plan_is_one_unexecuted_json_document_with_the_explicit_scope() {
    let output = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
        .args(["modules", "run", "render,core,render", "--plan"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("start module plan subprocess");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert_eq!(stdout.lines().count(), 1);
    assert!(stdout.starts_with("{\"schema_version\":1,\"kind\":\"selected-headless-plan\","));
    assert!(stdout.contains("\"status\":\"NOT_RUN\""));
    assert!(stdout.contains("\"modules\":[\"core\",\"render\"]"));
    assert!(stdout.contains(
        "\"args\":[\"test\",\"--locked\",\"-p\",\"stickymd-core\",\"-p\",\"stickymd-render\"]"
    ));
    assert!(stdout.trim_end().ends_with('}'));
    assert!(!stdout.contains("PASSED"));
}

#[test]
fn unknown_module_returns_a_nonzero_process_exit_code() {
    let output = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
        .args(["modules", "run", "unknown"])
        .output()
        .expect("start invalid module subprocess");
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown headless module"));
    assert!(output.stdout.is_empty());
}

#[test]
fn ci_full_plan_returns_one_source_scoped_unexecuted_json_document() {
    let output = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
        .args(["ci", "plan", "--full"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("start full CI plan subprocess");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let json = String::from_utf8(output.stdout).unwrap();
    assert_eq!(json.lines().count(), 1);
    assert!(json.starts_with("{\"schema_version\":1,\"kind\":\"headless-ci-plan\","));
    assert!(json.contains("\"status\":\"NOT_RUN\""));
    assert!(json.contains("\"full\":true"));
    assert!(json.contains("\"base\":null"));
    assert!(!json.contains("artifact_sha256"));
}

#[test]
fn ci_aggregate_process_distinguishes_success_cancelled_and_missing_jobs() {
    let base = [
        "ci",
        "verify",
        "--cancelled=false",
        "--full=false",
        "--modules=smoke",
        "--plan=success",
        "--dependency=success",
        "--quality=success",
        "--headless=success",
        "--release=skipped",
        "--portable=skipped",
    ];
    for (status, expected) in [
        ("--headless=success", true),
        ("--headless=cancelled", false),
        ("--headless=skipped", false),
    ] {
        let mut args = base;
        args[8] = status;
        let output = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
            .args(args)
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .output()
            .expect("start CI result subprocess");
        assert_eq!(output.status.success(), expected, "{status}");
        if !expected {
            assert!(output.stdout.is_empty());
            assert!(String::from_utf8_lossy(&output.stderr).contains("expected Success"));
        }
    }
}
