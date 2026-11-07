//! Name-filtered `cargo test` tasks against the sources, on every platform. It reads the source of
//! every product package, so it also sees tests another platform compiles. It only approximates
//! Rust's syntax; the Windows owner verification checks the real test lists.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-15-shared-core-migration

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use super::Task;
use super::filtered_tasks::{plans, selection};
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
/// the file an out-of-line `mod name;` loads. A non-empty inline body is searched to the end of
/// the file, which is one of this check's approximations.
fn module_holds_test(file: &Path, lines: &[&str], index: usize, name: &str) -> bool {
    if lines[index].ends_with("{}") {
        return false;
    }
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
    for task in plans().unwrap() {
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
