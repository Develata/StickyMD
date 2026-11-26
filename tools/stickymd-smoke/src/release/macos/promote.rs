//! Local promotion of the macOS Release Exact Artifact on a Mac: take one successful attempt of
//! `release-macos`, download its artifact by id, compare it byte for byte with the user's copy,
//! check it, stage it atomically in `dist/exact-candidate-macos/`, and only then write the
//! candidate receipt. The resolver (`resolve`) re-checks the staged bytes against that receipt.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use super::content;
use super::package::run;
use crate::integrity;
use crate::release::platform::Platform;
use crate::release::platform_inputs::{ALGORITHM, Reuse};
use crate::release::temporary::TemporaryDirectory;
use crate::release::{json, workflow};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

mod install;
mod set;
use install::install;
pub(crate) use set::verify_set;
use set::{Receipt, check_set};

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

/// `release` must be a full SHA contained in `origin/main` (the remote-tracking ref itself,
/// fix-round review R3-1), as the release authorization names it.
pub(crate) fn check_release(root: &Path, release: &str) -> Result<(), String> {
    integrity::validate_hex(release, 40, "release commit")?;
    crate::release::attachments::on_main(root, release)
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
