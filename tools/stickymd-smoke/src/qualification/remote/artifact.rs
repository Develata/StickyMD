//! The authoritative copy of the recorded Windows workflow artifact. Promote asks GitHub for the
//! artifact by the id the remote receipt recorded, checks that the answer is that artifact of
//! that run and build source, and downloads exactly that id; a receipt whose id was changed to
//! another artifact's fails here instead of being written into the candidate.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::RemoteWorkflow;
use crate::qualification::receipt;
use crate::release::json;

pub(super) struct DownloadedArtifact {
    root: PathBuf,
    pub(super) path: PathBuf,
}

impl Drop for DownloadedArtifact {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
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

/// GitHub's answer for the recorded artifact must be that artifact: the same id and name, not
/// expired, uploaded by the recorded run on the recorded build source.
pub(super) fn check_metadata(text: &str, remote: &RemoteWorkflow) -> Result<(), String> {
    let artifact = json::parse(text)?;
    let id = artifact.field("id")?.unsigned()?;
    if id != remote.artifact_id {
        return Err(format!(
            "GitHub answered for artifact {id}, not the recorded {}",
            remote.artifact_id
        ));
    }
    if artifact.field("name")?.string()? != remote.artifact_name {
        return Err(format!(
            "artifact {id} is not named {}",
            remote.artifact_name
        ));
    }
    if !matches!(artifact.field("expired")?, json::Value::Bool(false)) {
        return Err(format!("artifact {id} has expired"));
    }
    let run = artifact.field("workflow_run")?;
    if run.field("id")?.unsigned()? != remote.run_id {
        return Err(format!(
            "artifact {id} does not belong to the recorded run {}",
            remote.run_id
        ));
    }
    if run.field("head_sha")?.string()? != remote.source_commit {
        return Err(format!(
            "artifact {id} was built on another commit than the recorded {}",
            remote.source_commit
        ));
    }
    Ok(())
}

/// Checks the recorded artifact on GitHub, downloads that id, and extracts it into a private
/// directory holding exactly the three expected files.
pub(super) fn download(root: &Path, remote: &RemoteWorkflow) -> Result<DownloadedArtifact, String> {
    let endpoint = format!(
        "repos/{{owner}}/{{repo}}/actions/artifacts/{}",
        remote.artifact_id
    );
    let metadata = gh(root, &["api", &endpoint])?;
    check_metadata(
        std::str::from_utf8(&metadata).map_err(|_| "the artifact metadata is not UTF-8")?,
        remote,
    )?;
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| format!("system clock precedes Unix epoch: {error}"))?
        .as_nanos();
    let base = std::env::temp_dir().join(format!(
        "stickymd-recorded-artifact-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&base)
        .map_err(|error| format!("cannot create artifact download directory: {error}"))?;
    let download = DownloadedArtifact {
        root: base.clone(),
        path: base.join("payload"),
    };
    let archive = base.join("artifact.zip");
    let bytes = gh(root, &["api", &format!("{endpoint}/zip")])?;
    fs::write(&archive, bytes).map_err(|error| format!("cannot save the artifact: {error}"))?;
    fs::create_dir(&download.path)
        .map_err(|error| format!("cannot create artifact payload directory: {error}"))?;
    super::extract_zip(&archive, &download.path)?;
    let expected_zip = format!(
        "StickyMD-{}-windows-x64-portable.zip",
        receipt::workspace_version(root)?
    );
    let observed = member_set(&download.path)?;
    let expected = expected_member_set(&[&expected_zip, "SHA256SUMS.txt", "SBOM.spdx.json"]);
    if observed != expected {
        return Err(format!(
            "recorded artifact member set is {observed:?}, expected {expected:?}"
        ));
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn remote() -> RemoteWorkflow {
        RemoteWorkflow {
            source_commit: "a".repeat(40),
            cargo_lock_sha256: "b".repeat(64),
            run_id: 123,
            attempt: 1,
            artifact_id: 456,
            artifact_name: receipt::RELEASE_ARTIFACT_NAME.to_owned(),
            workflow: "release".to_owned(),
            url: "https://example.invalid/run".to_owned(),
        }
    }

    fn answer(id: u64, name: &str, expired: bool, run: u64, head: &str) -> String {
        format!(
            r#"{{"id":{id},"name":"{name}","expired":{expired},"workflow_run":{{"id":{run},"head_sha":"{head}"}}}}"#
        )
    }

    #[test]
    fn only_the_recorded_artifact_of_the_recorded_run_and_source_is_accepted() {
        let remote = remote();
        let name = receipt::RELEASE_ARTIFACT_NAME;
        let head = "a".repeat(40);
        check_metadata(&answer(456, name, false, 123, &head), &remote).unwrap();
        for (label, text) in [
            // A receipt whose id was changed to another artifact: GitHub answers for that one.
            ("id", answer(457, name, false, 123, &head)),
            ("name", answer(456, "other", false, 123, &head)),
            ("expired", answer(456, name, true, 123, &head)),
            ("run", answer(456, name, false, 124, &head)),
            ("head", answer(456, name, false, 123, &"b".repeat(40))),
        ] {
            assert!(check_metadata(&text, &remote).is_err(), "{label}");
        }
        assert!(check_metadata("{}", &remote).is_err());
    }
}
