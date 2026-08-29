//! Remote workflow evidence and downloaded-artifact promotion.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::receipt::{self, Candidate};
use super::{decisions, json, source_freeze};

pub(super) const REMOTE_RECEIPT: &str = "dist/evidence/remote-workflow.json";
pub(super) const DOWNLOADED_RECEIPT: &str = "dist/evidence/downloaded-artifact-smoke.json";

#[derive(Clone, Debug, Eq, PartialEq)]
struct RemoteWorkflow {
    source_commit: String,
    cargo_lock_sha256: String,
    run_id: u64,
    attempt: u64,
    artifact_id: u64,
    artifact_name: String,
    workflow: String,
    url: String,
}

pub(super) fn record_workflow(root: &Path, run_id: u64, attempt: u64) -> Result<(), String> {
    let source = source_freeze::read(root)?;
    source_freeze::validate_against_repository(root, &source)?;
    let release_decisions = decisions::read(root, &source)?;
    if decisions::status(&release_decisions, "PUSH") != Some("USER APPROVED") {
        return Err(
            "remote workflow evidence requires explicit USER-approved PUSH authority".to_owned(),
        );
    }
    if receipt::upstream_commit(root)? != source.source_commit {
        return Err("remote workflow evidence requires Source Freeze to match upstream".to_owned());
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
    if head != source.source_commit {
        return Err(format!(
            "remote run head {head} does not match Source Freeze {}",
            source.source_commit
        ));
    }
    if conclusion != "success" {
        return Err(format!("remote workflow conclusion is `{conclusion}`"));
    }
    if workflow != "release" {
        return Err(format!(
            "remote workflow is `{workflow}`, expected `release`"
        ));
    }
    let (artifact_id, artifact_name) = query_release_artifact(root, run_id)?;
    let evidence = RemoteWorkflow {
        source_commit: source.source_commit,
        cargo_lock_sha256: source.cargo_lock_sha256,
        run_id,
        attempt,
        artifact_id,
        artifact_name,
        workflow,
        url,
    };
    write_remote(root, &evidence)
}

pub(super) fn verify_downloaded(root: &Path, zip: &Path) -> Result<(), String> {
    let source = source_freeze::read(root)?;
    source_freeze::validate_against_repository(root, &source)?;
    let remote = read_remote(root)?;
    if remote.source_commit != source.source_commit
        || remote.cargo_lock_sha256 != source.cargo_lock_sha256
    {
        return Err("remote workflow receipt is stale for the current Source Freeze".to_owned());
    }
    let supplied_zip = zip
        .canonicalize()
        .map_err(|error| format!("cannot resolve downloaded ZIP {}: {error}", zip.display()))?;
    let supplied_parent = supplied_zip
        .parent()
        .ok_or_else(|| "downloaded ZIP has no parent directory".to_owned())?;
    let supplied_checksum = supplied_parent.join("SHA256SUMS.txt");
    let supplied_sbom = supplied_parent.join("SBOM.spdx.json");
    for input in [&supplied_checksum, &supplied_sbom] {
        if !input.is_file() {
            return Err(format!(
                "downloaded workflow artifact is incomplete: {}",
                input.display()
            ));
        }
    }
    let zip_name = supplied_zip
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "downloaded ZIP name is not UTF-8".to_owned())?
        .to_owned();
    let expected_zip_name = format!("StickyMD-{}-windows-x64-portable.zip", source.version);
    if zip_name != expected_zip_name {
        return Err(format!(
            "downloaded ZIP is {zip_name}, expected Source Freeze artifact {expected_zip_name}"
        ));
    }
    let authoritative = download_recorded_artifact(root, &remote)?;
    let zip = authoritative.path.join(&zip_name);
    let checksum = authoritative.path.join("SHA256SUMS.txt");
    let sbom = authoritative.path.join("SBOM.spdx.json");
    verify_supplied_copy(&[
        (&supplied_zip, &zip, "ZIP"),
        (&supplied_checksum, &checksum, "SHA256SUMS.txt"),
        (&supplied_sbom, &sbom, "SBOM"),
    ])?;
    let parent = &authoritative.path;
    let zip_sha256 = receipt::sha256(&zip)?;
    let sbom_sha256 = receipt::sha256(&sbom)?;
    receipt::verify_checksum_manifest(parent, &zip_name, &zip_sha256, &sbom_sha256)?;
    run_package_verifier(root, &zip)?;

    let temporary = unique_staging_directory(root)?;
    fs::create_dir_all(&temporary)
        .map_err(|error| format!("cannot create {}: {error}", temporary.display()))?;
    let outcome = (|| {
        fs::copy(&zip, temporary.join(&zip_name))
            .map_err(|error| format!("cannot stage downloaded ZIP: {error}"))?;
        fs::copy(&checksum, temporary.join("SHA256SUMS.txt"))
            .map_err(|error| format!("cannot stage SHA256SUMS.txt: {error}"))?;
        fs::copy(&sbom, temporary.join("SBOM.spdx.json"))
            .map_err(|error| format!("cannot stage SBOM.spdx.json: {error}"))?;
        extract_zip(&zip, &temporary)?;
        let executable = temporary.join("StickyMD/StickyMD.exe");
        if !executable.is_file() {
            return Err("downloaded ZIP does not contain StickyMD/StickyMD.exe".to_owned());
        }
        crate::pe_dependencies::verify_portable_executable(&executable)?;
        let exe_sha256 = receipt::sha256(&executable)?;
        let candidate = Candidate {
            source_commit: source.source_commit.clone(),
            version: source.version.clone(),
            cargo_lock_sha256: source.cargo_lock_sha256.clone(),
            exe_sha256,
            zip_sha256: zip_sha256.clone(),
            sbom_sha256: sbom_sha256.clone(),
            target: source.target.clone(),
            workflow_run_id: remote.run_id,
            workflow_attempt: remote.attempt,
            artifact_id: remote.artifact_id,
            artifact_name: remote.artifact_name.clone(),
            zip_name: zip_name.clone(),
        };
        promote_staging(root, &temporary)?;
        receipt::write_candidate(root, &candidate)?;
        write_downloaded(root, &candidate)?;
        println!("PROMOTED_CANDIDATE_SOURCE={}", candidate.source_commit);
        println!("PROMOTED_CANDIDATE_EXE_SHA256={}", candidate.exe_sha256);
        println!("PROMOTED_CANDIDATE_ZIP_SHA256={}", candidate.zip_sha256);
        println!("PROMOTED_CANDIDATE_ARTIFACT_ID={}", candidate.artifact_id);
        Ok(())
    })();
    if temporary.exists() {
        let cleanup = fs::remove_dir_all(&temporary)
            .map_err(|error| format!("cannot remove {}: {error}", temporary.display()));
        outcome.and(cleanup)
    } else {
        outcome
    }
}

struct DownloadedArtifact {
    root: PathBuf,
    path: PathBuf,
}

impl Drop for DownloadedArtifact {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn download_recorded_artifact(
    root: &Path,
    remote: &RemoteWorkflow,
) -> Result<DownloadedArtifact, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "stickymd-recorded-artifact-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&path)
        .map_err(|error| format!("cannot create artifact download directory: {error}"))?;
    let mut download = DownloadedArtifact {
        root: path.clone(),
        path,
    };
    let status = Command::new("gh")
        .args(["run", "download", &remote.run_id.to_string(), "--name"])
        .arg(&remote.artifact_name)
        .arg("--dir")
        .arg(&download.path)
        .current_dir(root)
        .status()
        .map_err(|error| format!("cannot download recorded workflow artifact: {error}"))?;
    if !status.success() {
        return Err(format!(
            "recorded workflow artifact download failed with {status}"
        ));
    }
    let expected_zip = format!(
        "StickyMD-{}-windows-x64-portable.zip",
        receipt::workspace_version(root)?
    );
    let expected = [expected_zip.as_str(), "SHA256SUMS.txt", "SBOM.spdx.json"];
    let nested_path = download.root.join(&remote.artifact_name);
    let payload = if nested_path.is_dir() {
        nested_path
    } else {
        download.root.clone()
    };
    let observed = member_set(&payload)?;
    let expected = expected_member_set(&expected);
    if observed != expected {
        return Err(format!(
            "recorded artifact member set is {observed:?}, expected {expected:?}"
        ));
    }
    download.path = payload;
    Ok(download)
}

fn member_set(directory: &Path) -> Result<Vec<String>, String> {
    let mut observed = fs::read_dir(directory)
        .map_err(|error| format!("cannot inspect recorded artifact: {error}"))?
        .map(|entry| {
            entry
                .map_err(|error| format!("cannot inspect recorded artifact member: {error}"))
                .and_then(|entry| {
                    if !entry
                        .file_type()
                        .map_err(|error| format!("cannot inspect artifact member type: {error}"))?
                        .is_file()
                    {
                        return Err("recorded artifact contains a non-file member".to_owned());
                    }
                    entry
                        .file_name()
                        .into_string()
                        .map_err(|_| "recorded artifact member name is not UTF-8".to_owned())
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    observed.sort();
    Ok(observed)
}

fn expected_member_set(expected: &[&str]) -> Vec<String> {
    let mut expected = expected
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<Vec<_>>();
    expected.sort();
    expected
}

fn verify_supplied_copy(pairs: &[(&Path, &Path, &str)]) -> Result<(), String> {
    for (supplied, authoritative, label) in pairs {
        if receipt::sha256(supplied)? != receipt::sha256(authoritative)? {
            return Err(format!(
                "supplied {label} is not byte-identical to the recorded workflow artifact"
            ));
        }
    }
    Ok(())
}

fn query_release_artifact(root: &Path, run_id: u64) -> Result<(u64, String), String> {
    let repository = receipt::command_text(
        root,
        "gh",
        &[
            "repo",
            "view",
            "--json",
            "nameWithOwner",
            "--jq",
            ".nameWithOwner",
        ],
    )?;
    let endpoint = format!("repos/{repository}/actions/runs/{run_id}/artifacts");
    let filter = format!(
        ".artifacts[] | select(.name == \"{}\" and .expired == false) | [.id, .name] | @tsv",
        receipt::RELEASE_ARTIFACT_NAME
    );
    let output = Command::new("gh")
        .args(["api", &endpoint, "--jq", &filter])
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot query workflow artifacts: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "gh artifact query failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    parse_artifact_query(
        &String::from_utf8(output.stdout)
            .map_err(|error| format!("gh artifact output is not UTF-8: {error}"))?,
    )
}

fn parse_artifact_query(output: &str) -> Result<(u64, String), String> {
    let rows = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>();
    if rows.len() != 1 {
        return Err(format!(
            "remote run must expose exactly one non-expired `{}` artifact; observed {}",
            receipt::RELEASE_ARTIFACT_NAME,
            rows.len()
        ));
    }
    let (id, name) = rows[0]
        .split_once('\t')
        .ok_or_else(|| "workflow artifact query output is malformed".to_owned())?;
    let id = id
        .parse::<u64>()
        .map_err(|error| format!("workflow artifact id is invalid: {error}"))?;
    if id == 0 || name != receipt::RELEASE_ARTIFACT_NAME {
        return Err("workflow artifact identity is invalid".to_owned());
    }
    Ok((id, name.to_owned()))
}

fn read_remote(root: &Path) -> Result<RemoteWorkflow, String> {
    let document = receipt::read_receipt(&root.join(REMOTE_RECEIPT))?;
    if json::u64_field(&document, "schema_version")? != 2
        || json::string_field(&document, "status")? != "PASSED"
    {
        return Err("remote workflow receipt is missing, failed, or stale".to_owned());
    }
    let remote = RemoteWorkflow {
        source_commit: json::string_field(&document, "source_commit")?,
        cargo_lock_sha256: json::string_field(&document, "cargo_lock_sha256")?,
        run_id: json::u64_field(&document, "run_id")?,
        attempt: json::u64_field(&document, "attempt")?,
        artifact_id: json::u64_field(&document, "artifact_id")?,
        artifact_name: json::string_field(&document, "artifact_name")?,
        workflow: json::string_field(&document, "workflow")?,
        url: json::string_field(&document, "url")?,
    };
    if remote.run_id == 0 || remote.attempt == 0 || remote.artifact_id == 0 {
        return Err("remote workflow receipt contains zero identity".to_owned());
    }
    if remote.artifact_name != receipt::RELEASE_ARTIFACT_NAME || remote.workflow != "release" {
        return Err("remote workflow receipt names the wrong workflow artifact".to_owned());
    }
    Ok(remote)
}

fn write_remote(root: &Path, remote: &RemoteWorkflow) -> Result<(), String> {
    let document = format!(
        concat!(
            "{{\"schema_version\":2,\"status\":\"PASSED\",",
            "\"source_commit\":\"{}\",\"cargo_lock_sha256\":\"{}\",",
            "\"run_id\":{},\"attempt\":{},\"artifact_id\":{},",
            "\"artifact_name\":\"{}\",\"workflow\":\"{}\",\"url\":\"{}\"}}\n"
        ),
        json::escape(&remote.source_commit),
        json::escape(&remote.cargo_lock_sha256),
        remote.run_id,
        remote.attempt,
        remote.artifact_id,
        json::escape(&remote.artifact_name),
        json::escape(&remote.workflow),
        json::escape(&remote.url),
    );
    receipt::write_receipt(root, REMOTE_RECEIPT, &document)
}

fn write_downloaded(root: &Path, candidate: &Candidate) -> Result<(), String> {
    let document = format!(
        concat!(
            "{{\"schema_version\":2,\"status\":\"PASSED\",",
            "\"source_commit\":\"{}\",\"exe_sha256\":\"{}\",",
            "\"zip_sha256\":\"{}\",\"sbom_sha256\":\"{}\",",
            "\"workflow_run_id\":{},\"workflow_attempt\":{},\"artifact_id\":{},",
            "\"runtime_smoke\":true,\"promoted\":true}}\n"
        ),
        json::escape(&candidate.source_commit),
        json::escape(&candidate.exe_sha256),
        json::escape(&candidate.zip_sha256),
        json::escape(&candidate.sbom_sha256),
        candidate.workflow_run_id,
        candidate.workflow_attempt,
        candidate.artifact_id,
    );
    receipt::write_receipt(root, DOWNLOADED_RECEIPT, &document)
}

fn promote_staging(root: &Path, temporary: &Path) -> Result<(), String> {
    receipt::invalidate_promoted_candidate(root)?;
    let destination = receipt::candidate_directory(root);
    fs::rename(temporary, &destination).map_err(|error| {
        format!(
            "cannot promote {} to {}: {error}",
            temporary.display(),
            destination.display()
        )
    })
}

fn extract_zip(zip: &Path, destination: &Path) -> Result<(), String> {
    let status = Command::new("tar")
        .arg("-xf")
        .arg(zip)
        .arg("-C")
        .arg(destination)
        .status()
        .map_err(|error| format!("cannot extract downloaded ZIP: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err("downloaded ZIP extraction failed".to_owned())
    }
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

fn unique_staging_directory(root: &Path) -> Result<PathBuf, String> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_nanos();
    Ok(root.join("dist").join(format!(
        ".exact-candidate-promotion-{}-{nonce}",
        std::process::id()
    )))
}

#[cfg(test)]
mod tests {
    use super::{parse_artifact_query, verify_supplied_copy};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn artifact_query_requires_one_exact_nonzero_identity() {
        assert_eq!(
            parse_artifact_query("9716585230\tstickymd-windows-x64-release\n"),
            Ok((9716585230, "stickymd-windows-x64-release".to_owned()))
        );
        assert!(parse_artifact_query("").is_err());
        assert!(parse_artifact_query("1\twrong\n").is_err());
        assert!(
            parse_artifact_query(
                "1\tstickymd-windows-x64-release\n2\tstickymd-windows-x64-release\n"
            )
            .is_err()
        );
    }

    #[test]
    fn supplied_artifact_copy_must_match_authoritative_bytes() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-artifact-copy-{nonce}"));
        fs::create_dir(&root).expect("root");
        let supplied = root.join("supplied");
        let authoritative = root.join("authoritative");
        fs::write(&supplied, b"same").expect("supplied");
        fs::write(&authoritative, b"same").expect("authoritative");
        assert!(verify_supplied_copy(&[(&supplied, &authoritative, "ZIP")]).is_ok());
        fs::write(&supplied, b"different").expect("change supplied");
        assert!(verify_supplied_copy(&[(&supplied, &authoritative, "ZIP")]).is_err());
        fs::remove_dir_all(root).expect("cleanup");
    }
}
