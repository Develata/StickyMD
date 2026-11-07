//! Name-filtered `cargo test` runs. Cargo exits 0 when a filter matches nothing, so a run that
//! passed no test fails here instead of staying green.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use std::collections::BTreeSet;
use std::process::Command;

use super::captured_failure_detail;
use crate::headless::Module;

/// The packages a Cargo test command selects and its name filter, if it has one.
pub(super) fn selection<'a>(args: &[&'a str]) -> Option<(BTreeSet<&'a str>, &'a str)> {
    let mut args = args.iter().copied();
    if args.next() != Some("test") {
        return None;
    }
    let mut packages = BTreeSet::new();
    let mut filter = None;
    while let Some(arg) = args.next() {
        match arg {
            "--" => break,
            "-p" | "--package" => {
                packages.extend(args.next());
            }
            "--test" | "--bench" | "--manifest-path" | "--features" => {
                args.next();
            }
            "--workspace" => {
                packages.extend(Module::ALL.iter().filter_map(|module| module.package()));
            }
            flag if flag.starts_with('-') => {}
            name => filter = Some(name),
        }
    }
    filter.map(|filter| (packages, filter))
}

/// The tests a `cargo test` run reports as passed, over all its test binaries.
pub(crate) fn passed_tests(stdout: &str) -> u64 {
    stdout
        .lines()
        .filter_map(|line| line.strip_prefix("test result: ok. "))
        .filter_map(|rest| rest.split(' ').next()?.parse::<u64>().ok())
        .sum()
}

/// Runs a name-filtered test command. Its output is captured to count the passed tests and goes
/// to stderr, so stdout stays machine-readable.
pub(super) fn run(mut command: Command, label: &str) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("cannot start `{label}`: {error}"))?;
    let detail = captured_failure_detail(&output.stdout, &output.stderr);
    if !output.status.success() {
        return Err(if detail.is_empty() {
            format!("`{label}` failed with {}", output.status)
        } else {
            format!("`{label}` failed with {}\n{detail}", output.status)
        });
    }
    if !detail.is_empty() {
        eprintln!("[{label}]\n{detail}");
    }
    if passed_tests(&String::from_utf8_lossy(&output.stdout)) == 0 {
        return Err(format!(
            "`{label}` ran no test: its name filter matched none"
        ));
    }
    Ok(())
}
