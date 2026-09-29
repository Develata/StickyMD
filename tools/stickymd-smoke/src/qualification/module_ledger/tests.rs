//! Regression tests for last-success preservation and compatibility.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{
    CompatibleSuccess, ModuleId, compatible_success, record_success, success_path, success_status,
};
use crate::qualification::receipt::{self, Candidate, RELEASE_ARTIFACT_NAME};

#[test]
fn changed_input_requires_rerun_without_overwriting_last_success() {
    let root = fixture();
    let candidate = candidate();
    write_evidence(&root, ModuleId::G4, "first pass");
    record_success(&root, ModuleId::G4, &candidate).expect("record first success");
    let ledger_before = fs::read(success_path(&root, ModuleId::G4)).expect("read ledger");
    assert!(
        compatible_success(&root, ModuleId::G4)
            .expect("compatibility")
            .is_some()
    );

    fs::write(
        root.join("tools/stickymd-smoke/src/qualification/g4/cases/dock.rs"),
        "changed",
    )
    .expect("change G4 input");
    assert!(
        compatible_success(&root, ModuleId::G4)
            .expect("changed compatibility")
            .is_none()
    );
    assert_eq!(
        ledger_before,
        fs::read(success_path(&root, ModuleId::G4)).expect("read unchanged ledger")
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn precomputed_planning_input_still_verifies_archived_evidence_and_schema() {
    let root = fixture();
    let module = ModuleId::G4;
    write_evidence(&root, module, "complete pass");
    record_success(&root, module, &candidate()).unwrap();
    let input = super::fingerprint::calculate(&root, module).unwrap();
    let planned = super::compatible_success_for_input(&root, module, &input)
        .unwrap()
        .unwrap();
    assert_eq!(
        Some(planned.clone()),
        compatible_success(&root, module).unwrap()
    );
    fs::write(&planned.evidence_path, b"tampered archive").unwrap();
    assert!(super::compatible_success_for_input(&root, module, &input).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn canonical_reserved_paths_are_rejected_before_tasks_or_evidence_writes() {
    let root = fixture();
    let canonical_root = root.canonicalize().unwrap();
    for receipt in [
        crate::qualification::workspace_tests::RECEIPT,
        crate::cli::ResourceModule::Math.receipt(),
        "dist/evidence/module-success/resources-window.json",
        "dist/evidence/module-success/evidence/resources-window-fixture.json",
    ] {
        let alias = canonical_root.join(receipt);
        assert!(super::matches_receipt(&root, &alias, receipt));
        assert!(crate::qualification::validate_public_evidence_path(&root, &alias).is_err());
        crate::atomic_evidence::write(&root.join(receipt), b"previous success").unwrap();
        let alias = root.join(receipt).canonicalize().unwrap();
        let options = crate::cli::Options::parse([
            "phase".into(),
            "00".into(),
            format!("--evidence-file={}", alias.display()),
        ])
        .unwrap();
        let error = crate::runner::execute(&root, &options).unwrap_err();
        assert!(error.contains("coordinator-owned"), "{error}");
        assert!(crate::evidence::emit(&root, "phase-00", &[], None, Some(&alias)).is_err());
        assert_eq!(fs::read(root.join(receipt)).unwrap(), b"previous success");
    }
    let runtime = canonical_root.join(ModuleId::Runtime.receipt());
    assert_eq!(
        super::module_for_receipt(&root, &runtime),
        Some(ModuleId::Runtime)
    );
    let options = crate::cli::Options::parse([
        "phase".into(),
        "00".into(),
        format!("--evidence-file={}", runtime.display()),
    ])
    .unwrap();
    assert!(crate::qualification::smoke_scope::validate(&root, &options).is_err());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn evidence_directory_aliases_cannot_bypass_reserved_paths_before_file_creation() {
    let root = fixture();
    let target = root.join("dist/evidence");
    let alias = root.join("linked-evidence");
    fs::create_dir_all(&target).unwrap();
    #[cfg(windows)]
    {
        let output = Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:SMOKE_ALIAS_PATH -Target $env:SMOKE_ALIAS_TARGET -ErrorAction Stop | Out-Null; (New-Object -ComObject Scripting.FileSystemObject).GetFolder($env:SMOKE_ALIAS_ROOT).ShortPath"])
            .env("SMOKE_ALIAS_PATH", &alias)
            .env("SMOKE_ALIAS_TARGET", &target)
            .env("SMOKE_ALIAS_ROOT", &root)
            .output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let short_root = PathBuf::from(String::from_utf8(output.stdout).unwrap().trim());
        assert!(super::matches_receipt(
            &root,
            &short_root.join(ModuleId::Runtime.receipt()),
            ModuleId::Runtime.receipt()
        ));
    }
    #[cfg(unix)]
    std::os::unix::fs::symlink(&target, &alias).unwrap();
    let path = alias.join("source-success/workspace-tests.json");
    assert!(!path.exists());
    assert!(crate::qualification::validate_public_evidence_path(&root, &path).is_err());
    let module = alias.join("resources/math.json");
    assert!(crate::qualification::validate_public_evidence_path(&root, &module).is_err());
    for path in [
        "module-success/resources-window.json",
        "module-success/evidence/new.json",
    ] {
        assert!(
            crate::qualification::validate_public_evidence_path(&root, &alias.join(path)).is_err()
        );
    }
    assert!(
        crate::qualification::validate_public_evidence_path(
            &root,
            &alias.join("module-success-diagnostic.json")
        )
        .is_ok()
    );
    assert_eq!(
        super::module_for_receipt(&root, &alias.join("runtime-qualification.json")),
        Some(ModuleId::Runtime)
    );
    #[cfg(windows)]
    fs::remove_dir(&alias).unwrap();
    #[cfg(unix)]
    fs::remove_file(&alias).unwrap();
    fs::remove_dir_all(root).unwrap();
}

#[cfg(windows)]
#[test]
fn win32_aliases_are_reserved_without_changing_verbatim_path_identity() {
    let root = fixture();
    for relative in [
        "dist/evidence/resources/window.json.",
        "dist/evidence/resources/window.json ",
    ] {
        assert!(
            crate::qualification::validate_public_evidence_path(&root, Path::new(relative))
                .is_err(),
            "{relative}"
        );
    }
    fs::create_dir_all(root.join("dist/evidence/resources")).unwrap();
    for relative in [
        "dist/evidence/resources. /window.json",
        "dist/evidence/resources.../window.json",
        "dist/evidence/resources/.. /window.json",
        "dist/evidence/resources/.../window.json",
    ] {
        let error = crate::qualification::validate_public_evidence_path(&root, Path::new(relative))
            .unwrap_err();
        assert!(error.contains("ambiguous directories"), "{error}");
    }
    let device = PathBuf::from(format!(
        r"\\.\{}",
        root.join("dist/evidence/resources/window.json.").display()
    ));
    assert!(crate::qualification::validate_public_evidence_path(&root, &device).is_err());
    let verbatim = root
        .canonicalize()
        .unwrap()
        .join("dist/evidence/resources/window.json.");
    assert!(!super::matches_receipt(
        &root,
        &verbatim,
        crate::cli::ResourceModule::Window.receipt()
    ));
    assert!(crate::qualification::validate_public_evidence_path(&root, &verbatim).is_ok());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn successful_rerun_atomically_promotes_new_evidence() {
    let root = fixture();
    let candidate = candidate();
    write_evidence(&root, ModuleId::G4, "first pass");
    record_success(&root, ModuleId::G4, &candidate).expect("record first success");
    let first = compatible_success(&root, ModuleId::G4)
        .expect("first compatibility")
        .expect("first success");

    write_evidence(&root, ModuleId::G4, "second pass");
    record_success(&root, ModuleId::G4, &candidate).expect("record second success");
    let second = compatible_success(&root, ModuleId::G4)
        .expect("second compatibility")
        .expect("second success");
    assert_ne!(first.evidence_path, second.evidence_path);
    assert!(second.document.contains("second pass"));
    assert!(!first.evidence_path.exists());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn failed_result_cannot_replace_the_last_success() {
    let root = fixture();
    let candidate = candidate();
    write_evidence(&root, ModuleId::G4, "passing run");
    record_success(&root, ModuleId::G4, &candidate).expect("record success");
    let ledger_before = fs::read(success_path(&root, ModuleId::G4)).expect("read ledger");
    let success_before = compatible_success(&root, ModuleId::G4)
        .expect("compatibility")
        .expect("success");

    let failed = "{\"worktree_dirty\":false,\"results\":[{\"id\":\"run\",\"status\":\"FAILED\"}]}";
    receipt::write_receipt(&root, ModuleId::G4.receipt(), failed).expect("write failed result");
    assert!(record_success(&root, ModuleId::G4, &candidate).is_err());
    assert_eq!(
        ledger_before,
        fs::read(success_path(&root, ModuleId::G4)).expect("read preserved ledger")
    );
    assert_eq!(
        compatible_success(&root, ModuleId::G4)
            .expect("preserved compatibility")
            .expect("preserved success")
            .document,
        success_before.document
    );
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn status_distinguishes_current_candidate_run_from_reused_success() {
    let candidate = candidate();
    let mut success = CompatibleSuccess {
        module: ModuleId::G4,
        origin_source_commit: candidate.source_commit.clone(),
        origin_exe_sha256: candidate.exe_sha256.clone(),
        origin_zip_sha256: candidate.zip_sha256.clone(),
        evidence_path: PathBuf::from("evidence.json"),
        document: String::new(),
    };
    assert_eq!(success_status(&success, Some(&candidate)), "RAN_PASS");
    success.origin_zip_sha256 = "f".repeat(64);
    assert_eq!(success_status(&success, Some(&candidate)), "REUSED_PASS");
}

#[test]
fn resource_failures_preserve_completed_groups_and_old_aggregate_is_not_imported() {
    use crate::cli::ResourceModule::{Math, SourcePreview, Window};
    let root = fixture();
    let candidate = candidate();
    for group in [SourcePreview, Math] {
        write_resource_evidence(&root, group);
        record_success(&root, ModuleId::Resource(group), &candidate).unwrap();
    }
    let before = fs::read(success_path(&root, ModuleId::Resource(Math))).unwrap();
    let mut partial = crate::resource_plan::tests::valid_resource_result(Window);
    partial.measurements.remove(0);
    let document = crate::resource_plan::tests::document(Window, &partial);
    receipt::write_receipt(&root, Window.receipt(), &document).unwrap();
    assert!(record_success(&root, ModuleId::Resource(Window), &candidate).is_err());
    assert!(
        compatible_success(&root, ModuleId::Resource(Window))
            .unwrap()
            .is_none()
    );
    for group in [SourcePreview, Math] {
        assert!(
            compatible_success(&root, ModuleId::Resource(group))
                .unwrap()
                .is_some()
        );
    }
    assert_eq!(
        before,
        fs::read(success_path(&root, ModuleId::Resource(Math))).unwrap()
    );
    let old = root.join("dist/evidence/resources-qualification.json");
    fs::write(&old, "legacy aggregate").unwrap();
    super::record_for_receipt(&root, &old).unwrap();
    assert!(super::module_for_receipt(&root, &old).is_none());
    assert!(
        compatible_success(&root, ModuleId::Resource(Window))
            .unwrap()
            .is_none()
    );
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resource_input_or_candidate_drift_cannot_promote_an_old_measurement() {
    use crate::cli::ResourceModule::Window;
    let root = fixture();
    let module = ModuleId::Resource(Window);
    let candidate = candidate();
    write_resource_evidence(&root, Window);
    record_success(&root, module, &candidate).unwrap();
    let ledger = fs::read(success_path(&root, module)).unwrap();
    let mut wrong_candidate = candidate.clone();
    wrong_candidate.exe_sha256 = "0".repeat(64);
    assert!(record_success(&root, module, &wrong_candidate).is_err());
    let input = root.join("tools/stickymd-smoke/src/runtime/resources/window.rs");
    fs::create_dir_all(input.parent().unwrap()).unwrap();
    fs::write(input, "modified during measurement").unwrap();
    assert!(record_success(&root, module, &candidate).is_err());
    assert_eq!(ledger, fs::read(success_path(&root, module)).unwrap());
    assert!(compatible_success(&root, module).unwrap().is_none());
    fs::remove_dir_all(root).unwrap();
}

fn write_resource_evidence(root: &Path, group: crate::cli::ResourceModule) {
    let result = crate::resource_plan::tests::valid_resource_result(group);
    let document = crate::resource_plan::tests::document(group, &result);
    let input = super::fingerprint::calculate(root, ModuleId::Resource(group)).unwrap();
    let document = format!(
        "{{\"resource_input_fingerprint\":\"{input}\",{}",
        &document[1..]
    );
    receipt::write_receipt(root, group.receipt(), &document).unwrap();
}

fn write_evidence(root: &Path, module: ModuleId, contents: &str) {
    let document = format!(
        "{{\"worktree_dirty\":false,\"results\":[{{\"id\":\"{contents}\",\"status\":\"PASSED\"}}]}}"
    );
    receipt::write_receipt(root, module.receipt(), &document).expect("write evidence");
}

fn fixture() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("stickymd-module-ledger-{nonce}"));
    let input = root.join("tools/stickymd-smoke/src/qualification/g4/cases/dock.rs");
    fs::create_dir_all(input.parent().expect("parent")).expect("mkdir");
    fs::write(&input, "initial").expect("write input");
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

fn candidate() -> Candidate {
    Candidate {
        source_commit: "a".repeat(40),
        version: "0.1.0".to_owned(),
        cargo_lock_sha256: "b".repeat(64),
        exe_sha256: "c".repeat(64),
        zip_sha256: "d".repeat(64),
        sbom_sha256: "e".repeat(64),
        target: "x86_64-pc-windows-msvc".to_owned(),
        workflow_run_id: 1,
        workflow_attempt: 1,
        artifact_id: 2,
        artifact_name: RELEASE_ARTIFACT_NAME.to_owned(),
        zip_name: "StickyMD-0.1.0-windows-x64-portable.zip".to_owned(),
    }
}
