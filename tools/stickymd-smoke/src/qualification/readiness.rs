//! Fail-closed release readiness over Source Freeze and promoted exact evidence.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::path::Path;

use super::receipt::{self, Candidate};
use super::{
    automated_readiness, decisions, g3_readiness, g4_readiness, g5_readiness, json,
    manual_readiness, module_ledger, remote, source_freeze,
};

const READINESS_RECEIPT: &str = "dist/evidence/release-readiness.json";

pub(super) fn evaluate(root: &Path, explain: bool) -> Result<(), String> {
    let mut blockers = Vec::new();
    let source = match source_freeze::read(root) {
        Ok(source) => {
            if let Err(error) = source_freeze::validate_against_repository(root, &source) {
                blockers.push(error);
            }
            Some(source)
        }
        Err(error) => {
            blockers.push(format!("Source Freeze receipt: {error}"));
            None
        }
    };
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
            blockers.push(format!("promoted release candidate receipt: {error}"));
            None
        }
    };
    if let (Some(source), Some(candidate)) = (&source, &candidate)
        && (source.source_commit != candidate.source_commit
            || source.cargo_lock_sha256 != candidate.cargo_lock_sha256
            || source.version != candidate.version)
    {
        blockers.push("promoted candidate identity differs from Source Freeze".to_owned());
    }

    let release_decisions = source
        .as_ref()
        .map(|source| decisions::read(root, source))
        .transpose()
        .unwrap_or_else(|error| {
            blockers.push(error);
            None
        })
        .unwrap_or_default();
    for key in [
        "STARTUP-RELEASE-BOUNDARY",
        "RELEASE-VERSION",
        "MANUAL-RISK-POLICY",
        "UNSIGNED-POLICY",
        "INDEPENDENT-EVIDENCE-COLLECTION",
    ] {
        require_decision(&release_decisions, key, "USER APPROVED", &mut blockers);
    }
    if let (Some(source), Some(candidate)) = (&source, &candidate) {
        check_remote(root, source, candidate, &mut blockers);
        check_downloaded(root, candidate, &mut blockers);
        let automated_ok = automated_readiness::check(root, source, candidate, &mut blockers);
        g3_readiness::check(root, candidate, &mut blockers);
        g4_readiness::check(root, candidate, &mut blockers);
        g5_readiness::check(root, candidate, &mut blockers);
        manual_readiness::check(
            root,
            candidate,
            &release_decisions,
            automated_ok,
            &mut blockers,
        );
    }
    let document = render_readiness(candidate.as_ref(), &blockers);
    receipt::write_receipt(root, READINESS_RECEIPT, &document)?;
    module_ledger::print_status_for_candidate(root, candidate.as_ref())?;
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

fn check_remote(
    root: &Path,
    source: &source_freeze::SourceFreeze,
    candidate: &Candidate,
    blockers: &mut Vec<String>,
) {
    let document = match receipt::read_receipt(&root.join(remote::REMOTE_RECEIPT)) {
        Ok(document) => document,
        Err(error) => {
            blockers.push(format!("remote workflow receipt: {error}"));
            return;
        }
    };
    expect_u64(&document, "schema_version", 2, "remote workflow", blockers);
    expect_string(&document, "status", "PASSED", "remote workflow", blockers);
    for (key, expected) in [
        ("source_commit", source.source_commit.as_str()),
        ("cargo_lock_sha256", source.cargo_lock_sha256.as_str()),
        ("artifact_name", candidate.artifact_name.as_str()),
        ("workflow", "release"),
    ] {
        expect_string(&document, key, expected, "remote workflow", blockers);
    }
    for (key, expected) in [
        ("run_id", candidate.workflow_run_id),
        ("attempt", candidate.workflow_attempt),
        ("artifact_id", candidate.artifact_id),
    ] {
        expect_u64(&document, key, expected, "remote workflow", blockers);
    }
    if document.contains("\"exe_sha256\"") {
        blockers.push("remote workflow receipt must not predeclare final EXE hash".to_owned());
    }
}

fn check_downloaded(root: &Path, candidate: &Candidate, blockers: &mut Vec<String>) {
    let document = match receipt::read_receipt(&root.join(remote::DOWNLOADED_RECEIPT)) {
        Ok(document) => document,
        Err(error) => {
            blockers.push(format!("downloaded artifact receipt: {error}"));
            return;
        }
    };
    expect_u64(
        &document,
        "schema_version",
        2,
        "downloaded artifact",
        blockers,
    );
    expect_string(
        &document,
        "status",
        "PASSED",
        "downloaded artifact",
        blockers,
    );
    for (key, expected) in [
        ("source_commit", candidate.source_commit.as_str()),
        ("exe_sha256", candidate.exe_sha256.as_str()),
        ("zip_sha256", candidate.zip_sha256.as_str()),
        ("sbom_sha256", candidate.sbom_sha256.as_str()),
    ] {
        expect_string(&document, key, expected, "downloaded artifact", blockers);
    }
    for (key, expected) in [
        ("workflow_run_id", candidate.workflow_run_id),
        ("workflow_attempt", candidate.workflow_attempt),
        ("artifact_id", candidate.artifact_id),
    ] {
        expect_u64(&document, key, expected, "downloaded artifact", blockers);
    }
    for key in ["runtime_smoke", "promoted"] {
        match json::bool_field(&document, key) {
            Ok(true) => {}
            Ok(false) => blockers.push(format!("downloaded artifact {key} did not pass")),
            Err(error) => blockers.push(format!("downloaded artifact {key}: {error}")),
        }
    }
}

fn expect_string(
    document: &str,
    key: &str,
    expected: &str,
    label: &str,
    blockers: &mut Vec<String>,
) {
    match json::string_field(document, key) {
        Ok(actual) if actual == expected => {}
        Ok(actual) => blockers.push(format!(
            "STALE RECEIPT: {label} {key} is {actual}, expected {expected}"
        )),
        Err(error) => blockers.push(format!("{label} {key}: {error}")),
    }
}

fn expect_u64(document: &str, key: &str, expected: u64, label: &str, blockers: &mut Vec<String>) {
    match json::u64_field(document, key) {
        Ok(actual) if actual == expected => {}
        Ok(actual) => blockers.push(format!(
            "STALE RECEIPT: {label} {key} is {actual}, expected {expected}"
        )),
        Err(error) => blockers.push(format!("{label} {key}: {error}")),
    }
}

fn require_decision(
    decisions: &[decisions::Decision],
    key: &str,
    required: &str,
    blockers: &mut Vec<String>,
) {
    match decisions::status(decisions, key) {
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
        "{{\"schema_version\":2,\"status\":\"{status}\",\"source_commit\":\"{}\",\"exe_sha256\":\"{}\",\"zip_sha256\":\"{}\",\"blockers\":[",
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
    use super::{check_downloaded, render_readiness};
    use crate::qualification::receipt::{self, Candidate, RELEASE_ARTIFACT_NAME};
    use crate::qualification::remote::DOWNLOADED_RECEIPT;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn readiness_has_only_ready_or_not_ready_states() {
        assert!(render_readiness(None, &[]).contains("\"status\":\"READY\""));
        assert!(
            render_readiness(None, &["manual missing".to_owned()])
                .contains("\"status\":\"NOT_READY\"")
        );
    }

    #[test]
    fn downloaded_receipt_must_bind_promotion_and_workflow_identity() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-downloaded-receipt-{nonce}"));
        let candidate = candidate();
        let stale = format!(
            concat!(
                "{{\"schema_version\":2,\"status\":\"PASSED\",",
                "\"source_commit\":\"{}\",\"exe_sha256\":\"{}\",",
                "\"zip_sha256\":\"{}\",\"sbom_sha256\":\"{}\",",
                "\"workflow_run_id\":{},\"workflow_attempt\":{},\"artifact_id\":{},",
                "\"runtime_smoke\":true,\"promoted\":false}}"
            ),
            candidate.source_commit,
            candidate.exe_sha256,
            "f".repeat(64),
            candidate.sbom_sha256,
            candidate.workflow_run_id,
            candidate.workflow_attempt,
            candidate.artifact_id,
        );
        receipt::write_receipt(&root, DOWNLOADED_RECEIPT, &stale).expect("write receipt");
        let mut blockers = Vec::new();
        check_downloaded(&root, &candidate, &mut blockers);
        assert!(blockers.iter().any(|item| item.contains("zip_sha256")));
        assert!(blockers.iter().any(|item| item.contains("promoted")));
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
            target: "x86_64-pc-windows-msvc".to_owned(),
            workflow_run_id: 11,
            workflow_attempt: 1,
            artifact_id: 22,
            artifact_name: RELEASE_ARTIFACT_NAME.to_owned(),
            zip_name: "StickyMD-0.1.0-windows-x64-portable.zip".to_owned(),
        }
    }
}
