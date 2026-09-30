#![cfg(windows)]

use super::support::{
    TemporaryDirectory,
    powershell::{Host, POWERSHELL_7, ScriptFixture, WINDOWS_POWERSHELL, assert_pass},
};

#[test]
fn powershell_51_preserves_release_interfaces_utf8_exit_codes_and_caller_state() {
    check_release_wrappers(WINDOWS_POWERSHELL);
}

#[test]
fn powershell_7_preserves_release_interfaces_utf8_exit_codes_and_caller_state() {
    check_release_wrappers(POWERSHELL_7);
}

fn check_release_wrappers(host: Host) {
    // Both cases read the repository and the same prebuilt CLI. Fixture/package writes
    // stay in distinct edition directories, even with identical clock values. Cargo owns
    // dependency metadata/cache access; neither case starts a Cargo build.
    let fixture = ScriptFixture::new(
        "release-wrappers",
        &format!("{}-中文 {} [with spaces]", host.edition, "a".repeat(64)),
        &format!(
            "{}\n{}\n{}",
            include_str!("release_wrappers.ps1"),
            include_str!("release_outputs.ps1"),
            include_str!("release_syft.ps1")
        ),
    );
    // Package names include a source SHA and a temporary suffix; stay below WinPS 5.1 MAX_PATH.
    let outputs = TemporaryDirectory::new("outputs", &format!("{}-中文 space", host.edition));
    let Some(output) = fixture.run(host, |command| {
        command.env("STICKYMD_TEST_OUTPUT_DIRECTORY", outputs.path());
    }) else {
        return;
    };
    assert_pass(
        host,
        &output,
        &[
            "RELEASE_WRAPPERS=PASS",
            "RELEASE_OUTPUTS=PASS",
            "RELEASE_SYFT=PASS",
        ],
    );
}
