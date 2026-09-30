#![cfg(windows)]

use super::support::powershell::{HOSTS, ScriptFixture, assert_pass};

#[test]
fn phase_wrappers_use_the_real_router_and_restore_caller_state() {
    for host in HOSTS {
        let fixture = ScriptFixture::new(
            "phase-entry",
            &format!("{}-中文 space", host.edition),
            include_str!("phase_entry.ps1"),
        );
        let Some(output) = fixture.run(host, |command| {
            command.env_remove("STICKYMD_PHASE_BASELINE");
        }) else {
            continue;
        };
        assert_pass(host, &output, &["PHASE_ENTRY=PASS"]);
    }
}
