//! Name-filtered `cargo test` tasks. Cargo exits 0 when a filter matches nothing, so a test that
//! moves to another package would stop running while its task stays green. The unit-test owner
//! verification checks every task listed here against the real test lists.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use std::collections::BTreeSet;

use super::{Task, build_plan};
use crate::cli::{Options, Phase};
use crate::headless::Module;

/// A name-filtered `cargo test` task as Cargo will run it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct FilteredTask {
    pub(crate) label: &'static str,
    pub(crate) packages: BTreeSet<&'static str>,
    /// The product packages the task leaves out.
    pub(crate) unselected: BTreeSet<&'static str>,
    pub(crate) filter: &'static str,
    /// `--ignored`: only ignored tests run. Otherwise only the others do.
    pub(crate) ignored: bool,
    /// `--lib`: only the library target runs.
    pub(crate) lib_only: bool,
}

/// The packages a Cargo test command selects and its name filter, if it has one.
pub(super) fn selection(args: &[&'static str]) -> Option<(BTreeSet<&'static str>, &'static str)> {
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

fn runs_ignored(args: &[&str]) -> bool {
    args.iter()
        .skip_while(|arg| **arg != "--")
        .any(|arg| *arg == "--ignored")
}

/// The first argument the list check cannot model. A flag that changes which tests run must be
/// taught to the check, not skipped by it.
fn unmodelled<'a>(args: &[&'a str], filter: &str) -> Option<&'a str> {
    let mut args = args.iter().copied().skip(1);
    let mut harness = false;
    while let Some(arg) = args.next() {
        let known = if harness {
            matches!(arg, "--ignored" | "--nocapture" | "--test-threads=1")
        } else if arg == "--" {
            harness = true;
            true
        } else if arg == "-p" {
            args.next();
            true
        } else {
            arg == filter || matches!(arg, "--release" | "--locked" | "--lib" | "--workspace")
        };
        if !known {
            return Some(arg);
        }
    }
    None
}

/// A name a test command gives the harness itself, after `--`. `selection` sees no filter there,
/// so such a task would otherwise escape the check.
fn harness_filter<'a>(args: &[&'a str]) -> Option<&'a str> {
    if args.first() != Some(&"test") {
        return None;
    }
    args.iter()
        .copied()
        .skip_while(|arg| *arg != "--")
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
}

/// Every plan the CLI can build. A module run narrows these tasks to a package that may hold no
/// match, which is why the whole plans are checked and never the projected ones.
pub(super) fn plans() -> Result<Vec<Task>, String> {
    let mut tasks = Vec::new();
    for arguments in [vec!["all", "--ci"], vec!["all", "--performance"]] {
        let options = Options::parse(arguments.into_iter().map(str::to_owned))?;
        tasks.extend(build_plan(&options)?);
    }
    for phase in Phase::ALL {
        for extra in [None, Some("--performance")] {
            let arguments = ["phase", phase.number()]
                .into_iter()
                .chain(extra)
                .map(str::to_owned);
            tasks.extend(build_plan(&Options::parse(arguments)?)?);
        }
    }
    Ok(tasks)
}

/// Every distinct name-filtered test task of the whole plans.
pub(super) fn all() -> Result<Vec<FilteredTask>, String> {
    let products = Module::ALL
        .iter()
        .filter(|module| **module != Module::Smoke)
        .filter_map(|module| module.package())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    let mut tasks = Vec::new();
    for task in plans()? {
        let Task::Cargo { label, args, .. } = task else {
            continue;
        };
        let Some((packages, filter)) = selection(&args) else {
            if let Some(name) = harness_filter(&args) {
                return Err(format!(
                    "`{label}`: the filter check cannot model `{name}` after `--`"
                ));
            }
            continue;
        };
        if let Some(arg) = unmodelled(&args, filter) {
            return Err(format!("`{label}`: the filter check cannot model `{arg}`"));
        }
        let ignored = runs_ignored(&args);
        let lib_only = args.contains(&"--lib");
        if seen.insert((packages.clone(), filter, ignored, lib_only)) {
            let unselected = products.difference(&packages).copied().collect();
            tasks.push(FilteredTask {
                label,
                packages,
                unselected,
                filter,
                ignored,
                lib_only,
            });
        }
    }
    Ok(tasks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_filter_is_the_argument_that_is_no_flag_or_flag_value() {
        let args = [
            "test",
            "-p",
            "a",
            "--release",
            "--locked",
            "name",
            "--",
            "--ignored",
        ];
        assert_eq!(selection(&args), Some((BTreeSet::from(["a"]), "name")));
        assert!(runs_ignored(&args));
        assert_eq!(selection(&["test", "--workspace", "--locked"]), None);
        assert_eq!(selection(&["run", "-p", "a", "name"]), None);
        assert!(!runs_ignored(&["test", "--ignored-looking", "name"]));
    }

    #[test]
    fn an_argument_the_check_cannot_model_is_named() {
        let plain = ["test", "--release", "-p", "a", "name", "--", "--ignored"];
        assert_eq!(unmodelled(&plain, "name"), None);
        let target = ["test", "-p", "a", "--test", "t", "name"];
        assert_eq!(unmodelled(&target, "name"), Some("--test"));
        let exact = ["test", "-p", "a", "name", "--", "--exact"];
        assert_eq!(unmodelled(&exact, "name"), Some("--exact"));
        let tail = ["test", "-p", "a", "name", "--", "test"];
        assert_eq!(unmodelled(&tail, "name"), Some("test"));
        assert_eq!(
            harness_filter(&["test", "-p", "a", "--", "name", "--ignored"]),
            Some("name")
        );
        assert_eq!(
            harness_filter(&["test", "-p", "a", "--", "--ignored"]),
            None
        );
        assert_eq!(harness_filter(&["run", "--", "name"]), None);
    }

    #[test]
    fn the_whole_plans_hold_distinct_filtered_tasks() {
        let tasks = all().unwrap();
        assert!(!tasks.is_empty());
        let phase6 = tasks
            .iter()
            .find(|task| task.filter == "phase6_" && task.ignored)
            .unwrap();
        assert!(phase6.packages.contains("stickymd-render"));
        assert!(phase6.unselected.is_empty());
        let phase15 = tasks.iter().find(|task| task.filter == "phase15_").unwrap();
        assert!(phase15.unselected.contains("stickymd-app"));
    }
}
