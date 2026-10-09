//! Read-only report of which modules can reuse a last success and why others rerun.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::path::Path;

use super::store::LedgerStore;
use super::{CompatibleSuccess, ModuleId, compatible_success_for_input, fingerprint};
use crate::qualification::module_registry::modules;
use crate::qualification::receipt::{self, Candidate};

pub(in crate::qualification) fn print_status(root: &Path) -> Result<(), String> {
    let candidate = receipt::read_candidate(root).ok();
    print_status_for_candidate(root, candidate.as_ref())
}

pub(in crate::qualification) fn print_status_for_candidate(
    root: &Path,
    candidate: Option<&Candidate>,
) -> Result<(), String> {
    let store = LedgerStore::for_repository(root)?;
    println!("LEDGER_STORE={}", store.root().display());
    // One planning pass reads each shared input once for every module.
    let all = modules().collect::<Vec<_>>();
    let digests = fingerprint::PlanningInputs::read(root)?
        .calculate_many(root, &all)?
        .digests;
    for (module, current) in all.into_iter().zip(digests) {
        match compatible_success_for_input(root, module, &current) {
            Ok(Some(success)) => println!(
                "MODULE={} STATUS={} ORIGIN_SOURCE={} ORIGIN_VERSION={} ORIGIN_EXE={} EVIDENCE={}",
                module.as_str(),
                success_status(&success, candidate),
                success.origin_source_commit,
                success.origin_version,
                success.origin_exe_sha256,
                success.evidence_path.display()
            ),
            Ok(None) => println!(
                "MODULE={} STATUS=RUN_REQUIRED REASON={}",
                module.as_str(),
                rerun_reason(root, module)
            ),
            Err(error) => println!(
                "MODULE={} STATUS=RUN_REQUIRED REASON={}",
                module.as_str(),
                error.replace(['\r', '\n'], " ")
            ),
        }
    }
    Ok(())
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

pub(in crate::qualification) fn rerun_reason(root: &Path, module: ModuleId) -> &'static str {
    match LedgerStore::for_repository(root)
        .and_then(|store| store.has_module_records(module.as_str()))
    {
        Ok(true) => "INPUT_FINGERPRINT_CHANGED",
        Ok(false) => "NO_LAST_SUCCESS",
        Err(_) => "LEDGER_STORE_UNAVAILABLE",
    }
}
