//! Windows host invocation and encoding facts, without wrapper-specific assertions.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::{
    fs,
    io::ErrorKind,
    os::windows::process::CommandExt,
    path::Path,
    process::{Command, Output},
};

use super::{TemporaryDirectory, repository_root};

#[derive(Clone, Copy)]
pub struct Host {
    pub executable: &'static str,
    pub edition: &'static str,
    required: bool,
}

pub const WINDOWS_POWERSHELL: Host = Host {
    executable: "powershell.exe",
    edition: "ps51",
    required: true,
};
pub const POWERSHELL_7: Host = Host {
    executable: "pwsh.exe",
    edition: "ps7",
    required: false,
};
pub const HOSTS: [Host; 2] = [WINDOWS_POWERSHELL, POWERSHELL_7];

pub struct ScriptFixture {
    directory: TemporaryDirectory,
}

impl ScriptFixture {
    pub fn new(label: &str, suffix: &str, script: &str) -> Self {
        let directory = TemporaryDirectory::new(label, suffix);
        // WinPS 5.1 otherwise decodes a UTF-8 file as the machine's legacy code page.
        fs::write(
            directory.path().join("check.ps1"),
            format!("\u{feff}{script}"),
        )
        .expect("write the UTF-8 BOM compatibility script");
        Self { directory }
    }

    pub fn path(&self) -> &Path {
        self.directory.path()
    }

    pub fn run(&self, host: Host, configure: impl FnOnce(&mut Command)) -> Option<Output> {
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        let mut command = Command::new(host.executable);
        // Console, environment and writes belong to this child/fixture. The prebuilt
        // CLI is shared read-only; no case starts Cargo or refreshes the shared Git index.
        command
            .creation_flags(CREATE_NO_WINDOW)
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
            ])
            .arg(self.path().join("check.ps1"))
            .env_remove("PSModulePath")
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env("STICKYMD_TEST_ROOT", repository_root())
            .env("STICKYMD_TEST_EXE", env!("CARGO_BIN_EXE_stickymd-smoke"))
            .env("STICKYMD_TEST_DIRECTORY", self.path());
        configure(&mut command);
        match command.output() {
            Ok(output) => Some(output),
            Err(error) if !host.required && error.kind() == ErrorKind::NotFound => {
                eprintln!("NOT_TESTED: PowerShell 7 is unavailable");
                None
            }
            Err(error) => panic!("start {} compatibility check: {error}", host.executable),
        }
    }
}

pub fn assert_pass(host: Host, output: &Output, markers: &[&str]) {
    assert!(
        output.status.success(),
        "{}: {}",
        host.executable,
        diagnostic(output)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    for marker in markers {
        assert!(
            stdout.contains(marker),
            "{}: missing {marker}: {}",
            host.executable,
            diagnostic(output)
        );
    }
}

pub fn diagnostic(output: &Output) -> String {
    fn bounded(bytes: &[u8]) -> String {
        const EDGE: usize = 4096;
        if bytes.len() <= EDGE * 2 {
            return String::from_utf8_lossy(bytes).into_owned();
        }
        format!(
            "{}\n[{} output bytes omitted]\n{}",
            String::from_utf8_lossy(&bytes[..EDGE]),
            bytes.len() - EDGE * 2,
            String::from_utf8_lossy(&bytes[bytes.len() - EDGE..])
        )
    }
    format!(
        "exit={} stdout={} stderr={}",
        output.status,
        bounded(&output.stdout),
        bounded(&output.stderr)
    )
}
