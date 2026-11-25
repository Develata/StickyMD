//! The resolver: re-reads the candidate that `release macos-promote` staged and checks it against
//! its receipt, the build source's inputs and the reuse record, before any artifact-bound macOS
//! runner or the readiness check uses it. It writes nothing.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use super::content;
use super::promote::{
    ARTIFACT_PREFIX, RECEIPT, REUSE_RECORD, STAGING, check_release, reuse_record,
};
use crate::release::json;
use crate::release::platform::Platform;
use crate::release::platform_inputs::{ALGORITHM, Reuse, fingerprint, verify_reuse};
use crate::{integrity, repository};
use std::fs;
use std::path::{Path, PathBuf};

/// What [`resolve`] read and checked: the staged bundle, and the identities of the receipt it
/// checked the staging against (the release gate binds evidence to these, plan 11
/// #macos-readiness-check).
pub(crate) struct Resolved {
    pub app: PathBuf,
    pub receipt_sha256: String,
    pub zip_sha256: String,
    pub executable_sha256: String,
    pub bundle_sha256: String,
    /// The signing the receipt records and the resolver requires (`AD_HOC`).
    pub signing: String,
}

/// Whether a candidate receipt is in place at all; the resolver alone says whether it holds.
/// Only a missing receipt is absent; any other error is reported as it is.
pub(crate) fn receipt_present(root: &Path) -> Result<bool, String> {
    match fs::symlink_metadata(root.join(RECEIPT)) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(format!(
            "cannot inspect the macOS candidate receipt: {error}"
        )),
    }
}

/// The staged candidate the artifact-bound macOS runners use. The receipt must be complete; the
/// staged files, the bundle and the executable must match it; the staged ZIP's README must name
/// its build source, whose fingerprint must match it; and that source must be reusable on
/// `release`, with the reuse record matching when it is not `release` itself.
pub(crate) fn resolve(root: &Path, release: &str) -> Result<Resolved, String> {
    let bytes = fs::read(root.join(RECEIPT))
        .map_err(|error| format!("no macOS candidate receipt: {error}"))?;
    let receipt =
        json::parse(std::str::from_utf8(&bytes).map_err(|_| "the receipt is not UTF-8")?)?;
    let field =
        |name: &str| -> Result<String, String> { Ok(receipt.field(name)?.string()?.to_owned()) };
    if receipt.field("schema_version")?.unsigned()? != 1
        || field("platform")? != "macos-arm64"
        || field("origin")? != "GITHUB_WORKFLOW_ARTIFACT"
        || field("build_inputs_algorithm")? != ALGORITHM
        || field("target")? != "aarch64-apple-darwin"
        || field("signing")? != "AD_HOC"
    {
        return Err("the macOS candidate receipt is not a complete schema 1 receipt".to_owned());
    }
    for name in ["workflow_run_id", "artifact_id"] {
        receipt.field(name)?.unsigned()?;
    }
    let attempt = receipt.field("workflow_attempt")?.unsigned()?;
    if field("artifact_name")? != format!("{ARTIFACT_PREFIX}{attempt}") {
        return Err("the receipt's artifact is not named for its attempt".to_owned());
    }
    for name in ["cargo_lock_sha256", "app_inputs"] {
        integrity::validate_sha256(&field(name)?, name)?;
    }
    let version = repository::workspace_version(root)?;
    if field("version")? != version {
        return Err(format!("the receipt is for another version than {version}"));
    }
    let staging = root.join(STAGING);
    let zip_name = field("zip_name")?;
    if zip_name != Platform::MacOsArm64.archive_name(&version) {
        return Err(format!(
            "the receipt names {zip_name}, not this version's package"
        ));
    }
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
    // The three staged files must also agree with each other, as Promote checked them (review
    // round 2, finding 1): the checksum file lists exactly the ZIP and the SBOM with the receipt's
    // digests, and the ZIP itself (not only the extracted copy beside it) holds the receipt's
    // bundle and executable and passes the package check.
    let checksums = Platform::MacOsArm64.checksum_file();
    let sbom_name = Platform::MacOsArm64.sbom_file();
    let manifest = fs::read_to_string(staging.join(checksums))
        .map_err(|error| format!("cannot read the staged {checksums}: {error}"))?;
    integrity::verify_manifest_text(
        &manifest,
        &[
            (zip_name.as_str(), field("zip_sha256")?.as_str()),
            (sbom_name, field("sbom_sha256")?.as_str()),
        ],
    )
    .map_err(|error| {
        format!("the staged {checksums} does not list the receipt's files: {error}")
    })?;
    crate::release::attachments::check_manifest_names(&manifest, &[&zip_name, sbom_name])?;
    let packaged = super::package::verify(&staging.join(&zip_name), None, &version)?;
    for (what, packaged, key) in [
        ("archive", packaged.zip_sha256.as_str(), "zip_sha256"),
        (
            "bundle",
            packaged.bundle_sha256.as_str(),
            "app_bundle_manifest_sha256",
        ),
        (
            "executable",
            packaged.executable_sha256.as_str(),
            "app_executable_sha256",
        ),
        ("product inputs", packaged.app_inputs.as_str(), "app_inputs"),
        ("signing", packaged.signing, "signing"),
        (
            "build source",
            packaged.source_commit.as_str(),
            "source_commit",
        ),
    ] {
        if packaged != field(key)? {
            return Err(format!(
                "the staged ZIP's {what} is not the receipt's ({packaged})"
            ));
        }
    }
    let sbom = fs::read_to_string(staging.join(sbom_name))
        .map_err(|error| format!("cannot read the staged {sbom_name}: {error}"))?;
    content::validate_sbom(&sbom, &packaged.members)?;
    let app = staging.join(content::APP);
    if crate::mac_common::fixture::bundle_hash(&app)? != field("app_bundle_manifest_sha256")? {
        return Err("the staged bundle does not match the receipt".to_owned());
    }
    if integrity::sha256(&app.join("Contents/MacOS/StickyMD"))? != field("app_executable_sha256")? {
        return Err("the staged executable does not match the receipt".to_owned());
    }
    if crate::mac_common::fixture::app_inputs(&app)?.as_deref()
        != Some(field("app_inputs")?.as_str())
    {
        return Err("the staged bundle's product inputs differ from the receipt".to_owned());
    }
    let source_commit = field("source_commit")?;
    integrity::validate_hex(&source_commit, 40, "the receipt's source commit")?;
    // Both are facts Promote computed; recompute them, so a receipt that misstates either fails.
    let lock = crate::release::platform_inputs::git(root)
        .args(["cat-file", "blob", &format!("{source_commit}:Cargo.lock")])
        .output()
        .map_err(|error| format!("cannot read Cargo.lock at {source_commit}: {error}"))?;
    if !lock.status.success()
        || integrity::sha256_bytes(&lock.stdout)? != field("cargo_lock_sha256")?
    {
        return Err("the receipt's Cargo.lock digest is not the build source's".to_owned());
    }
    let source = field("source_commit")?;
    if super::listing::verify_archive(&staging.join(&zip_name))? != source {
        return Err("the staged package names another build source than the receipt".to_owned());
    }
    if fingerprint(root, Platform::MacOsArm64, &source)? != field("build_inputs")? {
        return Err("the build source's inputs differ from the receipt".to_owned());
    }
    check_release(root, release)?;
    // The record must be exactly the one Promote writes for this judgement and receipt, and
    // absent when the candidate was built on R itself.
    let record_path = root.join(REUSE_RECORD);
    let recorded = match fs::symlink_metadata(&record_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(format!("cannot inspect the reuse record: {error}")),
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err("the reuse record is not a regular file".to_owned());
        }
        Ok(_) => Some(fs::read(&record_path).map_err(|error| error.to_string())?),
    };
    match (
        verify_reuse(root, Platform::MacOsArm64, &source, release)?,
        recorded,
    ) {
        (Reuse::SameCommit, None) => {}
        (Reuse::SameCommit, Some(_)) => {
            return Err(
                "a reuse record lies beside a candidate built on the release commit".to_owned(),
            );
        }
        (Reuse::Ancestor { .. }, None) => {
            return Err(format!("no reuse record for {source} on {release}"));
        }
        (Reuse::Ancestor { fingerprint }, Some(record)) => {
            let expected = reuse_record(
                &source,
                release,
                &field("build_inputs")?,
                &fingerprint,
                &integrity::sha256_bytes(&bytes)?,
            );
            if record != expected.as_bytes() {
                return Err(format!(
                    "the reuse record does not cover {source} on {release}"
                ));
            }
        }
    }
    Ok(Resolved {
        app,
        receipt_sha256: integrity::sha256_bytes(&bytes)?,
        zip_sha256: field("zip_sha256")?.to_ascii_lowercase(),
        executable_sha256: field("app_executable_sha256")?.to_ascii_lowercase(),
        bundle_sha256: field("app_bundle_manifest_sha256")?.to_ascii_lowercase(),
        signing: field("signing")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::temporary::TemporaryDirectory;

    /// A staged ZIP other than the one the receipt names does not resolve: the gate's package
    /// class and everything bound to the candidate then fail (plan 11 #macos-release-gate).
    #[test]
    fn a_staged_zip_other_than_the_receipt_s_does_not_resolve() {
        let scratch = TemporaryDirectory::new("macos-resolve-repacked").unwrap();
        let root = scratch.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace.package]\nversion = \"9.8.7\"\n",
        )
        .unwrap();
        let zip_name = Platform::MacOsArm64.archive_name("9.8.7");
        let staging = root.join(STAGING);
        fs::create_dir_all(&staging).unwrap();
        let released = b"the workflow's archive";
        let files = [
            (zip_name.clone(), &released[..]),
            (Platform::MacOsArm64.checksum_file().to_owned(), b"sums"),
            (Platform::MacOsArm64.sbom_file().to_owned(), b"sbom"),
        ];
        for (name, bytes) in &files {
            fs::write(staging.join(name), bytes).unwrap();
        }
        let digest = |bytes: &[u8]| integrity::sha256_bytes(bytes).unwrap();
        let hex = "a".repeat(64);
        let receipt = format!(
            "{{\"schema_version\":1,\"origin\":\"GITHUB_WORKFLOW_ARTIFACT\",\"platform\":\"macos-arm64\",\"source_commit\":\"{}\",\"version\":\"9.8.7\",\"cargo_lock_sha256\":\"{hex}\",\"build_inputs\":\"{hex}\",\"build_inputs_algorithm\":\"{ALGORITHM}\",\"app_inputs\":\"{hex}\",\"workflow_run_id\":1,\"workflow_attempt\":1,\"artifact_id\":2,\"artifact_name\":\"{ARTIFACT_PREFIX}1\",\"zip_name\":\"{zip_name}\",\"zip_sha256\":\"{}\",\"checksums_sha256\":\"{}\",\"sbom_sha256\":\"{}\",\"app_executable_sha256\":\"{hex}\",\"app_bundle_manifest_sha256\":\"{hex}\",\"target\":\"aarch64-apple-darwin\",\"signing\":\"AD_HOC\"}}\n",
            "1".repeat(40),
            digest(released),
            digest(b"sums"),
            digest(b"sbom"),
        );
        fs::create_dir_all(root.join("dist/evidence")).unwrap();
        fs::write(root.join(RECEIPT), receipt).unwrap();
        // The bundle is the same; only the archive was made again from it.
        fs::write(staging.join(&zip_name), b"ditto -c -k of the staged .app").unwrap();
        let error = resolve(root, &"1".repeat(40)).err().unwrap();
        assert!(
            error.contains(&format!("the staged {zip_name} does not match the receipt")),
            "{error}"
        );
        assert_eq!(receipt_present(root), Ok(true));
    }

    /// Review round 2, finding 1: a ZIP rebuilt from the `.app` with the receipt rewritten to
    /// name it still fails, because the staged checksum file names the workflow's ZIP. The file
    /// is made by the real generator, so only the ZIP's digest differs (review round 4).
    #[test]
    fn a_receipt_rewritten_for_a_rebuilt_zip_fails_the_checksum_file() {
        let scratch = TemporaryDirectory::new("macos-resolve-rewritten").unwrap();
        let root = scratch.path();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace.package]\nversion = \"9.8.7\"\n",
        )
        .unwrap();
        let zip_name = Platform::MacOsArm64.archive_name("9.8.7");
        let staging = root.join(STAGING);
        fs::create_dir_all(&staging).unwrap();
        let digest = |bytes: &[u8]| integrity::sha256_bytes(bytes).unwrap();
        let rebuilt = b"ditto -c -k of the staged .app";
        let sums = integrity::render_checksums(&[
            (zip_name.as_str(), &digest(b"the workflow's archive")),
            (Platform::MacOsArm64.sbom_file(), &digest(b"sbom")),
        ])
        .unwrap();
        fs::write(staging.join(&zip_name), rebuilt).unwrap();
        fs::write(staging.join(Platform::MacOsArm64.checksum_file()), &sums).unwrap();
        fs::write(staging.join(Platform::MacOsArm64.sbom_file()), b"sbom").unwrap();
        let hex = "a".repeat(64);
        let receipt = format!(
            "{{\"schema_version\":1,\"origin\":\"GITHUB_WORKFLOW_ARTIFACT\",\"platform\":\"macos-arm64\",\"source_commit\":\"{}\",\"version\":\"9.8.7\",\"cargo_lock_sha256\":\"{hex}\",\"build_inputs\":\"{hex}\",\"build_inputs_algorithm\":\"{ALGORITHM}\",\"app_inputs\":\"{hex}\",\"workflow_run_id\":1,\"workflow_attempt\":1,\"artifact_id\":2,\"artifact_name\":\"{ARTIFACT_PREFIX}1\",\"zip_name\":\"{zip_name}\",\"zip_sha256\":\"{}\",\"checksums_sha256\":\"{}\",\"sbom_sha256\":\"{}\",\"app_executable_sha256\":\"{hex}\",\"app_bundle_manifest_sha256\":\"{hex}\",\"target\":\"aarch64-apple-darwin\",\"signing\":\"AD_HOC\"}}\n",
            "1".repeat(40),
            digest(rebuilt),
            digest(sums.as_bytes()),
            digest(b"sbom"),
        );
        fs::create_dir_all(root.join("dist/evidence")).unwrap();
        fs::write(root.join(RECEIPT), receipt).unwrap();
        let error = resolve(root, &"1".repeat(40)).err().unwrap();
        assert!(
            error.contains(&format!(
                "does not list the receipt's files: Checksum mismatch for {zip_name}"
            )),
            "{error}"
        );
    }
}
