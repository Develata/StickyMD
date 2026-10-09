//! Source-only and promoted-artifact automated receipt validation.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::path::Path;

use super::module_ledger;
use super::module_registry::ModuleId;
use super::receipt::Candidate;
use super::source_freeze::SourceFreeze;
use super::{json, receipt};
use crate::cli::ResourceModule;

const HEADLESS_CI_RECEIPT: &str = "dist/evidence/headless-ci-qualification.json";

#[derive(Clone, Copy)]
struct ArtifactReceiptContract {
    module: ModuleId,
    label: &'static str,
    required_task: &'static str,
}

const ARTIFACT_RECEIPTS: [ArtifactReceiptContract; 2] = [
    ArtifactReceiptContract {
        module: ModuleId::Runtime,
        label: "runtime qualification",
        required_task: "copied Release Phase 8 close-to-tray/show lifecycle",
    },
    ArtifactReceiptContract {
        module: ModuleId::Performance,
        label: "performance qualification",
        required_task: "copied Release Phase 9 editor-ready cold/warm startup matrix",
    },
];

fn artifact_receipts() -> impl Iterator<Item = ArtifactReceiptContract> {
    ARTIFACT_RECEIPTS.into_iter().chain(
        crate::resource_plan::GROUPS
            .into_iter()
            .map(resource_contract),
    )
}

const fn resource_contract(group: ResourceModule) -> ArtifactReceiptContract {
    ArtifactReceiptContract {
        module: ModuleId::Resource(group),
        label: group.name(),
        required_task: group.task_label(),
    }
}

pub(super) fn check(
    root: &Path,
    source: &SourceFreeze,
    _candidate: &Candidate,
    blockers: &mut Vec<String>,
) -> bool {
    let before = blockers.len();
    check_source_receipt(root, source, blockers);
    for contract in artifact_receipts() {
        check_artifact_receipt(root, contract, blockers);
    }
    blockers.len() == before
}

fn check_source_receipt(root: &Path, source: &SourceFreeze, blockers: &mut Vec<String>) {
    let document = match receipt::read_receipt(&root.join(HEADLESS_CI_RECEIPT)) {
        Ok(document) => document,
        Err(error) => {
            blockers.push(format!("headless CI qualification receipt: {error}"));
            return;
        }
    };
    check_common(
        &document,
        "headless CI qualification",
        "all",
        "requested headless CI task set",
        &source.source_commit,
        blockers,
    );
}

fn check_artifact_receipt(
    root: &Path,
    contract: ArtifactReceiptContract,
    blockers: &mut Vec<String>,
) {
    let success = match module_ledger::compatible_success(root, contract.module) {
        Ok(Some(success)) => success,
        Ok(None) => {
            blockers.push(format!(
                "{} has no compatible last-success receipt for current module inputs",
                contract.label
            ));
            return;
        }
        Err(error) => {
            blockers.push(format!("{} last-success receipt: {error}", contract.label));
            return;
        }
    };
    let document = success.document;
    check_common(
        &document,
        contract.label,
        "phase-14",
        contract.required_task,
        &success.origin_source_commit,
        blockers,
    );
    match json::string_field(&document, "executable_sha256") {
        Ok(actual) if actual == success.origin_exe_sha256 => {}
        Ok(actual) => blockers.push(format!(
            "STALE RECEIPT: {} EXE hash is {actual}, expected {}",
            contract.label, success.origin_exe_sha256
        )),
        Err(error) => blockers.push(format!("{} EXE hash: {error}", contract.label)),
    }
}

fn check_common(
    document: &str,
    label: &str,
    suite: &str,
    required_task: &str,
    source_commit: &str,
    blockers: &mut Vec<String>,
) {
    match json::u64_field(document, "schema_version") {
        Ok(2) => {}
        Ok(version) => blockers.push(format!("{label} schema is {version}, expected 2")),
        Err(error) => blockers.push(format!("{label} schema: {error}")),
    }
    match json::string_field(document, "suite") {
        Ok(actual) if actual == suite => {}
        Ok(actual) => blockers.push(format!("{label} suite is {actual}, expected {suite}")),
        Err(error) => blockers.push(format!("{label} suite: {error}")),
    }
    match json::string_field(document, "commit") {
        Ok(actual) if actual == source_commit => {}
        Ok(actual) => blockers.push(format!(
            "STALE RECEIPT: {label} source commit is {actual}, expected {source_commit}"
        )),
        Err(error) => blockers.push(format!("{label} source commit: {error}")),
    }
    match json::bool_field(document, "worktree_dirty") {
        Ok(false) => {}
        Ok(true) => blockers.push(format!("{label} was recorded from a dirty tree")),
        Err(error) => blockers.push(format!("{label} worktree state: {error}")),
    }
    let count = document
        .match_indices(&format!("\"id\":\"{required_task}\""))
        .count();
    if count != 1 {
        blockers.push(format!(
            "{label} contains required task `{required_task}` {count} times, expected exactly once"
        ));
    }
    match json::result_status_values(document) {
        Ok(statuses)
            if !statuses.is_empty() && statuses.iter().all(|status| status == "PASSED") => {}
        Ok(statuses) => blockers.push(format!(
            "{label} contains a non-PASSED result: {statuses:?}"
        )),
        Err(error) => blockers.push(format!("{label} statuses: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::{ARTIFACT_RECEIPTS, HEADLESS_CI_RECEIPT, check};
    use crate::qualification::module_ledger;
    use crate::qualification::module_registry::ModuleId;
    use crate::qualification::receipt::{self, Candidate, RELEASE_ARTIFACT_NAME};
    use crate::qualification::source_freeze::SourceFreeze;
    use std::fs;
    use std::process::Command;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn source_only_headless_can_differ_from_final_exe_but_artifact_receipts_cannot() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-automated-receipts-{nonce}"));
        let source = source();
        let candidate = candidate();
        fs::create_dir_all(&root).expect("fixture root");
        assert!(
            Command::new("git")
                .arg("init")
                .arg("--quiet")
                .current_dir(&root)
                .status()
                .expect("git init")
                .success()
        );
        let headless = receipt_document(
            &source.source_commit,
            "f".repeat(64).as_str(),
            "all",
            "requested headless CI task set",
        );
        receipt::write_receipt(&root, HEADLESS_CI_RECEIPT, &headless).expect("headless");
        for contract in super::artifact_receipts() {
            let document = module_document(&root, contract, &candidate.exe_sha256);
            receipt::write_receipt(&root, contract.module.receipt(), &document)
                .expect("artifact receipt");
            module_ledger::record_success(&root, contract.module, &candidate)
                .expect("record module success");
        }
        let mut blockers = Vec::new();
        assert!(check(&root, &source, &candidate, &mut blockers));

        // A window sentinel (or aggregate PASS) cannot stand in for any missing group.
        for group in crate::resource_plan::GROUPS {
            let module = ModuleId::Resource(group);
            let path = module_ledger::store::LedgerStore::for_repository(&root)
                .unwrap()
                .module_record(
                    group.ledger_id(),
                    &module_ledger::fingerprint::calculate(&root, module).unwrap(),
                )
                .unwrap();
            let saved = fs::read(&path).unwrap();
            fs::remove_file(&path).unwrap();
            blockers.clear();
            assert!(!check(&root, &source, &candidate, &mut blockers));
            assert!(blockers.iter().any(|item| item.contains(group.name())));
            crate::atomic_evidence::write(&path, &saved).unwrap();
        }

        let stale = module_document(&root, ARTIFACT_RECEIPTS[0], &"0".repeat(64));
        receipt::write_receipt(&root, ARTIFACT_RECEIPTS[0].module.receipt(), &stale)
            .expect("stale");
        module_ledger::record_success(&root, ARTIFACT_RECEIPTS[0].module, &candidate)
            .expect("record stale module receipt");
        blockers.clear();
        assert!(!check(&root, &source, &candidate, &mut blockers));
        assert!(blockers.iter().any(|item| item.contains("EXE hash")));
        fs::remove_dir_all(root).expect("cleanup");
    }

    fn module_document(
        root: &std::path::Path,
        contract: super::ArtifactReceiptContract,
        exe: &str,
    ) -> String {
        use crate::qualification::module_registry::ModuleId;
        if let ModuleId::Resource(group) = contract.module {
            let result = crate::resource_plan::tests::valid_resource_result(group);
            let base = crate::resource_plan::tests::document(group, &result);
            let input = module_ledger::fingerprint::calculate(root, contract.module).unwrap();
            return format!(
                "{{\"resource_input_fingerprint\":\"{input}\",{}",
                &base[1..]
            );
        }
        let tasks =
            crate::runner::formal_task_labels(contract.module == ModuleId::Runtime).unwrap();
        let results = tasks
            .into_iter()
            .map(|id| crate::evidence::EvidenceResult {
                id: id.into(),
                status: crate::evidence::EvidenceStatus::Passed,
                detail: None,
                measurements: Vec::new(),
                gates: Vec::new(),
                samples: Vec::new(),
            })
            .collect::<Vec<_>>();
        crate::evidence::render_json(
            &"a".repeat(40),
            false,
            None,
            Some(exe),
            "phase-14",
            &results,
            None,
        )
    }

    fn source() -> SourceFreeze {
        SourceFreeze {
            source_commit: "a".repeat(40),
            version: "0.1.0".to_owned(),
            cargo_lock_sha256: "b".repeat(64),
            rustc: "rustc test".to_owned(),
            target: "x86_64-pc-windows-msvc".to_owned(),
            remote_synced: true,
        }
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

    fn receipt_document(commit: &str, exe: &str, suite: &str, task: &str) -> String {
        format!(
            concat!(
                "{{\"schema_version\":2,\"commit\":\"{}\",\"worktree_dirty\":false,",
                "\"artifact_sha256\":null,\"executable_sha256\":\"{}\",",
                "\"suite\":\"{}\",\"qualification_environment\":{{\"status\":\"VALID\"}},",
                "\"results\":[{{\"id\":\"{}\",\"status\":\"PASSED\"}}]}}"
            ),
            commit, exe, suite, task
        )
    }
}
