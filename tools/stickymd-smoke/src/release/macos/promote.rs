//! Local promotion of the macOS Release Exact Artifact on a Mac: take one successful attempt of
//! `release-macos`, download its artifact by id, compare it byte for byte with the user's copy,
//! check it, stage it atomically in `dist/exact-candidate-macos/`, and only then write the
//! candidate receipt. The resolver (`resolve`) re-checks the staged bytes against that receipt.
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
use crate::{integrity, repository};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod install;
use install::install;

pub(super) const STAGING: &str = "dist/exact-candidate-macos";
pub(super) const RECEIPT: &str = "dist/evidence/release-candidate-macos.json";
pub(super) const REUSE_RECORD: &str = "dist/evidence/release-reuse-macos-arm64.json";
/// The promotion-check record, written after the receipt (M5 gate review B2).
pub(super) const PROMOTION_CHECK: &str = super::promotion_check::PATH;
/// The promotion lock: exclusive for a promotion, shared for the last reading of `macos-ready`.
pub(super) const PROMOTION_LOCK: &str = "dist/evidence/.macos-promotion.lock";
/// The artifact of attempt `n` is named for it, so each attempt's artifact is distinct.
pub(super) const ARTIFACT_PREFIX: &str = "stickymd-macos-arm64-release-";

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
pub(crate) fn check_release(root: &Path, release: &str) -> Result<(), String> {
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

/// `release`, lowercase, once it is a commit `check_release` accepts and the checkout is a clean
/// checkout of it, the source identity `macos-ready` requires (plan 11 #macos-release-artifact,
/// "与发布提交的关系").
fn on_release(root: &Path, release: &str) -> Result<String, String> {
    check_release(root, release)?;
    let release = release.to_ascii_lowercase();
    super::ready::identity::require(root, &release, "macos-promote")?;
    Ok(release)
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
#[derive(Debug)]
pub(super) struct PromotionLock {
    _file: fs::File,
}

impl PromotionLock {
    pub(super) fn take(root: &Path) -> Result<Self, String> {
        let path = root.join(PROMOTION_LOCK);
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
            Err(fs::TryLockError::WouldBlock) => Err(format!(
                "another macOS promotion or a readiness check holds {}",
                path.display()
            )),
            Err(fs::TryLockError::Error(error)) => {
                Err(format!("cannot lock {}: {error}", path.display()))
            }
        }
    }

    /// The lock shared, for reading the candidate while no promotion can swap it (M5 gate review
    /// B1): the file is opened read-only and never created, so `None` means no promotion has
    /// ever run here; a promotion holding it is an error.
    pub(super) fn share(root: &Path) -> Result<Option<Self>, String> {
        let path = root.join(PROMOTION_LOCK);
        let file = match fs::File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("cannot open {}: {error}", path.display())),
        };
        match file.try_lock_shared() {
            Ok(()) => Ok(Some(Self { _file: file })),
            Err(fs::TryLockError::WouldBlock) => Err(format!(
                "a macOS promotion is in progress ({} is held); check again when it is done",
                path.display()
            )),
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
/// moved into place, the reuse record (if any) written, and the receipt written last, then the
/// promotion-check record on R. Any failure before the receipt puts the old candidate, receipt
/// and records back. The checkout must be a clean R at the start and again before the receipt.
pub(crate) fn promote(root: &Path, request: &Request<'_>) -> Result<String, String> {
    // The promotion's re-checks count only on a clean checkout of R (M5 gate review B2): before
    // the lock is taken or anything downloaded, and again before the receipt is written.
    let release = on_release(root, request.release)?;
    let request = &Request {
        release: &release,
        ..*request
    };
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
    commit(root, &release, &staged, record.as_deref(), &json)?;
    if let Err(error) = download.close() {
        eprintln!("warning: promoted; the download directory was not removed: {error}");
    }
    Ok(json)
}

/// Installs the prepared candidate with `receipt` and, after it, the promotion-check record
/// naming `release` and the receipt's SHA-256; the checkout must still be a clean `release` just
/// before the receipt is written (M5 gate review B2).
fn commit(
    root: &Path,
    release: &str,
    staged: &Path,
    reuse: Option<&str>,
    receipt: &str,
) -> Result<(), String> {
    let check =
        super::promotion_check::render(release, &integrity::sha256_bytes(receipt.as_bytes())?);
    install(
        root,
        staged,
        install::Outputs {
            reuse,
            receipt,
            promotion: &check,
        },
        || on_release(root, release).map(drop),
    )
}

/// The reuse record Promote writes beside the receipt and the resolver compares byte for byte.
pub(super) fn reuse_record(
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

#[cfg(test)]
pub(super) mod tests {
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

    fn sample_receipt() -> Receipt {
        let hex = |digit: char| std::iter::repeat_n(digit, 64).collect::<String>();
        Receipt {
            source: "1".repeat(40),
            version: "0.3.0".into(),
            cargo_lock_sha256: hex('a'),
            build_inputs: hex('b'),
            run_id: 7,
            attempt: 2,
            artifact_id: 12,
            artifact_name: format!("{ARTIFACT_PREFIX}2"),
            zip_name: Platform::MacOsArm64.archive_name("0.3.0"),
            zip_sha256: hex('c'),
            checksums_sha256: hex('d'),
            sbom_sha256: hex('e'),
            facts: package::Facts {
                zip: PathBuf::from("/Users/someone/Downloads/x.zip"),
                zip_sha256: hex('c'),
                source_commit: "1".repeat(40),
                executable_sha256: hex('f'),
                bundle_sha256: hex('0'),
                app_inputs: hex('9'),
                signing: "AD_HOC",
                members: std::collections::BTreeMap::new(),
            },
        }
    }

    /// M5 gate review B2, sample (a): the receipt is the attempt's facts alone, with no release
    /// commit, time or machine path, so the same attempt promoted again on R writes the same
    /// bytes and USER records bound to its digest still stand; only the promotion-check record
    /// names R.
    #[test]
    fn the_same_attempt_gives_the_same_receipt_whatever_the_release_commit() {
        let first = sample_receipt().json();
        assert_eq!(first, sample_receipt().json());
        let release = "2".repeat(40);
        assert!(!first.contains(&release), "{first}");
        assert!(!first.contains("/Users/"), "{first}");
        for word in ["time", "unix", "date", "release_source"] {
            assert!(!first.contains(word), "{word}: {first}");
        }
        let digest = integrity::sha256_bytes(first.as_bytes()).unwrap();
        assert_ne!(
            super::super::promotion_check::render(&release, &digest),
            super::super::promotion_check::render(&"3".repeat(40), &digest)
        );
    }

    /// Runs `git` in a throwaway repository, with a fixed identity for its commits.
    pub(in crate::release::macos) fn scratch_git(root: &Path, arguments: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(root)
            .args(arguments)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {arguments:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    /// A throwaway checkout that ignores `dist/`, with one commit R on `origin/main`; returns R.
    fn release_checkout(root: &Path) -> String {
        scratch_git(root, &["init", "-q", "-b", "main"]);
        fs::write(root.join(".gitignore"), "/dist/\n").unwrap();
        fs::write(root.join("a.txt"), "1").unwrap();
        scratch_git(root, &["add", "-A"]);
        scratch_git(root, &["commit", "-q", "-m", "r"]);
        let r = scratch_git(root, &["rev-parse", "HEAD"]);
        scratch_git(root, &["update-ref", "refs/remotes/origin/main", &r]);
        r
    }

    /// M5 gate review B2: the source identity is checked again just before the receipt, and the
    /// record written after it names R and the SHA-256 of the receipt as written, the digest the
    /// resolver returns and the gate compares.
    #[test]
    fn the_commit_rechecks_r_and_records_the_receipt_it_wrote() {
        let scratch = TemporaryDirectory::new("macos-promote-commit").unwrap();
        let root = scratch.path();
        let r = release_checkout(root);
        let staged = root.join("dist/.new");
        fs::create_dir_all(&staged).unwrap();
        fs::write(root.join("a.txt"), "dirty").unwrap();
        let error = commit(root, &r, &staged, None, "the receipt").unwrap_err();
        assert!(error.contains("the work tree or index differs"), "{error}");
        assert!(!root.join(RECEIPT).exists() && !root.join(PROMOTION_CHECK).exists());
        assert!(!staged.exists() && !root.join(STAGING).exists());
        scratch_git(root, &["checkout", "-q", "--", "a.txt"]);
        fs::create_dir_all(&staged).unwrap();
        commit(root, &r, &staged, None, "the receipt").unwrap();
        let written = fs::read(root.join(RECEIPT)).unwrap();
        assert_eq!(written, b"the receipt");
        let check =
            super::super::promotion_check::parse(&fs::read(root.join(PROMOTION_CHECK)).unwrap())
                .unwrap();
        assert_eq!(check.release_source, r);
        assert_eq!(
            check.candidate_receipt_sha256,
            integrity::sha256_bytes(&written).unwrap()
        );
    }

    /// M5 gate review B2, sample (c): a promotion is refused, before it takes the lock or
    /// downloads anything, on a checkout whose HEAD is not R, or that is not clean.
    #[test]
    fn promote_refuses_a_checkout_other_than_a_clean_r() {
        let scratch = TemporaryDirectory::new("macos-promote-identity").unwrap();
        let root = scratch.path();
        let r = release_checkout(root);
        fs::write(root.join("a.txt"), "2").unwrap();
        scratch_git(root, &["commit", "-q", "-am", "later"]);
        let later = scratch_git(root, &["rev-parse", "HEAD"]);
        scratch_git(root, &["update-ref", "refs/remotes/origin/main", &later]);
        let zip = root.join("nowhere/StickyMD.zip");
        let request = |release| Request {
            run_id: 1,
            attempt: 1,
            release,
            zip: &zip,
        };
        // HEAD is a later commit on origin/main, with the same version and files as R.
        let error = promote(root, &request(&r)).unwrap_err();
        assert!(
            error.contains(&format!("clean checkout of {r}; HEAD is {later}")),
            "{error}"
        );
        // HEAD is R, with a change in the work tree.
        scratch_git(root, &["checkout", "-q", &r]);
        fs::write(root.join("a.txt"), "dirty").unwrap();
        let error = promote(root, &request(&r.to_ascii_uppercase())).unwrap_err();
        assert!(error.contains("the work tree or index differs"), "{error}");
        assert!(!root.join("dist").exists(), "nothing was locked or staged");
        // A clean checkout of R passes, its commit named in any case.
        scratch_git(root, &["checkout", "-q", "--", "a.txt"]);
        assert_eq!(on_release(root, &r.to_ascii_uppercase()).unwrap(), r);
    }
}
