//! Phase 13 exact-candidate qualification; this tooling never mutates product state.

mod campaign;
mod decisions;
mod json;
mod manual;
mod readiness;
mod receipt;
mod remote;

use std::path::Path;

use crate::cli::{ManualCommand, QualificationCommand};
use crate::evidence::{self, EvidenceResult, EvidenceStatus};
use crate::qualification_environment::{self, QualificationEnvironmentStatus};

pub(crate) fn execute(root: &Path, command: QualificationCommand) -> Result<(), String> {
    match command {
        QualificationCommand::Environment { evidence_file } => {
            record_environment(root, evidence_file.as_deref())
        }
        QualificationCommand::LocalCampaign => campaign::run(root),
        QualificationCommand::Candidate => {
            let candidate = receipt::generate_candidate(root)?;
            decisions::project(root, &candidate)?;
            println!("RELEASE_SOURCE_COMMIT={}", candidate.source_commit);
            println!("RELEASE_EXE_SHA256={}", candidate.exe_sha256);
            println!("RELEASE_ZIP_SHA256={}", candidate.zip_sha256);
            println!("REMOTE_SYNCED={}", candidate.remote_synced);
            Ok(())
        }
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
        QualificationEnvironmentStatus::EnvironmentBlocked => Err("Qualification environment is blocked by locked/non-interactive desktop. Unlock the active Windows session and rerun Phase 13 evidence campaign.".to_owned()),
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
