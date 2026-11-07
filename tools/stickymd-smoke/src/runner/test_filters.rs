//! Name-filtered `cargo test` tasks against the sources they select. Cargo exits 0 when a filter
//! matches nothing, so a test that moves to another package would otherwise stop running while
//! its task stays green. Every filter must match a test in a selected package, and no product
//! package outside the selector may hold a matching test.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::filtered_run::selection;
use super::{Task, build_plan};
use crate::cli::Options;
use crate::headless::Module;

/// The name a line declares with `keyword`, as in `fn name(` or `pub mod name;`.
fn declared_name<'a>(line: &'a str, keyword: &str) -> Option<&'a str> {
    let line = line.strip_prefix("pub ").unwrap_or(line);
    line.strip_prefix(keyword)?
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .next()
}

/// Whether the function declared at `lines[index]` carries `#[test]`, looking up through the
/// attributes and comments directly above it.
fn is_test_function(lines: &[&str], index: usize) -> bool {
    lines[..index]
        .iter()
        .rev()
        .take_while(|line| line.starts_with("#[") || line.starts_with("//"))
        .any(|line| *line == "#[test]")
}

/// Whether the module declared at `lines[index]` of `file` holds a test: in its own body, or in
/// the file an out-of-line `mod name;` loads.
fn module_holds_test(file: &Path, lines: &[&str], index: usize, name: &str) -> bool {
    if !lines[index].ends_with(';') {
        return lines[index..].contains(&"#[test]");
    }
    let Some(directory) = file.parent() else {
        return false;
    };
    let stem = file.file_stem().unwrap_or_default();
    [
        directory.join(format!("{name}.rs")),
        directory.join(name).join("mod.rs"),
        directory.join(stem).join(format!("{name}.rs")),
    ]
    .iter()
    .any(|path| fs::read_to_string(path).is_ok_and(|text| text.contains("#[test]")))
}

/// Whether `text`, the source of `file`, declares a test whose path contains `filter`: a `#[test]`
/// function with it in its name, or a module with it in its name that holds a test.
fn declares_test(file: &Path, text: &str, filter: &str) -> bool {
    let lines = text.lines().map(str::trim).collect::<Vec<_>>();
    lines.iter().enumerate().any(|(index, line)| {
        if let Some(name) = declared_name(line, "fn ") {
            name.contains(filter) && is_test_function(&lines, index)
        } else if let Some(name) = declared_name(line, "mod ") {
            name.contains(filter) && module_holds_test(file, &lines, index, name)
        } else {
            false
        }
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
                && fs::read_to_string(&path).is_ok_and(|text| declares_test(&path, &text, filter))
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
}

fn declares(text: &str, filter: &str) -> bool {
    declares_test(Path::new("no-such-directory/lib.rs"), text, filter)
}

#[test]
fn only_a_test_function_or_a_module_holding_one_counts() {
    assert!(declares("#[test]\nfn phase8_one() {}", "phase8_"));
    assert!(declares(
        "#[test]\n#[ignore = \"Release\"]\n/// Why.\nfn phase8_one() {}",
        "phase8_"
    ));
    assert!(declares(
        "pub mod phase8_tests {\n    #[test]\n    fn one() {}\n}",
        "phase8_"
    ));
    assert!(!declares("fn phase8_helper() {}", "phase8_"));
    assert!(!declares(
        "#[test]\nfn other() {}\nfn phase8_helper() {}",
        "phase8_"
    ));
    assert!(!declares("mod phase8_tests {}", "phase8_"));
    assert!(!declares("mod phase8_tests;", "phase8_"));
    assert!(!declares("// fn phase8_one()", "phase8_"));
    assert!(!declares(
        "#[test]\nfn other() { phase8_one(); }",
        "phase8_"
    ));
}
