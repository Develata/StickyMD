//! The record of macOS GUI acceptance and its reuse rule (USER 2026-11-12, report
//! `2026-11-11-macos-gui-rerun-reuse`). Every row of a finished `mac-gui` run and every USER
//! receipt is appended to one ledger. A later run skips a row whose PASS was recorded for the
//! same app, driver and VM image; a FAIL or NOT_TESTED is never reused; a USER receipt holds only
//! for the app executable it names. It plans and records; it launches nothing.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod decide;
mod options;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod record;
#[cfg(target_os = "macos")]
mod run;
#[cfg(test)]
mod tests;

pub(crate) use options::{Options, parse};

pub(crate) fn execute(options: &Options, root: &std::path::Path) -> Result<(), String> {
    if options.help {
        println!("{}", options::HELP);
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    return run::execute(options, root);
    #[cfg(not(target_os = "macos"))]
    {
        let _ = root;
        Err("mac-ledger requires macOS".into())
    }
}
