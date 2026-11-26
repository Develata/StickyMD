//! The tests of the eight packages that are ignored on macOS, by their whole target (package,
//! Cargo kinds and name) and full libtest name as each target's own `--list --ignored` names them,
//! with their role (plan 11 #macos-readiness-check: a new or moved ignored test is classified
//! before it can pass, a move to a target of another kind included). The gate compares every test
//! a receipt's listing marks ignored with this table, so an `#[ignore]` in any form (plain,
//! `cfg_attr` on one line or many, a doc-test's `ignore`) is caught by what libtest reports, not
//! by reading the source (review round 4, BLOCKER 3; fix round, BLOCKER 4).
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::Target;

/// Why a test is ignored on macOS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Role {
    /// A timing, profile or diagnostic run that no gate row relies on.
    NoGateRow,
    /// Run by name in the explicit run of this gate item.
    Explicit(&'static str),
}

/// `(package, kind, target name, test, role)`, the kind as the receipt joins Cargo's kinds.
pub(super) const IGNORED: [(&str, &str, &str, &str, Role); 33] = [
    (
        "stickymd-app",
        "lib",
        "stickymd_app",
        "execution::preview_worker::tests::phase6_source_edit_during_math_build_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-app",
        "lib",
        "stickymd_app",
        "flow::window::reducer::phase8_window_tests::phase8_performance_window_reducer_and_geometry",
        Role::NoGateRow,
    ),
    (
        "stickymd-app",
        "lib",
        "stickymd_app",
        "source_search::tests::phase14_one_mib_unicode_case_insensitive_search_p95_is_bounded",
        Role::NoGateRow,
    ),
    (
        "stickymd-core",
        "lib",
        "stickymd_core",
        "assets::tests::phase7_managed_scan_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-desktop",
        "lib",
        "stickymd_desktop",
        "controls::tests::phase8_performance_control_layout_and_hit_test",
        Role::NoGateRow,
    ),
    (
        "stickymd-desktop",
        "lib",
        "stickymd_desktop",
        "shell::tests::phase3_source_pipeline_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-mac",
        "bin",
        "stickymd-mac",
        "assets::transaction::tests::image_paste_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-mac",
        "bin",
        "stickymd-mac",
        "export::tests::export_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-mac",
        "bin",
        "stickymd-mac",
        "note_root::tests::on_a_case_sensitive_volume_case_makes_another_key",
        Role::Explicit("same-key"),
    ),
    (
        "stickymd-mac",
        "bin",
        "stickymd-mac",
        "paths::tests::an_exfat_volume_is_refused_and_keeps_no_probe_file",
        Role::Explicit("P16-S06"),
    ),
    (
        "stickymd-posix",
        "lib",
        "stickymd_posix",
        "user::tests::print_home_directory",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "image::tests::phase7_image_decode_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "math::engine::tests::phase6_formula_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "math_action::tests::phase11b_performance_math_action_one_mib",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "math_action::tests::phase11b_performance_math_action_stays_linear_after_reference_definitions",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::pipeline::audit_tests::phase5_preview_release_baseline_image_adapter_scroll",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::pipeline::audit_tests::phase5_preview_release_baseline_long_code_scroll",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::pipeline::horizontal_tests::phase5_preview_release_baseline_horizontal_long_line",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::pipeline::image_refresh_tests::phase7_image_band_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::pipeline::resource_tests::phase10_preview_resource_cache_retention_diagnostic",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::pipeline::resource_tests::phase10_preview_resource_glyph_growth_diagnostic",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::pipeline::resource_tests::phase6_preview_resource_formula_budget_diagnostic",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::pipeline::tests::phase10_zoom_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::pipeline::tests::phase5_preview_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::pipeline::tests::phase6_math_document_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::pipeline::text_reuse_tests::phase5_preview_release_baseline_text_reuse",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::semantic_conversion::tests::phase11b_performance_one_mib_with_one_thousand_math_nodes",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "preview::text_layout::tests::phase14_preview_selection_geometry_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "source::projection::startup_tests::source_initialization_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "lib",
        "stickymd_render",
        "source::scrollbar::tests::scrollbar_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-smoke",
        "bin",
        "stickymd-smoke",
        "qualification::module_ledger::fingerprint::tests::module_planning_profile",
        Role::NoGateRow,
    ),
    (
        "stickymd-smoke",
        "bin",
        "stickymd-smoke",
        "qualification::module_ledger::reuse_tests::lookup_all_profile",
        Role::NoGateRow,
    ),
    (
        "stickymd-smoke",
        "bin",
        "stickymd-smoke",
        "qualification::module_ledger::store::tests::link_scan_profile",
        Role::NoGateRow,
    ),
];

/// The role of an ignored test of `target`, or `None` when it is not classified.
pub(super) fn role(target: &Target, test: &str) -> Option<Role> {
    IGNORED
        .iter()
        .find(|(package, kind, name, n, _)| {
            *package == target.package && *kind == target.kind && *name == target.name && *n == test
        })
        .map(|(.., role)| *role)
}

/// How the targets' own `--list --ignored` listings differ from the table, by package, kind,
/// target name and full test name: an ignored test the table lacks, then an entry no listing
/// names, so a rename shows as both (plan 11 #macos-readiness-check, the Phase 18 check;
/// whole-gate review A1). Empty when they agree.
pub(super) fn differences(listed: &[super::output::Listed]) -> Vec<String> {
    type Key<'a> = (&'a str, &'a str, &'a str, &'a str);
    let mut found: Vec<Key> = listed
        .iter()
        .flat_map(|l| {
            l.ignored.iter().map(|test| {
                (
                    l.target.package.as_str(),
                    l.target.kind.as_str(),
                    l.target.name.as_str(),
                    test.as_str(),
                )
            })
        })
        .collect();
    found.sort_unstable();
    let mut table: Vec<Key> = IGNORED
        .iter()
        .map(|(package, kind, name, test, _)| (*package, *kind, *name, *test))
        .collect();
    table.sort_unstable();
    let named = |(package, kind, name, test): &Key| format!("{package} {kind} {name}: {test}");
    let extra = found
        .iter()
        .filter(|key| table.binary_search(key).is_err())
        .map(|key| format!("ignored but not classified: {}", named(key)));
    let missing = table
        .iter()
        .filter(|key| found.binary_search(key).is_err())
        .map(|key| format!("classified but not ignored: {}", named(key)));
    extra.chain(missing).collect()
}
