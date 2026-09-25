use super::*;

fn selected(paths: &[&str]) -> Selection {
    select(
        &paths
            .iter()
            .map(|path| (*path).to_owned())
            .collect::<Vec<_>>(),
    )
}

#[test]
fn ci_reverse_dependencies_cover_downstream_behavior_without_selecting_unrelated_experiments() {
    assert_eq!(
        selected(&["crates/stickymd-core/src/undo.rs"]).modules,
        [Module::Core, Module::Render, Module::Windows]
    );
    assert_eq!(
        selected(&["crates/stickymd-render/tests/table_math_pipes.rs"]).modules,
        [Module::Render, Module::Windows]
    );
    assert_eq!(
        selected(&["apps/stickymd-win/src/app.rs"]).modules,
        [Module::Windows]
    );
    assert_eq!(
        selected(&["tools/stickymd-smoke/src/package_path.rs"]).modules,
        [Module::Smoke]
    );
    assert_eq!(
        selected(&["experiments/phase-01/persistence/src/main.rs"]).modules,
        [Module::Persistence]
    );
}

#[test]
fn ci_shared_and_unknown_inputs_choose_full_even_alongside_known_changes() {
    for path in [
        "Cargo.toml",
        "crates/stickymd-core/Cargo.toml",
        "experiments/phase-01/persistence/Cargo.lock",
        ".cargo/config.toml",
        "rust-toolchain.toml",
        "docs/plan/11_testing_and_release.md",
        "docs/acceptance-cases/phase-00.md",
        "docs/features/00_v1_product_behavior.md",
        "tools/stickymd-smoke/src/ci/git.rs",
        "tools/stickymd-smoke/src/runner/headless.rs",
        ".github/workflows/ci.yml",
        "tools/release/package.ps1",
        "new-module/src/lib.rs",
        "docs/unknown.md",
        "tools/stickymd-smoke-old/lib.rs",
        "apps/stickymd-win/build.rs",
        "crates/stickymd-render/tests/fixtures/rendering-stress.md",
    ] {
        assert!(
            selected(&["README.md", "crates/stickymd-core/src/undo.rs", path]).full,
            "{path}"
        );
    }
}

#[test]
fn ci_documentation_keeps_governance_only_and_never_masks_owned_fixtures() {
    for path in [
        "README.md",
        "CONTRIBUTING.md",
        "docs/report/audit.md",
        "docs/tasks/maintenance.md",
        ".github/ISSUE_TEMPLATE/bug-report.yml",
    ] {
        let selection = selected(&[path]);
        assert!(!selection.full && selection.modules.is_empty(), "{path}");
    }
    assert_eq!(
        selected(&["crates/stickymd-render/tests/fixtures/note.md"]).modules,
        [Module::Render, Module::Windows]
    );
    assert!(selected(&[]).modules.is_empty());
}

#[test]
fn ci_union_deduplicates_modules_and_preserves_both_owners_of_a_move() {
    let selection = selected(&[
        "crates/stickymd-render/old.rs",
        "experiments/phase-01/markdown-math/new.rs",
        "crates/stickymd-render/another.rs",
    ]);
    assert_eq!(
        selection.modules,
        [Module::Render, Module::Windows, Module::MarkdownMath]
    );
}
