//! Stable content fingerprints for functional qualification modules.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::ModuleId;
use crate::cli::ResourceModule;

mod normalize;
mod stream;
pub(in crate::qualification) use stream::Batch;

const GLOBAL: u64 = 1 << 0;
const STARTUP: u64 = 1 << 1;
const SHELL: u64 = 1 << 2;
const EDITOR: u64 = 1 << 3;
const PREVIEW: u64 = 1 << 4;
const MATH: u64 = 1 << 5;
const IMAGES: u64 = 1 << 6;
const ASSETS: u64 = 1 << 7;
const PERSISTENCE: u64 = 1 << 8;
const EXPORT: u64 = 1 << 9;
const RUNTIME_HARNESS: u64 = 1 << 10;
const PERFORMANCE_HARNESS: u64 = 1 << 11;
const RESOURCES_HARNESS: u64 = 1 << 12;
const G3_HARNESS: u64 = 1 << 13;
const G4_HARNESS: u64 = 1 << 14;
const G5_HARNESS: u64 = 1 << 15;
const WINDOW_RESOURCES: u64 = 1 << 16;
const ZOOM_RESOURCES: u64 = 1 << 17;
const ALL_PRODUCT: u64 =
    STARTUP | SHELL | EDITOR | PREVIEW | MATH | IMAGES | ASSETS | PERSISTENCE | EXPORT;
const ALL_HARNESS: u64 = RUNTIME_HARNESS
    | PERFORMANCE_HARNESS
    | RESOURCES_HARNESS
    | G3_HARNESS
    | G4_HARNESS
    | G5_HARNESS
    | WINDOW_RESOURCES
    | ZOOM_RESOURCES;
const ALL_MODULES: u64 = ALL_PRODUCT | ALL_HARNESS | GLOBAL;

pub(in crate::qualification) fn calculate(root: &Path, module: ModuleId) -> Result<String, String> {
    PlanningInputs::read(root)?.calculate(root, module)
}

/// A short-lived planning pass; callers must use fresh calculate() before promotion/reuse.
pub(in crate::qualification) struct PlanningInputs {
    tracked: Vec<String>,
}

impl PlanningInputs {
    pub(in crate::qualification) fn read(root: &Path) -> Result<Self, String> {
        Ok(Self {
            tracked: tracked_files(root)?,
        })
    }

    pub(in crate::qualification) fn calculate(
        &self,
        root: &Path,
        module: ModuleId,
    ) -> Result<String, String> {
        calculate_from(root, Some(module), &self.tracked, &[])
    }

    pub(in crate::qualification) fn calculate_many(
        &self,
        root: &Path,
        modules: &[ModuleId],
    ) -> Result<Batch, String> {
        stream::calculate(
            root,
            &modules.iter().copied().map(Some).collect::<Vec<_>>(),
            &self.tracked,
            &[],
        )
    }
}

pub(in crate::qualification) fn workspace_inputs(
    root: &Path,
    execution_identity: &[u8],
) -> Result<String, String> {
    calculate_from(root, None, &tracked_files(root)?, execution_identity)
}

fn calculate_from(
    root: &Path,
    module: Option<ModuleId>,
    tracked: &[String],
    extra: &[u8],
) -> Result<String, String> {
    Ok(stream::calculate(root, &[module], tracked, extra)?
        .digests
        .remove(0))
}

fn domains(module: ModuleId) -> u64 {
    match module {
        ModuleId::Runtime => ALL_PRODUCT | RUNTIME_HARNESS | GLOBAL,
        // The Phase 7 image/export Release baseline runs `export_snapshot`.
        ModuleId::Performance => {
            STARTUP | EDITOR | PREVIEW | PERSISTENCE | EXPORT | PERFORMANCE_HARNESS | GLOBAL
        }
        ModuleId::Resource(group) => {
            ALL_PRODUCT
                | RESOURCES_HARNESS
                | GLOBAL
                | match group {
                    ResourceModule::Window => WINDOW_RESOURCES,
                    ResourceModule::Zoom => ZOOM_RESOURCES,
                    _ => 0,
                }
        }
        ModuleId::G3 => EDITOR | IMAGES | ASSETS | PERSISTENCE | EXPORT | G3_HARNESS | GLOBAL,
        ModuleId::G4 => SHELL | EDITOR | PREVIEW | MATH | PERSISTENCE | G4_HARNESS | GLOBAL,
        ModuleId::G5 => SHELL | EDITOR | PREVIEW | MATH | IMAGES | G5_HARNESS | GLOBAL,
    }
}

fn tracked_files(root: &Path) -> Result<Vec<String>, String> {
    let output = Command::new("git")
        .args([
            "ls-files",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start git ls-files: {error}"))?;
    if !output.status.success() {
        return Err("git ls-files failed while fingerprinting qualification inputs".to_owned());
    }
    let mut files = output
        .stdout
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
        .map(|path| {
            String::from_utf8(path.to_vec())
                .map(|path| path.replace('\\', "/"))
                .map_err(|error| format!("tracked path is not UTF-8: {error}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    files.sort();
    Ok(files)
}

fn temporary_path() -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock is before Unix epoch: {error}"))?
        .as_nanos();
    Ok(temporary_path_at(nonce))
}

fn temporary_path_at(nonce: u128) -> PathBuf {
    // Windows CI clocks may return the same value to concurrent callers.
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let sequence = SEQUENCE.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "stickymd-module-fingerprint-{}-{nonce}-{sequence}.bin",
        std::process::id()
    ))
}

fn path_domains(path: &str) -> u64 {
    if path.starts_with("dist/evidence/") {
        return 0;
    }
    if is_global_input(path) {
        return GLOBAL;
    }
    if path == "crates/stickymd-render/tests/fixtures/rendering-stress.md" {
        return G5_HARNESS | PREVIEW | MATH | IMAGES;
    }
    if path.starts_with("crates/stickymd-render/tests/fixtures/qualification-images/") {
        return G5_HARNESS;
    }
    if path == "tests/fixtures/performance/typical-note-seed.md" {
        return PERFORMANCE_HARNESS;
    }
    if path == "tests/fixtures/performance/resource-note-seed.md" {
        return RESOURCES_HARNESS;
    }
    if is_non_behavior_document(path) || is_judgement_projection(path) {
        return 0;
    }
    if path.starts_with("docs/plan/") || path.starts_with("docs/acceptance-cases/") {
        return ALL_MODULES;
    }
    product_domains(path).unwrap_or_else(|| harness_domains(path))
}

fn is_global_input(path: &str) -> bool {
    matches!(
        path,
        "Cargo.toml"
            | "Cargo.lock"
            | "rust-toolchain.toml"
            | ".cargo/config.toml"
            | "apps/stickymd-win/Cargo.toml"
            | "crates/stickymd-core/Cargo.toml"
            | "crates/stickymd-render/Cargo.toml"
            | "tools/stickymd-smoke/Cargo.toml"
            | "docs/plan/11_testing_and_release.md"
            | "docs/acceptance-cases/phase-14.md"
            | "tools/stickymd-smoke/src/qualification/module_evidence.rs"
            | "tools/stickymd-smoke/src/qualification/module_registry.rs"
            | "tools/stickymd-smoke/src/qualification/module_ledger.rs"
            | "tools/stickymd-smoke/src/qualification/module_ledger/record.rs"
            | "tools/stickymd-smoke/src/qualification/windows_build.rs"
            | "tools/stickymd-smoke/src/qualification/module_ledger/fingerprint.rs"
            | "tools/stickymd-smoke/src/qualification/module_ledger/fingerprint/normalize.rs"
            | "tools/stickymd-smoke/src/qualification/module_ledger/fingerprint/stream.rs"
            | "tools/stickymd-smoke/src/qualification/module_ledger/store.rs"
            | "tools/stickymd-smoke/src/atomic_evidence.rs"
            | "tools/stickymd-smoke/src/qualification/receipt.rs"
    )
}

/// Inputs that no functional qualification module executes or reads. Governance and
/// package checks still consume several of them; those run on every change through CI
/// selection or as exact-byte gates, not through the functional ledger.
fn is_non_behavior_document(path: &str) -> bool {
    path.starts_with("docs/report/")
        || path.starts_with("docs/tasks/")
        || path.starts_with("docs/reference/")
        || path.starts_with("docs/phases/")
        || path.starts_with("docs/release-notes/")
        || path.starts_with("docs/adr/")
        || path.starts_with("docs/overview/")
        || path.starts_with("docs/features/")
        || path.starts_with("tests/")
        || path.starts_with("benches/")
        // README images and license texts are packaged files, verified by exact-byte gates.
        || path.starts_with("assets/readme/")
        || path.starts_with("assets/licenses/")
        || matches!(
            path,
            "README.md"
                | "README.en.md"
                | "README.zh-CN.md"
                | "CHANGELOG.md"
                | "CONTRIBUTING.md"
                | "SECURITY.md"
                | "AGENTS.md"
                | "LICENSE"
                | "THIRD_PARTY_NOTICES.md"
                | "docs/AGENTS.md"
                | "docs/plan/AGENTS.md"
                | "docs/coverage-matrix.md"
                | "docs/release-checklist.md"
        )
}

/// Tool code that only reports or aggregates module judgements made elsewhere: it neither
/// runs a module nor decides whether archived evidence is acceptable (that is
/// `module_evidence`, the record schema and the ledger, all `GLOBAL`), so editing it must
/// not invalidate any functional module. The tool's own tests and CI still cover it.
fn is_judgement_projection(path: &str) -> bool {
    matches!(
        path,
        "tools/stickymd-smoke/src/qualification/module_ledger/status.rs"
            | "tools/stickymd-smoke/src/qualification/readiness.rs"
    )
}

/// Product sources, classified by `classified_product_domains`. A product file without an
/// explicit rule still invalidates every product domain, so an unclassified file can only
/// widen; `every_product_source_has_an_explicit_rule` rejects it in CI.
fn product_domains(path: &str) -> Option<u64> {
    PRODUCT_ROOTS
        .iter()
        .any(|root| path.starts_with(root))
        .then(|| classified_product_domains(path).unwrap_or(ALL_PRODUCT))
}

const PRODUCT_ROOTS: &[&str] = &[
    "apps/stickymd-win/src/",
    "crates/stickymd-core/src/",
    "crates/stickymd-render/src/",
    "assets/",
];

/// Code every module's sessions depend on, whatever the module exercises:
/// - the launch path and every event-loop turn: startup, configuration and preferences, the
///   note load and managed-asset boundary check, recovery inspection, window and platform
///   setup, intent routing, the editor session and its reducer, search state, the preview
///   flow (shown at launch, ticked every turn), the frame surface, source rendering (the
///   default view), the caret overlay and the shared core;
/// - the persistence that follows any edit or preference change: autosave, reconciliation of
///   the app's own writes, and the atomic publish used for the note and the configuration;
/// - the `StickyApp` state machine (`app.rs`, `app/`): its files share one mutable state that
///   per-turn code reads (for example `export_in_flight` and `asset_paste_pending` feed the
///   window guards), so even a feature handler there changes every-turn behaviour.
///
/// A pattern ending in `/` names a directory; any other pattern names one file.
const EVERY_SESSION: &[&str] = &[
    "apps/stickymd-win/src/main.rs",
    "apps/stickymd-win/src/app.rs",
    "apps/stickymd-win/src/app/",
    "apps/stickymd-win/src/startup/",
    "apps/stickymd-win/src/config/",
    "apps/stickymd-win/src/surface.rs",
    "apps/stickymd-win/src/assets/mod.rs",
    "apps/stickymd-win/src/assets/safe_boundary.rs",
    "apps/stickymd-win/src/assets/storage.rs",
    "apps/stickymd-win/src/flow/mod.rs",
    "apps/stickymd-win/src/flow/editor.rs",
    "apps/stickymd-win/src/flow/persistence.rs",
    "apps/stickymd-win/src/flow/preferences.rs",
    "apps/stickymd-win/src/flow/preview.rs",
    "apps/stickymd-win/src/flow/reconciliation.rs",
    "apps/stickymd-win/src/flow/recovery.rs",
    "apps/stickymd-win/src/flow/save.rs",
    "apps/stickymd-win/src/flow/window/",
    "apps/stickymd-win/src/instruction/",
    "apps/stickymd-win/src/interaction/mod.rs",
    "apps/stickymd-win/src/interaction/search.rs",
    "apps/stickymd-win/src/interaction/session.rs",
    "apps/stickymd-win/src/persistence/",
    "apps/stickymd-win/src/platform/mod.rs",
    "apps/stickymd-win/src/platform/windows/mod.rs",
    "apps/stickymd-win/src/platform/windows/atomic_file.rs",
    "apps/stickymd-win/src/platform/windows/caret_overlay.rs",
    "apps/stickymd-win/src/platform/windows/diagnostic_event.rs",
    "apps/stickymd-win/src/platform/windows/file_identity.rs",
    "apps/stickymd-win/src/platform/windows/file_watch.rs",
    "apps/stickymd-win/src/platform/windows/monitor.rs",
    "apps/stickymd-win/src/platform/windows/native_message.rs",
    "apps/stickymd-win/src/platform/windows/program_dir.rs",
    "apps/stickymd-win/src/platform/windows/single_instance.rs",
    "apps/stickymd-win/src/platform/windows/tool_window.rs",
    "apps/stickymd-win/src/platform/windows/tray.rs",
    "apps/stickymd-win/src/platform/windows/window_opacity.rs",
    "apps/stickymd-win/src/platform/windows/window_topmost.rs",
    "crates/stickymd-core/src/",
    "crates/stickymd-render/src/lib.rs",
    "crates/stickymd-render/src/scroll.rs",
    "crates/stickymd-render/src/source/",
];

/// Files that had no rule before the explicit table. Their closure has not been traced, so
/// they keep every product domain rather than being narrowed without evidence.
const NOT_YET_NARROWED: &[&str] = &[
    "apps/stickymd-win/src/test_support.rs",
    "apps/stickymd-win/src/platform/windows/export_dialog.rs",
    "apps/stickymd-win/src/platform/windows/local_image_file.rs",
    "apps/stickymd-win/src/platform/windows/local_image_file/",
    "apps/stickymd-win/src/platform/windows/managed_file.rs",
    "apps/stickymd-win/src/platform/windows/message_box.rs",
    "apps/stickymd-win/src/platform/windows/shell.rs",
];

/// Code reached only through the named features, with the domains those features feed. The
/// first matching pattern wins, after `EVERY_SESSION` and `NOT_YET_NARROWED`.
const FEATURE_SOURCES: &[(&str, u64)] = &[
    ("crates/stickymd-render/src/math/", PREVIEW | MATH),
    (
        "crates/stickymd-render/src/image.rs",
        PREVIEW | IMAGES | EXPORT,
    ),
    (
        "crates/stickymd-render/src/image/",
        PREVIEW | IMAGES | EXPORT,
    ),
    (
        "crates/stickymd-render/src/preview/",
        PREVIEW | MATH | IMAGES | EXPORT | EDITOR,
    ),
    (
        "crates/stickymd-render/src/math_text.rs",
        PREVIEW | EDITOR | MATH | IMAGES | EXPORT,
    ),
    (
        "crates/stickymd-render/src/math_text/",
        PREVIEW | EDITOR | MATH | IMAGES | EXPORT,
    ),
    // Image paste staging and local image paths; the storage itself runs every session.
    (
        "apps/stickymd-win/src/assets/",
        ASSETS | IMAGES | PERSISTENCE,
    ),
    ("apps/stickymd-win/src/export/", EXPORT | ASSETS | IMAGES),
    // The worker starts only when Preview or Split becomes visible.
    ("apps/stickymd-win/src/preview/", PREVIEW | MATH | IMAGES),
    ("apps/stickymd-win/src/interaction/navigation.rs", EDITOR),
    ("apps/stickymd-win/src/source_search.rs", EDITOR),
    // Paste reads encoded images from the clipboard and stages them as managed assets.
    (
        "apps/stickymd-win/src/flow/clipboard.rs",
        EDITOR | ASSETS | IMAGES,
    ),
    (
        "apps/stickymd-win/src/platform/windows/clipboard.rs",
        EDITOR | ASSETS | IMAGES,
    ),
];
/// Every product source has an explicit rule and every rule still names a tracked file, so
/// a new or renamed file cannot fall back silently and a stale rule cannot linger. Governance
/// runs this on every CI plan, including plans that skip the smoke package's own tests.
pub(crate) fn verify_product_classification(root: &Path) -> Result<(), String> {
    let tracked = tracked_files(root)?;
    let unclassified = tracked
        .iter()
        .filter(|path| {
            product_domains(path).is_some()
                && path_domains(path) != 0
                && classified_product_domains(path).is_none()
        })
        .collect::<Vec<_>>();
    if !unclassified.is_empty() {
        return Err(format!(
            "product files without a domain rule in qualification/module_ledger/fingerprint.rs: {unclassified:?}"
        ));
    }
    let stale = EVERY_SESSION
        .iter()
        .chain(NOT_YET_NARROWED)
        .copied()
        .chain(FEATURE_SOURCES.iter().map(|(pattern, _)| *pattern))
        .filter(|pattern| !tracked.iter().any(|path| matches_pattern(path, pattern)))
        .collect::<Vec<_>>();
    if !stale.is_empty() {
        return Err(format!(
            "product domain rules that match no tracked file: {stale:?}"
        ));
    }
    Ok(())
}

fn classified_product_domains(path: &str) -> Option<u64> {
    if EVERY_SESSION
        .iter()
        .chain(NOT_YET_NARROWED)
        .any(|pattern| matches_pattern(path, pattern))
    {
        return Some(ALL_PRODUCT);
    }
    FEATURE_SOURCES
        .iter()
        .find(|(pattern, _)| matches_pattern(path, pattern))
        .map(|(_, domains)| *domains)
}

fn matches_pattern(path: &str, pattern: &str) -> bool {
    if pattern.ends_with('/') {
        path.starts_with(pattern)
    } else {
        path == pattern
    }
}
fn harness_domains(path: &str) -> u64 {
    if path.starts_with("tools/stickymd-smoke/src/runtime/resources/window")
        || path.starts_with("tools/stickymd-smoke/src/runtime/window_stress")
        || path == "tools/stickymd-smoke/src/runtime/resources/comparison.rs"
        || path == "tools/stickymd-smoke/src/resource_plan/window_comparison.rs"
        || path == "tools/stickymd-smoke/src/window_control/resource_comparison.rs"
    {
        return WINDOW_RESOURCES;
    }
    if path.starts_with("tools/stickymd-smoke/src/runtime/resources/zoom")
        || path == "tools/stickymd-smoke/src/resource_plan/zoom.rs"
    {
        return ZOOM_RESOURCES;
    }
    if path == "tools/stickymd-smoke/src/runtime/resources.rs"
        || path.starts_with("tools/stickymd-smoke/src/runtime/resources/")
        || path.starts_with("tools/stickymd-smoke/src/resource_plan")
        || path == "tools/stickymd-smoke/src/runner/resource_session.rs"
        || path == "tools/stickymd-smoke/src/qualification/resource_modules.rs"
        || path.starts_with("tools/stickymd-smoke/src/qualification/resource_modules/")
        || path == "tools/stickymd-smoke/src/runner/resource_progress.rs"
    {
        return RESOURCES_HARNESS;
    }
    if path.starts_with("tools/stickymd-smoke/src/qualification/g3") {
        return G3_HARNESS;
    }
    if path.starts_with("tools/stickymd-smoke/src/qualification/g4") {
        return G4_HARNESS;
    }
    if path.starts_with("tools/stickymd-smoke/src/qualification/g5") {
        return G5_HARNESS;
    }
    if path.ends_with("managed_process.rs")
        || path.starts_with("tools/stickymd-smoke/src/window_control")
    {
        return ALL_HARNESS;
    }
    if path.starts_with("tools/stickymd-smoke/src/qualification/exact_desktop")
        || path.ends_with("helpers/windows-uia.ps1")
    {
        return G3_HARNESS | G4_HARNESS | G5_HARNESS;
    }
    if path.ends_with("tools/stickymd-smoke/src/runtime.rs")
        || path.ends_with("tools/stickymd-smoke/src/runner.rs")
        || path == "tools/stickymd-smoke/src/runner/candidate_input.rs"
        || path.ends_with("tools/stickymd-smoke/src/evidence.rs")
        || path.ends_with("tools/stickymd-smoke/src/process_metrics.rs")
        || path.ends_with("tools/stickymd-smoke/src/ready_event.rs")
    {
        return RUNTIME_HARNESS | PERFORMANCE_HARNESS | RESOURCES_HARNESS;
    }
    if path.starts_with("tools/stickymd-smoke/src/")
        || path.starts_with("tools/stickymd-smoke/helpers/")
        || path.starts_with("tools/smoke/")
    {
        return ALL_HARNESS;
    }
    GLOBAL
}

#[cfg(test)]
mod tests;
