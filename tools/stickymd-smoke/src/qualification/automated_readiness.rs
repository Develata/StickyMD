//! Exact-candidate validation for independently collected automated receipts.

use std::path::Path;

use super::receipt::Candidate;
use super::{json, receipt};

const AUTOMATED_RECEIPT: &str = "dist/evidence/automated-qualification.json";
const HEADLESS_CI_RECEIPT: &str = "dist/evidence/headless-ci-qualification.json";
const PERFORMANCE_RECEIPT: &str = "dist/evidence/performance-qualification.json";
const RUNTIME_RECEIPT: &str = "dist/evidence/runtime-qualification.json";
const RESOURCES_RECEIPT: &str = "dist/evidence/resources-qualification.json";

#[derive(Clone, Copy)]
struct ReceiptContract {
    path: &'static str,
    label: &'static str,
    suite: &'static str,
    required_task: &'static str,
    binds_artifact: bool,
}

const RECEIPTS: [ReceiptContract; 5] = [
    ReceiptContract {
        path: AUTOMATED_RECEIPT,
        label: "release qualification",
        suite: "phase-14",
        required_task: "portable package verification",
        binds_artifact: true,
    },
    ReceiptContract {
        path: HEADLESS_CI_RECEIPT,
        label: "headless CI qualification",
        suite: "all",
        required_task: "requested headless CI task set",
        binds_artifact: false,
    },
    ReceiptContract {
        path: PERFORMANCE_RECEIPT,
        label: "performance qualification",
        suite: "phase-14",
        required_task: "copied Release Phase 9 editor-ready cold/warm startup matrix",
        binds_artifact: false,
    },
    ReceiptContract {
        path: RUNTIME_RECEIPT,
        label: "runtime qualification",
        suite: "phase-14",
        required_task: "copied Release Phase 8 close-to-tray/show lifecycle",
        binds_artifact: false,
    },
    ReceiptContract {
        path: RESOURCES_RECEIPT,
        label: "resource qualification",
        suite: "phase-14",
        required_task: "copied Release Phase 8 hidden-window resource matrix",
        binds_artifact: false,
    },
];

pub(super) fn check(root: &Path, candidate: &Candidate, blockers: &mut Vec<String>) -> bool {
    let before = blockers.len();
    for contract in RECEIPTS {
        check_receipt(root, candidate, contract, blockers);
    }
    blockers.len() == before
}

fn check_receipt(
    root: &Path,
    candidate: &Candidate,
    contract: ReceiptContract,
    blockers: &mut Vec<String>,
) {
    let document = match receipt::read_receipt(&root.join(contract.path)) {
        Ok(document) => document,
        Err(error) => {
            blockers.push(format!("{} receipt: {error}", contract.label));
            return;
        }
    };
    match json::u64_field(&document, "schema_version") {
        Ok(2) => {}
        Ok(version) => blockers.push(format!(
            "{} schema is {version}, expected 2",
            contract.label
        )),
        Err(error) => blockers.push(format!("{} schema: {error}", contract.label)),
    }
    match json::string_field(&document, "suite") {
        Ok(suite) if suite == contract.suite => {}
        Ok(suite) => blockers.push(format!(
            "{} suite is {suite}, expected {}",
            contract.label, contract.suite
        )),
        Err(error) => blockers.push(format!("{} suite: {error}", contract.label)),
    }
    for (actual, expected, label) in [
        (
            json::string_field(&document, "commit"),
            candidate.source_commit.as_str(),
            "source commit",
        ),
        (
            json::string_field(&document, "executable_sha256"),
            candidate.exe_sha256.as_str(),
            "EXE hash",
        ),
    ] {
        match actual {
            Ok(actual) if actual == expected => {}
            Ok(actual) => blockers.push(format!(
                "STALE RECEIPT: {} {label} is {actual}, expected {expected}",
                contract.label
            )),
            Err(error) => blockers.push(format!("{} {label}: {error}", contract.label)),
        }
    }
    if contract.binds_artifact {
        match json::string_field(&document, "artifact_sha256") {
            Ok(actual) if actual == candidate.zip_sha256 => {}
            Ok(actual) => blockers.push(format!(
                "STALE RECEIPT: {} ZIP hash is {actual}, expected {}",
                contract.label, candidate.zip_sha256
            )),
            Err(error) => blockers.push(format!("{} ZIP hash: {error}", contract.label)),
        }
    }
    match json::bool_field(&document, "worktree_dirty") {
        Ok(false) => {}
        Ok(true) => blockers.push(format!("{} was recorded from a dirty tree", contract.label)),
        Err(error) => blockers.push(format!("{} worktree state: {error}", contract.label)),
    }
    let task_occurrences = document
        .match_indices(&format!("\"id\":\"{}\"", contract.required_task))
        .count();
    if task_occurrences != 1 {
        blockers.push(format!(
            "{} contains required task `{}` {task_occurrences} times, expected exactly once",
            contract.label, contract.required_task,
        ));
    }
    match json::result_status_values(&document) {
        Ok(statuses)
            if !statuses.is_empty() && statuses.iter().all(|status| status == "PASSED") => {}
        Ok(statuses) => blockers.push(format!(
            "{} contains a non-PASSED result: {statuses:?}",
            contract.label
        )),
        Err(error) => blockers.push(format!("{} statuses: {error}", contract.label)),
    }
}

#[cfg(test)]
mod tests {
    use super::{AUTOMATED_RECEIPT, RECEIPTS, check};
    use crate::qualification::receipt::{self, Candidate};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn every_exact_local_qualification_mode_is_required() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-automated-receipts-{nonce}"));
        let candidate = candidate();
        let release = receipt_document(
            &candidate,
            "phase-14",
            "portable package verification",
            Some(&candidate.zip_sha256),
        );
        receipt::write_receipt(&root, AUTOMATED_RECEIPT, &release).expect("write receipt");

        let mut blockers = Vec::new();
        check(&root, &candidate, &mut blockers);
        assert_eq!(
            blockers
                .iter()
                .filter(|blocker| blocker.contains("receipt: cannot read"))
                .count(),
            RECEIPTS.len() - 1
        );

        for contract in RECEIPTS.into_iter().skip(1) {
            let document =
                receipt_document(&candidate, contract.suite, contract.required_task, None);
            receipt::write_receipt(&root, contract.path, &document).expect("write receipt");
        }
        blockers.clear();
        check(&root, &candidate, &mut blockers);
        assert!(blockers.is_empty(), "unexpected blockers: {blockers:?}");
        fs::remove_dir_all(root).expect("cleanup");
    }

    fn candidate() -> Candidate {
        Candidate {
            source_commit: "a".repeat(40),
            version: "0.1.0".to_owned(),
            cargo_lock_sha256: "b".repeat(64),
            exe_sha256: "c".repeat(64),
            zip_sha256: "d".repeat(64),
            sbom_sha256: "e".repeat(64),
            rustc: "rustc test".to_owned(),
            target: "x86_64-pc-windows-msvc".to_owned(),
            remote_synced: false,
        }
    }

    fn receipt_document(
        candidate: &Candidate,
        suite: &str,
        task: &str,
        artifact: Option<&str>,
    ) -> String {
        let artifact = artifact.map_or_else(|| "null".to_owned(), |hash| format!("\"{hash}\""));
        format!(
            concat!(
                "{{\"schema_version\":2,\"commit\":\"{}\",\"worktree_dirty\":false,",
                "\"artifact_sha256\":{},\"executable_sha256\":\"{}\",",
                "\"suite\":\"{}\",\"qualification_environment\":{{\"status\":\"VALID\"}},",
                "\"results\":[{{\"id\":\"{}\",",
                "\"status\":\"PASSED\"}}]}}\n"
            ),
            candidate.source_commit, artifact, candidate.exe_sha256, suite, task
        )
    }
}
