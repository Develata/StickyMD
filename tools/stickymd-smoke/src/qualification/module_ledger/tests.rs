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
