//! Opt-in execution of G4 cases against explicitly supplied local build bytes.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::{exact_checks, exact_desktop, receipt};
use crate::qualification_environment::{self, QualificationEnvironmentStatus};

/// No candidate resolver, exact receipt, or success ledger is called here. The
/// retained fixture and stdout are local preflight diagnostics only. Run serially
/// with USER authorization and STICKYMD_PREFLIGHT_EXE naming the local executable.
pub(in crate::qualification) fn run(
    case: &exact_checks::DeclaredCase,
    operation: exact_desktop::CaseOperation,
) {
    crate::window_control::enable_per_monitor_v2_dpi_awareness().unwrap();
    let repository =
        crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
    let compiled_repository =
        crate::governance::find_repository_root(std::path::Path::new(env!("CARGO_MANIFEST_DIR")))
            .unwrap();
    assert_eq!(
        fs::canonicalize(&repository).unwrap(),
        fs::canonicalize(&compiled_repository).unwrap(),
        "preflight binary was compiled from another checkout; use an isolated target directory"
    );
    let status = Command::new("git")
        .args(["status", "--porcelain", "--untracked-files=all"])
        .current_dir(&repository)
        .output()
        .unwrap();
    assert!(status.status.success(), "cannot inspect preflight checkout");
    assert!(
        status.stdout.is_empty(),
        "preflight requires a clean checkout"
    );
    let head = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(&repository)
        .output()
        .unwrap();
    assert!(
        head.status.success(),
        "cannot read preflight harness commit"
    );
    let source = PathBuf::from(
        std::env::var_os("STICKYMD_PREFLIGHT_EXE")
            .expect("set STICKYMD_PREFLIGHT_EXE to the explicitly tested local EXE"),
    );
    assert!(
        source.is_absolute() && source.is_file(),
        "invalid local EXE"
    );
    let environment = qualification_environment::inspect();
    assert_eq!(
        environment.status,
        QualificationEnvironmentStatus::Valid,
        "{}",
        environment.summary()
    );
    assert!(
        exact_desktop::running_stickymd_processes()
            .unwrap()
            .is_empty(),
        "close existing StickyMD instances; preflight never terminates them"
    );
    crate::input_injection::require_running().unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "stickymd-c2d-preflight-{}-{nonce}",
        std::process::id()
    ));
    let program = root.join(case.id.to_ascii_lowercase());
    fs::create_dir_all(&program).unwrap();
    let copied = program.join("StickyMD.exe");
    fs::copy(&source, &copied).unwrap();
    let digest = receipt::sha256(&source).unwrap();
    assert_eq!(digest, receipt::sha256(&copied).unwrap());
    println!("LOCAL_PREFLIGHT_CASE={}", case.id);
    println!(
        "LOCAL_PREFLIGHT_COMPILED_CHECKOUT={}",
        compiled_repository.display()
    );
    println!(
        "LOCAL_PREFLIGHT_HARNESS_EXE_SHA256={}",
        receipt::sha256(&std::env::current_exe().unwrap()).unwrap()
    );
    println!(
        "LOCAL_PREFLIGHT_HARNESS_COMMIT={}",
        String::from_utf8_lossy(&head.stdout).trim()
    );
    println!("LOCAL_PREFLIGHT_EXE_SHA256={digest}");
    println!("LOCAL_PREFLIGHT_FIXTURE={}", root.display());
    let outcome = operation(&repository, &program);
    let cleanup = crate::input_injection::stop_reason();
    println!("LOCAL_PREFLIGHT_INPUT_CLEANUP={cleanup:?}");
    let evidence = outcome.unwrap();
    assert!(cleanup.is_none(), "input cleanup failed: {cleanup:?}");
    let recorded = evidence.checks.into_sorted();
    assert_eq!(exact_checks::mismatch(case.checks, &recorded), None);
    assert!(
        exact_desktop::running_stickymd_processes()
            .unwrap()
            .is_empty()
    );
    for check in recorded {
        println!("LOCAL_PREFLIGHT_CHECK={check}");
    }
    for artifact in evidence.artifacts {
        println!(
            "LOCAL_PREFLIGHT_ARTIFACT={} sha256={}",
            artifact.path, artifact.sha256
        );
    }
}

#[test]
#[ignore = "exclusive Windows desktop; local EXE only, no exact-candidate evidence"]
fn c2d_search_local_preflight() {
    run(&exact_checks::G4[2], super::cases::g4_03);
}

#[test]
#[ignore = "exclusive Windows desktop and real IMEs; local EXE only"]
fn c2d_ime_local_preflight() {
    run(&exact_checks::G4[5], super::cases::g4_06);
}

#[test]
#[ignore = "exclusive Windows desktop, tray input and physical Alt+F4; local EXE only"]
fn c2d_tray_local_preflight() {
    run(&exact_checks::G4[0], super::cases::g4_01);
}

#[test]
#[ignore = "exclusive Windows desktop and dock input; local EXE only"]
fn c2d_dock_local_preflight() {
    run(&exact_checks::G4[1], super::cases::g4_02);
}

#[test]
#[ignore = "exclusive Windows desktop and dock/second-launch input; local EXE only"]
fn c2d_identity_local_preflight() {
    run(&exact_checks::G4[4], super::cases::g4_05);
}

const TWO_COPIES_CHECK: &str = "portable-copies-note-config-and-exit-isolated";
const TWO_COPIES_CASE: exact_checks::DeclaredCase = exact_checks::DeclaredCase {
    id: "P08-A11",
    checks: &[TWO_COPIES_CHECK],
};

fn check_two_copies(
    repository: &std::path::Path,
    program: &std::path::Path,
    initial_note: &str,
) -> Result<exact_desktop::CaseEvidence, String> {
    exact_desktop::seed_note(program, initial_note)?;
    let executable = program.join("StickyMD.exe");
    let mut child = exact_desktop::start_until_editor_ready(&executable)?;
    let window = crate::window_control::visible_window(child.id())?;
    crate::runtime::portable_isolation::run(repository, program, &executable, &mut child, window)?;
    child.kill_and_wait()?;
    let mut evidence = exact_desktop::CaseEvidence::default();
    evidence.checks.record(TWO_COPIES_CHECK, &[]);
    Ok(evidence)
}

#[test]
#[ignore = "exclusive Windows desktop and two owned tray icons; local EXE only"]
fn c2d_two_copies_local_preflight() {
    run(&TWO_COPIES_CASE, |repository, program| {
        check_two_copies(repository, program, "First portable copy original")
    });
}

#[test]
#[ignore = "exclusive Windows desktop and two owned tray icons; local EXE only"]
fn c2d_two_copies_empty_note_local_preflight() {
    run(&TWO_COPIES_CASE, |repository, program| {
        check_two_copies(repository, program, "")
    });
}
