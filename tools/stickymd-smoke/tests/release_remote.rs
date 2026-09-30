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
            command.env("RELEASE_TAG", format!("v{}", env!("CARGO_PKG_VERSION")));
        }) else {
            continue;
        };
        assert_pass(host, &output, &["REMOTE_STEPS=PASS"]);
    }
}
