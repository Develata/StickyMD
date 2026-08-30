//! Promoted exact-artifact identity, canonical staging, and receipt persistence.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{json, source_freeze};

pub(super) const CANDIDATE_RECEIPT: &str = "dist/evidence/release-candidate.json";
pub(super) const EXACT_CANDIDATE_DIRECTORY: &str = "dist/exact-candidate";
pub(super) const RELEASE_ARTIFACT_NAME: &str = "stickymd-windows-x64-release";
pub(super) const RELEASE_TARGET: &str = "x86_64-pc-windows-msvc";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Candidate {
    pub(super) source_commit: String,
    pub(super) version: String,
    pub(super) cargo_lock_sha256: String,
    pub(super) exe_sha256: String,
    pub(super) zip_sha256: String,
    pub(super) sbom_sha256: String,
    pub(super) target: String,
    pub(super) workflow_run_id: u64,
    pub(super) workflow_attempt: u64,
    pub(super) artifact_id: u64,
    pub(super) artifact_name: String,
    pub(super) zip_name: String,
}

pub(super) fn read_candidate(root: &Path) -> Result<Candidate, String> {
    let document = read_receipt(&root.join(CANDIDATE_RECEIPT))?;
    if json::u64_field(&document, "schema_version")? != 2 {
        return Err("release-candidate receipt schema is not version 2".to_owned());
    }
    if json::string_field(&document, "origin")? != "GITHUB_WORKFLOW_ARTIFACT" {
        return Err("release-candidate origin is not a GitHub workflow artifact".to_owned());
    }
    if json::string_field(&document, "authenticode")? != "UNSIGNED" {
        return Err("release-candidate Authenticode policy must be explicitly UNSIGNED".to_owned());
    }
    let candidate = Candidate {
        source_commit: json::string_field(&document, "source_commit")?,
        version: json::string_field(&document, "version")?,
        cargo_lock_sha256: json::string_field(&document, "cargo_lock_sha256")?,
        exe_sha256: json::string_field(&document, "exe_sha256")?,
        zip_sha256: json::string_field(&document, "zip_sha256")?,
        sbom_sha256: json::string_field(&document, "sbom_sha256")?,
        target: json::string_field(&document, "target")?,
        workflow_run_id: json::u64_field(&document, "workflow_run_id")?,
        workflow_attempt: json::u64_field(&document, "workflow_attempt")?,
        artifact_id: json::u64_field(&document, "artifact_id")?,
        artifact_name: json::string_field(&document, "artifact_name")?,
        zip_name: json::string_field(&document, "zip_name")?,
    };
    validate_hex(&candidate.source_commit, 40, "source commit")?;
    for (value, label) in [
        (&candidate.cargo_lock_sha256, "Cargo.lock SHA-256"),
        (&candidate.exe_sha256, "EXE SHA-256"),
        (&candidate.zip_sha256, "ZIP SHA-256"),
        (&candidate.sbom_sha256, "SBOM SHA-256"),
    ] {
        validate_sha256(value, label)?;
    }
    if candidate.workflow_run_id == 0
        || candidate.workflow_attempt == 0
        || candidate.artifact_id == 0
    {
        return Err("release-candidate workflow identity contains zero".to_owned());
    }
    if candidate.artifact_name != RELEASE_ARTIFACT_NAME {
        return Err(format!(
            "release-candidate artifact name is {}, expected {RELEASE_ARTIFACT_NAME}",
            candidate.artifact_name
        ));
    }
    if candidate.target != RELEASE_TARGET {
        return Err(format!(
            "release-candidate target is {}, expected {RELEASE_TARGET}",
            candidate.target
        ));
    }
    validate_zip_name(&candidate.zip_name, &candidate.version)?;
    Ok(candidate)
}

pub(super) fn validate_candidate_against_repository(
    root: &Path,
    candidate: &Candidate,
) -> Result<(), String> {
    ensure_clean(root)?;
    let source = source_freeze::read(root)?;
    source_freeze::validate_against_repository(root, &source)?;
    if source.source_commit != candidate.source_commit
        || source.version != candidate.version
        || source.cargo_lock_sha256 != candidate.cargo_lock_sha256
        || source.target != candidate.target
    {
        return Err("STALE RECEIPT: promoted candidate does not match Source Freeze".to_owned());
    }
    let executable = candidate_executable(root);
    if !executable.is_file() || sha256(&executable)? != candidate.exe_sha256 {
        return Err("STALE RECEIPT: promoted candidate EXE is missing or changed".to_owned());
    }
    crate::pe_dependencies::verify_portable_executable(&executable)?;
    let zip = candidate_zip(root, candidate);
    if !zip.is_file() || sha256(&zip)? != candidate.zip_sha256 {
        return Err("STALE RECEIPT: promoted candidate ZIP is missing or changed".to_owned());
    }
    let sbom = candidate_sbom(root);
    if !sbom.is_file() || sha256(&sbom)? != candidate.sbom_sha256 {
        return Err("STALE RECEIPT: promoted candidate SBOM is missing or changed".to_owned());
    }
    verify_checksum_manifest(
        &candidate_directory(root),
        &candidate.zip_name,
        &candidate.zip_sha256,
        &candidate.sbom_sha256,
    )
}

pub(super) fn resolve_release_executable(root: &Path) -> Result<PathBuf, String> {
    if root.join(CANDIDATE_RECEIPT).is_file() {
        let candidate = read_candidate(root)?;
        validate_candidate_against_repository(root, &candidate)?;
        return Ok(candidate_executable(root));
    }
    if root.join(source_freeze::SOURCE_FREEZE_RECEIPT).is_file() {
        return Err(
            "Promoted Candidate is required after Source Freeze; refusing Local Preflight fallback"
                .to_owned(),
        );
    }
    let local = root.join("target/release/stickymd-win.exe");
    if local.is_file() {
        Ok(local)
    } else {
        Err(format!(
            "Release executable is missing: {}; run Local Preflight or Promote a workflow artifact first",
            local.display()
        ))
    }
}

pub(super) fn candidate_directory(root: &Path) -> PathBuf {
    root.join(EXACT_CANDIDATE_DIRECTORY)
}

pub(super) fn candidate_executable(root: &Path) -> PathBuf {
    candidate_directory(root).join("StickyMD/StickyMD.exe")
}

pub(super) fn candidate_zip(root: &Path, candidate: &Candidate) -> PathBuf {
    candidate_directory(root).join(&candidate.zip_name)
}

pub(super) fn candidate_sbom(root: &Path) -> PathBuf {
    candidate_directory(root).join("SBOM.spdx.json")
}

pub(super) fn invalidate_promoted_candidate(root: &Path) -> Result<(), String> {
    let receipt = root.join(CANDIDATE_RECEIPT);
    if receipt.is_file() {
        fs::remove_file(&receipt)
            .map_err(|error| format!("cannot invalidate {}: {error}", receipt.display()))?;
    }
    let staging = candidate_directory(root);
    if staging.is_dir() {
        fs::remove_dir_all(&staging)
            .map_err(|error| format!("cannot invalidate {}: {error}", staging.display()))?;
    }
    Ok(())
}

pub(super) fn write_candidate(root: &Path, candidate: &Candidate) -> Result<(), String> {
    let document = format!(
        concat!(
            "{{\"schema_version\":2,",
            "\"origin\":\"GITHUB_WORKFLOW_ARTIFACT\",",
            "\"source_commit\":\"{}\",",
            "\"version\":\"{}\",",
            "\"cargo_lock_sha256\":\"{}\",",
            "\"exe_sha256\":\"{}\",",
            "\"zip_sha256\":\"{}\",",
            "\"sbom_sha256\":\"{}\",",
            "\"target\":\"{}\",",
            "\"workflow_run_id\":{},",
            "\"workflow_attempt\":{},",
            "\"artifact_id\":{},",
            "\"artifact_name\":\"{}\",",
            "\"zip_name\":\"{}\",",
            "\"authenticode\":\"UNSIGNED\"",
            "}}\n"
        ),
        json::escape(&candidate.source_commit),
        json::escape(&candidate.version),
        json::escape(&candidate.cargo_lock_sha256),
        json::escape(&candidate.exe_sha256),
        json::escape(&candidate.zip_sha256),
        json::escape(&candidate.sbom_sha256),
        json::escape(&candidate.target),
        candidate.workflow_run_id,
        candidate.workflow_attempt,
        candidate.artifact_id,
        json::escape(&candidate.artifact_name),
        json::escape(&candidate.zip_name),
    );
    write_receipt(root, CANDIDATE_RECEIPT, &document)
}

pub(super) fn write_receipt(root: &Path, relative: &str, contents: &str) -> Result<(), String> {
    let path = root.join(relative);
    crate::atomic_evidence::write(&path, contents.as_bytes())
}

pub(super) fn read_receipt(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

pub(super) fn command_text(
    root: &Path,
    program: &str,
    arguments: &[&str],
) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot start `{program}`: {error}"))?;
    if !output.status.success() {
        return Err(format!("`{program} {}` failed", arguments.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("`{program}` output is not UTF-8: {error}"))
}

pub(super) fn sha256(path: &Path) -> Result<String, String> {
    #[cfg(windows)]
    let output = Command::new("certutil")
        .args(["-hashfile"])
        .arg(path)
        .arg("SHA256")
        .output();
    #[cfg(not(windows))]
    let output = Command::new("sha256sum").arg(path).output();
    let output = output.map_err(|error| format!("cannot hash {}: {error}", path.display()))?;
    if !output.status.success() {
        return Err(format!("SHA-256 command failed for {}", path.display()));
    }
    let hash = String::from_utf8_lossy(&output.stdout)
        .split_whitespace()
        .find(|token| token.len() == 64 && token.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .map(str::to_ascii_lowercase)
        .ok_or_else(|| format!("SHA-256 output is malformed for {}", path.display()))?;
    validate_sha256(&hash, "SHA-256")?;
    Ok(hash)
}

pub(super) fn workspace_version(root: &Path) -> Result<String, String> {
    let manifest = fs::read_to_string(root.join("Cargo.toml"))
        .map_err(|error| format!("cannot read Cargo.toml: {error}"))?;
    manifest
        .lines()
        .find_map(|line| {
            line.trim()
                .strip_prefix("version = \"")
                .and_then(|value| value.strip_suffix('"'))
        })
        .map(str::to_owned)
        .ok_or_else(|| "workspace version is missing".to_owned())
}

pub(super) fn ensure_clean(root: &Path) -> Result<(), String> {
    let status = command_text(
        root,
        "git",
        &["status", "--porcelain", "--untracked-files=normal"],
    )?;
    if status.is_empty() {
        Ok(())
    } else {
        Err("candidate evidence requires a clean worktree".to_owned())
    }
}

pub(super) fn upstream_commit(root: &Path) -> Result<String, String> {
    command_text(root, "git", &["rev-parse", "@{upstream}"])
}

pub(super) fn verify_checksum_manifest(
    directory: &Path,
    zip_name: &str,
    zip_hash: &str,
    sbom_hash: &str,
) -> Result<(), String> {
    let manifest = fs::read_to_string(directory.join("SHA256SUMS.txt"))
        .map_err(|error| format!("cannot read SHA256SUMS.txt: {error}"))?;
    let expected_zip = format!("{zip_hash} *{zip_name}");
    let expected_sbom = format!("{sbom_hash} *SBOM.spdx.json");
    if !manifest.lines().any(|line| line == expected_zip)
        || !manifest.lines().any(|line| line == expected_sbom)
    {
        return Err("SHA256SUMS.txt does not bind the exact candidate ZIP and SBOM".to_owned());
    }
    Ok(())
}

pub(super) fn validate_sha256(value: &str, label: &str) -> Result<(), String> {
    validate_hex(value, 64, label)
}

pub(super) fn validate_hex(value: &str, length: usize, label: &str) -> Result<(), String> {
    if value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("{label} is not a {length}-digit hexadecimal value"))
    }
}

fn validate_zip_name(value: &str, version: &str) -> Result<(), String> {
    let expected = format!("StickyMD-{version}-windows-x64-portable.zip");
    if value == expected && !value.contains(['/', '\\']) {
        Ok(())
    } else {
        Err(format!(
            "release-candidate ZIP name is {value}, expected {expected}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{Candidate, write_candidate};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn candidate_receipt_round_trips_promoted_workflow_artifact_identity() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-candidate-{nonce}"));
        let hash = "a".repeat(64);
        let candidate = Candidate {
            source_commit: "b".repeat(40),
            version: "0.1.0".to_owned(),
            cargo_lock_sha256: hash.clone(),
            exe_sha256: hash.clone(),
            zip_sha256: hash.clone(),
            sbom_sha256: hash,
            target: super::RELEASE_TARGET.to_owned(),
            workflow_run_id: 101,
            workflow_attempt: 2,
            artifact_id: 303,
            artifact_name: super::RELEASE_ARTIFACT_NAME.to_owned(),
            zip_name: "StickyMD-0.1.0-windows-x64-portable.zip".to_owned(),
        };
        write_candidate(&root, &candidate).expect("write candidate");
        assert_eq!(super::read_candidate(&root), Ok(candidate));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn candidate_receipt_rejects_noncanonical_zip_name_or_target() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-candidate-invalid-{nonce}"));
        let hash = "a".repeat(64);
        let mut candidate = Candidate {
            source_commit: "b".repeat(40),
            version: "0.1.0".to_owned(),
            cargo_lock_sha256: hash.clone(),
            exe_sha256: hash.clone(),
            zip_sha256: hash.clone(),
            sbom_sha256: hash,
            target: super::RELEASE_TARGET.to_owned(),
            workflow_run_id: 101,
            workflow_attempt: 2,
            artifact_id: 303,
            artifact_name: super::RELEASE_ARTIFACT_NAME.to_owned(),
            zip_name: "StickyMD-0.1.0-local-rc-bbbbbbbbbbbb-windows-x64-portable.zip".to_owned(),
        };
        write_candidate(&root, &candidate).expect("write candidate");
        assert!(super::read_candidate(&root).is_err());
        candidate.zip_name = "StickyMD-0.1.0-windows-x64-portable.zip".to_owned();
        candidate.target = "aarch64-pc-windows-msvc".to_owned();
        write_candidate(&root, &candidate).expect("rewrite candidate");
        assert!(super::read_candidate(&root).is_err());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn resolver_never_falls_back_after_source_freeze_or_malformed_candidate() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-resolver-{nonce}"));
        fs::create_dir_all(root.join("target/release")).expect("target");
        fs::create_dir_all(root.join("dist/evidence")).expect("evidence");
        fs::write(root.join("target/release/stickymd-win.exe"), b"local").expect("local");
        fs::write(
            root.join(crate::qualification::source_freeze::SOURCE_FREEZE_RECEIPT),
            "{}",
        )
        .expect("source receipt");
        assert!(
            super::resolve_release_executable(&root)
                .expect_err("source freeze must block fallback")
                .contains("Promoted Candidate")
        );
        fs::write(root.join(super::CANDIDATE_RECEIPT), "{}").expect("candidate");
        assert!(super::resolve_release_executable(&root).is_err());
        fs::remove_dir_all(root).expect("cleanup");
    }
}
