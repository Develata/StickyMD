use std::process::Command;

#[cfg(windows)]
#[test]
fn resource_diagnostic_options_wrapper_never_silently_dispatches_a_qualification_action() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .unwrap();
    let output = super::powershell_command("powershell.exe").args(["-NoProfile", "-NonInteractive", "-Command", r#"
$ErrorActionPreference = 'Stop'
function cargo {
    if ($args -notcontains '--resource-resume' -or $args -contains 'qualification') {
        throw 'Unexpected diagnostic Cargo dispatch'
    }
    $global:LASTEXITCODE = 0
    if ($args -contains '--resource-plan') { 'PLAN_FORWARDED' } elseif ($args -contains '--resource-failure-first') { 'FAILURE_FIRST_FORWARDED' } else { 'RESUME_FORWARDED' }
}
$script = Join-Path $env:STICKYMD_TEST_ROOT 'tools/smoke/phase-14.ps1'
foreach ($mode in @('ResourcePlan', 'ResourceFailureFirst', 'ResourceResume')) {
foreach ($action in @('SourceFreeze', 'Environment', 'WindowStress', 'Campaign')) {
    $parameters = @{ Resources=$true; ResourceResume=$true }
    $parameters[$mode] = $true
    $parameters[$action] = $true
    $rejected = $false
    try { & $script @parameters } catch {
        if ($_.Exception.Message -notlike "$mode requires*") { throw }
        $rejected = $true
    }
    if (-not $rejected) { throw 'Diagnostic option silently executed another action' }
}
}
& $script -Resources -ResourceModule zoom -ResourceResume -ResourcePlan -EvidenceFile target/diagnostics/plan.json
& $script -Resources -ResourceModule zoom -ResourceResume -ResourceFailureFirst -EvidenceFile target/diagnostics/first.json
& $script -Resources -ResourceModule zoom -ResourceResume -EvidenceFile target/diagnostics/resume.json
"#]).env("STICKYMD_TEST_ROOT", root).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .collect::<Vec<_>>(),
        [
            "PLAN_FORWARDED",
            "FAILURE_FIRST_FORWARDED",
            "RESUME_FORWARDED"
        ]
    );
}

#[test]
fn resource_plan_is_read_only_even_when_program_and_source_identity_are_missing() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("stickymd-plan-{}-{nonce}", std::process::id()));
    std::fs::create_dir_all(root.join("target")).unwrap();
    for marker in ["Cargo.toml", "AGENTS.md"] {
        std::fs::write(root.join(marker), "").unwrap();
    }
    std::fs::write(root.join(".gitignore"), "target/\n").unwrap();
    std::fs::write(root.join("target/keep.json"), "preserved").unwrap();
    assert!(
        Command::new("git")
            .args(["init", "--quiet"])
            .current_dir(&root)
            .status()
            .unwrap()
            .success()
    );
    let output = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
        .args([
            "phase",
            "14",
            "--resources",
            "--resource-module=zoom",
            "--resource-resume",
            "--resource-plan",
            "--resource-failure-first",
            "--evidence-file=target/keep.json",
        ])
        .env_remove("STICKYMD_SMOKE_RESOURCE_CASE")
        .current_dir(&root)
        .output()
        .unwrap();
    let unchanged = std::fs::read_to_string(root.join("target/keep.json")).unwrap() == "preserved";
    let outputs = std::fs::read_dir(root.join("target")).unwrap().count();
    let formal = root.join("dist").exists();
    assert!(root.starts_with(std::env::temp_dir()));
    std::fs::remove_dir_all(&root).unwrap();
    assert!(output.status.success(), "{output:?}");
    let json = String::from_utf8(output.stdout).unwrap();
    assert!(
        json.contains("\"status\":\"NOT_RUN\"") && json.contains("RESUME_DISABLED"),
        "{json}"
    );
    assert!(unchanged && outputs == 1 && !formal);
    assert!(!String::from_utf8_lossy(&output.stderr).contains("TASK_TIMING"));
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("RESOURCE_FAILURE_FIRST unit=none status=FIXED_ORDER")
    );
}

#[test]
fn diagnostic_commands_preserve_internal_success_files() {
    for receipt in [
        "dist/evidence/module-success/resources-window.json",
        "dist/evidence/module-success/evidence/resources-window-fixture.json",
    ] {
        assert_protected_output(receipt, "", true);
    }
}

#[cfg(windows)]
#[test]
fn uncreated_formal_outputs_reject_win32_trailing_dot_and_space_aliases() {
    for receipt in [
        "dist/evidence/resources-qualification.json",
        "dist/evidence/resources/window.json",
        "dist/evidence/source-success/workspace-tests.json",
    ] {
        for suffix in [".", " "] {
            assert_protected_output(receipt, suffix, false);
        }
    }
}

fn assert_protected_output(receipt: &str, suffix: &str, existing: bool) {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "stickymd-reserved-output-{}-{nonce}",
        std::process::id()
    ));
    let destination = root.join(receipt);
    std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
    for marker in ["Cargo.toml", "AGENTS.md"] {
        std::fs::write(root.join(marker), "").unwrap();
    }
    if existing {
        std::fs::write(&destination, "preserved receipt").unwrap();
    }
    let output = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
        .args(["phase", "00"])
        .arg(format!("--evidence-file={receipt}{suffix}"))
        .current_dir(&root)
        .output()
        .unwrap();
    let unchanged = if existing {
        std::fs::read_to_string(&destination).unwrap() == "preserved receipt"
    } else {
        !destination.exists()
    };
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(
        unchanged,
        "diagnostic wrote protected destination: {receipt}{suffix:?}"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("formal qualification output"),
        "{output:?}"
    );
}

#[test]
fn partial_resource_requests_fail_before_overwriting_formal_evidence() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "stickymd-resource-scope-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir_all(root.join("dist/evidence/resources")).unwrap();
    for marker in ["Cargo.toml", "AGENTS.md"] {
        std::fs::write(root.join(marker), "").unwrap();
    }
    let resource_path = "dist/evidence/resources-qualification.json";
    let child_path = "dist/evidence/resources/window.json";
    for path in [resource_path, child_path] {
        std::fs::write(root.join(path), "preserved receipt").unwrap();
    }
    for (selection, extra, path, filter) in [
        ("14", Some("--resource-module=window"), resource_path, None),
        ("08", None, resource_path, None),
        ("14", None, resource_path, Some("source")),
        ("14", None, child_path, None),
        ("14", Some("--resource-failure-first"), resource_path, None),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"));
        command
            .args(["phase", selection, "--resources"])
            .arg(format!("--evidence-file={path}"))
            .current_dir(&root)
            .env_remove("STICKYMD_SMOKE_RESOURCE_CASE");
        if let Some(extra) = extra {
            command.arg(extra);
            if extra == "--resource-failure-first" {
                command.arg("--resource-resume");
            }
        }
        if let Some(filter) = filter {
            command.env("STICKYMD_SMOKE_RESOURCE_CASE", filter);
        }
        let output = command.output().unwrap();
        assert_eq!(output.status.code(), Some(1));
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("formal qualification output"),
            "{:?}",
            output
        );
        assert_eq!(
            std::fs::read_to_string(root.join(path)).unwrap(),
            "preserved receipt"
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[cfg(not(windows))]
#[test]
fn unsupported_gui_qualification_emits_not_tested_and_returns_failure() {
    let output = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
        .args(["qualification", "environment"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let evidence = String::from_utf8(output.stdout).unwrap();
    assert!(evidence.contains("\"UNSUPPORTED\""), "{evidence}");
    assert!(evidence.contains("\"NOT_TESTED\""), "{evidence}");
    assert!(!evidence.contains("\"PASSED\""), "{evidence}");
    assert!(String::from_utf8_lossy(&output.stderr).contains("unsupported on this host"));
}

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
        vec!["release", "verify-workflow", "--source-sha", "abc"],
        vec![
            "release",
            "verify-workflow",
            "--source-sha",
            "abc",
            "--workflow-json",
            "missing",
            "--runtime",
        ],
        vec![
            "release",
            "notices",
            "--destination",
            "one",
            "--destination",
            "two",
        ],
        vec!["release", "workspace-version", "--runtime"],
        vec!["release", "prepare-package"],
        vec!["release", "prepare-package", "--exe", "missing.exe"],
        vec!["release", "syft-plan", "--runtime"],
        vec![
            "release",
            "syft-publish",
            "--kind",
            "unknown",
            "--input",
            "file",
        ],
        vec!["release", "syft-verify"],
        vec!["release", "verify-package", "--exact-candidate"],
        vec!["release", "checksums", "--zip", "missing"],
        vec!["release", "publish-sbom", "--input", "missing"],
        vec!["release", "checksums", "--zip=a", "--output=b", "--runtime"],
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
fn checksum_match_does_not_make_malformed_sbom_a_valid_package() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "stickymd-invalid-sbom-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("fixture.zip"), b"abc").unwrap();
    std::fs::write(root.join("SBOM.spdx.json"), b"abc").unwrap();
    let hash = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
    std::fs::write(
        root.join("SHA256SUMS.txt"),
        format!("{hash} *fixture.zip\n{hash} *SBOM.spdx.json\n"),
    )
    .unwrap();
    let result = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
        .args(["release", "verify-package", "--package-directory"])
        .arg(&root)
        .arg("--zip")
        .arg(root.join("fixture.zip"))
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    std::fs::remove_dir_all(root).unwrap();
    assert_eq!(result.status.code(), Some(1));
    assert!(result.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&result.stderr).contains("JSON"),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
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
    assert!(stdout.contains("\"name\":\"task.execution_seconds\""));
    assert!(String::from_utf8_lossy(&output.stderr).contains("TASK_TIMING"));
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
        // Planning reads Git status without refreshing the index shared by other cases.
        .env("GIT_OPTIONAL_LOCKS", "0")
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
    assert!(json.contains("\"smoke_needed\":true"));
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
        "--smoke=success",
    ];
    for (index, job) in [(8, "headless"), (11, "smoke")] {
        for status in ["success", "failure", "cancelled", "skipped"] {
            let argument = format!("--{job}={status}");
            let mut args = base;
            args[index] = &argument;
            let output = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
                .args(args)
                .current_dir(env!("CARGO_MANIFEST_DIR"))
                .output()
                .expect("start CI result subprocess");
            assert_eq!(output.status.success(), status == "success", "{argument}");
            if status != "success" {
                assert!(output.stdout.is_empty());
                assert!(String::from_utf8_lossy(&output.stderr).contains("expected Success"));
            }
        }
    }
    let missing = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
        .args(&base[..11])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert!(String::from_utf8_lossy(&missing.stderr).contains("missing CI result field smoke"));
}
