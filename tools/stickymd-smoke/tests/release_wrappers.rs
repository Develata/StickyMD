#![cfg(windows)]

use std::{fs, path::Path, process::Command, time::SystemTime};

#[test]
fn release_wrappers_preserve_interfaces_utf8_exit_codes_and_caller_state() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .unwrap();
    for shell in ["powershell.exe", "pwsh.exe"] {
        if Command::new(shell)
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
            continue;
        }
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let fixture = std::env::temp_dir().join(format!(
            "stickymd-release-wrappers-{}-{nonce}-中文 {} [with spaces]",
            std::process::id(),
            "a".repeat(64)
        ));
        fs::create_dir(&fixture).unwrap();
        let script = fixture.join("check.ps1");
        fs::write(
            &script,
            format!("\u{feff}{}", include_str!("release_wrappers.ps1")),
        )
        .unwrap();
        let output = Command::new(shell)
            // Start each host with its own built-in modules, not the parent host's edition.
            .env_remove("PSModulePath")
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
            .output()
            .expect("run release wrapper integration");
        fs::remove_dir_all(&fixture).unwrap();
        assert!(
            output.status.success(),
            "{shell}: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("RELEASE_WRAPPERS=PASS"));
    }
}
