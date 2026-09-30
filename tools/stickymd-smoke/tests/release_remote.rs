#![cfg(windows)]

#[test]
fn actual_tag_and_draft_steps_reject_bad_observations_before_remote_mutations() {
    use std::{fs, path::Path, time::SystemTime};
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    for shell in ["powershell.exe", "pwsh.exe"] {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory = std::env::temp_dir().join(format!(
            "stickymd-remote-{}-{nonce}-中文 space",
            std::process::id()
        ));
        fs::create_dir(&directory).unwrap();
        let script = directory.join("check.ps1");
        fs::write(
            &script,
            format!("\u{feff}{}", include_str!("release_remote.ps1")),
        )
        .unwrap();
        let result = super::powershell_command(shell)
            .args(["-NoProfile", "-NonInteractive", "-File"])
            .arg(&script)
            .env("STICKYMD_TEST_ROOT", root)
            .env("STICKYMD_TEST_EXE", env!("CARGO_BIN_EXE_stickymd-smoke"))
            .env("STICKYMD_TEST_DIRECTORY", &directory)
            .env("RELEASE_TAG", format!("v{}", env!("CARGO_PKG_VERSION")))
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
        assert!(String::from_utf8_lossy(&output.stdout).contains("REMOTE_STEPS=PASS"));
    }
}
