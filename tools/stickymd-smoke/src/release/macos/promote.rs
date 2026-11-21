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
use crate::release::platform_inputs::{ALGORITHM, Reuse, fingerprint, verify_reuse};
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
            "{{\"schema_version\":1,\"origin\":\"GITHUB_WORKFLOW_ARTIFACT\",\"platform\":\"macos-arm64\",\"source_commit\":\"{}\",\"version\":\"{}\",\"cargo_lock_sha256\":\"{}\",\"build_inputs\":\"{}\",\"build_inputs_algorithm\":\"{}\",\"app_inputs\":\"{}\",\"workflow_run_id\":{},\"workflow_attempt\":{},\"artifact_id\":{},\"artifact_name\":\"{}\",\"zip_name\":\"{}\",\"zip_sha256\":\"{}\",\"checksums_sha256\":\"{}\",\"sbom_sha256\":\"{}\",\"app_executable_sha256\":\"{}\",\"app_bundle_manifest_sha256\":\"{}\",\"target\":\"aarch64-apple-darwin\",\"signing\":\"{}\"}}\n",
            self.source,
            escape_json(&self.version),
            self.cargo_lock_sha256,
            self.build_inputs,
            ALGORITHM,
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
    let contained = crate::release::platform_inputs::git(root)
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
/// What [`verify_set`] establishes about a downloaded macOS set.
pub(crate) struct CheckedSet {
    pub version: String,
    pub zip_name: String,
    pub checksums_name: String,
    pub zip_sha256: String,
    pub sbom_sha256: String,
    pub facts: package::Facts,
}

/// The three files of the macOS set in `directory` and nothing else: the ZIP passes the package
/// check, the checksum file lists exactly the ZIP and the SBOM with their hashes, and the SBOM
/// describes this very package. Writes nothing.
pub(crate) fn verify_set(root: &Path, directory: &Path) -> Result<CheckedSet, String> {
    let version = repository::workspace_version(root)?;
    let [zip_name, checksums_name, sbom_name] = platform_files(Platform::MacOsArm64, &version);
    let mut names = Vec::new();
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        // Only plain files: a link would let the set borrow bytes from outside the directory.
        if !entry
            .file_type()
            .map_err(|error| error.to_string())?
            .is_file()
        {
            return Err(format!("the artifact's {name} is not a plain file"));
        }
        names.push(name);
    }
    names.sort();
    let mut expected = vec![zip_name.clone(), checksums_name.clone(), sbom_name.clone()];
    expected.sort();
    if names != expected {
        return Err(format!(
            "the artifact holds {names:?}, expected {expected:?}"
        ));
    }
    let zip = directory.join(&zip_name);
    let facts = package::verify(&zip, None, &version)?;
    let zip_sha256 = facts.zip_sha256.clone();
    let sbom_sha256 = integrity::sha256(&directory.join(&sbom_name))?;
    let manifest =
        fs::read_to_string(directory.join(&checksums_name)).map_err(|error| error.to_string())?;
    integrity::verify_manifest_text(
        &manifest,
        &[(&zip_name, &zip_sha256), (&sbom_name, &sbom_sha256)],
    )?;
    super::super::attachments::check_manifest_names(&manifest, &[&zip_name, &sbom_name])?;
    let sbom = fs::read_to_string(directory.join(&sbom_name)).map_err(|error| error.to_string())?;
    // The SBOM must describe the bytes of this very package, not only its file names.
    content::validate_sbom(&sbom, &facts.members)?;
    Ok(CheckedSet {
        version,
        zip_name,
        checksums_name,
        zip_sha256,
        sbom_sha256,
        facts,
    })
}

pub(crate) fn check_set(
    root: &Path,
    directory: &Path,
    request_attempt: (u64, u64),
    artifact: (u64, String),
    release: &str,
) -> Result<(Receipt, Reuse), String> {
    let CheckedSet {
        version,
        zip_name,
        checksums_name,
        zip_sha256,
        sbom_sha256,
        facts,
    } = verify_set(root, directory)?;
    let source = facts.source_commit.clone();
    let reuse = verify_reuse(root, Platform::MacOsArm64, &source, release)?;
    let lock = crate::release::platform_inputs::git(root)
        .args(["cat-file", "blob", &format!("{source}:Cargo.lock")])
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

/// One promotion at a time: an advisory lock on a file that stays in place. The operating
/// system releases it when the holder exits, killed or not, so an interrupted promotion does
/// not block the next one.
struct PromotionLock {
    _file: fs::File,
}

impl PromotionLock {
    fn take(root: &Path) -> Result<Self, String> {
        let path = root.join("dist/evidence/.macos-promotion.lock");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let file = fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
        match file.try_lock() {
            Ok(()) => Ok(Self { _file: file }),
            Err(fs::TryLockError::WouldBlock) => {
                Err(format!("another macOS promotion holds {}", path.display()))
            }
            Err(fs::TryLockError::Error(error)) => {
                Err(format!("cannot lock {}: {error}", path.display()))
            }
        }
    }
}

/// The attempt `request` names, checked to be a successful `workflow_dispatch` run of the
/// macOS release workflow file; returns its head commit.
fn checked_attempt(root: &Path, request: &Request<'_>) -> Result<String, String> {
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
    workflow::verify_origin(
        &attempt,
        ".github/workflows/release-macos.yml",
        "workflow_dispatch",
    )?;
    let head = attempt.field("head_sha")?.string()?.to_owned();
    workflow::verify_named_run(
        &head,
        &head,
        attempt.field("conclusion")?.string()?,
        attempt.field("name")?.string()?,
        Platform::MacOsArm64.workflow(),
    )?;
    Ok(head.to_ascii_lowercase())
}

/// The whole promotion, serialized by a lock. The new set is downloaded, checked and staged in a
/// private directory first; only then is the old candidate retired and kept aside, the new one
/// moved into place, the reuse record (if any) written, and the receipt written last. Any failure
/// on the way puts the old candidate, receipt and record back.
pub(crate) fn promote(root: &Path, request: &Request<'_>) -> Result<String, String> {
    check_release(root, request.release)?;
    let _lock = PromotionLock::take(root)?;
    let head = checked_attempt(root, request)?;
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
    if receipt.facts.source_commit != head {
        return Err(format!(
            "the package names source {}, the run head is {head}",
            receipt.facts.source_commit
        ));
    }
    // The user's copy: the very ZIP they name, and its two companions beside it.
    let user_zip_name = request
        .zip
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("the ZIP path has no file name")?;
    if user_zip_name != receipt.zip_name {
        return Err(format!("{user_zip_name} is not {}", receipt.zip_name));
    }
    let user = request.zip.parent().ok_or("the ZIP has no directory")?;
    for (copy, name) in [
        (request.zip.to_owned(), receipt.zip_name.clone()),
        (
            user.join(Platform::MacOsArm64.checksum_file()),
            Platform::MacOsArm64.checksum_file().to_owned(),
        ),
        (
            user.join(Platform::MacOsArm64.sbom_file()),
            Platform::MacOsArm64.sbom_file().to_owned(),
        ),
    ] {
        if integrity::sha256(&copy)? != integrity::sha256(&set.join(&name))? {
            return Err(format!(
                "{} differs from the workflow artifact",
                copy.display()
            ));
        }
    }
    let staged = prepare(root, &set, &receipt)?;
    let json = receipt.json();
    let record = match &reuse {
        Reuse::Ancestor { fingerprint } => Some(reuse_record(
            &receipt.source,
            request.release,
            &receipt.build_inputs,
            fingerprint,
            &integrity::sha256_bytes(json.as_bytes())?,
        )),
        Reuse::SameCommit => None,
    };
    install(root, &staged, record.as_deref(), &json)?;
    if let Err(error) = download.close() {
        eprintln!("warning: promoted; the download directory was not removed: {error}");
    }
    Ok(json)
}

/// The reuse record Promote writes beside the receipt and the resolver compares byte for byte.
fn reuse_record(
    build: &str,
    release: &str,
    build_inputs: &str,
    release_inputs: &str,
    receipt_sha256: &str,
) -> String {
    format!(
        "{{\"schema_version\":1,\"platform\":\"macos-arm64\",\"build_source\":\"{build}\",\"release_source\":\"{}\",\"build_inputs\":\"{build_inputs}\",\"release_inputs\":\"{release_inputs}\",\"algorithm\":\"{ALGORITHM}\",\"candidate_receipt_sha256\":\"{receipt_sha256}\"}}\n",
        release.to_ascii_lowercase(),
    )
}

/// Copies the checked set into a private directory under `dist/` with the bundle extracted, and
/// checks the staged bundle is the one the receipt describes.
fn prepare(root: &Path, set: &Path, receipt: &Receipt) -> Result<PathBuf, String> {
    let staging = root.join(format!(
        "dist/.exact-candidate-macos-new-{}",
        std::process::id()
    ));
    if staging.exists() {
        fs::remove_dir_all(&staging).map_err(|error| error.to_string())?;
    }
    fs::create_dir_all(&staging).map_err(|error| error.to_string())?;
    let prepared = (|| {
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
        let app = staging.join(content::APP);
        if crate::mac_common::fixture::bundle_hash(&app)? != receipt.facts.bundle_sha256 {
            return Err("the staged bundle differs from the checked one".to_owned());
        }
        Ok(())
    })();
    if let Err(error) = prepared {
        let _ = fs::remove_dir_all(&staging);
        return Err(error);
    }
    Ok(staging)
}

/// One change `install` made, undone in reverse order if a later step fails.
enum Step {
    ReceiptRetired(Vec<u8>),
    RecordRetired(Vec<u8>),
    OldSetAside,
    NewInPlace,
    RecordWritten,
}

/// The bytes at `path`, `None` only when nothing is there; any other error stops the caller
/// before it changes anything.
fn existing(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

/// Swaps the prepared candidate in. The receipt is retired first, so from then on the resolver
/// refuses the staging directory; the old candidate is kept aside; the new one moves in, then
/// the reuse record, and the receipt last: writing it is the commit. A failure before that
/// undoes every step in reverse order (the old receipt last) and reports any step it could not
/// undo. After the commit, leftover cleanup is only reported: the new candidate stands.
fn install(root: &Path, staged: &Path, record: Option<&str>, receipt: &str) -> Result<(), String> {
    let target = root.join(STAGING);
    let receipt_path = root.join(RECEIPT);
    let record_path = root.join(REUSE_RECORD);
    let aside = root.join(format!(
        "dist/.exact-candidate-macos-old-{}",
        std::process::id()
    ));
    let old_receipt = existing(&receipt_path)?;
    let old_record = existing(&record_path)?;
    let mut done = Vec::new();
    let io = |what: &str, error: std::io::Error| format!("cannot {what}: {error}");
    let outcome = (|| {
        if let Some(bytes) = old_receipt {
            fs::remove_file(&receipt_path).map_err(|error| io("retire the receipt", error))?;
            done.push(Step::ReceiptRetired(bytes));
        }
        if let Some(bytes) = old_record {
            fs::remove_file(&record_path).map_err(|error| io("retire the reuse record", error))?;
            done.push(Step::RecordRetired(bytes));
        }
        if target.exists() {
            fs::rename(&target, &aside)
                .map_err(|error| io("set the old candidate aside", error))?;
            done.push(Step::OldSetAside);
        }
        fs::rename(staged, &target).map_err(|error| io("move the candidate into place", error))?;
        done.push(Step::NewInPlace);
        if let Some(record) = record {
            atomic_evidence::write(&record_path, record.as_bytes())?;
            done.push(Step::RecordWritten);
        }
        atomic_evidence::write(&receipt_path, receipt.as_bytes())
    })();
    if let Err(error) = outcome {
        let mut failures = Vec::new();
        for step in done.into_iter().rev() {
            let undone = match step {
                Step::RecordWritten => fs::remove_file(&record_path).map_err(|e| e.to_string()),
                Step::NewInPlace => fs::remove_dir_all(&target).map_err(|e| e.to_string()),
                Step::OldSetAside => fs::rename(&aside, &target).map_err(|e| e.to_string()),
                Step::RecordRetired(bytes) => atomic_evidence::write(&record_path, &bytes),
                Step::ReceiptRetired(bytes) => atomic_evidence::write(&receipt_path, &bytes),
            };
            if let Err(failure) = undone {
                failures.push(failure);
            }
        }
        let _ = fs::remove_dir_all(staged);
        return Err(if failures.is_empty() {
            format!("{error}; the previous candidate is back in place")
        } else {
            format!(
                "{error}; the previous candidate could not be fully restored: {}",
                failures.join("; ")
            )
        });
    }
    if aside.exists()
        && let Err(error) = fs::remove_dir_all(&aside)
    {
        eprintln!(
            "warning: promoted; the previous candidate stays at {}: {error}",
            aside.display()
        );
    }
    Ok(())
}

/// The staged candidate the artifact-bound macOS runners use. The receipt must be complete; the
/// staged files, the bundle and the executable must match it; the staged ZIP's README must name
/// its build source, whose fingerprint must match it; and that source must be reusable on
/// `release`, with the reuse record matching when it is not `release` itself.
pub(crate) fn resolve(root: &Path, release: &str) -> Result<PathBuf, String> {
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
    let app = staging.join(content::APP);
    if crate::mac_common::fixture::bundle_hash(&app)? != field("app_bundle_manifest_sha256")? {
        return Err("the staged bundle does not match the receipt".to_owned());
    }
    if integrity::sha256(&app.join("Contents/MacOS/StickyMD"))? != field("app_executable_sha256")? {
        return Err("the staged executable does not match the receipt".to_owned());
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
    Ok(app)
}

#[cfg(test)]
mod tests {
    /// The downloaded set holds exactly the three plain files: a missing one, an extra one and a
    /// link are refused before any of them is read.
    #[test]
    fn the_set_check_refuses_missing_extra_and_linked_files() {
        let root =
            crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
        let version = crate::repository::workspace_version(&root).unwrap();
        let [zip, checksums, sbom] = super::platform_files(super::Platform::MacOsArm64, &version);
        let directory = super::TemporaryDirectory::new("macos-set").unwrap();
        let path = directory.path();
        for name in [&zip, &checksums] {
            std::fs::write(path.join(name), b"x").unwrap();
        }
        let error = super::verify_set(&root, path).err().unwrap();
        assert!(error.contains("the artifact holds"), "{error}");
        std::fs::write(path.join("extra.txt"), b"x").unwrap();
        std::fs::write(path.join(&sbom), b"x").unwrap();
        let error = super::verify_set(&root, path).err().unwrap();
        assert!(error.contains("the artifact holds"), "{error}");
        std::fs::remove_file(path.join("extra.txt")).unwrap();
        std::fs::remove_file(path.join(&sbom)).unwrap();
        std::os::unix::fs::symlink(path.join(&zip), path.join(&sbom)).unwrap();
        let error = super::verify_set(&root, path).err().unwrap();
        assert!(error.contains("not a plain file"), "{error}");
        directory.close().unwrap();
    }

    use super::*;

    fn tree(root: &Path, candidate: &str, receipt: &str, record: Option<&str>) {
        fs::create_dir_all(root.join(STAGING)).unwrap();
        fs::write(root.join(STAGING).join("marker"), candidate).unwrap();
        fs::create_dir_all(root.join("dist/evidence")).unwrap();
        fs::write(root.join(RECEIPT), receipt).unwrap();
        if let Some(record) = record {
            fs::write(root.join(REUSE_RECORD), record).unwrap();
        }
    }

    fn read(root: &Path) -> (String, String, Option<String>) {
        (
            fs::read_to_string(root.join(STAGING).join("marker")).unwrap(),
            fs::read_to_string(root.join(RECEIPT)).unwrap(),
            fs::read_to_string(root.join(REUSE_RECORD)).ok(),
        )
    }

    #[test]
    fn install_swaps_the_candidate_and_writes_the_receipt_last() {
        let scratch = TemporaryDirectory::new("macos-install").unwrap();
        let root = scratch.path();
        tree(root, "old", "old receipt", Some("old record"));
        let staged = root.join("dist/.new");
        fs::create_dir_all(&staged).unwrap();
        fs::write(staged.join("marker"), "new").unwrap();
        install(root, &staged, None, "new receipt").unwrap();
        // Same-commit promotion: the old ancestor record must not survive beside the new receipt.
        assert_eq!(read(root), ("new".into(), "new receipt".into(), None));
        assert!(!staged.exists());
        let leftovers = fs::read_dir(root.join("dist"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .filter(|name| name.starts_with(".exact-candidate-macos"))
            .collect::<Vec<_>>();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn a_failed_install_puts_the_old_candidate_receipt_and_record_back() {
        let scratch = TemporaryDirectory::new("macos-install-failure").unwrap();
        let root = scratch.path();
        tree(root, "old", "old receipt", Some("old record"));
        // The prepared directory vanished: moving it into place fails after the old one moved.
        let error = install(
            root,
            &root.join("dist/.gone"),
            Some("new record"),
            "new receipt",
        )
        .unwrap_err();
        assert!(error.contains("into place"), "{error}");
        assert_eq!(
            read(root),
            (
                "old".into(),
                "old receipt".into(),
                Some("old record".into())
            )
        );
    }

    #[test]
    fn an_unreadable_receipt_stops_install_before_any_change() {
        let scratch = TemporaryDirectory::new("macos-install-unreadable").unwrap();
        let root = scratch.path();
        tree(root, "old", "old receipt", None);
        fs::remove_file(root.join(RECEIPT)).unwrap();
        // A directory where the receipt belongs cannot be read as one, nor taken as absent.
        fs::create_dir(root.join(RECEIPT)).unwrap();
        let staged = root.join("dist/.new");
        fs::create_dir_all(&staged).unwrap();
        assert!(install(root, &staged, None, "new receipt").is_err());
        assert_eq!(
            fs::read_to_string(root.join(STAGING).join("marker")).unwrap(),
            "old"
        );
        assert!(staged.exists());
    }

    #[test]
    fn a_second_promotion_waits_for_the_lock() {
        let scratch = TemporaryDirectory::new("macos-lock").unwrap();
        let first = PromotionLock::take(scratch.path()).unwrap();
        assert!(PromotionLock::take(scratch.path()).is_err());
        drop(first);
        // Other tests spawn processes in parallel; a child forked while the lock was held shares
        // its open file description until it execs, so the release can take a moment to show.
        let retaken = (0..200).any(|_| {
            PromotionLock::take(scratch.path()).is_ok() || {
                std::thread::sleep(std::time::Duration::from_millis(10));
                false
            }
        });
        assert!(retaken, "the lock is released when its holder is dropped");
    }

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
