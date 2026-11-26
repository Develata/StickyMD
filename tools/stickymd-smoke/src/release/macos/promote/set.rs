//! What a downloaded macOS set establishes: exactly its three files, consistent with each other
//! and with the package inside, and the candidate receipt built from them and the attempt.
//! Nothing is staged here.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use super::super::content;
use super::super::package;
use crate::evidence::escape_json;
use crate::release::attachments::platform_files;
use crate::release::platform::Platform;
use crate::release::platform_inputs::{ALGORITHM, Reuse, fingerprint, verify_reuse};
use crate::{integrity, repository};
use std::fs;
use std::path::Path;

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
    crate::release::attachments::check_manifest_names(&manifest, &[&zip_name, &sbom_name])?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::macos::promote::ARTIFACT_PREFIX;
    use crate::release::temporary::TemporaryDirectory;
    use std::path::PathBuf;

    /// The downloaded set holds exactly the three plain files: a missing one, an extra one and a
    /// link are refused before any of them is read.
    #[test]
    fn the_set_check_refuses_missing_extra_and_linked_files() {
        let root =
            crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
        let version = crate::repository::workspace_version(&root).unwrap();
        let [zip, checksums, sbom] = super::platform_files(super::Platform::MacOsArm64, &version);
        let directory = TemporaryDirectory::new("macos-set").unwrap();
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
            crate::release::macos::promotion_check::render(&release, &digest),
            crate::release::macos::promotion_check::render(&"3".repeat(40), &digest)
        );
    }
}
