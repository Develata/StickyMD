//! Exact-artifact identity and receipt persistence.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::json;

pub(super) const EVIDENCE_DIRECTORY: &str = "dist/evidence";
pub(super) const CANDIDATE_RECEIPT: &str = "dist/evidence/release-candidate.json";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Candidate {
    pub(super) source_commit: String,
    pub(super) version: String,
    pub(super) cargo_lock_sha256: String,
    pub(super) exe_sha256: String,
    pub(super) zip_sha256: String,
    pub(super) sbom_sha256: String,
    pub(super) rustc: String,
    pub(super) target: String,
    pub(super) remote_synced: bool,
}

pub(super) fn generate_candidate(root: &Path) -> Result<Candidate, String> {
    ensure_clean(root)?;
    let source_commit = command_text(root, "git", &["rev-parse", "HEAD"])?;
    validate_hex(&source_commit, 40, "source commit")?;
    let version = workspace_version(root)?;
    let short = &source_commit[..12];
    let zip_name = format!("StickyMD-{version}-local-rc-{short}-windows-x64-portable.zip");
    let zip = root.join("dist").join(&zip_name);
    let executable = root.join("target/release/stickymd-win.exe");
    let sbom = root.join("dist/SBOM.spdx.json");
    for path in [&zip, &executable, &sbom, &root.join("Cargo.lock")] {
        if !path.is_file() {
            return Err(format!(
                "candidate input is missing: {}; run Phase 14 release qualification first",
                path.display()
            ));
        }
    }
    let zip_sha256 = sha256(&zip)?;
    let sbom_sha256 = sha256(&sbom)?;
    verify_checksum_manifest(root, &zip_name, &zip_sha256, &sbom_sha256)?;
    let candidate = Candidate {
        source_commit: source_commit.clone(),
        version,
        cargo_lock_sha256: sha256(&root.join("Cargo.lock"))?,
        exe_sha256: sha256(&executable)?,
        zip_sha256,
        sbom_sha256,
        rustc: command_text(root, "rustc", &["--version", "--verbose"])?,
        target: "x86_64-pc-windows-msvc".to_owned(),
        remote_synced: upstream_commit(root).is_ok_and(|upstream| upstream == source_commit),
    };
    write_candidate(root, &candidate)?;
    Ok(candidate)
}

pub(super) fn read_candidate(root: &Path) -> Result<Candidate, String> {
    let document = read_receipt(&root.join(CANDIDATE_RECEIPT))?;
    if json::u64_field(&document, "schema_version")? != 1 {
        return Err("release-candidate receipt schema is not version 1".to_owned());
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
        rustc: json::string_field(&document, "rustc")?,
        target: json::string_field(&document, "target")?,
        remote_synced: json::bool_field(&document, "remote_synced")?,
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
    Ok(candidate)
}

pub(super) fn validate_candidate_against_repository(
    root: &Path,
    candidate: &Candidate,
) -> Result<(), String> {
    ensure_clean(root)?;
    let head = command_text(root, "git", &["rev-parse", "HEAD"])?;
    if head != candidate.source_commit {
        return Err(format!(
            "STALE RECEIPT: candidate source {} does not match HEAD {head}",
            candidate.source_commit
        ));
    }
    if workspace_version(root)? != candidate.version {
        return Err("STALE RECEIPT: candidate version differs from Cargo.toml".to_owned());
    }
    if sha256(&root.join("Cargo.lock"))? != candidate.cargo_lock_sha256 {
        return Err("STALE RECEIPT: Cargo.lock hash changed".to_owned());
    }
    let executable = root.join("target/release/stickymd-win.exe");
    if !executable.is_file() || sha256(&executable)? != candidate.exe_sha256 {
        return Err("STALE RECEIPT: Release EXE is missing or changed".to_owned());
    }
    let zip = candidate_zip(root, candidate);
    if !zip.is_file() || sha256(&zip)? != candidate.zip_sha256 {
        return Err("STALE RECEIPT: exact portable ZIP is missing or changed".to_owned());
    }
    let sbom = root.join("dist/SBOM.spdx.json");
    if !sbom.is_file() || sha256(&sbom)? != candidate.sbom_sha256 {
        return Err("STALE RECEIPT: SBOM is missing or changed".to_owned());
    }
    Ok(())
}

pub(super) fn candidate_zip(root: &Path, candidate: &Candidate) -> PathBuf {
    root.join("dist").join(format!(
        "StickyMD-{}-local-rc-{}-windows-x64-portable.zip",
        candidate.version,
        &candidate.source_commit[..12]
    ))
}

pub(super) fn write_receipt(root: &Path, relative: &str, contents: &str) -> Result<(), String> {
    let directory = root.join(EVIDENCE_DIRECTORY);
    fs::create_dir_all(&directory)
        .map_err(|error| format!("cannot create {}: {error}", directory.display()))?;
    let path = root.join(relative);
    fs::write(&path, contents).map_err(|error| format!("cannot write {}: {error}", path.display()))
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

fn write_candidate(root: &Path, candidate: &Candidate) -> Result<(), String> {
    let json = format!(
        concat!(
            "{{\"schema_version\":1,",
            "\"source_commit\":\"{}\",",
            "\"version\":\"{}\",",
            "\"cargo_lock_sha256\":\"{}\",",
            "\"exe_sha256\":\"{}\",",
            "\"zip_sha256\":\"{}\",",
            "\"sbom_sha256\":\"{}\",",
            "\"rustc\":\"{}\",",
            "\"target\":\"{}\",",
            "\"authenticode\":\"UNSIGNED\",",
            "\"remote_synced\":{}",
            "}}\n"
        ),
        json::escape(&candidate.source_commit),
        json::escape(&candidate.version),
        json::escape(&candidate.cargo_lock_sha256),
        json::escape(&candidate.exe_sha256),
        json::escape(&candidate.zip_sha256),
        json::escape(&candidate.sbom_sha256),
        json::escape(&candidate.rustc),
        json::escape(&candidate.target),
        candidate.remote_synced,
    );
    write_receipt(root, CANDIDATE_RECEIPT, &json)
}

fn workspace_version(root: &Path) -> Result<String, String> {
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

fn ensure_clean(root: &Path) -> Result<(), String> {
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

fn upstream_commit(root: &Path) -> Result<String, String> {
    command_text(root, "git", &["rev-parse", "@{upstream}"])
}

fn verify_checksum_manifest(
    root: &Path,
    zip_name: &str,
    zip_hash: &str,
    sbom_hash: &str,
) -> Result<(), String> {
    let manifest = fs::read_to_string(root.join("dist/SHA256SUMS.txt"))
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

fn validate_sha256(value: &str, label: &str) -> Result<(), String> {
    validate_hex(value, 64, label)
}

fn validate_hex(value: &str, length: usize, label: &str) -> Result<(), String> {
    if value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("{label} is not a {length}-digit hexadecimal value"))
    }
}

#[cfg(test)]
mod tests {
    use super::{Candidate, write_candidate};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn candidate_receipt_round_trips_exact_artifact_identity() {
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
            rustc: "rustc test".to_owned(),
            target: "x86_64-pc-windows-msvc".to_owned(),
            remote_synced: false,
        };
        write_candidate(&root, &candidate).expect("write candidate");
        assert_eq!(super::read_candidate(&root), Ok(candidate));
        fs::remove_dir_all(root).expect("cleanup");
    }
}
