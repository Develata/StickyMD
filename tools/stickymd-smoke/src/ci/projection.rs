//! GitHub job inputs projected from one Rust-owned selection plan.
//! plan_ref: docs/plan/11_testing_and_release.md#modular-headless-ci

use super::{Checks, selection::Selection};
use crate::headless::Module;

fn quote(value: &str) -> String {
    format!("\"{}\"", crate::evidence::escape_json(value))
}

fn array(values: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    format!(
        "[{}]",
        values
            .into_iter()
            .map(|value| quote(value.as_ref()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(super) fn json(head: &str, base: Option<&str>, dirty: bool, selection: &Selection) -> String {
    let checks = Checks::for_modules(selection.full, &selection.modules);
    let windows = if selection.full {
        ["tests", "performance"]
            .map(|mode| format!("{{\"module\":\"all\",\"mode\":\"{mode}\",\"full\":true}}"))
            .join(",")
    } else {
        selection
            .modules
            .iter()
            .map(|module| {
                format!(
                    "{{\"module\":{},\"mode\":\"all\",\"full\":false}}",
                    quote(module.name())
                )
            })
            .collect::<Vec<_>>()
            .join(",")
    };
    let mut lint = vec!["clippy"];
    if selection.full {
        lint.push("--workspace");
    } else {
        for package in selection
            .modules
            .iter()
            .filter_map(|module| module.package())
        {
            lint.extend(["-p", package]);
        }
    }
    lint.extend(["--all-targets", "--locked", "--", "-D", "warnings"]);
    let mut portable = vec!["--locked"];
    for module in [Module::Core, Module::Render] {
        if selection.full || selection.modules.contains(&module) {
            portable.extend(["-p", module.cargo_name()]);
        }
    }
    format!(
        "{{\"schema_version\":1,\"kind\":\"headless-ci-plan\",\"status\":\"NOT_RUN\",\"head\":{},\"base\":{},\"worktree_dirty\":{dirty},\"full\":{},\"modules\":{},\"reasons\":{},\"windows_matrix\":{{\"include\":[{windows}]}},\"dependency_needed\":{},\"quality_needed\":{},\"headless_needed\":{},\"release_needed\":{},\"portable_needed\":{},\"lint_args\":{},\"portable_args\":{}}}",
        quote(head),
        base.map(quote).unwrap_or_else(|| "null".to_owned()),
        selection.full,
        array(selection.modules.iter().map(|module| module.name())),
        array(&selection.reasons),
        checks.dependency,
        checks.quality,
        checks.headless,
        checks.release,
        checks.portable,
        array(lint),
        array(portable)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ci_full_matrix_uses_legacy_shards_and_empty_selection_has_no_matrix_jobs() {
        let full = json(
            &"a".repeat(40),
            None,
            false,
            &Selection::full("missing baseline".to_owned()),
        );
        assert!(full.contains("\"module\":\"all\",\"mode\":\"tests\",\"full\":true"));
        assert!(full.contains("\"module\":\"all\",\"mode\":\"performance\",\"full\":true"));
        assert!(full.contains("\"lint_args\":[\"clippy\",\"--workspace\","));
        let empty = json(
            &"a".repeat(40),
            Some(&"b".repeat(40)),
            false,
            &super::super::selection::select(&[]),
        );
        assert!(empty.contains("\"windows_matrix\":{\"include\":[]}"));
        assert!(empty.contains("\"headless_needed\":false"));
        assert!(empty.contains("\"status\":\"NOT_RUN\""));
        assert!(!empty.contains("PASSED"));
    }
}
