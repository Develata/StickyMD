//! The tests of the eight packages that are ignored on macOS, by package, target and full libtest
//! name as each target's own `--list --ignored` names them, with their role (plan 11
//! #macos-readiness-check: a new or moved ignored test is classified before it can pass). The gate
//! compares every test a receipt's listing marks ignored with this table, so an `#[ignore]` in any
//! form (plain, `cfg_attr` on one line or many, a doc-test's `ignore`) is caught by what libtest
//! reports, not by reading the source (review round 4, BLOCKER 3).
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

/// Why a test is ignored on macOS.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Role {
    /// A timing, profile or diagnostic run that no gate row relies on.
    NoGateRow,
    /// Run by name in the explicit run of this gate item.
    Explicit(&'static str),
}

/// `(package, target name, test, role)`.
pub(super) const IGNORED: [(&str, &str, &str, Role); 33] = [
    (
        "stickymd-app",
        "stickymd_app",
        "execution::preview_worker::tests::phase6_source_edit_during_math_build_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-app",
        "stickymd_app",
        "flow::window::reducer::phase8_window_tests::phase8_performance_window_reducer_and_geometry",
        Role::NoGateRow,
    ),
    (
        "stickymd-app",
        "stickymd_app",
        "source_search::tests::phase14_one_mib_unicode_case_insensitive_search_p95_is_bounded",
        Role::NoGateRow,
    ),
    (
        "stickymd-core",
        "stickymd_core",
        "assets::tests::phase7_managed_scan_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-desktop",
        "stickymd_desktop",
        "controls::tests::phase8_performance_control_layout_and_hit_test",
        Role::NoGateRow,
    ),
    (
        "stickymd-desktop",
        "stickymd_desktop",
        "shell::tests::phase3_source_pipeline_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-mac",
        "stickymd-mac",
        "assets::transaction::tests::image_paste_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-mac",
        "stickymd-mac",
        "export::tests::export_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-mac",
        "stickymd-mac",
        "note_root::tests::on_a_case_sensitive_volume_case_makes_another_key",
        Role::Explicit("same-key"),
    ),
    (
        "stickymd-mac",
        "stickymd-mac",
        "paths::tests::an_exfat_volume_is_refused_and_keeps_no_probe_file",
        Role::Explicit("P16-S06"),
    ),
    (
        "stickymd-posix",
        "stickymd_posix",
        "user::tests::print_home_directory",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "image::tests::phase7_image_decode_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "math::engine::tests::phase6_formula_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "math_action::tests::phase11b_performance_math_action_one_mib",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "math_action::tests::phase11b_performance_math_action_stays_linear_in_shifted_paragraphs",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::pipeline::audit_tests::phase5_preview_release_baseline_image_adapter_scroll",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::pipeline::audit_tests::phase5_preview_release_baseline_long_code_scroll",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::pipeline::horizontal_tests::phase5_preview_release_baseline_horizontal_long_line",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::pipeline::image_refresh_tests::phase7_image_band_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::pipeline::resource_tests::phase10_preview_resource_cache_retention_diagnostic",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::pipeline::resource_tests::phase10_preview_resource_glyph_growth_diagnostic",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::pipeline::resource_tests::phase6_preview_resource_formula_budget_diagnostic",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::pipeline::tests::phase10_zoom_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::pipeline::tests::phase5_preview_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::pipeline::tests::phase6_math_document_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::pipeline::text_reuse_tests::phase5_preview_release_baseline_text_reuse",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::semantic_conversion::tests::phase11b_performance_one_mib_with_one_thousand_math_nodes",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "preview::text_layout::tests::phase14_preview_selection_geometry_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "source::projection::startup_tests::source_initialization_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-render",
        "stickymd_render",
        "source::scrollbar::tests::scrollbar_release_baseline",
        Role::NoGateRow,
    ),
    (
        "stickymd-smoke",
        "stickymd-smoke",
        "qualification::module_ledger::fingerprint::tests::module_planning_profile",
        Role::NoGateRow,
    ),
    (
        "stickymd-smoke",
        "stickymd-smoke",
        "qualification::module_ledger::reuse_tests::lookup_all_profile",
        Role::NoGateRow,
    ),
    (
        "stickymd-smoke",
        "stickymd-smoke",
        "qualification::module_ledger::store::tests::link_scan_profile",
        Role::NoGateRow,
    ),
];

/// The role of an ignored test, or `None` when it is not classified.
pub(super) fn role(package: &str, target: &str, test: &str) -> Option<Role> {
    IGNORED
        .iter()
        .find(|(p, t, n, _)| *p == package && *t == target && *n == test)
        .map(|(_, _, _, role)| *role)
}
