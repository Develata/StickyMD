//! Local promotion of the macOS Release Exact Artifact on a Mac: take one successful attempt of
//! `release-macos`, download its artifact by id, compare it byte for byte with the user's copy,
//! check it, stage it atomically in `dist/exact-candidate-macos/`, and only then write the
//! candidate receipt. The resolver re-checks the staged bytes against that receipt.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use super::content;
use super::package::{self, run};
use crate::evidence::escape_json;
use crate::release::attachments::platform_files;
use crate::release::platform::Platform;
use crate::release::platform_inputs::{Reuse, fingerprint, verify_reuse};
use crate::release::temporary::TemporaryDirectory;
use crate::release::{json, workflow};
use crate::{atomic_evidence, integrity, repository};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const STAGING: &str = "dist/exact-candidate-macos";
const RECEIPT: &str = "dist/evidence/release-candidate-macos.json";
const REUSE_RECORD: &str = "dist/evidence/release-reuse-macos-arm64.json";
/// The artifact of attempt `n` is named for it, so each attempt's artifact is distinct.
const ARTIFACT_PREFIX: &str = "stickymd-macos-arm64-release-";

pub(crate) struct Request<'a> {
    pub run_id: u64,
    pub attempt: u64,
    pub release: &'a str,
    /// The user's downloaded ZIP; its checksum file and SBOM lie beside it.
    pub zip: &'a Path,
}

fn gh(root: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("gh")
        .args(arguments)
        .current_dir(root)
        .output()
        .map_err(|error| format!("cannot run gh: {error}"))?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(format!(
            "gh {arguments:?} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// The one unexpired artifact of `run` named for `attempt`, from the run's artifact listing.
pub(crate) fn artifact_of_attempt(listing: &str, attempt: u64) -> Result<(u64, String), String> {
    let wanted = format!("{ARTIFACT_PREFIX}{attempt}");
    let document = json::parse(listing)?;
    let mut found = Vec::new();
    for artifact in document.field("artifacts")?.array()? {
        let name = artifact.field("name")?.string()?;
        let expired = matches!(artifact.field("expired")?, json::Value::Bool(true));
        if name == wanted && !expired {
            found.push((artifact.field("id")?.unsigned()?, name.to_owned()));
        }
    }
    match found.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(format!("the run has no unexpired artifact {wanted}")),
        _ => Err(format!("the run has more than one artifact {wanted}")),
    }
}

/// Facts of the promoted candidate; written as the receipt, which names no release commit and
/// no machine path, so promoting the same artifact again writes the same bytes.
pub(crate) struct Receipt {
    pub source: String,
    pub version: String,
    pub cargo_lock_sha256: String,
    pub build_inputs: String,
    pub run_id: u64,
    pub attempt: u64,
    pub artifact_id: u64,
    pub artifact_name: String,
    pub zip_name: String,
    pub zip_sha256: String,
    pub checksums_sha256: String,
    pub sbom_sha256: String,
    pub facts: package::Facts,
}

impl Receipt {
    pub(crate) fn json(&self) -> String {
        format!(
            "{{\"schema_version\":1,\"origin\":\"GITHUB_WORKFLOW_ARTIFACT\",\"platform\":\"macos-arm64\",\"source_commit\":\"{}\",\"version\":\"{}\",\"cargo_lock_sha256\":\"{}\",\"build_inputs\":\"{}\",\"build_inputs_algorithm\":\"v1\",\"app_inputs\":\"{}\",\"workflow_run_id\":{},\"workflow_attempt\":{},\"artifact_id\":{},\"artifact_name\":\"{}\",\"zip_name\":\"{}\",\"zip_sha256\":\"{}\",\"checksums_sha256\":\"{}\",\"sbom_sha256\":\"{}\",\"app_executable_sha256\":\"{}\",\"app_bundle_manifest_sha256\":\"{}\",\"target\":\"aarch64-apple-darwin\",\"signing\":\"{}\"}}\n",
            self.source,
            escape_json(&self.version),
            self.cargo_lock_sha256,
            self.build_inputs,
            self.facts.app_inputs,
            self.run_id,
            self.attempt,
            self.artifact_id,
            escape_json(&self.artifact_name),
            escape_json(&self.zip_name),
            self.zip_sha256,
            self.checksums_sha256,
            self.sbom_sha256,
            self.facts.executable_sha256,
            self.facts.bundle_sha256,
            self.facts.signing
        )
    }
}

/// `release` must be a full SHA contained in `origin/main`, as the release authorization names it.
fn check_release(root: &Path, release: &str) -> Result<(), String> {
    integrity::validate_hex(release, 40, "release commit")?;
    let contained = Command::new("git")
        .current_dir(root)
        .args(["merge-base", "--is-ancestor", release, "origin/main"])
        .status()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if contained.success() {
        Ok(())
    } else {
        Err(format!(
            "release commit {release} is not contained in origin/main"
        ))
    }
}

/// Checks a downloaded set (the artifact's three files in `directory`) and the bundle inside,
/// and returns the receipt; nothing is staged here.
pub(crate) fn check_set(
    root: &Path,
    directory: &Path,
    request_attempt: (u64, u64),
    artifact: (u64, String),
    release: &str,
) -> Result<(Receipt, Reuse), String> {
    let version = repository::workspace_version(root)?;
    let [zip_name, checksums_name, sbom_name] = platform_files(Platform::MacOsArm64, &version);
    let mut names = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .map(|entry| entry.map(|entry| entry.file_name().to_string_lossy().into_owned()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())?;
    names.sort();
    let mut expected = vec![zip_name.clone(), checksums_name.clone(), sbom_name.clone()];
    expected.sort();
    if names != expected {
        return Err(format!(
            "the artifact holds {names:?}, expected {expected:?}"
        ));
    }
    let zip = directory.join(&zip_name);
    let zip_sha256 = integrity::sha256(&zip)?;
    let sbom_sha256 = integrity::sha256(&directory.join(&sbom_name))?;
    let manifest =
        fs::read_to_string(directory.join(&checksums_name)).map_err(|error| error.to_string())?;
    integrity::verify_manifest_text(
        &manifest,
        &[(&zip_name, &zip_sha256), (&sbom_name, &sbom_sha256)],
    )?;
    let sbom = fs::read_to_string(directory.join(&sbom_name)).map_err(|error| error.to_string())?;
    content::validate_sbom(&sbom)?;
    let facts = package::verify(&zip, None, &version)?;
    let source = facts.source_commit.clone();
    let reuse = verify_reuse(root, Platform::MacOsArm64, &source, release)?;
    let lock = Command::new("git")
        .current_dir(root)
        .args(["show", &format!("{source}:Cargo.lock")])
        .output()
        .map_err(|error| format!("cannot read Cargo.lock at {source}: {error}"))?;
    if !lock.status.success() {
        return Err(format!("cannot read Cargo.lock at {source}"));
    }
    let receipt = Receipt {
        build_inputs: fingerprint(root, Platform::MacOsArm64, &source)?,
        cargo_lock_sha256: integrity::sha256_bytes(&lock.stdout)?,
        source,
        version,
        run_id: request_attempt.0,
        attempt: request_attempt.1,
        artifact_id: artifact.0,
        artifact_name: artifact.1,
        zip_name,
        zip_sha256,
        checksums_sha256: integrity::sha256(&directory.join(&checksums_name))?,
        sbom_sha256,
        facts,
    };
    Ok((receipt, reuse))
}

/// The whole promotion. The previous candidate and its receipt are retired before the new
/// staging takes its place, and the receipt is written last, so any failure leaves either the
/// old candidate untouched or no receipt at all, which the resolver refuses.
pub(crate) fn promote(root: &Path, request: &Request<'_>) -> Result<String, String> {
    check_release(root, request.release)?;
    let attempt_json = gh(
        root,
        &[
            "api",
            &format!(
                "repos/{{owner}}/{{repo}}/actions/runs/{}/attempts/{}",
                request.run_id, request.attempt
            ),
        ],
    )?;
    let attempt = json::parse(std::str::from_utf8(&attempt_json).map_err(|_| "not UTF-8")?)?;
    if attempt.field("run_attempt")?.unsigned()? != request.attempt {
        return Err("GitHub answered for another attempt".to_owned());
    }
    let head = attempt.field("head_sha")?.string()?.to_owned();
    workflow::verify_named_run(
        &head,
        &head,
        attempt.field("conclusion")?.string()?,
        attempt.field("name")?.string()?,
        Platform::MacOsArm64.workflow(),
    )?;
    let listing = gh(
        root,
        &[
            "api",
            &format!(
                "repos/{{owner}}/{{repo}}/actions/runs/{}/artifacts?per_page=100",
                request.run_id
            ),
        ],
    )?;
    let artifact = artifact_of_attempt(
        std::str::from_utf8(&listing).map_err(|_| "not UTF-8")?,
        request.attempt,
    )?;
    let download = TemporaryDirectory::new("macos-promotion")?;
    let archive = download.path().join("artifact.zip");
    let bytes = gh(
        root,
        &[
            "api",
            &format!(
                "repos/{{owner}}/{{repo}}/actions/artifacts/{}/zip",
                artifact.0
            ),
        ],
    )?;
    fs::write(&archive, bytes).map_err(|error| error.to_string())?;
    let set = download.path().join("set");
    run(
        "/usr/bin/ditto",
        &[
            "-x".as_ref(),
            "-k".as_ref(),
            archive.as_os_str(),
            set.as_os_str(),
        ],
    )?;
    let (receipt, reuse) = check_set(
        root,
        &set,
        (request.run_id, request.attempt),
        artifact,
        request.release,
    )?;
    if receipt.facts.source_commit != head.to_ascii_lowercase() {
        return Err(format!(
            "the package names source {}, the run head is {head}",
            receipt.facts.source_commit
        ));
    }
    // The user's copy must be the same bytes, file by file.
    let user = request.zip.parent().ok_or("the ZIP has no directory")?;
    for name in [
        &receipt.zip_name,
        &Platform::MacOsArm64.checksum_file().to_owned(),
        &Platform::MacOsArm64.sbom_file().to_owned(),
    ] {
        if integrity::sha256(&user.join(name))? != integrity::sha256(&set.join(name))? {
            return Err(format!("{name} differs from the workflow artifact"));
        }
    }
    stage(root, &set, &receipt)?;
    let json = receipt.json();
    atomic_evidence::write(&root.join(RECEIPT), json.as_bytes())?;
    if let Reuse::Ancestor { fingerprint } = &reuse {
        let record = format!(
            "{{\"schema_version\":1,\"platform\":\"macos-arm64\",\"build_source\":\"{}\",\"release_source\":\"{}\",\"build_inputs\":\"{}\",\"release_inputs\":\"{fingerprint}\",\"algorithm\":\"v1\",\"candidate_receipt_sha256\":\"{}\"}}\n",
            receipt.source,
            request.release.to_ascii_lowercase(),
            receipt.build_inputs,
            integrity::sha256_bytes(json.as_bytes())?
        );
        atomic_evidence::write(&root.join(REUSE_RECORD), record.as_bytes())?;
    }
    download.close()?;
    Ok(json)
}

fn stage(root: &Path, set: &Path, receipt: &Receipt) -> Result<(), String> {
    let staging = root.join(format!(
        "dist/.exact-candidate-macos-{}",
        std::process::id()
    ));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    for entry in fs::read_dir(set).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        let name = path.file_name().ok_or("a file has no name")?;
        fs::copy(&path, staging.join(name)).map_err(|error| error.to_string())?;
    }
    run(
        "/usr/bin/ditto",
        &[
            "-x".as_ref(),
            "-k".as_ref(),
            staging.join(&receipt.zip_name).as_os_str(),
            staging.as_os_str(),
        ],
    )?;
    for evidence in [RECEIPT, REUSE_RECORD] {
        match fs::remove_file(root.join(evidence)) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("cannot retire {evidence}: {error}")),
        }
    }
    let target = root.join(STAGING);
    if target.exists() {
        fs::remove_dir_all(&target).map_err(|error| error.to_string())?;
    }
    fs::rename(&staging, &target).map_err(|error| error.to_string())
}

/// The staged candidate the artifact-bound macOS runners use: the receipt, the staged files and
/// the extracted bundle must agree, and the build source must still be reusable on `release`.
pub(crate) fn resolve(root: &Path, release: &str) -> Result<PathBuf, String> {
    let text = fs::read_to_string(root.join(RECEIPT))
        .map_err(|error| format!("no macOS candidate receipt: {error}"))?;
    let receipt = json::parse(&text)?;
    let field =
        |name: &str| -> Result<String, String> { Ok(receipt.field(name)?.string()?.to_owned()) };
    let staging = root.join(STAGING);
    let zip_name = field("zip_name")?;
    for (name, key) in [
        (zip_name.clone(), "zip_sha256"),
        (
            Platform::MacOsArm64.checksum_file().to_owned(),
            "checksums_sha256",
        ),
        (Platform::MacOsArm64.sbom_file().to_owned(), "sbom_sha256"),
    ] {
        if integrity::sha256(&staging.join(&name))? != field(key)? {
            return Err(format!("the staged {name} does not match the receipt"));
        }
    }
    let app = staging.join(content::APP);
    if crate::mac_common::fixture::bundle_hash(&app)? != field("app_bundle_manifest_sha256")? {
        return Err("the staged bundle does not match the receipt".to_owned());
    }
    if integrity::sha256(&app.join("Contents/MacOS/StickyMD"))? != field("app_executable_sha256")? {
        return Err("the staged executable does not match the receipt".to_owned());
    }
    check_release(root, release)?;
    verify_reuse(
        root,
        Platform::MacOsArm64,
        &field("source_commit")?,
        release,
    )?;
    Ok(app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_artifact_of_exactly_the_requested_attempt_is_chosen() {
        let listing = r#"{"total_count":3,"artifacts":[
            {"id":11,"name":"stickymd-macos-arm64-release-1","expired":false},
            {"id":12,"name":"stickymd-macos-arm64-release-2","expired":false},
            {"id":13,"name":"stickymd-macos-arm64-release-3","expired":true}]}"#;
        assert_eq!(
            artifact_of_attempt(listing, 2).unwrap(),
            (12, "stickymd-macos-arm64-release-2".to_owned())
        );
        assert!(
            artifact_of_attempt(listing, 3)
                .unwrap_err()
                .contains("no unexpired")
        );
        assert!(artifact_of_attempt(listing, 4).is_err());
        let doubled = listing.replace(
            "\"id\":11,\"name\":\"stickymd-macos-arm64-release-1\"",
            "\"id\":11,\"name\":\"stickymd-macos-arm64-release-2\"",
        );
        assert!(
            artifact_of_attempt(&doubled, 2)
                .unwrap_err()
                .contains("more than one")
        );
    }
}
