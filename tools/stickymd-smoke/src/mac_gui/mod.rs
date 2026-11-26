//! External GUI acceptance only; no app control channel or qualification authority.
//! I01/I02 need configured real IMEs and composition assertions. I03 injects the key repeats
//! itself and checks them against a TextEdit control (`hold`). W05 and W06's Space switch need Spaces/full-screen orchestration. W10 reads the
//! presenter diagnostic and captures the window (`visual`). W12's logout is not driven: it ends
//! the VM's GUI session, and loginwindow ends this driver with it; W12 sends the product the quit request and the
//! SIGTERM a logout brings directly instead. R03 opens a quarantined copy through LaunchServices
//! with the user-approved quarantine bit preset (USER 2026-11-19; `translocation`), from the
//! release ZIP given with `--zip` (`staging`). W15 needs
//! Windows. These are never inferred PASS.
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
mod hold;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod hover;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod input;
pub(crate) mod latency;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod lifecycle;
#[cfg(target_os = "macos")]
mod live;
pub(crate) mod logout;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod model;
#[cfg(target_os = "macos")]
mod native;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod observation;
mod options;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod pixels;
#[cfg(target_os = "macos")]
mod runner;
#[cfg(test)]
mod scenario_tests;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod scenarios;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod snapshot;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod staging;
#[cfg(test)]
mod tests;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod translocation;
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
mod visual;
/// The rows `mac-gui` runs, which mac-ledger alone reads (on macOS and in tests).
#[cfg(any(target_os = "macos", test))]
pub(crate) use options::IDS;
pub(crate) use options::{Options, parse};
/// An F01 run failed before or after the product's spawn, as `mac-gui` writes it, for
/// mac-ledger's tests (gate review D2).
#[cfg(all(test, target_os = "macos"))]
pub(crate) use runner::failed_f01_evidence;
/// The names `mac-gui` may write into --out, for the shared --out rule's tests.
#[cfg(all(test, target_os = "macos"))]
pub(crate) use runner::written_here;
/// A run's R03 facts for mac-ledger's tests.
#[cfg(test)]
pub(crate) use translocation::facts::sample as r03_sample_facts;
/// R03's checks, which mac-ledger requires of an R03 row and recomputes from its facts.
#[cfg(any(target_os = "macos", test))]
pub(crate) use translocation::{
    CHECKS as R03_CHECKS, SNAPSHOT_ONLY_UNWRITTEN as R03_SNAPSHOT_ONLY_UNWRITTEN,
    UNWRITTEN as R03_UNWRITTEN, ZIP_CHECK as R03_ZIP_CHECK, facts::rejudge as r03_rejudge,
};
/// Chooses `label` in the product's menu-bar menu through Accessibility, as the GUI scenarios
/// do. `mac-resources` uses it to reach the hidden state. Returns how long the status item took
/// to appear in the AX tree.
#[cfg(target_os = "macos")]
pub(crate) fn choose_status_menu_item(
    pid: u32,
    label: &str,
) -> Result<std::time::Duration, String> {
    native::accessibility::menu(pid, label).map(|(waited, _)| waited)
}
/// Whether this binary may use Accessibility, by the same nonprompting preflight as `mac-gui`.
#[cfg(target_os = "macos")]
pub(crate) fn accessibility_trusted() -> bool {
    native::permissions::check().ax
}
/// The rows `mac-gui` executes; the rest are deferred by design and always NOT_TESTED.
#[cfg(any(target_os = "macos", test))]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub(crate) fn executable_ids() -> Vec<&'static str> {
    options::IDS
        .iter()
        .copied()
        .filter(|id| scenarios::skipped(id).is_none())
        .collect()
}
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
