//! Fail-closed release-readiness projection over exact evidence receipts.

use std::path::Path;

use super::receipt::{self, Candidate};
use super::{decisions, json};

const AUTOMATED_RECEIPT: &str = "dist/evidence/automated-qualification.json";
const HEADLESS_CI_RECEIPT: &str = "dist/evidence/headless-ci-qualification.json";
const PERFORMANCE_RECEIPT: &str = "dist/evidence/performance-qualification.json";
const RUNTIME_RECEIPT: &str = "dist/evidence/runtime-qualification.json";
const RESOURCES_RECEIPT: &str = "dist/evidence/resources-qualification.json";
const MANUAL_RECEIPT: &str = "dist/evidence/manual-acceptance.json";
const REMOTE_RECEIPT: &str = "dist/evidence/remote-workflow.json";
const DOWNLOADED_RECEIPT: &str = "dist/evidence/downloaded-artifact-smoke.json";
const READINESS_RECEIPT: &str = "dist/evidence/release-readiness.json";

#[derive(Clone, Copy)]
struct AutomatedReceiptContract {
    path: &'static str,
    label: &'static str,
    suite: &'static str,
    required_task: &'static str,
    binds_artifact: bool,
}

const AUTOMATED_RECEIPTS: [AutomatedReceiptContract; 5] = [
    AutomatedReceiptContract {
        path: AUTOMATED_RECEIPT,
        label: "release qualification",
        suite: "phase-13",
        required_task: "portable package verification",
        binds_artifact: true,
    },
    AutomatedReceiptContract {
        path: HEADLESS_CI_RECEIPT,
        label: "headless CI qualification",
        suite: "all",
        required_task: "requested headless CI task set",
        binds_artifact: false,
    },
    AutomatedReceiptContract {
        path: PERFORMANCE_RECEIPT,
        label: "performance qualification",
        suite: "phase-13",
        required_task: "copied Release Phase 9 editor-ready cold/warm startup matrix",
        binds_artifact: false,
    },
    AutomatedReceiptContract {
        path: RUNTIME_RECEIPT,
        label: "runtime qualification",
        suite: "phase-13",
        required_task: "copied Release Phase 8 close-to-tray/show lifecycle",
        binds_artifact: false,
    },
    AutomatedReceiptContract {
        path: RESOURCES_RECEIPT,
        label: "resource qualification",
        suite: "phase-13",
        required_task: "copied Release Phase 8 hidden-window resource matrix",
        binds_artifact: false,
    },
];

pub(super) fn evaluate(root: &Path, explain: bool) -> Result<(), String> {
    let mut blockers = Vec::new();
    let candidate = match receipt::read_candidate(root) {
        Ok(candidate) => {
            if let Err(error) = receipt::validate_candidate_against_repository(root, &candidate) {
                blockers.push(error);
            }
            Some(candidate)
        }
        Err(error) => {
            blockers.push(format!("release candidate receipt: {error}"));
            None
        }
    };
    let decisions = candidate
        .as_ref()
        .map(|candidate| decisions::read(root, candidate))
        .transpose()
        .unwrap_or_else(|error| {
            blockers.push(error);
            None
        })
        .unwrap_or_default();
    require_decision(
        &decisions,
        "WARM-STARTUP-GATE",
        "USER APPROVED",
        &mut blockers,
    );
    require_decision(
        &decisions,
        "RELEASE-VERSION",
        "USER APPROVED",
        &mut blockers,
    );
    require_decision(
        &decisions,
        "UNSIGNED-POLICY",
        "USER APPROVED",
        &mut blockers,
    );
    if let Some(candidate) = &candidate {
        check_automated(root, candidate, &mut blockers);
        check_manual(root, candidate, &decisions, &mut blockers);
        check_bound_receipt(root, REMOTE_RECEIPT, candidate, true, &mut blockers);
        check_downloaded(root, candidate, &mut blockers);
    }
    let document = render_readiness(candidate.as_ref(), &blockers);
    receipt::write_receipt(root, READINESS_RECEIPT, &document)?;
    if explain || !blockers.is_empty() {
        if blockers.is_empty() {
            println!("Release readiness: READY");
        } else {
            println!(
                "Release readiness: NOT_READY ({} blocker(s))",
                blockers.len()
            );
            for blocker in &blockers {
                println!("- {blocker}");
            }
        }
    }
    if blockers.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "release is NOT_READY; see {}",
            root.join(READINESS_RECEIPT).display()
        ))
    }
}

fn check_automated(root: &Path, candidate: &Candidate, blockers: &mut Vec<String>) {
    for contract in AUTOMATED_RECEIPTS {
        check_automated_receipt(root, candidate, contract, blockers);
    }
}

fn check_automated_receipt(
    root: &Path,
    candidate: &Candidate,
    contract: AutomatedReceiptContract,
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
    let checks = [
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
    ];
    for (actual, expected, label) in checks {
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
    let task_marker = format!("\"id\":\"{}\"", contract.required_task);
    let task_occurrences = document.match_indices(&task_marker).count();
    if task_occurrences != 1 {
        blockers.push(format!(
            "{} contains required task `{}` {task_occurrences} times, expected exactly once",
            contract.label, contract.required_task,
        ));
    }
    match json::status_values(&document) {
        Ok(statuses)
            if !statuses.is_empty() && statuses.iter().all(|status| status == "PASSED") => {}
        Ok(statuses) => blockers.push(format!(
            "{} contains a non-PASSED result: {statuses:?}",
            contract.label
        )),
        Err(error) => blockers.push(format!("{} statuses: {error}", contract.label)),
    }
}

fn check_manual(
    root: &Path,
    candidate: &Candidate,
    decisions: &[decisions::Decision],
    blockers: &mut Vec<String>,
) {
    let document = match receipt::read_receipt(&root.join(MANUAL_RECEIPT)) {
        Ok(document) => document,
        Err(error) => {
            blockers.push(format!("mandatory manual acceptance receipt: {error}"));
            return;
        }
    };
    match json::u64_field(&document, "schema_version") {
        Ok(1) => {}
        Ok(version) => blockers.push(format!("manual receipt schema is {version}, expected 1")),
        Err(error) => blockers.push(format!("manual receipt schema: {error}")),
    }
    check_identity(&document, candidate, "manual", blockers);
    match json::string_field(&document, "zip_sha256") {
        Ok(hash) if hash == candidate.zip_sha256 => {}
        Ok(hash) => blockers.push(format!(
            "STALE RECEIPT: manual zip_sha256 is {hash}, expected {}",
            candidate.zip_sha256
        )),
        Err(error) => blockers.push(format!("manual ZIP identity: {error}")),
    }
    let cases = match manual_case_statuses(&document) {
        Ok(cases) if !cases.is_empty() => cases,
        Ok(_) => {
            blockers.push("manual receipt contains no cases".to_owned());
            return;
        }
        Err(error) => {
            blockers.push(format!("manual receipt statuses: {error}"));
            return;
        }
    };
    let observed_ids: Vec<_> = cases.iter().map(|case| case.case_id.as_str()).collect();
    let expected_ids: Vec<_> = (1..=44).map(|number| format!("P12-M{number:02}")).collect();
    if observed_ids != expected_ids.iter().map(String::as_str).collect::<Vec<_>>() {
        blockers.push(format!(
            "manual receipt cases must be exactly P12-M01..P12-M44; observed {observed_ids:?}"
        ));
        return;
    }
    for case in cases {
        if case.source_commit != candidate.source_commit || case.exe_sha256 != candidate.exe_sha256
        {
            blockers.push(format!(
                "STALE RECEIPT: {} carries a different source/EXE identity",
                case.case_id
            ));
            continue;
        }
        match case.status.as_str() {
            "MANUAL_PASS" => {}
            "MANUAL_FAIL" => blockers.push(format!("manual acceptance {} failed", case.case_id)),
            "NOT_TESTED" => {
                let waiver = format!("WAIVER-{}", case.case_id);
                if decision_status(decisions, &waiver) != Some("USER APPROVED") {
                    blockers.push(format!(
                        "mandatory manual acceptance {} is NOT_TESTED without its own USER waiver",
                        case.case_id
                    ));
                }
            }
            _ => blockers.push(format!(
                "manual acceptance {} contains invalid status {}",
                case.case_id, case.status
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ManualObservation {
    case_id: String,
    status: String,
    source_commit: String,
    exe_sha256: String,
}

fn manual_case_statuses(document: &str) -> Result<Vec<ManualObservation>, String> {
    let marker = "\"case_id\":";
    let mut cases = Vec::new();
    let mut offset = 0usize;
    while let Some(relative) = document[offset..].find(marker) {
        let start = offset + relative;
        let next = document[start + marker.len()..]
            .find(marker)
            .map_or(document.len(), |next| start + marker.len() + next);
        let object = &document[start..next];
        cases.push(ManualObservation {
            case_id: json::string_field(object, "case_id")?,
            status: json::string_field(object, "status")?,
            source_commit: json::string_field(object, "source_commit")?,
            exe_sha256: json::string_field(object, "exe_sha256")?,
        });
        offset = next;
    }
    Ok(cases)
}

fn check_bound_receipt(
    root: &Path,
    relative: &str,
    candidate: &Candidate,
    require_success: bool,
    blockers: &mut Vec<String>,
) {
    let document = match receipt::read_receipt(&root.join(relative)) {
        Ok(document) => document,
        Err(error) => {
            blockers.push(format!(
                "{}: {error}",
                relative.replace("dist/evidence/", "")
            ));
            return;
        }
    };
    match json::u64_field(&document, "schema_version") {
        Ok(1) => {}
        Ok(version) => blockers.push(format!("{relative} schema is {version}, expected 1")),
        Err(error) => blockers.push(format!("{relative} schema: {error}")),
    }
    check_identity(&document, candidate, relative, blockers);
    if require_success {
        match json::string_field(&document, "status") {
            Ok(status) if status == "PASSED" => {}
            Ok(status) => blockers.push(format!("{relative} status is {status}")),
            Err(error) => blockers.push(format!("{relative}: {error}")),
        }
    }
}

fn check_downloaded(root: &Path, candidate: &Candidate, blockers: &mut Vec<String>) {
    check_bound_receipt(root, DOWNLOADED_RECEIPT, candidate, true, blockers);
    let document = match receipt::read_receipt(&root.join(DOWNLOADED_RECEIPT)) {
        Ok(document) => document,
        Err(_) => return,
    };
    match json::string_field(&document, "zip_sha256") {
        Ok(actual) if actual == candidate.zip_sha256 => {}
        Ok(actual) => blockers.push(format!(
            "STALE RECEIPT: downloaded ZIP hash is {actual}, expected {}",
            candidate.zip_sha256
        )),
        Err(error) => blockers.push(format!("downloaded ZIP identity: {error}")),
    }
    match json::bool_field(&document, "runtime_smoke") {
        Ok(true) => {}
        Ok(false) => blockers.push("downloaded artifact runtime smoke did not pass".to_owned()),
        Err(error) => blockers.push(format!("downloaded runtime smoke: {error}")),
    }
}

fn check_identity(document: &str, candidate: &Candidate, label: &str, blockers: &mut Vec<String>) {
    for (key, expected) in [
        ("source_commit", candidate.source_commit.as_str()),
        ("exe_sha256", candidate.exe_sha256.as_str()),
    ] {
        match json::string_field(document, key) {
            Ok(actual) if actual == expected => {}
            Ok(actual) => blockers.push(format!(
                "STALE RECEIPT: {label} {key} is {actual}, expected {expected}"
            )),
            Err(error) => blockers.push(format!("{label} {key}: {error}")),
        }
    }
}

fn decision_status<'a>(decisions: &'a [decisions::Decision], key: &str) -> Option<&'a str> {
    decisions::status(decisions, key)
}

fn require_decision(
    decisions: &[decisions::Decision],
    key: &str,
    required: &str,
    blockers: &mut Vec<String>,
) {
    match decision_status(decisions, key) {
        Some(status) if status == required => {}
        Some(status) => blockers.push(format!("USER decision {key} is {status}")),
        None => blockers.push(format!("USER decision {key} is missing")),
    }
}

fn render_readiness(candidate: Option<&Candidate>, blockers: &[String]) -> String {
    let source = candidate.map_or("UNKNOWN", |value| value.source_commit.as_str());
    let exe = candidate.map_or("UNKNOWN", |value| value.exe_sha256.as_str());
    let zip = candidate.map_or("UNKNOWN", |value| value.zip_sha256.as_str());
    let status = if blockers.is_empty() {
        "READY"
    } else {
        "NOT_READY"
    };
    let mut output = format!(
        "{{\"schema_version\":1,\"status\":\"{status}\",\"source_commit\":\"{}\",\"exe_sha256\":\"{}\",\"zip_sha256\":\"{}\",\"blockers\":[",
        json::escape(source),
        json::escape(exe),
        json::escape(zip),
    );
    for (index, blocker) in blockers.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!("\"{}\"", json::escape(blocker)));
    }
    output.push_str("]}\n");
    output
}

#[cfg(test)]
mod tests {
    use super::{
        AUTOMATED_RECEIPT, AUTOMATED_RECEIPTS, DOWNLOADED_RECEIPT, ManualObservation,
        check_automated, check_downloaded, decision_status, manual_case_statuses, render_readiness,
    };
    use crate::qualification::decisions::Decision;
    use crate::qualification::receipt::{self, Candidate};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn decision_ledger_accepts_only_explicit_user_states() {
        let decisions = vec![Decision {
            key: "WARM-STARTUP-GATE".to_owned(),
            status: "USER APPROVED".to_owned(),
            evidence: "USER message".to_owned(),
        }];
        assert_eq!(
            decision_status(&decisions, "WARM-STARTUP-GATE"),
            Some("USER APPROVED")
        );
    }

    #[test]
    fn readiness_receipt_is_fail_closed_and_explains_blockers() {
        let json = render_readiness(None, &["manual missing".to_owned()]);
        assert!(json.contains("\"status\":\"NOT_READY\""));
        assert!(json.contains("manual missing"));
    }

    #[test]
    fn automated_readiness_requires_every_exact_local_qualification_mode() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-automated-receipts-{nonce}"));
        let candidate = Candidate {
            source_commit: "a".repeat(40),
            version: "0.1.0".to_owned(),
            cargo_lock_sha256: "b".repeat(64),
            exe_sha256: "c".repeat(64),
            zip_sha256: "d".repeat(64),
            sbom_sha256: "e".repeat(64),
            rustc: "rustc test".to_owned(),
            target: "x86_64-pc-windows-msvc".to_owned(),
            remote_synced: false,
        };
        let release = automated_receipt(
            &candidate,
            "phase-13",
            "portable package verification",
            Some(&candidate.zip_sha256),
        );
        receipt::write_receipt(&root, AUTOMATED_RECEIPT, &release).expect("write receipt");

        let mut blockers = Vec::new();
        check_automated(&root, &candidate, &mut blockers);
        assert_eq!(
            blockers
                .iter()
                .filter(|blocker| blocker.contains("receipt: cannot read"))
                .count(),
            AUTOMATED_RECEIPTS.len() - 1
        );

        for contract in AUTOMATED_RECEIPTS.into_iter().skip(1) {
            let document =
                automated_receipt(&candidate, contract.suite, contract.required_task, None);
            receipt::write_receipt(&root, contract.path, &document).expect("write receipt");
        }
        blockers.clear();
        check_automated(&root, &candidate, &mut blockers);
        assert!(blockers.is_empty(), "unexpected blockers: {blockers:?}");
        fs::remove_dir_all(root).expect("cleanup");
    }

    fn automated_receipt(
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
                "\"suite\":\"{}\",\"results\":[{{\"id\":\"{}\",",
                "\"status\":\"PASSED\"}}]}}\n"
            ),
            candidate.source_commit, artifact, candidate.exe_sha256, suite, task
        )
    }

    #[test]
    fn manual_statuses_remain_bound_to_specific_case_ids() {
        let document = concat!(
            "{\"cases\":[",
            "{\"case_id\":\"P12-M01\",\"status\":\"MANUAL_PASS\",\"source_commit\":\"a\",\"exe_sha256\":\"b\"},",
            "{\"case_id\":\"P12-M02\",\"status\":\"NOT_TESTED\",\"source_commit\":\"a\",\"exe_sha256\":\"b\"}]}",
        );
        assert_eq!(
            manual_case_statuses(document),
            Ok(vec![
                ManualObservation {
                    case_id: "P12-M01".to_owned(),
                    status: "MANUAL_PASS".to_owned(),
                    source_commit: "a".to_owned(),
                    exe_sha256: "b".to_owned(),
                },
                ManualObservation {
                    case_id: "P12-M02".to_owned(),
                    status: "NOT_TESTED".to_owned(),
                    source_commit: "a".to_owned(),
                    exe_sha256: "b".to_owned(),
                },
            ])
        );
    }

    #[test]
    fn downloaded_receipt_must_bind_zip_and_runtime_smoke() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-downloaded-receipt-{nonce}"));
        let candidate = Candidate {
            source_commit: "a".repeat(40),
            version: "0.1.0".to_owned(),
            cargo_lock_sha256: "b".repeat(64),
            exe_sha256: "c".repeat(64),
            zip_sha256: "d".repeat(64),
            sbom_sha256: "e".repeat(64),
            rustc: "rustc test".to_owned(),
            target: "x86_64-pc-windows-msvc".to_owned(),
            remote_synced: false,
        };
        let stale = format!(
            concat!(
                "{{\"schema_version\":1,\"status\":\"PASSED\",",
                "\"source_commit\":\"{}\",\"exe_sha256\":\"{}\",",
                "\"zip_sha256\":\"{}\",\"runtime_smoke\":false}}\n"
            ),
            candidate.source_commit,
            candidate.exe_sha256,
            "f".repeat(64),
        );
        receipt::write_receipt(&root, DOWNLOADED_RECEIPT, &stale).expect("write receipt");
        let mut blockers = Vec::new();
        check_downloaded(&root, &candidate, &mut blockers);
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("downloaded ZIP hash"))
        );
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("runtime smoke did not pass"))
        );
        fs::remove_dir_all(root).expect("cleanup");
    }
}
