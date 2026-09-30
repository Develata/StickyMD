use std::{
    io::Write,
    process::{Command, Stdio},
};

fn verify(source: &str, observation: &str) -> std::process::Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"))
        .args([
            "release",
            "verify-workflow",
            "--source-sha",
            source,
            "--workflow-json",
            "-",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(observation.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn workflow_observation_cli_fails_closed_and_does_not_claim_candidate_qualification() {
    let sha = "a".repeat(40);
    let valid = format!(
        r#"{{"head_sha":"{sha}","conclusion":"success","name":"release","display_title":"中文 with spaces"}}"#
    );
    let output = verify(&sha, &valid);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"WORKFLOW_IDENTITY=PASS\n");
    for invalid in [
        "{}".to_owned(),
        valid.replace("success", "cancelled"),
        valid.replace("release", "ci"),
        valid.replace(&sha, &"b".repeat(40)),
        valid.replace("\"conclusion\":\"success\"", "\"conclusion\":null"),
        valid.replace("\"head_sha\"", "\"headSha\""),
    ] {
        let output = verify(&sha, &invalid);
        assert_eq!(output.status.code(), Some(1), "{invalid}");
        assert!(output.stdout.is_empty(), "failed identity emitted success");
    }
}

#[test]
fn workflow_observation_file_supports_unicode_paths_and_rejects_missing_input() {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "stickymd-workflow-{}-{nonce}-中文 space.json",
        std::process::id()
    ));
    let sha = "c".repeat(40);
    std::fs::write(
        &path,
        format!(r#"{{"head_sha":"{sha}","conclusion":"success","name":"release"}}"#),
    )
    .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_stickymd-smoke"));
    command
        .args([
            "release",
            "verify-workflow",
            "--source-sha",
            &sha,
            "--workflow-json",
        ])
        .arg(&path)
        .current_dir(env!("CARGO_MANIFEST_DIR"));
    let output = command.output().unwrap();
    std::fs::remove_file(&path).unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(output.stdout, b"WORKFLOW_IDENTITY=PASS\n");
    let missing = command.output().unwrap();
    assert_eq!(missing.status.code(), Some(1));
    assert!(missing.stdout.is_empty());
}

#[cfg(windows)]
#[test]
fn actual_workflow_step_queries_once_and_propagates_query_or_validation_failures() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let workflow = std::fs::read_to_string(root.join(".github/workflows/promote-release.yml"))
        .unwrap()
        .replace("\r\n", "\n");
    let body = workflow
        .split("- name: Verify original candidate workflow identity\n")
        .nth(1)
        .unwrap()
        .split("        run: |\n")
        .nth(1)
        .unwrap()
        .split("\n      - name:")
        .next()
        .unwrap()
        .lines()
        .map(|line| line.strip_prefix("          ").unwrap_or(line))
        .collect::<Vec<_>>()
        .join("\n");
    let script = format!(
        r#"
$ErrorActionPreference = 'Stop'
function gh {{
    if (($args -join '|') -cne 'api|repos/owner/StickyMD/actions/runs/123') {{ throw 'unexpected remote query' }}
    [IO.File]::AppendAllText($env:TRACE_PATH, "gh`n")
    $global:LASTEXITCODE = [int]$env:QUERY_EXIT
    if ($LASTEXITCODE -eq 0) {{ [IO.File]::ReadAllText($env:OBSERVATION_PATH) }}
}}
function cargo {{
    if (($args[0..4] -join ' ') -cne 'run --quiet -p stickymd-smoke --locked') {{ throw 'unexpected Cargo invocation' }}
    [IO.File]::AppendAllText($env:TRACE_PATH, "rust`n")
    $input | & $env:STICKYMD_TEST_EXE @($args[5..($args.Length - 1)])
}}
{body}
"#
    );
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let directory = std::env::temp_dir().join(format!(
        "stickymd-workflow-step-{}-{nonce}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let script_path = directory.join("step.ps1");
    let observation = directory.join("run.json");
    let trace = directory.join("calls.txt");
    std::fs::write(&script_path, script).unwrap();
    let source = "d".repeat(40);
    let valid = format!(r#"{{"head_sha":"{source}","conclusion":"success","name":"release"}}"#);
    for shell in ["powershell.exe", "pwsh.exe"] {
        if super::powershell_command(shell)
            .args(["-NoProfile", "-Command", "exit 0"])
            .status()
            .is_err()
        {
            assert_ne!(
                shell, "powershell.exe",
                "Windows PowerShell 5.1 is mandatory"
            );
            eprintln!("NOT_TESTED: PowerShell 7 is unavailable");
            continue;
        }
        for (input, query_exit, expected_exit) in [
            (valid.clone(), "0", 0),
            (valid.replace("success", "failure"), "0", 1),
            (valid.clone(), "23", 23),
        ] {
            std::fs::write(&observation, input).unwrap();
            std::fs::write(&trace, []).unwrap();
            let output = super::powershell_command(shell)
                .args([
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                ])
                .arg(&script_path)
                .current_dir(root)
                .env("GH_REPO", "owner/StickyMD")
                .env("ARTIFACT_RUN_ID", "123")
                .env("APPROVED_SOURCE_SHA", &source)
                .env("STICKYMD_TEST_EXE", env!("CARGO_BIN_EXE_stickymd-smoke"))
                .env("TRACE_PATH", &trace)
                .env("OBSERVATION_PATH", &observation)
                .env("QUERY_EXIT", query_exit)
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(expected_exit),
                "{shell}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                std::fs::read_to_string(&trace).unwrap(),
                if query_exit == "0" {
                    "gh\nrust\n"
                } else {
                    "gh\n"
                }
            );
            if expected_exit == 0 {
                assert!(String::from_utf8_lossy(&output.stdout).contains("WORKFLOW_IDENTITY=PASS"));
            } else {
                assert!(output.stdout.is_empty());
            }
        }
    }
    std::fs::remove_dir_all(directory).unwrap();
}
