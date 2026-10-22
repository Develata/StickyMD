//! Repository-contract validation used by every phase smoke.

mod actions;
mod input_injection;
mod manifest_harness;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::{Phase, Selection};

const REQUIRED_FILES: &[&str] = &[
    "AGENTS.md",
    "Cargo.toml",
    "Cargo.lock",
    "LICENSE",
    "THIRD_PARTY_NOTICES.md",
    "README.md",
    "README.zh-CN.md",
    "CHANGELOG.md",
    "SECURITY.md",
    "CONTRIBUTING.md",
    ".github/workflows/ci.yml",
    ".github/workflows/release.yml",
    ".github/workflows/promote-release.yml",
    ".github/workflows/scheduled.yml",
    "docs/AGENTS.md",
    "docs/coverage-matrix.md",
    "docs/features/00_v1_product_behavior.md",
    "docs/acceptance-cases/00_v1_acceptance.md",
    "docs/plan/00_engineering_constitution.md",
    "docs/plan/01_terminology.md",
    "docs/plan/02_positioning_and_scope.md",
    "docs/plan/03_system_architecture.md",
    "docs/plan/04_runtime_state_model.md",
    "docs/plan/05_document_persistence.md",
    "docs/plan/06_markdown_math_rendering.md",
    "docs/plan/07_editor_and_ime.md",
    "docs/plan/08_assets_and_export.md",
    "docs/plan/09_windows_shell.md",
    "docs/plan/10_performance_reliability.md",
    "docs/plan/11_testing_and_release.md",
    "tools/stickymd-smoke/Cargo.toml",
    "assets/licenses/SIL-OFL-1.1.txt",
    "assets/licenses/KaTeX-fonts-NOTICE.txt",
    "assets/licenses/Boost-1.0.txt",
    "assets/licenses/HarfRust-MIT.txt",
    "assets/licenses/RaTeX-MIT.txt",
    "docs/release-checklist.md",
    "docs/report/phase-09-performance-final.md",
    "docs/report/phase-09-release-readiness.md",
    "docs/phases/2026-08-22-phase-10-user-approved-ux-corrections.md",
    "docs/tasks/phase-10-ux-corrections-rc-requalification.md",
    "docs/acceptance-cases/phase-10.md",
    "docs/phases/2026-08-22-phase-11-rc-convergence.md",
    "docs/tasks/phase-11-rc-convergence.md",
    "docs/acceptance-cases/phase-11.md",
    "docs/phases/2026-08-22-phase-11-b-final-interaction-amendment.md",
    "docs/phases/2026-08-23-phase-12-final-release-qualification.md",
    "docs/tasks/phase-11-b-final-interaction-amendment.md",
    "docs/acceptance-cases/phase-11-b.md",
    "docs/tasks/phase-12-final-release-qualification.md",
    "docs/acceptance-cases/phase-12.md",
    "docs/report/phase-12-release-decisions.md",
    "docs/report/phase-12-final-qualification.md",
    "docs/report/phase-12-release-handoff.md",
    "docs/phases/2026-08-23-phase-13-exact-candidate-qualification.md",
    "docs/tasks/phase-13-exact-candidate-qualification.md",
    "docs/acceptance-cases/phase-13.md",
    "docs/report/phase-13-qualification-plan.md",
    "docs/report/phase-13-final-qualification.md",
    "docs/phases/2026-08-23-phase-14-release-policy-calibration.md",
    "docs/tasks/phase-14-release-gate-calibration.md",
    "docs/acceptance-cases/phase-14.md",
    "docs/report/phase-14-release-policy.md",
    "docs/report/phase-14-startup-attribution-plan.md",
    "docs/report/phase-14-final-qualification.md",
    "docs/report/phase-14-memory-attribution.md",
    "docs/report/phase-14-preview-selection-geometry-design.md",
    "docs/report/phase-14-real-ime-automation-design.md",
    "docs/reference/qualification-execution-model.md",
    "tools/manual/phase-14-guide.md",
    "docs/release-notes/0.1.0-draft.md",
    "tools/release/package.ps1",
    "tools/release/generate-third-party-notices.ps1",
    "tools/release/generate-sbom.ps1",
    "tools/release/verify-package.ps1",
    "tools/release/verify-promoted-artifact.ps1",
    "tools/release/invoke-smoke.ps1",
    "tools/release/archive-facts.ps1",
    "tools/release/resource-facts.ps1",
    "tools/stickymd-smoke/tests/release_wrappers.rs",
];

const FORBIDDEN_PACKAGES: &[&str] = &[
    "async-std",
    "cef",
    "curl",
    "egui",
    "gtk",
    "html5ever",
    "hyper",
    "iced",
    "onig",
    "onig_sys",
    "reqwest",
    "ratex-cairo",
    "ratex-ffi",
    "ratex-gtk4",
    "ratex-pdf",
    "ratex-render",
    "ratex-svg",
    "ratex-wasm",
    "rusqlite",
    "slint",
    "syntect",
    "tauri",
    "tokio",
    "ureq",
    "wgpu",
    "webview2",
    "wry",
];

#[derive(Debug)]
struct MatrixRow {
    line: usize,
    mode: String,
    status: String,
}

pub(crate) fn find_repository_root(start: &Path) -> Result<PathBuf, String> {
    for candidate in start.ancestors() {
        if candidate.join("Cargo.toml").is_file() && candidate.join("AGENTS.md").is_file() {
            return Ok(candidate.to_path_buf());
        }
    }
    Err(format!(
        "cannot locate repository root above {}",
        start.display()
    ))
}

pub(crate) fn verify(root: &Path) -> Result<(), String> {
    verify_required_files(root)?;
    verify_global_acceptance_sequence(root)?;
    verify_phase_artifacts(root)?;
    verify_phase8_frozen_trace(root)?;
    verify_phase8_shell_artifacts(root)?;
    verify_phase9_frozen_trace(root)?;
    verify_phase10_contract_trace(root)?;
    verify_phase11_contract_trace(root)?;
    verify_phase11b_contract_trace(root)?;
    verify_phase12_contract_trace(root)?;
    verify_phase13_contract_trace(root)?;
    verify_phase14_contract_trace(root)?;
    verify_release_infrastructure(root)?;
    verify_plan_refs(root)?;
    crate::qualification::verify_product_classification(root)?;
    verify_local_markdown_links(root)?;
    verify_forbidden_packages(root)?;
    verify_test_harnesses(root)?;
    input_injection::verify(root)?;
    Ok(())
}

/// Unit-test owners rely on libtest's default harness (plan 11): a `harness` key may only
/// configure a `[[bench]]` target of the workspace or its registered manifests
/// (`manifest_harness` reads them with TOML's lexical rules).
fn verify_test_harnesses(root: &Path) -> Result<(), String> {
    let manifests = std::iter::once("Cargo.toml".to_owned()).chain(
        crate::headless::Module::ALL
            .iter()
            .map(|module| format!("{}Cargo.toml", module.root())),
    );
    for relative in manifests {
        let path = root.join(&relative);
        if let Some(line) = manifest_harness::harness_outside_benches(&read_text(&path)?) {
            return Err(format!(
                "{relative}:{line} sets `harness` outside a [[bench]] target; unit-test owners need libtest's default harness"
            ));
        }
    }
    Ok(())
}

fn verify_phase14_contract_trace(root: &Path) -> Result<(), String> {
    let path = root.join("docs/acceptance-cases/phase-14.md");
    let content = read_text(&path)?;
    let observed = frozen_trace_ids(&content, &path, "P14-A")?;
    let expected: Vec<u16> = (1..=83).collect();
    if observed != expected {
        return Err(format!(
            "{} IDs must be exactly P14-A01..P14-A83; observed {observed:?}",
            path.display()
        ));
    }
    for guided in ["P14-G1", "P14-G2", "P14-G3", "P14-G4"] {
        let marker = format!("| {guided} |");
        if content.match_indices(&marker).count() != 1 {
            return Err(format!(
                "{} must contain {guided} exactly once",
                path.display()
            ));
        }
    }
    for row in read_matrix_rows(&path)? {
        let line = content.lines().nth(row.line - 1).unwrap_or_default();
        if (line.trim_start().starts_with("| P14-G1") || line.trim_start().starts_with("| P14-G2"))
            && (row.mode != "Guided Manual" || row.status != "NOT TESTED")
        {
            return Err(format!(
                "{}:{} Phase 14 guided row must remain Guided Manual / NOT TESTED",
                path.display(),
                row.line
            ));
        }
        if (line.trim_start().starts_with("| P14-G3")
            || line.trim_start().starts_with("| P14-G4")
            || line.trim_start().starts_with("| P14-G5"))
            && (row.mode != "Automated exact candidate" || row.status != "NOT TESTED")
        {
            return Err(format!(
                "{}:{} Phase 14 G3/G4/G5 row must remain Automated exact candidate / NOT TESTED until an exact receipt exists",
                path.display(),
                row.line
            ));
        }
    }
    Ok(())
}

fn verify_phase13_contract_trace(root: &Path) -> Result<(), String> {
    let path = root.join("docs/acceptance-cases/phase-13.md");
    let content = read_text(&path)?;
    for (prefix, last) in [("P13-A", 18_u16), ("P13-M", 5_u16)] {
        let observed = frozen_trace_ids(&content, &path, prefix)?;
        let expected: Vec<u16> = (1..=last).collect();
        if observed != expected {
            return Err(format!(
                "{} IDs must be exactly {prefix}01..{prefix}{last:02}; observed {observed:?}",
                path.display()
            ));
        }
    }
    for row in read_matrix_rows(&path)? {
        let line = content.lines().nth(row.line - 1).unwrap_or_default();
        if line.trim_start().starts_with("| P13-M")
            && (row.mode != "Manual" || row.status != "NOT TESTED")
        {
            return Err(format!(
                "{}:{} Phase 13 manual session must remain Manual / NOT TESTED; exact observations belong in dist/evidence",
                path.display(),
                row.line
            ));
        }
    }
    Ok(())
}

fn verify_phase12_contract_trace(root: &Path) -> Result<(), String> {
    let path = root.join("docs/acceptance-cases/phase-12.md");
    let content = read_text(&path)?;
    for (prefix, last) in [("P12-A", 18_u16), ("P12-M", 47_u16)] {
        let mut observed = frozen_trace_ids(&content, &path, prefix)?;
        // Manual rows are grouped by tier, so a later ID can sit in an earlier tier section;
        // sorting keeps the check exact (a duplicate or a missing ID still differs).
        if prefix == "P12-M" {
            observed.sort_unstable();
        }
        let expected: Vec<u16> = (1..=last).collect();
        if observed != expected {
            return Err(format!(
                "{} IDs must be exactly {prefix}01..{prefix}{last:02}; observed {observed:?}",
                path.display()
            ));
        }
    }
    for row in read_matrix_rows(&path)? {
        let line = content.lines().nth(row.line - 1).unwrap_or_default();
        if !line.trim_start().starts_with("| P12-M") {
            continue;
        }
        let exact = line
            .trim_start()
            .split('|')
            .nth(1)
            .map(str::trim)
            .and_then(crate::qualification::exact_groups::group_for_phase12_case)
            .is_some();
        let valid = if exact {
            row.mode == "Automated exact candidate" && row.status == "NOT TESTED"
        } else {
            row.mode == "Manual" && row.status == "NOT TESTED"
        };
        if !valid {
            return Err(format!(
                "{}:{} Phase 12 row mode/status does not match its manual or exact G3/G4/G5 authority",
                path.display(),
                row.line
            ));
        }
    }
    Ok(())
}

fn verify_phase11b_contract_trace(root: &Path) -> Result<(), String> {
    const LAST_ACCEPTANCE: u16 = 6;
    const LAST_DOD: u16 = 46;
    const LAST_MANUAL: u16 = 5;
    let path = root.join("docs/acceptance-cases/phase-11-b.md");
    let content = read_text(&path)?;
    for (prefix, last) in [
        ("P11B-A", LAST_ACCEPTANCE),
        ("P11B-D", LAST_DOD),
        ("P11B-M", LAST_MANUAL),
    ] {
        let observed = frozen_trace_ids(&content, &path, prefix)?;
        let expected: Vec<u16> = (1..=last).collect();
        if observed != expected {
            return Err(format!(
                "{} IDs must be exactly {prefix}01..{prefix}{last:02}; observed {observed:?}",
                path.display()
            ));
        }
    }
    for row in read_matrix_rows(&path)? {
        let line = content.lines().nth(row.line - 1).unwrap_or_default();
        if line.trim_start().starts_with("| P11B-M")
            && (row.mode != "Manual" || row.status != "NOT TESTED")
        {
            return Err(format!(
                "{}:{} Phase 11-B real-environment row must remain Manual / NOT TESTED until a receipt is checked in",
                path.display(),
                row.line
            ));
        }
    }
    Ok(())
}

fn verify_phase11_contract_trace(root: &Path) -> Result<(), String> {
    const LAST_DOD: u16 = 85;
    const MANUAL_DOD: &[u16] = &[
        28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 39, 40, 41, 42, 43, 44, 45, 46, 48, 49, 50,
    ];
    let path = root.join("docs/acceptance-cases/phase-11.md");
    let content = read_text(&path)?;
    let observed = frozen_trace_ids(&content, &path, "P11-D")?;
    let expected: Vec<u16> = (1..=LAST_DOD).collect();
    if observed != expected {
        return Err(format!(
            "{} Phase 11 DoD IDs must be exactly P11-D001..P11-D{LAST_DOD:03}; observed {observed:?}",
            path.display()
        ));
    }
    for row in read_matrix_rows(&path)? {
        let line = content.lines().nth(row.line - 1).unwrap_or_default();
        let Some(id) = line
            .trim_start()
            .strip_prefix("| P11-D")
            .and_then(|tail| tail.split_whitespace().next())
            .and_then(|digits| digits.parse::<u16>().ok())
        else {
            continue;
        };
        if MANUAL_DOD.contains(&id) && (row.mode != "Manual" || row.status != "NOT TESTED") {
            return Err(format!(
                "{}:{} Phase 11 real-environment row P11-D{id:03} must remain Manual / NOT TESTED until a receipt is checked in",
                path.display(),
                row.line
            ));
        }
    }
    Ok(())
}

fn verify_phase10_contract_trace(root: &Path) -> Result<(), String> {
    let path = root.join("docs/acceptance-cases/phase-10.md");
    let content = read_text(&path)?;
    let automated = frozen_trace_ids(&content, &path, "P10-A")?;
    let expected_automated: Vec<u16> = (1..=36).collect();
    if automated != expected_automated {
        return Err(format!(
            "{} automated IDs must be exactly P10-A01..P10-A36; observed {automated:?}",
            path.display()
        ));
    }
    let manual = frozen_trace_ids(&content, &path, "UX10-")?;
    let expected_manual: Vec<u16> = (1..=23).collect();
    if manual != expected_manual {
        return Err(format!(
            "{} manual IDs must be exactly UX10-01..UX10-23; observed {manual:?}",
            path.display()
        ));
    }
    for row in read_matrix_rows(&path)? {
        let line = content.lines().nth(row.line - 1).unwrap_or_default();
        if line.trim_start().starts_with("| UX10-")
            && (row.mode != "Manual" || row.status != "NOT TESTED")
        {
            return Err(format!(
                "{}:{} Phase 10 real-environment row must remain Manual / NOT TESTED until a receipt is checked in",
                path.display(),
                row.line
            ));
        }
    }
    Ok(())
}

fn verify_phase9_frozen_trace(root: &Path) -> Result<(), String> {
    const LAST_ROW: u16 = 125;
    const FIRST_MANUAL_ROW: u16 = 12;
    const LAST_MANUAL_ROW: u16 = 41;
    for relative in [
        "docs/phases/2026-08-21-phase-09-pre-release-convergence.md",
        "docs/tasks/phase-09-pre-release-convergence.md",
        "docs/report/phase-09-inherited-conditions.md",
        "docs/report/phase-09-release-blockers.md",
        "docs/report/phase-09-supply-chain.md",
        "docs/report/phase-09-release-workflow.md",
        "docs/report/phase-09-portable-package.md",
    ] {
        if !root.join(relative).is_file() {
            return Err(format!("required Phase 9 artifact is missing: {relative}"));
        }
    }

    let path = root.join("docs/acceptance-cases/phase-09.md");
    let content = read_text(&path)?;
    let observed = frozen_trace_ids(&content, &path, "P09-D")?;
    let expected: Vec<u16> = (1..=LAST_ROW).collect();
    if observed != expected {
        return Err(format!(
            "{} frozen DoD IDs must be exactly P09-D001..P09-D{LAST_ROW:03}; observed {observed:?}",
            path.display()
        ));
    }
    let rows = read_matrix_rows(&path)?;
    let frozen_rows = rows
        .iter()
        .filter(|row| {
            content
                .lines()
                .nth(row.line - 1)
                .is_some_and(|line| line.trim_start().starts_with("| P09-D"))
        })
        .collect::<Vec<_>>();
    if frozen_rows.len() != LAST_ROW as usize {
        return Err(format!(
            "{} does not expose all frozen Phase 9 rows to matrix validation",
            path.display()
        ));
    }
    for (index, row) in frozen_rows.iter().enumerate() {
        let id = index as u16 + 1;
        if (FIRST_MANUAL_ROW..=LAST_MANUAL_ROW).contains(&id)
            && (row.mode != "Manual" || row.status != "NOT TESTED")
        {
            return Err(format!(
                "{}:{} Phase 9 real-environment row P09-D{id:03} must remain Manual / NOT TESTED until a receipt is checked in",
                path.display(),
                row.line
            ));
        }
    }
    Ok(())
}

fn verify_release_infrastructure(root: &Path) -> Result<(), String> {
    let mut workflows: Vec<_> = [
        ".github/workflows/ci.yml",
        ".github/workflows/release.yml",
        ".github/workflows/promote-release.yml",
        ".github/workflows/scheduled.yml",
    ]
    .into_iter()
    .map(|relative| root.join(relative))
    .collect();
    for extension in ["yml", "yaml"] {
        collect_files(&root.join(".github/actions"), extension, &mut workflows)?;
    }
    for path in workflows {
        let content = read_text(&path)?;
        actions::verify_uses(root, &path, &content)?;
        for forbidden in [
            "pull_request_target",
            "packages: write",
            "actions: write",
            "security-events: write",
            "curl | sh",
            "curl|sh",
        ] {
            if content.contains(forbidden) {
                return Err(format!(
                    "{} contains forbidden release-workflow token `{forbidden}`",
                    path.display()
                ));
            }
        }
    }

    let release = read_text(&root.join(".github/workflows/release.yml"))?;
    for required in [
        "permissions:\n  contents: read",
        "workflow_dispatch:",
        "persist-credentials: false",
        "tools/release/package.ps1 -ExactCandidate",
        "tools/release/generate-sbom.ps1",
        "tools/release/verify-package.ps1",
        "qualification native-runtime --exe=target/release/stickymd-win.exe",
        "name: stickymd-windows-x64-release",
    ] {
        if !release.contains(required) {
            return Err(format!(
                "release workflow lacks required security token `{required}`"
            ));
        }
    }
    for forbidden in ["push:\n    tags:", "gh release", "actions/attest@"] {
        if release.contains(forbidden) {
            return Err(format!(
                "candidate workflow must not tag, attest, or create releases: `{forbidden}`"
            ));
        }
    }
    let promotion = read_text(&root.join(".github/workflows/promote-release.yml"))?;
    for required in [
        "operation:",
        "- tag",
        "- draft",
        "- publish",
        "source_sha:",
        "artifact_run_id:",
        "expected_zip_sha256:",
        "expected_sbom_sha256:",
        "run-id: ${{ inputs.artifact_run_id }}",
        "tools/release/verify-promoted-artifact.ps1",
        "subject-checksums: dist/promoted/SHA256SUMS.txt",
        "sbom-path: dist/promoted/SBOM.spdx.json",
        "gh release create",
        "gh release edit",
    ] {
        if !promotion.contains(required) {
            return Err(format!(
                "promotion workflow lacks exact-artifact token `{required}`"
            ));
        }
    }
    for forbidden in [
        "cargo build",
        "tools/release/package.ps1",
        "push:\n    tags:",
    ] {
        if promotion.contains(forbidden) {
            return Err(format!(
                "promotion workflow must reuse rather than rebuild exact artifact: `{forbidden}`"
            ));
        }
    }
    let ci = read_text(&root.join(".github/workflows/ci.yml"))?;
    if !ci.contains("qualification native-runtime --exe=target/release/stickymd-win.exe") {
        return Err("CI release build lacks the portable native-runtime gate".to_owned());
    }
    let cargo_config = read_text(&root.join(".cargo/config.toml"))?;
    for required in [
        "[target.x86_64-pc-windows-msvc]",
        "target-feature=+crt-static",
    ] {
        if !cargo_config.contains(required) {
            return Err(format!(
                "Windows target config lacks portable runtime token `{required}`"
            ));
        }
    }
    let package = read_text(&root.join("tools/release/package.ps1"))?;
    if package.contains("cargo build") {
        return Err("package.ps1 must not build the application".to_owned());
    }
    // Release semantics are exercised against compiled Rust by release::* tests and
    // tests/release_wrappers.rs (including actual packaged license bytes).
    // tests/release_workflow.rs executes the promotion step with the compiled CLI,
    // covering its remote query and fail-closed identity behavior instead of a URL token.
    // tests/release_remote.rs also executes actual tag/draft step bodies with observed
    // HTTP/GraphQL facts, verifying that rejection prevents downstream remote writes.
    // Phase 12-14 routing is covered by phase_entry tests against the compiled parser.
    // PowerShell source tokens are not evidence of a gate.
    let remote_promotion =
        read_text(&root.join("tools/stickymd-smoke/src/qualification/remote.rs"))?;
    for required in [
        "download_recorded_artifact",
        "Command::new(\"gh\")",
        ".args([\"run\", \"download\"",
        "verify_supplied_copy",
    ] {
        if !remote_promotion.contains(required) {
            return Err(format!(
                "remote promotion lacks authoritative-download token `{required}`"
            ));
        }
    }
    // Syft's fixed identity, cache hashes, and checksum manifest are owned by
    // release::syft. Its unit tests and the dual-host release wrapper tests
    // exercise corrupt downloads, retries, and preservation of the existing cache.
    Ok(())
}

fn verify_phase8_shell_artifacts(root: &Path) -> Result<(), String> {
    for relative in [
        "apps/stickymd-win/StickyMD.manifest",
        "apps/stickymd-win/build.rs",
        "docs/phases/2026-08-21-phase-08-windows-desktop-shell.md",
        "docs/tasks/phase-08-windows-desktop-shell.md",
        "docs/report/phase-08-windows-desktop-shell.md",
    ] {
        if !root.join(relative).is_file() {
            return Err(format!("required Phase 8 artifact is missing: {relative}"));
        }
    }

    let manifest_path = root.join("apps/stickymd-win/StickyMD.manifest");
    let manifest = read_text(&manifest_path)?;
    for marker in ["PerMonitorV2", "level=\"asInvoker\"", "uiAccess=\"false\""] {
        if !manifest.contains(marker) {
            return Err(format!(
                "{} must contain `{marker}`",
                manifest_path.display()
            ));
        }
    }

    let build_path = root.join("apps/stickymd-win/build.rs");
    let build = read_text(&build_path)?;
    for marker in [
        "set_manifest_file",
        "set_icon",
        "ProductName",
        ">PerMonitorV2<",
    ] {
        if !build.contains(marker) {
            return Err(format!(
                "{} must enforce embedded PerMonitorV2 manifest marker `{marker}`",
                build_path.display()
            ));
        }
    }

    for relative in [
        "crates/stickymd-core/src/lib.rs",
        "crates/stickymd-render/src/lib.rs",
    ] {
        let path = root.join(relative);
        if !read_text(&path)?.contains("#![forbid(unsafe_code)]") {
            return Err(format!(
                "{} must retain #![forbid(unsafe_code)]",
                path.display()
            ));
        }
    }

    let workspace_manifest = read_text(&root.join("Cargo.toml"))?;
    if workspace_manifest.contains("Win32_System_Registry") {
        return Err("Phase 8 must not introduce registry or auto-start capability".to_owned());
    }

    let lifecycle_path = root.join("apps/stickymd-win/src/app/lifecycle.rs");
    let lifecycle = read_text(&lifecycle_path)?;
    for marker in [
        ".with_decorations(false)",
        ".with_undecorated_shadow(true)",
        "CornerPreference::RoundSmall",
        "ControlFlow::Wait",
    ] {
        if !lifecycle.contains(marker) {
            return Err(format!(
                "{} is missing native shell marker `{marker}`",
                lifecycle_path.display()
            ));
        }
    }

    let window_projection_path = root.join("apps/stickymd-win/src/app/window_geometry_runtime.rs");
    let window_projection = read_text(&window_projection_path)?;
    if window_projection
        .matches("enumerate_active_displays()")
        .count()
        != 1
    {
        return Err(format!(
            "{} must enumerate CCD facts exactly once per monitor snapshot",
            window_projection_path.display()
        ));
    }
    for forbidden in ["DocumentState", "std::fs", "File::"] {
        if window_projection.contains(forbidden) {
            return Err(format!(
                "{} crosses the window projection boundary through `{forbidden}`",
                window_projection_path.display()
            ));
        }
    }

    let monitor_path = root.join("apps/stickymd-win/src/platform/windows/monitor.rs");
    if !read_text(&monitor_path)?.contains("info.rcWork") {
        return Err(format!(
            "{} must project the taskbar-excluded rcWork rectangle",
            monitor_path.display()
        ));
    }

    let tray_path = root.join("apps/stickymd-win/src/platform/windows/tray.rs");
    let tray = read_text(&tray_path)?;
    for marker in ["select_biased!", "recv(menu_events)", "recv(tray_events)"] {
        if !tray.contains(marker) {
            return Err(format!(
                "{} must retain event-driven tray marker `{marker}`",
                tray_path.display()
            ));
        }
    }
    for forbidden in ["thread::sleep", "recv_timeout", "try_recv"] {
        if tray.contains(forbidden) {
            return Err(format!(
                "{} must not poll tray events through `{forbidden}`",
                tray_path.display()
            ));
        }
    }

    let tray_uia_path = root.join("tools/stickymd-smoke/helpers/windows-uia.ps1");
    let tray_uia = read_text(&tray_uia_path)?;
    for marker in [
        "IsOffscreen",
        "Wait-ForProcessMenuItems",
        "Wait-ForProcessMenuClosed",
        "observed_items=",
        "UIA_TRAY_MENU_ITEMS_BEFORE=",
        "UIA_TRAY_MENU_UNOBSERVABLE",
    ] {
        if !tray_uia.contains(marker) {
            return Err(format!(
                "{} must retain acknowledged tray-menu interaction marker `{marker}`",
                tray_uia_path.display()
            ));
        }
    }
    // The cursor placement and right-click that open the menu, its one retry and the Esc that
    // closes it are injected by the Rust caller.
    let tray_driver_path =
        root.join("tools/stickymd-smoke/src/qualification/exact_desktop/tray.rs");
    let tray_driver = read_text(&tray_driver_path)?;
    for marker in [
        "pub(super) const TRAY_MENU_OPEN_ATTEMPTS: usize = 2;",
        "UIA_TRAY_MENU_OPEN",
        "escape_tray_popup",
    ] {
        if !tray_driver.contains(marker) {
            return Err(format!(
                "{} must retain acknowledged tray-menu interaction marker `{marker}`",
                tray_driver_path.display()
            ));
        }
    }

    let tray_candidates_path =
        root.join("tools/stickymd-smoke/src/qualification/exact_desktop/tray/multiple.rs");
    let tray_candidates = read_text(&tray_candidates_path)?;
    for marker in [
        "right_click_tray_icon",
        "TRAY_MENU_OPEN_ATTEMPTS",
        "active_tray_popup_owner",
    ] {
        if !tray_candidates.contains(marker) {
            return Err(format!(
                "{} must retain acknowledged tray candidate marker `{marker}`",
                tray_candidates_path.display()
            ));
        }
    }

    let flow_directory = root.join("apps/stickymd-win/src/flow/window");
    let mut flow_rust = Vec::new();
    collect_files(&flow_directory, "rs", &mut flow_rust)?;
    for path in flow_rust {
        let content = read_text(&path)?;
        for forbidden in ["winit::", "windows::", "HWND", "HMONITOR"] {
            if content.contains(forbidden) {
                return Err(format!(
                    "{} leaks platform type `{forbidden}` into the pure window domain",
                    path.display()
                ));
            }
        }
    }

    let mut production_rust = Vec::new();
    collect_files(&root.join("apps"), "rs", &mut production_rust)?;
    collect_files(&root.join("crates"), "rs", &mut production_rust)?;
    for path in production_rust {
        let content = read_text(&path)?;
        let lowercase = content.to_ascii_lowercase();
        for forbidden in ["acrylic", "mica"] {
            if lowercase
                .split(|character: char| !character.is_ascii_alphanumeric() && character != '_')
                .any(|token| token == forbidden)
            {
                return Err(format!(
                    "{} introduces forbidden Phase 8 visual `{forbidden}`",
                    path.display()
                ));
            }
        }
    }

    let mut windows_rust = Vec::new();
    collect_files(
        &root.join("apps/stickymd-win/src/platform/windows"),
        "rs",
        &mut windows_rust,
    )?;
    for path in windows_rust {
        let content = read_text(&path)?;
        let lines = content.lines().collect::<Vec<_>>();
        for (index, line) in lines.iter().enumerate() {
            if !line.contains("unsafe {") {
                continue;
            }
            let context_start = index.saturating_sub(6);
            if !lines[context_start..=index]
                .iter()
                .any(|candidate| candidate.contains("SAFETY:"))
            {
                return Err(format!(
                    "{}:{} unsafe block has no adjacent SAFETY invariant",
                    path.display(),
                    index + 1
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn verify_ready_status(root: &Path, selection: Selection) -> Result<(), String> {
    let phases: Vec<_> = match selection {
        Selection::Phase(phase) => vec![phase],
        Selection::All => Phase::ALL.to_vec(),
    };
    for phase in phases {
        let path = matrix_path(root, phase);
        for row in read_matrix_rows(&path)? {
            if row.mode == "Automated" && row.status != "AUTOMATED PASS" {
                return Err(format!(
                    "{}:{} automated row is `{}` instead of `AUTOMATED PASS`",
                    path.display(),
                    row.line,
                    row.status
                ));
            }
        }
    }
    Ok(())
}

fn verify_required_files(root: &Path) -> Result<(), String> {
    for relative in REQUIRED_FILES {
        if !root.join(relative).is_file() {
            return Err(format!("required file is missing: {relative}"));
        }
    }
    Ok(())
}

fn verify_global_acceptance_sequence(root: &Path) -> Result<(), String> {
    let path = root.join("docs/acceptance-cases/00_v1_acceptance.md");
    let content = read_text(&path)?;
    let mut observed = Vec::new();
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("## AC-") {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let number = digits
                .parse::<u8>()
                .map_err(|error| format!("invalid acceptance heading `{line}`: {error}"))?;
            observed.push(number);
        }
    }
    let expected: Vec<u8> = (1..=38).collect();
    if observed != expected {
        return Err(format!(
            "global acceptance IDs must be exactly AC-001..AC-038; observed {observed:?}"
        ));
    }
    Ok(())
}

fn verify_phase_artifacts(root: &Path) -> Result<(), String> {
    for phase in Phase::ALL {
        let matrix = matrix_path(root, phase);
        if !matrix.is_file() {
            return Err(format!("phase matrix is missing: {}", matrix.display()));
        }
        let rows = read_matrix_rows(&matrix)?;
        if rows.is_empty() || !rows.iter().any(|row| row.mode == "Automated") {
            return Err(format!(
                "{} must contain at least one automated row",
                matrix.display()
            ));
        }

        let script = root.join(format!("tools/smoke/phase-{}.ps1", phase.number()));
        // Existence belongs here. Routing and failure semantics are exercised by
        // phase_entry tests against every real wrapper on both PowerShell hosts.
        read_text(&script)?;
    }
    let all_script = root.join("tools/smoke/all.ps1");
    read_text(&all_script)?;
    Ok(())
}

fn verify_phase8_frozen_trace(root: &Path) -> Result<(), String> {
    const LAST_IMPLEMENTATION_ROW: u16 = 116;
    const LAST_MANUAL_ROW: u16 = 139;
    let path = root.join("docs/acceptance-cases/phase-08.md");
    let content = read_text(&path)?;
    let observed = frozen_trace_ids(&content, &path, "P08-D")?;
    let expected: Vec<u16> = (1..=LAST_MANUAL_ROW).collect();
    if observed != expected {
        return Err(format!(
            "{} frozen DoD IDs must be exactly P08-D001..P08-D{LAST_MANUAL_ROW:03}; observed {observed:?}",
            path.display()
        ));
    }
    let rows = read_matrix_rows(&path)?;
    let frozen_rows = rows
        .iter()
        .filter(|row| row.line > 0)
        .skip_while(|row| {
            content
                .lines()
                .nth(row.line - 1)
                .is_none_or(|line| !line.trim_start().starts_with("| P08-D"))
        })
        .take(LAST_MANUAL_ROW as usize)
        .collect::<Vec<_>>();
    if frozen_rows.len() != LAST_MANUAL_ROW as usize {
        return Err(format!(
            "{} does not expose all frozen Phase 8 rows to matrix validation",
            path.display()
        ));
    }
    for (index, row) in frozen_rows.iter().enumerate() {
        let id = index as u16 + 1;
        if id > LAST_IMPLEMENTATION_ROW && (row.mode != "Manual" || row.status != "NOT TESTED") {
            return Err(format!(
                "{}:{} frozen real-environment row P08-D{id:03} must remain Manual / NOT TESTED until a receipt is checked in",
                path.display(),
                row.line
            ));
        }
    }
    Ok(())
}

fn matrix_path(root: &Path, phase: Phase) -> PathBuf {
    root.join(format!("docs/acceptance-cases/phase-{}.md", phase.number()))
}

/// Whether `cell` is a bare row ID such as `P08-D001`, `P11B-A06`, `UX10-01` or `REL-CLI-01`:
/// hyphen-separated segments of uppercase letters and digits, starting with a letter, whose
/// last segment is optional uppercase letters followed by digits.
fn is_trace_id(cell: &str) -> bool {
    let segments = cell.split('-').collect::<Vec<_>>();
    let Some((last, leading)) = segments.split_last() else {
        return false;
    };
    let digits = last.trim_start_matches(|c: char| c.is_ascii_uppercase());
    !leading.is_empty()
        && cell.starts_with(|c: char| c.is_ascii_uppercase())
        && leading.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
        })
        && !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
}

/// Where a line sits relative to the table blocks of a phase matrix document.
#[derive(Clone, Copy, PartialEq)]
enum MatrixTable {
    Outside,
    /// After a `| ID | … |` header of this many columns, before its delimiter row.
    Header(usize),
    /// The data rows of a matrix table with this many columns.
    Data(usize),
    /// Any other table.
    Other,
}

/// One row whose first cell is a row ID, in the canonical form.
struct TraceRow<'a> {
    line: usize,
    id: &'a str,
    cells: Vec<&'a str>,
    /// Whether the row is a data row of a matrix table (an `ID` header).
    in_matrix: bool,
}

/// The cells of a table line: one optional outer pipe on each side, split on the rest.
fn table_cells(line: &str) -> Vec<&str> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);
    line.split('|').map(str::trim).collect()
}

/// `cell` without HTML tags and the inline markers `*`, `_` and `` ` ``, to recognise a row ID
/// or an `ID` header however it is decorated.
fn undecorated(cell: &str) -> String {
    let mut text = String::with_capacity(cell.len());
    let mut in_tag = false;
    for c in cell.chars() {
        match c {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            '*' | '_' | '`' => {}
            _ if !in_tag => text.push(c),
            _ => {}
        }
    }
    text.trim().to_owned()
}

/// Whether `line` is a table delimiter row such as `| --- | :---: |` with `width` columns.
fn is_delimiter(line: &str, width: usize) -> bool {
    let cells = table_cells(line);
    cells.len() == width
        && cells
            .iter()
            .all(|cell| cell.contains('-') && cell.bytes().all(|b| b == b'-' || b == b':'))
}

/// The ID rows of `content` under the restricted matrix syntax every governance check reads.
///
/// A table starts at a line beginning with `|` (or a header whose first cell reads `ID`) and
/// must end at a blank line: GFM would take a following text line as one more row. A matrix
/// table is `| ID | … |`, then a delimiter row of the same width, then data rows of that width,
/// each exactly `| <row ID> | …` — no indentation, one space after the pipe, a bare ID, no
/// escaped pipes — and no second header. Anywhere else a line whose first cell names a row ID,
/// bare or decorated with HTML tags, `*`, `_` or backticks, must use the same form; other
/// inline syntax (links, entities) is not recognised. Frozen IDs, mode/status rows and the registry
/// comparison all read these rows. The syntax is deliberately narrow; documents whose rows
/// must be complete (phase-08, plan 09 and phase-12 for the manual mapping) are also bound by
/// digest, so this reader is not their only guard.
fn trace_rows<'a>(content: &'a str, path: &Path) -> Result<Vec<TraceRow<'a>>, String> {
    let mut rows = Vec::new();
    let mut table = MatrixTable::Outside;
    let mut last = 0;
    for (index, line) in content.lines().enumerate() {
        last = index + 1;
        let refuse = |problem: &str| Err(format!("{}:{} {problem}", path.display(), index + 1));
        let cells = table_cells(line);
        let bare = undecorated(cells.first().copied().unwrap_or(""));
        let header = line.contains('|') && bare == "ID";
        if !line.trim_start().starts_with('|') && !header {
            match table {
                MatrixTable::Header(_) => return refuse("a matrix header has no delimiter row"),
                MatrixTable::Outside => {}
                _ if !line.trim().is_empty() => {
                    return refuse("a table must end with a blank line");
                }
                _ => {}
            }
            table = MatrixTable::Outside;
            if line.contains('|') && is_trace_id(&bare) {
                return refuse("a matrix row must start with `| `");
            }
            continue;
        }
        let id = line
            .strip_prefix("| ")
            .and_then(|rest| rest.split_once(" |"))
            .map(|(id, _)| id)
            .filter(|id| is_trace_id(id));
        table = match table {
            MatrixTable::Outside if header => {
                if !line.starts_with("| ID |") {
                    return refuse("a matrix header must start with `| ID |`");
                }
                MatrixTable::Header(cells.len())
            }
            MatrixTable::Outside => MatrixTable::Other,
            MatrixTable::Header(width) if is_delimiter(line, width) => MatrixTable::Data(width),
            MatrixTable::Header(_) => {
                return refuse("a matrix header must be followed by a delimiter of its width");
            }
            MatrixTable::Data(_) if id.is_none() => {
                return refuse("a matrix data row must be `| <row ID> | …` with a bare row ID");
            }
            MatrixTable::Data(width) if cells.len() != width => {
                return refuse("a matrix data row must have its header's width");
            }
            state => state,
        };
        if id.is_none() && is_trace_id(&bare) {
            return refuse("a matrix row must be `| <row ID> | …` with a bare row ID");
        }
        let Some(id) = id else {
            continue;
        };
        if line.contains("\\|") {
            return refuse("escaped pipes are not supported in matrix rows");
        }
        rows.push(TraceRow {
            line: index + 1,
            id,
            cells,
            in_matrix: matches!(table, MatrixTable::Data(_)),
        });
    }
    if matches!(table, MatrixTable::Header(_)) {
        return Err(format!(
            "{}:{last} a matrix header has no delimiter row",
            path.display()
        ));
    }
    Ok(rows)
}

/// The frozen IDs with `prefix` (for example `P08-D`), as numbers in document order.
fn frozen_trace_ids(content: &str, path: &Path, prefix: &str) -> Result<Vec<u16>, String> {
    let mut ids = Vec::new();
    for row in trace_rows(content, path)? {
        let Some(number) = row.id.strip_prefix(prefix) else {
            continue;
        };
        if number.is_empty() || !number.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        ids.push(number.parse::<u16>().map_err(|error| {
            format!("{}:{} invalid frozen ID: {error}", path.display(), row.line)
        })?);
    }
    Ok(ids)
}

fn read_matrix_rows(path: &Path) -> Result<Vec<MatrixRow>, String> {
    matrix_rows(&read_text(path)?, path)
}

/// The rows of `content` with their mode, status and evidence checked: every data row of a
/// matrix table, and any other `P…` row.
fn matrix_rows(content: &str, path: &Path) -> Result<Vec<MatrixRow>, String> {
    let mut rows = Vec::new();
    for row in trace_rows(content, path)? {
        if !row.in_matrix && !row.id.starts_with('P') {
            continue;
        }
        let at = |problem: String| format!("{}:{} {problem}", path.display(), row.line);
        let &[_, _, mode, evidence, status] = row.cells.as_slice() else {
            return Err(at("phase matrix row must contain 5 columns".to_owned()));
        };
        match mode {
            "Automated" | "Automated local" if matches!(status, "AUTOMATED PASS" | "BLOCKED") => {}
            "Manual" if matches!(status, "MANUAL PASS" | "NOT TESTED" | "BLOCKED") => {}
            "Guided Manual" if matches!(status, "NOT TESTED" | "BLOCKED") => {}
            "Automated exact candidate" if matches!(status, "NOT TESTED" | "BLOCKED") => {}
            _ => {
                return Err(at(format!(
                    "invalid mode/status pair `{mode}` / `{status}`"
                )));
            }
        }
        if evidence.is_empty() {
            return Err(at("evidence cell must not be empty".to_owned()));
        }
        if status == "MANUAL PASS" && !evidence.contains("receipt:") {
            return Err(at(
                "MANUAL PASS requires a checked-in `receipt:` reference".to_owned()
            ));
        }
        rows.push(MatrixRow {
            line: row.line,
            mode: mode.to_owned(),
            status: status.to_owned(),
        });
    }
    Ok(rows)
}

/// The `(id, action, expected)` of every Manual row with `prefix`, read as governance reads
/// matrix rows, for the registry comparison.
#[cfg(test)]
pub(crate) fn manual_rows(
    content: &str,
    path: &Path,
    prefix: &str,
) -> Result<Vec<(String, String, String)>, String> {
    Ok(trace_rows(content, path)?
        .into_iter()
        .filter(|row| row.id.starts_with(prefix) && row.cells.get(2) == Some(&"Manual"))
        .map(|row| {
            (
                row.id.to_owned(),
                row.cells[1].to_owned(),
                row.cells.get(3).copied().unwrap_or_default().to_owned(),
            )
        })
        .collect())
}

fn verify_plan_refs(root: &Path) -> Result<(), String> {
    let mut rust_files = Vec::new();
    collect_files(&root.join("apps"), "rs", &mut rust_files)?;
    collect_files(&root.join("crates"), "rs", &mut rust_files)?;
    let mut count = 0usize;
    for rust_file in rust_files {
        let content = read_text(&rust_file)?;
        for (index, line) in content.lines().enumerate() {
            let Some(reference) = line.trim().strip_prefix("//! plan_ref: ") else {
                continue;
            };
            count += 1;
            let (relative, anchor) = reference.split_once('#').ok_or_else(|| {
                format!(
                    "{}:{} plan_ref has no stable anchor",
                    rust_file.display(),
                    index + 1
                )
            })?;
            if !relative.starts_with("docs/plan/") {
                return Err(format!(
                    "{}:{} plan_ref is outside docs/plan",
                    rust_file.display(),
                    index + 1
                ));
            }
            let plan_path = root.join(relative);
            let plan = read_text(&plan_path)?;
            let marker = format!("<a id=\"{anchor}\"></a>");
            if !plan.contains(&marker) {
                return Err(format!(
                    "{}:{} missing stable anchor `{marker}` in {}",
                    rust_file.display(),
                    index + 1,
                    plan_path.display()
                ));
            }
        }
    }
    if count < 30 {
        return Err(format!(
            "expected at least 30 production plan_ref declarations, found {count}"
        ));
    }
    Ok(())
}

fn verify_local_markdown_links(root: &Path) -> Result<(), String> {
    let mut files = vec![root.join("README.md"), root.join("AGENTS.md")];
    collect_files(&root.join("docs"), "md", &mut files)?;
    for file in files {
        if file.starts_with(root.join("docs/phases")) {
            continue;
        }
        let content = read_text(&file)?;
        for (line, destination) in markdown_links(&content) {
            if is_external_or_anchor(&destination) {
                continue;
            }
            let without_fragment = destination
                .split_once('#')
                .map_or(destination.as_str(), |(path, _)| path)
                .trim();
            if without_fragment.is_empty() {
                continue;
            }
            let target = file
                .parent()
                .ok_or_else(|| format!("{} has no parent", file.display()))?
                .join(without_fragment.replace('/', std::path::MAIN_SEPARATOR_STR));
            if !target.exists() {
                return Err(format!(
                    "{}:{line} local Markdown link does not resolve: {destination}",
                    file.display()
                ));
            }
        }
    }
    Ok(())
}

fn markdown_links(content: &str) -> Vec<(usize, String)> {
    let mut links = Vec::new();
    let mut offset = 0usize;
    while let Some(relative_close) = content[offset..].find("](") {
        let close = offset + relative_close;
        let Some(relative_end) = content[close + 2..].find(')') else {
            break;
        };
        let end = close + 2 + relative_end;
        let open = content[..close].rfind('[');
        let is_image = open.is_some_and(|open| open > 0 && content.as_bytes()[open - 1] == b'!');
        if !is_image {
            let raw = content[close + 2..end].trim();
            let destination = raw.trim_matches(['<', '>']).to_owned();
            let line = content[..close]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1;
            links.push((line, destination));
        }
        offset = end + 1;
    }
    links
}

fn is_external_or_anchor(destination: &str) -> bool {
    let lower = destination.to_ascii_lowercase();
    destination.starts_with('#')
        || destination.starts_with('/')
        || destination.get(1..3) == Some(":/")
        || lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("file:")
}

fn verify_forbidden_packages(root: &Path) -> Result<(), String> {
    let lock = read_text(&root.join("Cargo.lock"))?;
    let forbidden: BTreeSet<_> = FORBIDDEN_PACKAGES.iter().copied().collect();
    for line in lock.lines() {
        let trimmed = line.trim();
        let Some(name) = trimmed
            .strip_prefix("name = \"")
            .and_then(|value| value.strip_suffix('"'))
        else {
            continue;
        };
        if forbidden.contains(name) {
            return Err(format!("forbidden package appears in Cargo.lock: {name}"));
        }
    }
    Ok(())
}

fn collect_files(
    directory: &Path,
    extension: &str,
    output: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| format!("cannot read directory entry: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?;
        if file_type.is_dir() {
            if entry.file_name() != "target" {
                collect_files(&entry.path(), extension, output)?;
            }
        } else if file_type.is_file()
            && entry
                .path()
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case(extension))
        {
            output.push(entry.path());
        }
    }
    Ok(())
}

fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::{frozen_trace_ids, is_trace_id, manual_rows, markdown_links, matrix_rows};

    #[test]
    fn markdown_link_parser_ignores_images() {
        let links = markdown_links("[plan](../plan/a.md#x) ![image](missing.png)");
        assert_eq!(links, vec![(1, "../plan/a.md#x".to_owned())]);
    }

    #[test]
    fn markdown_link_parser_reports_line_numbers() {
        let links = markdown_links("first\n[second](b.md)\n");
        assert_eq!(links, vec![(2, "b.md".to_owned())]);
    }

    #[test]
    fn frozen_trace_parser_keeps_order_and_ignores_other_rows() {
        let source = concat!(
            "| P08-A00 | route | Automated | evidence | AUTOMATED PASS |\n",
            "| P08-D001 | first | Automated | evidence | BLOCKED |\n",
            "| P08-D002 | second | Manual | evidence | NOT TESTED |\n",
        );
        assert_eq!(
            frozen_trace_ids(source, std::path::Path::new("phase-08.md"), "P08-D").unwrap(),
            vec![1, 2]
        );
    }

    /// Every way of writing a row that the ID, mode and registry checks would not all see is
    /// refused, so those checks never disagree about which rows exist.
    #[test]
    fn matrix_rows_refuse_rows_the_checks_would_not_all_see() {
        let path = std::path::Path::new("phase-12.md");
        let header =
            "| ID | Requirement | Mode | Evidence | Status |\n| --- | --- | --- | --- | --- |";
        let row = "| P12-M47 | act | Manual | expect | NOT TESTED |";
        let table = format!("{header}\n{row}");
        assert_eq!(matrix_rows(&table, path).unwrap().len(), 1);
        assert_eq!(frozen_trace_ids(&table, path, "P12-M").unwrap(), [47]);
        for bad in [
            "P12-M48 | act | Manual | expect | NOT TESTED",
            "|P12-M48 | act | Manual | expect | NOT TESTED |",
            "|  P12-M48 | act | Manual | expect | NOT TESTED |",
            "| \tP12-M48 | act | Manual | expect | NOT TESTED |",
            "  | P12-M48 | act | Manual | expect | NOT TESTED |",
            "| **P12-M48** | act | Manual | expect | NOT TESTED |",
            "| `P12-M48` | act | Manual | expect | NOT TESTED |",
            "**P12-M48** | act | Manual | expect | NOT TESTED",
            "| <b>P12-M48</b> | act | Manual | expect | NOT TESTED |",
            // A second header cannot restart the table, nor a text line continue it.
            "| ID |\n| **P12-M48** | act | Manual | expect | NOT TESTED |",
            "continuation\n| **P12-M48** | act | Manual | expect | NOT TESTED |",
            r"| P12-M48 | a \| b | Manual | expect | NOT TESTED |",
        ] {
            let document = format!("{table}\n{bad}");
            assert!(matrix_rows(&document, path).is_err(), "{bad}");
            assert!(frozen_trace_ids(&document, path, "P12-M").is_err(), "{bad}");
            assert!(manual_rows(&document, path, "P12-M").is_err(), "{bad}");
        }
        // A header must be canonical, decorated or not, and followed by a delimiter of its width
        // before any row, blank line or the end; data rows keep the header's width.
        for bad in [
            "ID | Requirement | Mode | Evidence | Status\n| --- | --- | --- | --- | --- |",
            "| ID | Requirement | Mode | Evidence | Status |\n| P12-M47 | act | Manual | expect | NOT TESTED |",
            "| <b>ID</b> | Requirement | Mode | Evidence | Status |\n| --- | --- | --- | --- | --- |",
            "| ID | Requirement | Mode | Evidence | Status |\n| --- |\n| P12-M47 | act | Manual | expect | NOT TESTED |",
            "| ID |\n| --- |\n| P12-M47 | act | Manual | expect | NOT TESTED |",
            "| ID | Requirement | Mode | Evidence | Status |\n\nprose",
            "| ID | Requirement | Mode | Evidence | Status |",
        ] {
            assert!(matrix_rows(bad, path).is_err(), "{bad}");
        }
        assert!(
            matrix_rows(
                "| ID | Requirement | Mode | Evidence | Status |\n| :--- | :---: | --- | --- | ---: |\n| P12-M47 | act | Manual | expect | NOT TESTED |",
                path
            )
            .is_ok()
        );
        // A decorated row ID in any other table is refused too.
        assert!(
            matrix_rows(
                "| Scope | entry |\n| --- | --- |\n| <b>P12-M48</b> | act |",
                path
            )
            .is_err()
        );
        // Every matrix row has its mode, status and evidence checked, whatever its ID family.
        let ux = "| ID | Requirement | Mode | Evidence | Status |\n| --- | --- | --- | --- | --- |";
        assert!(
            matrix_rows(
                &format!("{ux}\n| UX10-01 | copy | Manual | receipt | NOT TESTED |"),
                path
            )
            .is_ok()
        );
        for bad in [
            "| UX10-01 | copy | Manual |  | NOT TESTED |",
            "| UX10-01 | copy | Manual | receipt | MANUAL PASS |",
            "| REL-CLI-01 | identity | Automated somewhere | evidence | AUTOMATED PASS |",
        ] {
            assert!(matrix_rows(&format!("{ux}\n{bad}"), path).is_err(), "{bad}");
        }
        // Outside a matrix table a row ID still needs the canonical form.
        assert!(
            matrix_rows(
                "  P11B-A07 | act | Automated | evidence | AUTOMATED PASS |",
                path
            )
            .is_err()
        );
        // A blank line ends the table; other tables keep free first cells.
        assert!(
            matrix_rows(
                &format!("{table}\n\n| Scope | entry |\n| --- | --- |\n| any | x |"),
                path
            )
            .is_ok()
        );
        // Prose and other tables are not trace rows.
        assert!(matrix_rows("Rows | columns | P12 notes", path).is_ok());
        for id in ["P08-D001", "P11B-A06", "UX10-01", "REL-CLI-01"] {
            assert!(is_trace_id(id), "{id}");
        }
        for not_an_id in [
            "P12",
            "P12-",
            "Phase-12",
            "**P12-M48**",
            "`P12-M48`",
            "P12-M4a",
        ] {
            assert!(!is_trace_id(not_an_id), "{not_an_id}");
        }
    }
}
