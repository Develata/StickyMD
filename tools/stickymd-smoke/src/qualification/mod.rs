//! Phase 12 release qualification; this tools-only module never mutates product state.

mod decisions;
mod json;
mod manual;
mod readiness;
mod receipt;
mod remote;

use std::path::Path;

use crate::cli::QualificationCommand;

pub(crate) fn execute(root: &Path, command: QualificationCommand) -> Result<(), String> {
    match command {
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

pub(crate) fn record_manual(root: &Path) -> Result<(), String> {
    manual::record(root)
}
