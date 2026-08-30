//! Phase 14 exact-candidate qualification; this tooling never mutates product state.

mod automated_readiness;
mod campaign;
mod decisions;
#[cfg(windows)]
mod exact_desktop;
pub(crate) mod exact_groups;
mod exact_readiness;
#[cfg(windows)]
mod g3;
mod g3_readiness;
#[cfg(windows)]
mod g4;
mod g4_readiness;
#[cfg(windows)]
mod g5;
mod g5_readiness;
mod guided;
mod json;
mod manual;
mod manual_readiness;
mod manual_receipt;
mod module_ledger;
mod readiness;
mod receipt;
mod remote;
pub(crate) mod repetition;
mod source_freeze;
mod startup_attribution;

use std::path::Path;

use crate::cli::{G3Case, G4Case, G5Case, ManualCommand, QualificationCommand};
use crate::evidence::{self, EvidenceResult, EvidenceStatus};
use crate::qualification_environment::{self, QualificationEnvironmentStatus};

pub(crate) fn execute(root: &Path, command: QualificationCommand) -> Result<(), String> {
    match command {
        QualificationCommand::Environment { evidence_file } => {
            record_environment(root, evidence_file.as_deref())
        }
        QualificationCommand::LocalCampaign => campaign::run(root),
        QualificationCommand::Modules => module_ledger::print_status(root),
        QualificationCommand::SourceFreeze => {
            let source = source_freeze::create(root)?;
            decisions::project(root, &source)?;
            println!("RELEASE_SOURCE_COMMIT={}", source.source_commit);
            println!("RELEASE_CARGO_LOCK_SHA256={}", source.cargo_lock_sha256);
            println!("REMOTE_SYNCED={}", source.remote_synced);
            Ok(())
        }
        QualificationCommand::StartupAttribution => startup_attribution::record(root),
        QualificationCommand::WindowStress(options) => run_window_stress(root, options),
        QualificationCommand::NativeRuntime { executable } => {
            let executable = if executable.is_absolute() {
                executable
            } else {
                root.join(executable)
            };
            let report = crate::pe_dependencies::verify_portable_executable(&executable)?;
            println!("PORTABLE_NATIVE_DEPENDENCIES={}", report.imports.join(","));
            println!("DEVELOPER_RUNTIME_IMPORTS=none");
            Ok(())
        }
        QualificationCommand::G3Exact {
            zip,
            evidence_file,
            case,
        } => run_g3(root, zip.as_deref(), evidence_file.as_deref(), case),
        QualificationCommand::G4Exact {
            zip,
            evidence_file,
            case,
        } => run_g4(root, zip.as_deref(), evidence_file.as_deref(), case),
        QualificationCommand::G5Exact {
            zip,
            evidence_file,
            case,
        } => run_g5(root, zip.as_deref(), evidence_file.as_deref(), case),
        QualificationCommand::Decision {
            key,
            status,
            evidence,
        } => decisions::update(root, &key, &status, &evidence),
        QualificationCommand::Readiness { explain } => readiness::evaluate(root, explain),
        QualificationCommand::Remote { run_id, attempt } => {
            remote::record_workflow(root, run_id, attempt)
        }
        QualificationCommand::Downloaded { zip } => remote::verify_downloaded(root, &zip),
    }
}

#[cfg(windows)]
fn run_g3(
    root: &Path,
    zip: Option<&Path>,
    evidence_file: Option<&Path>,
    case: Option<G3Case>,
) -> Result<(), String> {
    g3::run(root, zip, evidence_file, case)
}

#[cfg(windows)]
fn run_g4(
    root: &Path,
    zip: Option<&Path>,
    evidence_file: Option<&Path>,
    case: Option<G4Case>,
) -> Result<(), String> {
    g4::run(root, zip, evidence_file, case)
}

#[cfg(windows)]
fn run_g5(
    root: &Path,
    zip: Option<&Path>,
    evidence_file: Option<&Path>,
    case: Option<G5Case>,
) -> Result<(), String> {
    g5::run(root, zip, evidence_file, case)
}

#[cfg(not(windows))]
fn run_g4(
    _root: &Path,
    _zip: Option<&Path>,
    _evidence_file: Option<&Path>,
    _case: Option<G4Case>,
) -> Result<(), String> {
    Err("G4 exact qualification requires Windows".to_owned())
}

#[cfg(not(windows))]
fn run_g3(
    _root: &Path,
    _zip: Option<&Path>,
    _evidence_file: Option<&Path>,
    _case: Option<G3Case>,
) -> Result<(), String> {
    Err("G3 exact qualification requires Windows".to_owned())
}

#[cfg(not(windows))]
fn run_g5(
    _root: &Path,
    _zip: Option<&Path>,
    _evidence_file: Option<&Path>,
    _case: Option<G5Case>,
) -> Result<(), String> {
    Err("G5 exact qualification requires Windows".to_owned())
}

#[cfg(windows)]
fn run_window_stress(root: &Path, options: crate::cli::WindowStressOptions) -> Result<(), String> {
    let environment = qualification_environment::inspect();
    if environment.status != QualificationEnvironmentStatus::Valid {
        return Err(format!(
            "window-stress diagnostic requires a valid interactive desktop: {}",
            environment.summary()
        ));
    }
    crate::runtime::run_window_stress_diagnostic(root, options)
}

#[cfg(not(windows))]
fn run_window_stress(
    _root: &Path,
    _options: crate::cli::WindowStressOptions,
) -> Result<(), String> {
    Err("window-stress diagnostic requires Windows".to_owned())
}

pub(super) fn record_environment(root: &Path, evidence_file: Option<&Path>) -> Result<(), String> {
    let environment = qualification_environment::inspect();
    let status = match environment.status {
        QualificationEnvironmentStatus::Valid => EvidenceStatus::Passed,
        QualificationEnvironmentStatus::EnvironmentBlocked
        | QualificationEnvironmentStatus::Unsupported => EvidenceStatus::NotTested,
        QualificationEnvironmentStatus::Error => EvidenceStatus::Failed,
    };
    let results = [EvidenceResult {
        id: "qualification environment preflight".to_owned(),
        status,
        detail: (status != EvidenceStatus::Passed).then(|| environment.summary()),
        measurements: Vec::new(),
        gates: Vec::new(),
        samples: Vec::new(),
    }];
    evidence::emit(
        root,
        "qualification-environment",
        &results,
        Some(&environment),
        evidence_file,
    )?;
    match environment.status {
        QualificationEnvironmentStatus::Valid => Ok(()),
        QualificationEnvironmentStatus::EnvironmentBlocked => Err("Qualification environment is blocked by locked/non-interactive desktop. Unlock the active Windows session and rerun the Phase 14 evidence campaign.".to_owned()),
        QualificationEnvironmentStatus::Unsupported => {
            Err("qualification environment is unsupported on this host".to_owned())
        }
        QualificationEnvironmentStatus::Error => Err(format!(
            "qualification environment inspection failed: {}",
            environment.summary()
        )),
    }
}

pub(crate) fn record_manual(root: &Path, command: ManualCommand) -> Result<(), String> {
    manual::execute(root, command)
}

pub(crate) fn release_executable(root: &Path) -> Result<std::path::PathBuf, String> {
    receipt::resolve_release_executable(root)
}

pub(crate) fn reuse_last_success_for_evidence(root: &Path, path: &Path) -> Result<bool, String> {
    module_ledger::reuse_for_receipt(root, path)
}

pub(crate) fn record_last_success_for_evidence(root: &Path, path: &Path) -> Result<(), String> {
    module_ledger::record_for_receipt(root, path)
}
