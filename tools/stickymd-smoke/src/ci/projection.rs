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
            .filter(|module| module.windows_lane())
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
            .filter(|module| module.windows_lane())
            .filter_map(|module| module.package())
        {
            lint.extend(["-p", package]);
        }
    }
    lint.extend(["--all-targets", "--locked", "--", "-D", "warnings"]);
    let mut portable = vec!["--locked"];
    for module in [Module::Core, Module::Render, Module::App, Module::Desktop] {
        if selection.full || selection.modules.contains(&module) {
            portable.extend(["-p", module.cargo_name()]);
        }
    }
    // The macOS job owns the macOS host and runs the shared crates it is built on, natively:
    // off macOS the host is a stub without tests (plan 11 Phase 16).
    let mut mac_test = vec!["test", "--locked"];
    for module in [
        Module::Core,
        Module::Render,
        Module::App,
        Module::Desktop,
        Module::Mac,
    ] {
        if selection.full || selection.modules.contains(&module) {
            mac_test.extend(["-p", module.cargo_name()]);
        }
    }
    let mac_lint = [
        "clippy",
        "-p",
        Module::Mac.cargo_name(),
        "--all-targets",
        "--locked",
        "--",
        "-D",
        "warnings",
    ];
    format!(
        "{{\"schema_version\":1,\"kind\":\"headless-ci-plan\",\"status\":\"NOT_RUN\",\"head\":{},\"base\":{},\"worktree_dirty\":{dirty},\"full\":{},\"modules\":{},\"reasons\":{},\"windows_matrix\":{{\"include\":[{windows}]}},\"smoke_needed\":{},\"dependency_needed\":{},\"quality_needed\":{},\"headless_needed\":{},\"release_needed\":{},\"portable_needed\":{},\"mac_needed\":{},\"lint_args\":{},\"portable_args\":{},\"mac_lint_args\":{},\"mac_test_args\":{}}}",
        quote(head),
        base.map(quote).unwrap_or_else(|| "null".to_owned()),
        selection.full,
        array(selection.modules.iter().map(|module| module.name())),
        array(&selection.reasons),
        checks.smoke,
        checks.dependency,
        checks.quality,
        checks.headless,
        checks.release,
        checks.portable,
        checks.mac,
        array(lint),
        array(portable),
        array(mac_lint),
        array(mac_test)
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
        assert!(full.contains(
            "\"portable_args\":[\"--locked\",\"-p\",\"stickymd-core\",\"-p\",\"stickymd-render\",\"-p\",\"stickymd-app\",\"-p\",\"stickymd-desktop\"]"
        ));
        assert!(full.contains("\"smoke_needed\":true"));
        let empty = json(
            &"a".repeat(40),
            Some(&"b".repeat(40)),
            false,
            &super::super::selection::select(&[]),
        );
        assert!(empty.contains("\"windows_matrix\":{\"include\":[]}"));
        assert!(empty.contains("\"headless_needed\":false"));
        assert!(empty.contains("\"mac_needed\":false"));
        assert!(empty.contains("\"smoke_needed\":false"));
        assert!(empty.contains("\"status\":\"NOT_RUN\""));
        assert!(!empty.contains("PASSED"));
    }

    /// A change in the Shared Desktop reaches Linux through the portable lane, alone: it is not
    /// a dependency of the crates below it.
    #[test]
    fn the_portable_lane_builds_the_shared_desktop_when_it_changes() {
        let plan = json(
            &"a".repeat(40),
            Some(&"b".repeat(40)),
            false,
            &super::super::selection::select(&["crates/stickymd-desktop/src/lib.rs".to_owned()]),
        );
        assert!(plan.contains("\"portable_needed\":true"));
        assert!(plan.contains("\"portable_args\":[\"--locked\",\"-p\",\"stickymd-desktop\"]"));
    }

    /// A change confined to the macOS host plans the macOS job and neither Windows lane (its
    /// stub there has no tests), and its Windows clippy cannot degrade into a selector-less run.
    #[test]
    fn a_macos_host_change_plans_the_macos_job_and_no_windows_lane() {
        let plan = json(
            &"a".repeat(40),
            Some(&"b".repeat(40)),
            false,
            &super::super::selection::select(&["apps/stickymd-mac/src/main.rs".to_owned()]),
        );
        for fragment in [
            "\"modules\":[\"mac\"]",
            "\"windows_matrix\":{\"include\":[]}",
            "\"quality_needed\":false",
            "\"headless_needed\":false",
            "\"release_needed\":false",
            "\"portable_needed\":false",
            "\"mac_needed\":true",
            "\"mac_lint_args\":[\"clippy\",\"-p\",\"stickymd-mac\",\"--all-targets\",\"--locked\",\"--\",\"-D\",\"warnings\"]",
            "\"mac_test_args\":[\"test\",\"--locked\",\"-p\",\"stickymd-mac\"]",
        ] {
            assert!(plan.contains(fragment), "{fragment}: {plan}");
        }
    }

    /// A change below the macOS host reaches it through the reverse dependency: the macOS job
    /// tests the changed shared crate and everything built on it, the Windows lanes keep their
    /// own packages, and the full plan names all five packages.
    #[test]
    fn a_shared_crate_change_reaches_the_macos_job_and_keeps_the_windows_lanes_without_the_host() {
        let plan = json(
            &"a".repeat(40),
            Some(&"b".repeat(40)),
            false,
            &super::super::selection::select(&["crates/stickymd-app/src/flow/save.rs".to_owned()]),
        );
        assert!(plan.contains("\"mac_needed\":true"), "{plan}");
        assert!(
            plan.contains("\"mac_test_args\":[\"test\",\"--locked\",\"-p\",\"stickymd-app\",\"-p\",\"stickymd-desktop\",\"-p\",\"stickymd-mac\"]"),
            "{plan}"
        );
        assert!(
            plan.contains("\"lint_args\":[\"clippy\",\"-p\",\"stickymd-app\",\"-p\",\"stickymd-desktop\",\"-p\",\"stickymd-win\",\"--all-targets\""),
            "{plan}"
        );
        assert!(!plan.contains("\"module\":\"mac\""), "{plan}");
        let full = json(
            &"a".repeat(40),
            None,
            false,
            &Selection::full("missing baseline".to_owned()),
        );
        assert!(full.contains("\"mac_needed\":true"));
        assert!(full.contains(
            "\"mac_test_args\":[\"test\",\"--locked\",\"-p\",\"stickymd-core\",\"-p\",\"stickymd-render\",\"-p\",\"stickymd-app\",\"-p\",\"stickymd-desktop\",\"-p\",\"stickymd-mac\"]"
        ));
    }

    #[test]
    fn linux_smoke_lane_is_selected_only_for_tooling_changes() {
        for (path, needed) in [
            ("tools/stickymd-smoke/src/package_path.rs", true),
            ("crates/stickymd-core/src/lib.rs", false),
            ("docs/report/ci-measurement.md", false),
        ] {
            let plan = json(
                &"a".repeat(40),
                Some(&"b".repeat(40)),
                false,
                &super::super::selection::select(&[path.to_owned()]),
            );
            assert!(
                plan.contains(&format!("\"smoke_needed\":{needed}")),
                "{plan}"
            );
        }
    }
}
