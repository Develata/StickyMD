//! Fail-closed release-readiness projection over exact evidence receipts.

use std::path::Path;

use super::receipt::{self, Candidate};
use super::{
    automated_readiness, decisions, g3_readiness, g4_readiness, g5_readiness, json,
    manual_readiness,
};

const REMOTE_RECEIPT: &str = "dist/evidence/remote-workflow.json";
const DOWNLOADED_RECEIPT: &str = "dist/evidence/downloaded-artifact-smoke.json";
const READINESS_RECEIPT: &str = "dist/evidence/release-readiness.json";

pub(super) fn evaluate(root: &Path, explain: bool) -> Result<(), String> {
    let mut blockers = Vec::new();
    let mut remote_pending = Vec::new();
    let candidate = match receipt::read_candidate(root) {
        Ok(candidate) => {
            if let Err(error) = receipt::validate_candidate_against_repository(root, &candidate) {
                blockers.push(error);
            }
            if candidate.version != "0.1.0" {
                blockers.push(format!(
                    "candidate version is {}, expected USER-approved 0.1.0",
                    candidate.version
                ));
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
        "STARTUP-RELEASE-BOUNDARY",
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
        "MANUAL-RISK-POLICY",
        "USER APPROVED",
        &mut blockers,
    );
    require_decision(
        &decisions,
        "UNSIGNED-POLICY",
        "USER APPROVED",
        &mut blockers,
    );
    require_decision(
        &decisions,
        "INDEPENDENT-EVIDENCE-COLLECTION",
        "USER APPROVED",
        &mut blockers,
    );
    if let Some(candidate) = &candidate {
        let automated_ok = automated_readiness::check(root, candidate, &mut blockers);
        g3_readiness::check(root, candidate, &mut blockers);
        g4_readiness::check(root, candidate, &mut blockers);
        g5_readiness::check(root, candidate, &mut blockers);
        manual_readiness::check(root, candidate, &decisions, automated_ok, &mut blockers);
        check_optional_remote(root, candidate, &mut remote_pending);
    }
    let document = render_readiness(candidate.as_ref(), &blockers, &remote_pending);
    receipt::write_receipt(root, READINESS_RECEIPT, &document)?;
    if explain || !blockers.is_empty() {
        if blockers.is_empty() && remote_pending.is_empty() {
            println!("Release readiness: REMOTE_READY");
        } else if blockers.is_empty() {
            println!("Release readiness: LOCAL_READY (remote evidence pending)");
            for pending in &remote_pending {
                println!("- {pending}");
            }
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

fn check_optional_remote(root: &Path, candidate: &Candidate, pending: &mut Vec<String>) {
    if !root.join(REMOTE_RECEIPT).is_file() {
        pending.push("remote workflow receipt is pending USER push authorization".to_owned());
    } else {
        check_bound_receipt(root, REMOTE_RECEIPT, candidate, true, pending);
    }
    if !root.join(DOWNLOADED_RECEIPT).is_file() {
        pending.push("downloaded artifact receipt is pending remote qualification".to_owned());
    } else {
        check_downloaded(root, candidate, pending);
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

fn render_readiness(
    candidate: Option<&Candidate>,
    blockers: &[String],
    remote_pending: &[String],
) -> String {
    let source = candidate.map_or("UNKNOWN", |value| value.source_commit.as_str());
    let exe = candidate.map_or("UNKNOWN", |value| value.exe_sha256.as_str());
    let zip = candidate.map_or("UNKNOWN", |value| value.zip_sha256.as_str());
    let status = if !blockers.is_empty() {
        "NOT_READY"
    } else if remote_pending.is_empty() {
        "REMOTE_READY"
    } else {
        "LOCAL_READY"
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
    output.push_str("],\"remote_pending\":[");
    for (index, pending) in remote_pending.iter().enumerate() {
        if index > 0 {
            output.push(',');
        }
        output.push_str(&format!("\"{}\"", json::escape(pending)));
    }
    output.push_str("]}\n");
    output
}

#[cfg(test)]
mod tests {
    use super::{DOWNLOADED_RECEIPT, check_downloaded, decision_status, render_readiness};
    use crate::qualification::decisions::Decision;
    use crate::qualification::receipt::{self, Candidate};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn decision_ledger_accepts_only_explicit_user_states() {
        let decisions = vec![Decision {
            key: "STARTUP-RELEASE-BOUNDARY".to_owned(),
            status: "USER APPROVED".to_owned(),
            evidence: "USER message".to_owned(),
        }];
        assert_eq!(
            decision_status(&decisions, "STARTUP-RELEASE-BOUNDARY"),
            Some("USER APPROVED")
        );
    }

    #[test]
    fn readiness_receipt_is_fail_closed_and_explains_blockers() {
        let json = render_readiness(None, &["manual missing".to_owned()], &[]);
        assert!(json.contains("\"status\":\"NOT_READY\""));
        assert!(json.contains("manual missing"));
    }

    #[test]
    fn local_readiness_is_distinct_from_remote_qualification() {
        let json = render_readiness(
            None,
            &[],
            &["remote workflow receipt is pending".to_owned()],
        );
        assert!(json.contains("\"status\":\"LOCAL_READY\""));
        assert!(json.contains("remote workflow receipt is pending"));
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
