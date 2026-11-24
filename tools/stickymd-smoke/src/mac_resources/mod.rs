//! macOS resource measurement command; no product or qualification authority. Its finished runs
//! in the VM are recorded in the mac-gui ledger for P16-R01 and P16-R02 (`rows`).
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

#[cfg(any(target_os = "macos", test))]
mod model;
#[cfg(target_os = "macos")]
mod native;
mod options;
#[cfg(any(target_os = "macos", test))]
mod report;
#[cfg(test)]
mod row_tests;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) mod rows;
#[cfg(target_os = "macos")]
mod runner;
#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use tests::evidence_json;

pub(crate) use options::{Options, parse};

pub(crate) fn execute(options: &Options) -> Result<(), String> {
    if options.help {
        println!("{}", options::HELP);
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    return runner::execute(options);
    #[cfg(not(target_os = "macos"))]
    Err("NOT_TESTED: mac-resources requires macOS; no measurements were made".to_owned())
}
