//! Repository-contract validation used by every phase smoke.

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
    verify_local_markdown_links(root)?;
    verify_forbidden_packages(root)?;
    Ok(())
}

fn verify_phase14_contract_trace(root: &Path) -> Result<(), String> {
    let path = root.join("docs/acceptance-cases/phase-14.md");
    let content = read_text(&path)?;
    let observed = frozen_trace_ids(&content, "P14-A")?;
    let expected: Vec<u16> = (1..=31).collect();
    if observed != expected {
        return Err(format!(
            "{} IDs must be exactly P14-A01..P14-A31; observed {observed:?}",
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
        let observed = frozen_trace_ids(&content, prefix)?;
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
    for (prefix, last) in [("P12-A", 17_u16), ("P12-M", 44_u16)] {
        let observed = frozen_trace_ids(&content, prefix)?;
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
        let observed = frozen_trace_ids(&content, prefix)?;
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
    let observed = frozen_trace_ids(&content, "P11-D")?;
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
    let automated = frozen_trace_ids(&content, "P10-A")?;
    let expected_automated: Vec<u16> = (1..=36).collect();
    if automated != expected_automated {
        return Err(format!(
            "{} automated IDs must be exactly P10-A01..P10-A36; observed {automated:?}",
            path.display()
        ));
    }
    let manual = frozen_trace_ids(&content, "UX10-")?;
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
    let observed = frozen_trace_ids(&content, "P09-D")?;
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
    for relative in [
        ".github/workflows/ci.yml",
        ".github/workflows/release.yml",
        ".github/workflows/scheduled.yml",
    ] {
        let path = root.join(relative);
        let content = read_text(&path)?;
        for (index, line) in content.lines().enumerate() {
            let trimmed = line.trim();
            let Some(action) = trimmed.strip_prefix("uses:") else {
                continue;
            };
            let action = action
                .split_once('#')
                .map_or(action, |(value, _)| value)
                .trim();
            let Some((_, revision)) = action.rsplit_once('@') else {
                return Err(format!(
                    "{}:{} action is not pinned: {action}",
                    path.display(),
                    index + 1
                ));
            };
            if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Err(format!(
                    "{}:{} action must use a full immutable commit SHA: {action}",
                    path.display(),
                    index + 1
                ));
            }
        }
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
        "persist-credentials: false",
        "tools/release/package.ps1",
        "tools/release/generate-sbom.ps1",
        "tools/release/verify-package.ps1",
        "qualification native-runtime --exe=target/release/stickymd-win.exe",
        "subject-checksums: dist/SHA256SUMS.txt",
        "sbom-path: dist/SBOM.spdx.json",
        "gh release create",
        "--draft",
    ] {
        if !release.contains(required) {
            return Err(format!(
                "release workflow lacks required security token `{required}`"
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
    if !package.contains("generate-third-party-notices.ps1") {
        return Err("package.ps1 must generate notices from the frozen runtime graph".to_owned());
    }
    let notices = read_text(&root.join("tools/release/generate-third-party-notices.ps1"))?;
    for required in [
        "cargo metadata --format-version 1 --locked --filter-platform x86_64-pc-windows-msvc",
        "Runtime package",
        "Cargo.lock SHA-256",
    ] {
        if !notices.contains(required) {
            return Err(format!(
                "third-party notice generator lacks required contract token `{required}`"
            ));
        }
    }
    let sbom = read_text(&root.join("tools/release/generate-sbom.ps1"))?;
    for required in [
        "1.50.0",
        "815ee6973ec5dff6a671d7f41b0e78835a8c45b91d5a39f4743ea1cee833d3be",
        "bb8824a06c27c625fc103db5d7e9d7131ba2cc6e7c7a79318ee71686ede3c3f0",
    ] {
        if !sbom.contains(required) {
            return Err(format!(
                "SBOM script lacks pinned supply-chain token `{required}`"
            ));
        }
    }
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
        "$TrayMenuOpenAttempts = 2",
        "GetCursorPos",
        "IsOffscreen",
        "Wait-ForProcessMenuItems",
        "Wait-ForProcessMenuClosed",
        "observed_items=",
    ] {
        if !tray_uia.contains(marker) {
            return Err(format!(
                "{} must retain acknowledged tray-menu interaction marker `{marker}`",
                tray_uia_path.display()
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
        let content = read_text(&script)?;
        for needle in [
            "stickymd-smoke",
            "'phase'",
            &format!("'{}'", phase.number()),
        ] {
            if !content.contains(needle) {
                return Err(format!(
                    "{} does not route through Rust CLI token `{needle}`",
                    script.display()
                ));
            }
        }
    }
    let all_script = root.join("tools/smoke/all.ps1");
    let content = read_text(&all_script)?;
    if !content.contains("stickymd-smoke") || !content.contains("'all'") {
        return Err(format!(
            "{} must route through `stickymd-smoke all`",
            all_script.display()
        ));
    }
    Ok(())
}

fn verify_phase8_frozen_trace(root: &Path) -> Result<(), String> {
    const LAST_IMPLEMENTATION_ROW: u16 = 116;
    const LAST_MANUAL_ROW: u16 = 139;
    let path = root.join("docs/acceptance-cases/phase-08.md");
    let content = read_text(&path)?;
    let observed = frozen_trace_ids(&content, "P08-D")?;
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

fn frozen_trace_ids(content: &str, prefix: &str) -> Result<Vec<u16>, String> {
    let mut ids = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed
            .strip_prefix("| ")
            .and_then(|line| line.strip_prefix(prefix))
        else {
            continue;
        };
        let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
        if digits.is_empty() {
            continue;
        }
        ids.push(
            digits
                .parse::<u16>()
                .map_err(|error| format!("invalid frozen trace row `{line}`: {error}"))?,
        );
    }
    Ok(ids)
}

fn matrix_path(root: &Path, phase: Phase) -> PathBuf {
    root.join(format!("docs/acceptance-cases/phase-{}.md", phase.number()))
}

fn read_matrix_rows(path: &Path) -> Result<Vec<MatrixRow>, String> {
    let content = read_text(path)?;
    let mut rows = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if !line.trim_start().starts_with("| P") {
            continue;
        }
        let cells: Vec<_> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() != 5 {
            return Err(format!(
                "{}:{} phase matrix row must contain 5 columns",
                path.display(),
                index + 1
            ));
        }
        let mode = cells[2];
        let evidence = cells[3];
        let status = cells[4];
        match mode {
            "Automated" if matches!(status, "AUTOMATED PASS" | "BLOCKED") => {}
            "Manual" if matches!(status, "MANUAL PASS" | "NOT TESTED" | "BLOCKED") => {}
            "Guided Manual" if matches!(status, "NOT TESTED" | "BLOCKED") => {}
            "Automated exact candidate" if matches!(status, "NOT TESTED" | "BLOCKED") => {}
            _ => {
                return Err(format!(
                    "{}:{} invalid mode/status pair `{mode}` / `{status}`",
                    path.display(),
                    index + 1
                ));
            }
        }
        if evidence.is_empty() {
            return Err(format!(
                "{}:{} evidence cell must not be empty",
                path.display(),
                index + 1
            ));
        }
        if status == "MANUAL PASS" && !evidence.contains("receipt:") {
            return Err(format!(
                "{}:{} MANUAL PASS requires a checked-in `receipt:` reference",
                path.display(),
                index + 1
            ));
        }
        rows.push(MatrixRow {
            line: index + 1,
            mode: mode.to_owned(),
            status: status.to_owned(),
        });
    }
    Ok(rows)
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
            && entry.path().extension().and_then(|value| value.to_str()) == Some(extension)
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
    use super::{frozen_trace_ids, markdown_links};

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
        assert_eq!(frozen_trace_ids(source, "P08-D").unwrap(), vec![1, 2]);
    }
}
