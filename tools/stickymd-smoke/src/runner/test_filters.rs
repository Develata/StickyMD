//! Name-filtered `cargo test` tasks against the sources they select. Cargo exits 0 when a filter
//! matches nothing, so a test that moves to another package would otherwise stop running while
//! its task stays green. Every filter must match a test in a selected package, and no product
//! package outside the selector may hold a matching test.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::{Task, build_plan};
use crate::cli::Options;
use crate::headless::Module;

/// The packages a Cargo test command selects and its name filter, if it has one.
fn selection(args: &[&'static str]) -> Option<(BTreeSet<&'static str>, &'static str)> {
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

/// Whether a line declares a test function or a module whose name contains `filter`.
fn declares(line: &str, filter: &str) -> bool {
    let line = line.trim_start();
    let line = line.strip_prefix("pub ").unwrap_or(line);
    ["fn ", "mod "].iter().any(|keyword| {
        line.strip_prefix(keyword).is_some_and(|rest| {
            rest.split(|character: char| !character.is_alphanumeric() && character != '_')
                .next()
                .is_some_and(|name| name.contains(filter))
        })
    })
}

fn declared_in(directory: &Path, filter: &str) -> bool {
    let Ok(entries) = fs::read_dir(directory) else {
        return false;
    };
    entries.flatten().any(|entry| {
        let path = entry.path();
        if path.is_dir() {
            declared_in(&path, filter)
        } else {
            path.extension().is_some_and(|extension| extension == "rs")
                && fs::read_to_string(&path)
                    .is_ok_and(|text| text.lines().any(|line| declares(line, filter)))
        }
    })
}

fn tasks() -> Vec<Task> {
    let mut tasks = Vec::new();
    for arguments in [vec!["all", "--ci"], vec!["all", "--performance"]] {
        let options = Options::parse(arguments.into_iter().map(str::to_owned)).unwrap();
        tasks.extend(build_plan(&options).unwrap());
    }
    for phase in crate::cli::Phase::ALL {
        for extra in [None, Some("--performance")] {
            let arguments = ["phase", phase.number()]
                .into_iter()
                .chain(extra)
                .map(str::to_owned);
            tasks.extend(build_plan(&Options::parse(arguments).unwrap()).unwrap());
        }
    }
    tasks
}

#[test]
fn every_name_filter_selects_all_of_its_tests() {
    let repository =
        crate::governance::find_repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    let products = Module::ALL
        .into_iter()
        .filter(|module| *module != Module::Smoke)
        .filter_map(|module| module.package().map(|package| (package, module.root())))
        .collect::<Vec<_>>();
    let mut checked = BTreeSet::new();
    let mut errors = Vec::new();
    for task in tasks() {
        let Task::Cargo { label, args, .. } = task else {
            continue;
        };
        let Some((selected, filter)) = selection(&args) else {
            continue;
        };
        if !checked.insert((selected.clone(), filter)) {
            continue;
        }
        let holders = products
            .iter()
            .filter(|(_, root)| declared_in(&repository.join(root), filter))
            .map(|(package, _)| *package)
            .collect::<BTreeSet<_>>();
        if holders.is_disjoint(&selected) {
            errors.push(format!(
                "{label}: `{filter}` selects no test in {selected:?}"
            ));
        }
        let missed = holders.difference(&selected).collect::<Vec<_>>();
        if !missed.is_empty() {
            errors.push(format!(
                "{label}: `{filter}` selects {selected:?} but {missed:?} also hold matching tests"
            ));
        }
    }
    assert!(!checked.is_empty(), "no filtered test task was found");
    assert!(errors.is_empty(), "{errors:#?}");
}

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
    assert_eq!(selection(&["test", "--workspace", "--locked"]), None);
    assert_eq!(selection(&["run", "-p", "a", "name"]), None);
    assert!(declares("    fn phase8_one() {", "phase8_"));
    assert!(declares("pub mod phase8_window_tests {", "phase8_"));
    assert!(!declares("// fn phase8_one()", "phase8_"));
    assert!(!declares("    fn other() { phase8_one(); }", "phase8_"));
}
