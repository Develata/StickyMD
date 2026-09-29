#![cfg(windows)]

use std::{fs, path::Path, time::SystemTime};

use super::powershell_command;

#[test]
fn powershell_51_preserves_release_interfaces_utf8_exit_codes_and_caller_state() {
    check_release_wrappers("powershell.exe", "ps51");
}

#[test]
fn powershell_7_preserves_release_interfaces_utf8_exit_codes_and_caller_state() {
    check_release_wrappers("pwsh.exe", "ps7");
}

fn check_release_wrappers(shell: &str, edition: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    if powershell_command(shell)
        .args([
            "-NoProfile",
            "-Command",
            "$PSVersionTable.PSVersion.ToString()",
        ])
        .output()
        .is_err()
    {
        // PowerShell 7 is optional on the Windows test host; 5.1 is mandatory.
        assert_ne!(
            shell, "powershell.exe",
            "Windows PowerShell 5.1 is required"
        );
        eprintln!("NOT_TESTED: PowerShell 7 is unavailable");
        return;
    }
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    // Both cases read the repository and the same prebuilt CLI. Fixture/package writes
    // stay in distinct edition directories, even with identical clock values. Cargo owns
    // dependency metadata/cache access; neither case starts a Cargo build.
    let fixture = std::env::temp_dir().join(format!(
        "stickymd-release-wrappers-{}-{nonce}-{edition}-中文 {} [with spaces]",
        std::process::id(),
        "a".repeat(64)
    ));
    fs::create_dir(&fixture).unwrap();
    // Package names include a source SHA and a temporary suffix; stay below WinPS 5.1 MAX_PATH.
    let outputs = std::env::temp_dir().join(format!(
        "stickymd-outputs-{}-{nonce}-{edition}-中文 space",
        std::process::id()
    ));
    fs::create_dir(&outputs).unwrap();
    let script = fixture.join("check.ps1");
    fs::write(
        &script,
        format!(
            "\u{feff}{}\n{}",
            include_str!("release_wrappers.ps1"),
            include_str!("release_outputs.ps1")
        ),
    )
    .unwrap();
    let output = powershell_command(shell)
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
        ])
        .arg(&script)
        .env("STICKYMD_TEST_EXE", env!("CARGO_BIN_EXE_stickymd-smoke"))
        .env("STICKYMD_TEST_ROOT", root)
        .env("STICKYMD_TEST_DIRECTORY", &fixture)
        .env("STICKYMD_TEST_OUTPUT_DIRECTORY", &outputs)
        .output()
        .expect("run release wrapper integration");
    fs::remove_dir_all(&fixture).unwrap();
    fs::remove_dir_all(&outputs).unwrap();
    assert!(
        output.status.success(),
        "{shell}: stdout={} stderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("RELEASE_WRAPPERS=PASS"));
    assert!(String::from_utf8_lossy(&output.stdout).contains("RELEASE_OUTPUTS=PASS"));
}
