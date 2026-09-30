#![cfg(windows)]

#[test]
fn phase_wrappers_use_the_real_router_and_restore_caller_state() {
    use std::{
        fs,
        path::Path,
        time::{SystemTime, UNIX_EPOCH},
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for shell in ["powershell.exe", "pwsh.exe"] {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "stickymd-phase-entry-{}-{nonce}-中文 space",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let script = directory.join("check.ps1");
        fs::write(
            &script,
            format!("\u{feff}{}", include_str!("phase_entry.ps1")),
        )
        .unwrap();
        let result = super::powershell_command(shell)
            .args(["-NoProfile", "-NonInteractive", "-File"])
            .arg(&script)
            .env_remove("STICKYMD_PHASE_BASELINE")
            .env("STICKYMD_TEST_ROOT", root)
            .env("STICKYMD_TEST_EXE", env!("CARGO_BIN_EXE_stickymd-smoke"))
            .env("STICKYMD_TEST_DIRECTORY", &directory)
            .output();
        fs::remove_dir_all(&directory).unwrap();
        let output = match result {
            Ok(output) => output,
            Err(error) => {
                assert_ne!(shell, "powershell.exe", "{error}");
                eprintln!("NOT_TESTED: PowerShell 7 is unavailable");
                continue;
            }
        };
        assert!(
            output.status.success(),
            "{shell}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("PHASE_ENTRY=PASS"));
    }
}
