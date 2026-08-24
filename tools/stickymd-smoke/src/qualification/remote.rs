//! Remote-workflow and downloaded-artifact evidence binding.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::receipt;
use super::{decisions, json};

const REMOTE_RECEIPT: &str = "dist/evidence/remote-workflow.json";
const DOWNLOADED_RECEIPT: &str = "dist/evidence/downloaded-artifact-smoke.json";

pub(super) fn record_workflow(root: &Path, run_id: u64, attempt: u64) -> Result<(), String> {
    let candidate = receipt::read_candidate(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    let release_decisions = decisions::read(root, &candidate)?;
    if decisions::status(&release_decisions, "PUSH") != Some("USER APPROVED") {
        return Err(
            "remote workflow evidence requires explicit USER-approved PUSH authority".to_owned(),
        );
    }
    let output = Command::new("gh")
        .args([
            "run",
            "view",
            &run_id.to_string(),
            "--attempt",
            &attempt.to_string(),
            "--json",
            "headSha,conclusion,url,workflowName",
        ])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start `gh run view`: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "gh run view failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let document = String::from_utf8(output.stdout)
        .map_err(|error| format!("gh run JSON is not UTF-8: {error}"))?;
    let head = json::string_field(&document, "headSha")?;
    let conclusion = json::string_field(&document, "conclusion")?;
    let url = json::string_field(&document, "url")?;
    let workflow = json::string_field(&document, "workflowName")?;
    if head != candidate.source_commit {
        return Err(format!(
            "remote run head {head} does not match candidate {}",
            candidate.source_commit
        ));
    }
    if conclusion != "success" {
        return Err(format!("remote workflow conclusion is `{conclusion}`"));
    }
    let receipt_document = format!(
        concat!(
            "{{\"schema_version\":1,\"status\":\"PASSED\",",
            "\"source_commit\":\"{}\",\"exe_sha256\":\"{}\",",
            "\"run_id\":{},\"attempt\":{},\"workflow\":\"{}\",\"url\":\"{}\"}}\n"
        ),
        json::escape(&candidate.source_commit),
        json::escape(&candidate.exe_sha256),
        run_id,
        attempt,
        json::escape(&workflow),
        json::escape(&url),
    );
    receipt::write_receipt(root, REMOTE_RECEIPT, &receipt_document)
}

pub(super) fn verify_downloaded(root: &Path, zip: &Path) -> Result<(), String> {
    let candidate = receipt::read_candidate(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    let remote = receipt::read_receipt(&root.join(REMOTE_RECEIPT))?;
    if json::string_field(&remote, "status")? != "PASSED"
        || json::string_field(&remote, "source_commit")? != candidate.source_commit
    {
        return Err("remote workflow receipt is missing, failed, or stale".to_owned());
    }
    let zip = zip
        .canonicalize()
        .map_err(|error| format!("cannot resolve downloaded ZIP {}: {error}", zip.display()))?;
    if receipt::sha256(&zip)? != candidate.zip_sha256 {
        return Err("downloaded ZIP hash differs from the exact local candidate".to_owned());
    }
    run_package_verifier(root, &zip)?;
    let temporary = unique_temporary_directory()?;
    fs::create_dir_all(&temporary)
        .map_err(|error| format!("cannot create {}: {error}", temporary.display()))?;
    let outcome = (|| {
        let status = Command::new("tar")
            .arg("-xf")
            .arg(&zip)
            .arg("-C")
            .arg(&temporary)
            .status()
            .map_err(|error| format!("cannot extract downloaded ZIP: {error}"))?;
        if !status.success() {
            return Err("downloaded ZIP extraction failed".to_owned());
        }
        let executable = temporary.join("StickyMD/StickyMD.exe");
        crate::pe_dependencies::verify_portable_executable(&executable)?;
        let exe_hash = receipt::sha256(&executable)?;
        if exe_hash != candidate.exe_sha256 {
            return Err("downloaded EXE hash differs from manual candidate identity".to_owned());
        }
        let document = format!(
            concat!(
                "{{\"schema_version\":1,\"status\":\"PASSED\",",
                "\"source_commit\":\"{}\",\"exe_sha256\":\"{}\",",
                "\"zip_sha256\":\"{}\",\"runtime_smoke\":true}}\n"
            ),
            json::escape(&candidate.source_commit),
            json::escape(&exe_hash),
            json::escape(&candidate.zip_sha256),
        );
        receipt::write_receipt(root, DOWNLOADED_RECEIPT, &document)
    })();
    let cleanup = fs::remove_dir_all(&temporary)
        .map_err(|error| format!("cannot remove {}: {error}", temporary.display()));
    outcome.and(cleanup)
}

fn run_package_verifier(root: &Path, zip: &Path) -> Result<(), String> {
    let parent = zip
        .parent()
        .ok_or_else(|| "downloaded ZIP has no parent directory".to_owned())?;
    let status = Command::new("pwsh")
        .args(["-NoProfile", "-File"])
        .arg(root.join("tools/release/verify-package.ps1"))
        .arg("-PackageDirectory")
        .arg(parent)
        .arg("-ZipPath")
        .arg(zip)
        .arg("-ChecksumPath")
        .arg(parent.join("SHA256SUMS.txt"))
        .arg("-Runtime")
        .current_dir(root)
        .status()
        .map_err(|error| format!("cannot start package verifier: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "downloaded package verification failed with {status}"
        ))
    }
}

fn unique_temporary_directory() -> Result<std::path::PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_nanos();
    Ok(std::env::temp_dir().join(format!(
        "stickymd-downloaded-smoke-{}-{nonce}",
        std::process::id()
    )))
}
