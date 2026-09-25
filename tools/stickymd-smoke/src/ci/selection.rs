//! Conservative input classification and reverse dependency closure.
//! plan_ref: docs/plan/11_testing_and_release.md#modular-headless-ci

use crate::headless::Module;
use std::collections::BTreeSet;

pub(super) struct Selection {
    pub(super) full: bool,
    pub(super) modules: Vec<Module>,
    pub(super) reasons: Vec<String>,
}

impl Selection {
    pub(super) fn full(reason: String) -> Self {
        Self {
            full: true,
            modules: Module::ALL.to_vec(),
            reasons: vec![reason],
        }
    }
}

pub(super) fn select(paths: &[String]) -> Selection {
    let mut direct = BTreeSet::new();
    for path in paths {
        if is_shared(path) {
            return Selection::full(format!("shared input: {path}"));
        }
        if let Some(module) = Module::ALL
            .into_iter()
            .find(|module| path.starts_with(module.root()))
        {
            direct.insert(module);
        } else if !is_documentation(path) {
            return Selection::full(format!("unknown input: {path}"));
        }
    }
    let mut modules = direct.clone();
    loop {
        let before = modules.len();
        for module in Module::ALL {
            if module
                .dependencies()
                .iter()
                .any(|dependency| modules.contains(dependency))
            {
                modules.insert(module);
            }
        }
        if modules.len() == before {
            break;
        }
    }
    let reasons = if direct.is_empty() {
        vec![
            "documentation-only or empty diff; shared governance checks remain required".to_owned(),
        ]
    } else {
        vec![format!(
            "direct modules: {}; selected modules include reverse dependencies",
            direct
                .iter()
                .map(|module| module.name())
                .collect::<Vec<_>>()
                .join(",")
        )]
    };
    Selection {
        full: false,
        modules: modules.into_iter().collect(),
        reasons,
    }
}

fn is_shared(path: &str) -> bool {
    // This fixture is also embedded by the Windows smoke/G5 harness, outside Cargo edges.
    if path == "crates/stickymd-render/tests/fixtures/rendering-stress.md" {
        return true;
    }
    let file = path.rsplit('/').next().unwrap_or(path);
    matches!(file, "Cargo.toml" | "Cargo.lock" | "AGENTS.md" | "build.rs")
        || matches!(
            path,
            "rust-toolchain.toml"
                | "rust-toolchain"
                | "deny.toml"
                | "clippy.toml"
                | "rustfmt.toml"
                | "docs/coverage-matrix.md"
        )
        || [
            ".cargo/",
            ".github/workflows/",
            ".github/actions/",
            "docs/plan/",
            "docs/features/",
            "docs/acceptance-cases/",
            "docs/overview/",
            "tools/release/",
            "tools/smoke/",
        ]
        .iter()
        .any(|prefix| path.starts_with(prefix))
        || [
            "ci.rs",
            "ci/",
            "headless.rs",
            "headless/",
            "runner.rs",
            "runner/",
            "cli.rs",
            "main.rs",
            "governance.rs",
            "governance/",
            "evidence.rs",
            "repository.rs",
            "pe_dependencies.rs",
        ]
        .iter()
        .any(|name| {
            path.strip_prefix("tools/stickymd-smoke/src/")
                .is_some_and(|relative| {
                    if name.ends_with('/') {
                        relative.starts_with(name)
                    } else {
                        relative == *name
                    }
                })
        })
}

fn is_documentation(path: &str) -> bool {
    matches!(
        path,
        "README.md"
            | "README.zh-CN.md"
            | "CHANGELOG.md"
            | "CONTRIBUTING.md"
            | "SECURITY.md"
            | "LICENSE"
            | ".github/pull_request_template.md"
    ) || (path.starts_with(".github/ISSUE_TEMPLATE/")
        && (path.ends_with(".yml") || path.ends_with(".md")))
        || (path.ends_with(".md")
            && [
                "docs/report/",
                "docs/tasks/",
                "docs/phases/",
                "docs/reference/",
                "docs/release-notes/",
            ]
            .iter()
            .any(|prefix| path.starts_with(prefix)))
}

#[cfg(test)]
mod tests;
