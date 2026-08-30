//! Stable content fingerprints for functional qualification modules.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::ModuleId;
use crate::qualification::receipt;

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
const ALL_PRODUCT: u64 =
    STARTUP | SHELL | EDITOR | PREVIEW | MATH | IMAGES | ASSETS | PERSISTENCE | EXPORT;
const ALL_HARNESS: u64 = RUNTIME_HARNESS
    | PERFORMANCE_HARNESS
    | RESOURCES_HARNESS
    | G3_HARNESS
    | G4_HARNESS
    | G5_HARNESS;
const ALL_MODULES: u64 = ALL_PRODUCT | ALL_HARNESS | GLOBAL;

pub(super) fn calculate(root: &Path, module: ModuleId) -> Result<String, String> {
    let tracked = tracked_files(root)?;
    let temporary = temporary_path()?;
    let result =
        write_stream(root, module, &tracked, &temporary).and_then(|()| receipt::sha256(&temporary));
    let _ = fs::remove_file(&temporary);
    result
}

fn write_stream(
    root: &Path,
    module: ModuleId,
    tracked: &[String],
    temporary: &Path,
) -> Result<(), String> {
    let mut output = File::create(temporary)
        .map_err(|error| format!("cannot create module fingerprint stream: {error}"))?;
    output
        .write_all(b"StickyMD qualification module fingerprint v1\0")
        .map_err(io_error)?;
    output
        .write_all(module.as_str().as_bytes())
        .map_err(io_error)?;
    output.write_all(&[0]).map_err(io_error)?;
    for relative in tracked {
        if path_domains(relative) & domains(module) == 0 {
            continue;
        }
        let name = relative.as_bytes();
        output
            .write_all(&(name.len() as u64).to_le_bytes())
            .map_err(io_error)?;
        output.write_all(name).map_err(io_error)?;
        let path = root.join(relative.replace('/', std::path::MAIN_SEPARATOR_STR));
        let length = path
            .metadata()
            .map_err(|error| format!("cannot stat module input {relative}: {error}"))?
            .len();
        output.write_all(&length.to_le_bytes()).map_err(io_error)?;
        let mut input = File::open(&path)
            .map_err(|error| format!("cannot open module input {relative}: {error}"))?;
        std::io::copy(&mut input, &mut output)
            .map_err(|error| format!("cannot hash module input {relative}: {error}"))?;
    }
    output.sync_all().map_err(io_error)
}

fn domains(module: ModuleId) -> u64 {
    match module {
        ModuleId::Runtime => ALL_PRODUCT | RUNTIME_HARNESS | GLOBAL,
        ModuleId::Performance => {
            STARTUP | EDITOR | PREVIEW | PERSISTENCE | PERFORMANCE_HARNESS | GLOBAL
        }
        ModuleId::Resources => ALL_PRODUCT | RESOURCES_HARNESS | GLOBAL,
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
    Ok(std::env::temp_dir().join(format!(
        "stickymd-module-fingerprint-{}-{nonce}.bin",
        std::process::id()
    )))
}

fn path_domains(path: &str) -> u64 {
    if path.starts_with("dist/evidence/") {
        return 0;
    }
    if is_global_input(path) {
        return GLOBAL;
    }
    if path == "docs/plan/10_performance_reliability.md" {
        return PERFORMANCE_HARNESS | RESOURCES_HARNESS;
    }
    if path == "crates/stickymd-render/tests/fixtures/rendering-stress.md" {
        return G5_HARNESS | PREVIEW | MATH | IMAGES;
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
            | "tools/stickymd-smoke/src/qualification/module_ledger.rs"
            | "tools/stickymd-smoke/src/qualification/module_ledger/fingerprint.rs"
            | "tools/stickymd-smoke/src/atomic_evidence.rs"
            | "tools/stickymd-smoke/src/qualification/receipt.rs"
    )
}

fn is_non_behavior_document(path: &str) -> bool {
    path.starts_with("docs/report/")
        || path.starts_with("docs/tasks/")
        || path.starts_with("docs/reference/")
        || path.starts_with("docs/phases/")
        || path.starts_with("tests/")
        || path.starts_with("benches/")
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
    if path.starts_with("tools/stickymd-smoke/src/qualification/g3") {
        return G3_HARNESS;
    }
    if path.starts_with("tools/stickymd-smoke/src/qualification/g4") {
        return G4_HARNESS;
    }
    if path.starts_with("tools/stickymd-smoke/src/qualification/g5") {
        return G5_HARNESS;
    }
    if path.starts_with("tools/stickymd-smoke/src/qualification/exact_desktop")
        || path.ends_with("managed_process.rs")
        || path.starts_with("tools/stickymd-smoke/src/window_control")
        || path.ends_with("helpers/windows-uia.ps1")
    {
        return G3_HARNESS | G4_HARNESS | G5_HARNESS;
    }
    if path.ends_with("tools/stickymd-smoke/src/runtime.rs")
        || path.ends_with("tools/stickymd-smoke/src/runner.rs")
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

fn io_error(error: std::io::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::{GLOBAL, calculate, path_domains};
    use crate::qualification::module_ledger::ModuleId;
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn unrelated_group_harness_does_not_change_module_fingerprint() {
        let root = fixture();
        let before_g3 = calculate(&root, ModuleId::G3).expect("G3 fingerprint");
        let before_g4 = calculate(&root, ModuleId::G4).expect("G4 fingerprint");
        fs::write(
            root.join("tools/stickymd-smoke/src/qualification/g4/cases/dock.rs"),
            "changed",
        )
        .expect("change G4 harness");
        assert_eq!(before_g3, calculate(&root, ModuleId::G3).expect("G3 after"));
        assert_ne!(before_g4, calculate(&root, ModuleId::G4).expect("G4 after"));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn report_text_is_not_a_behavior_input_but_contract_is_global() {
        assert_eq!(path_domains("docs/report/note.md"), 0);
        assert_eq!(path_domains("dist/evidence/module-success/g4.json"), 0);
        assert_ne!(
            path_domains("docs/plan/11_testing_and_release.md") & GLOBAL,
            0
        );
    }

    fn fixture() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-module-fingerprint-{nonce}"));
        for path in [
            "tools/stickymd-smoke/src/qualification/g3/cases.rs",
            "tools/stickymd-smoke/src/qualification/g4/cases/dock.rs",
            "docs/report/note.md",
        ] {
            let path = root.join(path);
            fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
            fs::write(path, "initial").expect("write fixture");
        }
        assert!(
            Command::new("git")
                .arg("init")
                .arg("--quiet")
                .current_dir(&root)
                .status()
                .expect("git init")
                .success()
        );
        assert!(
            Command::new("git")
                .args(["add", "."])
                .current_dir(&root)
                .status()
                .expect("git add")
                .success()
        );
        root
    }
}
