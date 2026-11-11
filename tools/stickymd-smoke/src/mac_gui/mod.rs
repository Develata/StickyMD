//! External GUI acceptance only; no app control channel or qualification authority.
//! I01/I02 need configured real IMEs and composition assertions; I03 needs system repeat
//! settings. W05 and W06's Space switch need Spaces/full-screen orchestration; W10 needs
//! compositor/caret instrumentation; W12's logout needs a disposable session. R03 needs a
//! quarantined downloaded signed artifact; W15 needs Windows. These are never inferred PASS.
//! W11/W12 inject the save failure by taking write permission from the fixture's note directory
//! (USER 2026-11-11).
//! Off macOS the shared modules compile only for their unit tests; the live backend that reads
//! every field is macOS-only, hence the dead-code allowance there.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod editing;
#[cfg(target_os = "macos")]
mod fault;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod focus;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod hover;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod input;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod lifecycle;
#[cfg(target_os = "macos")]
mod live;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod model;
#[cfg(target_os = "macos")]
mod native;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod observation;
mod options;
#[cfg(target_os = "macos")]
mod runner;
#[cfg(test)]
mod scenario_tests;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod scenarios;
#[cfg(test)]
mod tests;
pub(crate) use options::{Options, parse};
pub(crate) fn execute(options: &Options) -> Result<(), String> {
    if options.help {
        println!("{}", options::HELP);
        return Ok(());
    }
    #[cfg(target_os = "macos")]
    return runner::execute(options);
    #[cfg(not(target_os = "macos"))]
    Err("mac-gui requires macOS; no application was launched".into())
}
