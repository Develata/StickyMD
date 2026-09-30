#![cfg(windows)]

use super::support::powershell::{HOSTS, ScriptFixture, assert_pass};

#[test]
fn actual_tag_and_draft_steps_reject_bad_observations_before_remote_mutations() {
    for host in HOSTS {
        let fixture = ScriptFixture::new(
            "remote",
            &format!("{}-中文 space", host.edition),
            include_str!("release_remote.ps1"),
        );
        let Some(output) = fixture.run(host, |command| {
            command
                .env("RELEASE_TAG", format!("v{}", env!("CARGO_PKG_VERSION")))
                // Exercise PowerShell's path normalization even without NTFS 8.3 names.
                .env("STICKYMD_TEST_DIRECTORY", fixture.path().join("."));
        }) else {
            continue;
        };
        assert_pass(host, &output, &["REMOTE_STEPS=PASS"]);
    }
}
