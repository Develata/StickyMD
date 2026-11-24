//! The record of macOS GUI acceptance and its reuse rule (USER 2026-11-12, report
//! `2026-11-11-macos-gui-rerun-reuse`; anchored on product inputs 2026-11-17; acceptance ladder
//! 2026-11-19). Every row of a finished `mac-gui` run, every USER receipt and every USER
//! revocation is appended to one ledger. A later run skips a row whose latest PASS was recorded
//! for the same product inputs, driver, VM image and system, unless any FAIL, NOT_TESTED or
//! revoked PASS of the row is recorded under them; a USER receipt holds only for the product
//! inputs it names; the
//! never-reused rows count only results of the batch being planned. It plans and records; it
//! launches nothing.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
#[cfg(target_os = "macos")]
mod append;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) mod decide;
mod options;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) mod record;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) mod release;
#[cfg(target_os = "macos")]
mod run;
#[cfg(target_os = "macos")]
pub(crate) use run::read_whole;
#[cfg(test)]
mod tests;

pub(crate) use options::{Options, is_batch, parse};

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
