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
        ModuleId::Performance => {
            STARTUP | EDITOR | PREVIEW | PERSISTENCE | PERFORMANCE_HARNESS | GLOBAL
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
    if is_non_behavior_document(path) {
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

fn product_domains(path: &str) -> Option<u64> {
    if path.starts_with("crates/stickymd-core/src/assets") {
        return Some(ASSETS | IMAGES | EXPORT | EDITOR | PERSISTENCE);
    }
    if path.starts_with("crates/stickymd-core/src/") {
        return Some(EDITOR | PERSISTENCE | STARTUP | ASSETS);
    }
    if path.starts_with("crates/stickymd-render/src/source/") {
        return Some(EDITOR | PREVIEW);
    }
    if path.starts_with("crates/stickymd-render/src/math/") {
        return Some(PREVIEW | MATH);
    }
    if path.starts_with("crates/stickymd-render/src/image") {
        return Some(PREVIEW | IMAGES | EXPORT);
    }
    if path.starts_with("crates/stickymd-render/src/preview/") {
        return Some(PREVIEW | MATH | IMAGES | EXPORT | EDITOR);
    }
    if path.starts_with("crates/stickymd-render/src/") {
        return Some(PREVIEW | EDITOR | MATH | IMAGES | EXPORT);
    }
    if path.starts_with("apps/stickymd-win/src/assets/") {
        return Some(ASSETS | IMAGES | PERSISTENCE);
    }
    if path.starts_with("apps/stickymd-win/src/export/") || path.ends_with("/export_runtime.rs") {
        return Some(EXPORT | ASSETS | IMAGES);
    }
    if is_persistence_path(path) {
        return Some(PERSISTENCE | STARTUP);
    }
    if is_shell_path(path) {
        return Some(SHELL);
    }
    if is_preview_path(path) {
        return Some(PREVIEW | MATH | IMAGES);
    }
    if is_editor_path(path) {
        return Some(EDITOR);
    }
    (path.starts_with("apps/stickymd-win/src/") || path.starts_with("assets/"))
        .then_some(ALL_PRODUCT)
}

fn is_persistence_path(path: &str) -> bool {
    path.starts_with("apps/stickymd-win/src/persistence/")
        || path.starts_with("apps/stickymd-win/src/startup/")
        || path.contains("persistence_runtime.rs")
        || path.contains("recovery_runtime.rs")
        || path.contains("reconciliation_runtime.rs")
        || path.starts_with("apps/stickymd-win/src/flow/persistence")
        || path.starts_with("apps/stickymd-win/src/flow/recovery")
        || path.starts_with("apps/stickymd-win/src/flow/reconciliation")
        || path.starts_with("apps/stickymd-win/src/flow/save")
        || path.contains("atomic_file.rs")
        || path.contains("file_watch.rs")
        || path.contains("single_instance.rs")
        || path.contains("program_dir.rs")
}

fn is_shell_path(path: &str) -> bool {
    path.starts_with("apps/stickymd-win/src/flow/window/")
        || path.contains("window_runtime.rs")
        || path.contains("window_interaction.rs")
        || path.contains("window_geometry_runtime.rs")
        || path.contains("toolbar_paint.rs")
        || path.contains("controls.rs")
        || path.contains("/platform/windows/tray.rs")
        || path.contains("/platform/windows/monitor.rs")
        || path.contains("/platform/windows/native_message.rs")
        || path.contains("/platform/windows/tool_window.rs")
        || path.contains("/platform/windows/window_")
}

fn is_preview_path(path: &str) -> bool {
    path.starts_with("apps/stickymd-win/src/preview/")
        || path.contains("preview_runtime.rs")
        || path.contains("preview_input.rs")
        || path.starts_with("apps/stickymd-win/src/flow/preview")
}

fn is_editor_path(path: &str) -> bool {
    path.starts_with("apps/stickymd-win/src/interaction/")
        || path.contains("source_search.rs")
        || path.contains("search_")
        || path.contains("caret_runtime.rs")
        || path.contains("/app/input.rs")
        || path.starts_with("apps/stickymd-win/src/flow/editor")
        || path.starts_with("apps/stickymd-win/src/flow/clipboard")
        || path.starts_with("apps/stickymd-win/src/instruction/")
        || path.contains("/platform/windows/clipboard.rs")
        || path.contains("/platform/windows/caret_overlay.rs")
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
