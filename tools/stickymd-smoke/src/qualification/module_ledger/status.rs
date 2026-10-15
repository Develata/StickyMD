//! Read-only report of which modules can reuse a last success and why others rerun.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::path::Path;

use super::{CompatibleSuccess, Lookup, Snapshot, lookup_all};
use crate::qualification::receipt::{self, Candidate};

pub(in crate::qualification) fn print_status(root: &Path) -> Result<(), String> {
    let candidate = receipt::read_candidate(root).ok();
    print_snapshot(&lookup_all(root)?, candidate.as_ref());
    Ok(())
}

/// Print lookups that were already validated, e.g. by readiness, without repeating them.
pub(in crate::qualification) fn print_snapshot(snapshot: &Snapshot, candidate: Option<&Candidate>) {
    println!("LEDGER_STORE={}", snapshot.store_root.display());
    for (module, lookup) in &snapshot.modules {
        match lookup {
            Lookup::Valid(success) => println!(
                "MODULE={} STATUS={} ORIGIN_SOURCE={} ORIGIN_VERSION={} ORIGIN_EXE={} EVIDENCE={}",
                module.as_str(),
                success_status(success, candidate),
                success.origin_source_commit,
                success.origin_version,
                success.origin_exe_sha256,
                success.evidence_path.display()
            ),
            Lookup::Invalid(reason) => println!(
                "MODULE={} STATUS=RUN_REQUIRED REASON={} DETAIL={}",
                module.as_str(),
                lookup.rerun_reason(),
                reason.replace(['\r', '\n'], " ")
            ),
            Lookup::Missing { .. } => println!(
                "MODULE={} STATUS=RUN_REQUIRED REASON={}",
                module.as_str(),
                lookup.rerun_reason()
            ),
        }
    }
}

pub(super) fn success_status(
    success: &CompatibleSuccess,
    candidate: Option<&Candidate>,
) -> &'static str {
    if candidate.is_some_and(|candidate| {
        success.origin_source_commit == candidate.source_commit
            && success.origin_exe_sha256 == candidate.exe_sha256
            && success.origin_zip_sha256 == candidate.zip_sha256
    }) {
        "RAN_PASS"
    } else {
        "REUSED_PASS"
    }
}
