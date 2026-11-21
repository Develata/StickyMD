//! The one reuse judgement Windows qualification applies wherever the build source S of the
//! promoted artifact meets another commit R: the remote record, Promote, the resolver, module
//! evidence and readiness. Equal commits are accepted without Git; otherwise the release tool's
//! rule decides (S an ancestor of R with the same Windows Platform Build Input Fingerprint).
//! Promote writes the audit record of a reuse, and readiness checks it against the judgement.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use super::json;
use crate::release::platform::Platform;
use crate::release::platform_inputs::{Reuse, verify_reuse};

pub(super) const REUSE_RECORD: &str = "dist/evidence/release-reuse-windows-x64.json";
const PLATFORM: Platform = Platform::WindowsX64;
/// The version `platform_inputs::fingerprint` writes into its stream header.
use crate::release::platform_inputs::ALGORITHM;

/// Whether the Windows artifact built on `build_source` (S) may stand for `release_source` (R).
/// Both must be full lowercase commit ids, so one commit is never spelled two ways in the
/// receipts that compare them. The same id needs no Git: every caller holds an R it has already
/// checked as a commit (the Source Freeze HEAD, or a run commit compared with it); anything else
/// goes to the reuse rule, which checks both objects.
pub(super) fn candidate_source_matches(
    root: &Path,
    build_source: &str,
    release_source: &str,
) -> Result<Reuse, String> {
    for (label, commit) in [
        ("build source", build_source),
        ("release commit", release_source),
    ] {
        if commit.len() != 40
            || !commit
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(format!(
                "{label} must be a full lowercase commit SHA: {commit}"
            ));
        }
    }
    if build_source == release_source {
        return Ok(Reuse::SameCommit);
    }
    let key = (
        root.to_path_buf(),
        build_source.to_owned(),
        release_source.to_owned(),
    );
    // The guard is a temporary of this statement: Git never runs under the lock.
    let cached = judged().get(&key).cloned();
    if let Some(fingerprint) = cached {
        return Ok(Reuse::Ancestor { fingerprint });
    }
    let reuse = verify_reuse(root, PLATFORM, build_source, release_source)?;
    if let Reuse::Ancestor { fingerprint } = &reuse {
        judged().insert(key, fingerprint.clone());
    }
    Ok(reuse)
}

/// Accepted judgements of two distinct commits in this process: (repository, S, R) to the
/// fingerprint. In one repository, commit ids name their history and trees and the input table
/// is compiled in, so an accepted answer cannot change; another repository is asked again, as it
/// may not hold those objects. A refusal is never kept, since Git itself may have failed.
/// Readiness asks the same question for the resolver and for every module's evidence, and each
/// answer otherwise costs a merge-base and two full-tree fingerprints.
type Judged = HashMap<(PathBuf, String, String), String>;

fn judged() -> MutexGuard<'static, Judged> {
    static JUDGED: OnceLock<Mutex<Judged>> = OnceLock::new();
    JUDGED
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// After Promote wrote the candidate receipt whose bytes hash to `candidate_receipt_sha256`:
/// a reuse gets its audit record; the same commit leaves none. The caller holds the manual
/// store lock, as for the receipt itself.
pub(super) fn write_record(
    root: &Path,
    reuse: &Reuse,
    build_source: &str,
    release_source: &str,
    candidate_receipt_sha256: &str,
) -> Result<(), String> {
    match reuse {
        Reuse::SameCommit => remove_record(root),
        Reuse::Ancestor { fingerprint } => crate::atomic_evidence::write(
            &root.join(REUSE_RECORD),
            render_record(
                build_source,
                release_source,
                fingerprint,
                candidate_receipt_sha256,
            )
            .as_bytes(),
        ),
    }
}

/// Retires the record with the candidate it describes.
pub(super) fn remove_record(root: &Path) -> Result<(), String> {
    let path = root.join(REUSE_RECORD);
    match fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!("cannot invalidate {}: {error}", path.display())),
    }
}

/// Readiness: the record on disk is exactly the one Promote writes for this judgement and
/// candidate receipt, and there is none when S equals R.
pub(super) fn check_record(
    root: &Path,
    reuse: &Reuse,
    build_source: &str,
    release_source: &str,
    candidate_receipt_sha256: &str,
) -> Result<(), String> {
    let path = root.join(REUSE_RECORD);
    // The directory entry itself decides presence: a dangling link is not "no record".
    let recorded = match fs::symlink_metadata(&path) {
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => return Err(format!("cannot inspect {}: {error}", path.display())),
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(format!("{} is not a regular file", path.display()));
        }
        Ok(_) => Some(
            fs::read(&path).map_err(|error| format!("cannot read {}: {error}", path.display()))?,
        ),
    };
    match (reuse, recorded) {
        (Reuse::SameCommit, None) => Ok(()),
        (Reuse::SameCommit, Some(_)) => Err(format!(
            "STALE RECEIPT: {REUSE_RECORD} exists but the candidate was built on the Source Freeze commit"
        )),
        (Reuse::Ancestor { .. }, None) => Err(format!(
            "{REUSE_RECORD} is missing for a candidate built on {build_source} and reused on {release_source}"
        )),
        (Reuse::Ancestor { fingerprint }, Some(bytes)) => {
            let expected = render_record(
                build_source,
                release_source,
                fingerprint,
                candidate_receipt_sha256,
            );
            if bytes == expected.as_bytes() {
                Ok(())
            } else {
                Err(format!(
                    "STALE RECEIPT: {REUSE_RECORD} does not describe the reuse of the current candidate"
                ))
            }
        }
    }
}

/// Both fingerprints are recorded although equal: their equality is what allowed the reuse.
fn render_record(
    build_source: &str,
    release_source: &str,
    fingerprint: &str,
    candidate_receipt_sha256: &str,
) -> String {
    format!(
        concat!(
            "{{\"schema_version\":1,\"platform\":\"{}\",",
            "\"build_source\":\"{}\",\"release_source\":\"{}\",",
            "\"build_inputs\":\"{}\",\"release_inputs\":\"{}\",",
            "\"algorithm\":\"{}\",\"candidate_receipt_sha256\":\"{}\"}}\n"
        ),
        PLATFORM.label(),
        json::escape(build_source),
        json::escape(release_source),
        json::escape(fingerprint),
        json::escape(fingerprint),
        ALGORITHM,
        json::escape(candidate_receipt_sha256),
    )
}

#[cfg(test)]
pub(super) mod tests;
