//! The Release Attachment Set and the checks the promotion workflow runs on it: the one approved
//! set is Windows + macOS, six files named for the version; every operation compares the files
//! it holds with that set by name and hash, then checks each platform's three files against each
//! other and its build source against the release commit.
//! plan_ref: docs/plan/11_testing_and_release.md#release-attachment-set
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use super::macos::{content, listing};
use super::platform::Platform;
use super::platform_inputs::{Reuse, verify_reuse};
use crate::{integrity, repository};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The three names a platform's set uses for `version`.
pub(crate) fn platform_files(platform: Platform, version: &str) -> [String; 3] {
    [
        platform.archive_name(version),
        platform.checksum_file().to_owned(),
        platform.sbom_file().to_owned(),
    ]
}

/// Parses `name=sha256,...`: exactly the six files of the approved set for `version`, each once,
/// each with a full lowercase SHA-256.
pub(crate) fn parse_spec(spec: &str, version: &str) -> Result<BTreeMap<String, String>, String> {
    let mut entries = BTreeMap::new();
    for item in spec
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let (name, hash) = item
            .split_once('=')
            .ok_or_else(|| format!("attachment `{item}` is not name=sha256"))?;
        integrity::artifact_name(name)?;
        integrity::validate_sha256(hash, name)?;
        if hash.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return Err(format!("the SHA-256 of {name} must be lowercase"));
        }
        if entries.insert(name.to_owned(), hash.to_owned()).is_some() {
            return Err(format!("attachment {name} is listed twice"));
        }
    }
    let expected = Platform::ALL
        .iter()
        .flat_map(|platform| platform_files(*platform, version))
        .collect::<Vec<_>>();
    let missing = expected
        .iter()
        .filter(|name| !entries.contains_key(*name))
        .collect::<Vec<_>>();
    let extra = entries
        .keys()
        .filter(|name| !expected.contains(name))
        .collect::<Vec<_>>();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(format!(
            "the attachment set must be exactly the Windows and macOS files of {version}; missing {missing:?}, extra {extra:?}"
        ));
    }
    Ok(entries)
}

/// The regular files directly in `directory` must be exactly `expected`, by name and SHA-256:
/// a missing, extra or stale file fails.
pub(crate) fn verify_directory(
    directory: &Path,
    expected: &BTreeMap<String, String>,
) -> Result<(), String> {
    let mut found = BTreeMap::new();
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("cannot list {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| "an attachment name is not UTF-8")?;
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        if !kind.is_file() {
            return Err(format!(
                "{name} in {} is not a regular file",
                directory.display()
            ));
        }
        found.insert(name, entry.path());
    }
    let missing = expected
        .keys()
        .filter(|name| !found.contains_key(*name))
        .collect::<Vec<_>>();
    let extra = found
        .keys()
        .filter(|name| !expected.contains_key(*name))
        .collect::<Vec<_>>();
    if !missing.is_empty() || !extra.is_empty() {
        return Err(format!(
            "{} does not hold exactly the approved attachments; missing {missing:?}, extra {extra:?}",
            directory.display()
        ));
    }
    for (name, path) in &found {
        let actual = integrity::sha256(path)?;
        if actual != expected[name] {
            return Err(format!(
                "{name} has SHA-256 {actual}, the approved set says {}",
                expected[name]
            ));
        }
    }
    Ok(())
}

pub(crate) struct ReleaseSet {
    pub release_source: String,
    pub build_sources: Vec<(Platform, String, Reuse)>,
}

impl ReleaseSet {
    pub(crate) fn json(&self) -> String {
        let platforms = self
            .build_sources
            .iter()
            .map(|(platform, source, reuse)| {
                let reuse = match reuse {
                    Reuse::SameCommit => "same-commit".to_owned(),
                    Reuse::Ancestor { fingerprint } => format!("ancestor:{fingerprint}"),
                };
                format!(
                    "\"{}\":{{\"build_source\":\"{source}\",\"reuse\":\"{reuse}\"}}",
                    platform.label()
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"release_source\":\"{}\",{platforms}}}",
            self.release_source
        )
    }
}

/// Every name a checksum manifest lists must be spelled exactly as one of `names`: the
/// attestation takes its subject names from these lines, so a name that only matches
/// case-insensitively would attest a file that is not the attachment.
pub(crate) fn check_manifest_names(manifest: &str, names: &[&str]) -> Result<(), String> {
    for line in manifest
        .trim_start_matches('\u{feff}')
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let (_, name) = line
            .split_once(" *")
            .ok_or_else(|| format!("Invalid checksum line: {line}"))?;
        if !names.contains(&name) {
            return Err(format!(
                "the checksum manifest names {name}, which is not spelled as an attachment"
            ));
        }
    }
    Ok(())
}

/// R must be on the main line: every promotion operation starts from a commit `origin/main`
/// reaches, whatever ref the workflow was dispatched from.
fn on_main(root: &Path, release: &str) -> Result<(), String> {
    let status = Command::new("git")
        .current_dir(root)
        .args(["merge-base", "--is-ancestor", release, "origin/main"])
        .status()
        .map_err(|error| format!("cannot run git: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!(
            "the release commit {release} is not on origin/main"
        ))
    }
}

/// The SHA-256 of each SBOM member of a macOS ZIP, from a `tar` extraction into a scratch
/// directory (after `listing::verify_archive` has checked the member list).
fn macos_member_hashes(zip: &Path) -> Result<BTreeMap<&'static str, String>, String> {
    let scratch = super::temporary::TemporaryDirectory::new("release-set-macos")?;
    let status = Command::new("tar")
        .arg("-xf")
        .arg(zip)
        .arg("-C")
        .arg(scratch.path())
        .status()
        .map_err(|error| format!("cannot run tar: {error}"))?;
    if !status.success() {
        return Err(format!("cannot extract {}", zip.display()));
    }
    let hashes = content::member_hashes(scratch.path())?;
    scratch.close()?;
    Ok(hashes)
}

/// The README a platform's ZIP carries, read with `tar` (bsdtar reads ZIP on every runner).
fn readme_of(zip: &Path) -> Result<String, String> {
    let output = Command::new("tar")
        .arg("-xOf")
        .arg(zip)
        .arg(content::README)
        .output()
        .map_err(|error| format!("cannot run tar: {error}"))?;
    if !output.status.success() {
        return Err(format!("cannot read the README of {}", zip.display()));
    }
    String::from_utf8(output.stdout).map_err(|_| "the package README is not UTF-8".to_owned())
}

/// Checks both platforms' directories against the approved set and each other: the files, the
/// checksum manifest, the SBOM, the macOS inventory, the build source each README names, and that
/// source's reuse on the release commit, which must be the checked-out HEAD and match the tag.
pub(crate) fn verify_release_set(
    root: &Path,
    release: &str,
    tag: &str,
    spec: &str,
    directories: &[(Platform, PathBuf)],
) -> Result<ReleaseSet, String> {
    let version = repository::workspace_version(root)?;
    super::identity::release_tag(&version, tag)?;
    let head = super::identity::source(root)?;
    if !head.eq_ignore_ascii_case(release) {
        return Err(format!(
            "checked-out HEAD {head} is not the release commit {release}"
        ));
    }
    on_main(root, release)?;
    let approved = parse_spec(spec, &version)?;
    let mut build_sources = Vec::new();
    for platform in Platform::ALL {
        let directory = directories
            .iter()
            .find(|(candidate, _)| *candidate == platform)
            .map(|(_, directory)| directory)
            .ok_or_else(|| format!("no directory for {}", platform.label()))?;
        let [zip_name, checksums_name, sbom_name] = platform_files(platform, &version);
        let expected = [&zip_name, &checksums_name, &sbom_name]
            .into_iter()
            .map(|name| (name.clone(), approved[name].clone()))
            .collect::<BTreeMap<_, _>>();
        verify_directory(directory, &expected)?;
        let manifest = fs::read_to_string(directory.join(&checksums_name))
            .map_err(|error| format!("cannot read {checksums_name}: {error}"))?;
        integrity::verify_manifest_text(
            &manifest,
            &[
                (zip_name.as_str(), approved[&zip_name].as_str()),
                (sbom_name.as_str(), approved[&sbom_name].as_str()),
            ],
        )?;
        check_manifest_names(&manifest, &[&zip_name, &sbom_name])?;
        let zip = directory.join(&zip_name);
        let sbom = fs::read_to_string(directory.join(&sbom_name))
            .map_err(|error| format!("cannot read {sbom_name}: {error}"))?;
        let source = match platform {
            Platform::WindowsX64 => {
                super::sbom::validate(&sbom)?;
                content::source_commit(&readme_of(&zip)?)?
            }
            Platform::MacOsArm64 => {
                let source = listing::verify_archive(&zip)?;
                content::validate_sbom(&sbom, &macos_member_hashes(&zip)?)?;
                source
            }
        };
        let reuse = verify_reuse(root, platform, &source, release)?;
        build_sources.push((platform, source, reuse));
    }
    Ok(ReleaseSet {
        release_source: release.to_ascii_lowercase(),
        build_sources,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::temporary::TemporaryDirectory;

    fn spec(version: &str, skip: Option<&str>, extra: Option<&str>) -> String {
        Platform::ALL
            .iter()
            .flat_map(|platform| platform_files(*platform, version))
            .filter(|name| Some(name.as_str()) != skip)
            .chain(extra.map(str::to_owned))
            .map(|name| format!("{name}={}", "a".repeat(64)))
            .collect::<Vec<_>>()
            .join(",")
    }

    #[test]
    fn the_only_approved_set_is_both_platforms_six_files() {
        let parsed = parse_spec(&spec("0.3.0", None, None), "0.3.0").unwrap();
        assert_eq!(parsed.len(), 6);
        assert!(parsed.contains_key("StickyMD-0.3.0-windows-x64-portable.zip"));
        assert!(parsed.contains_key("SBOM-macos-arm64.spdx.json"));
        // Windows alone, an extra file, another version, a bad hash: all refused.
        let windows_only = spec("0.3.0", Some("SHA256SUMS-macos-arm64.txt"), None);
        assert!(parse_spec(&windows_only, "0.3.0").is_err());
        assert!(parse_spec(&spec("0.3.0", None, Some("notes.txt")), "0.3.0").is_err());
        assert!(parse_spec(&spec("0.3.0", None, None), "0.3.1").is_err());
        let upper = spec("0.3.0", None, None).replacen("=a", "=A", 1);
        assert!(parse_spec(&upper, "0.3.0").is_err());
        let twice = format!(
            "{},{}",
            spec("0.3.0", None, None),
            spec("0.3.0", None, None)
        );
        assert!(parse_spec(&twice, "0.3.0").is_err());
    }

    #[test]
    fn checksum_names_must_be_spelled_as_the_attachments() {
        let hash = "a".repeat(64);
        let names = [
            "StickyMD-0.3.0-macos-arm64.zip",
            "SBOM-macos-arm64.spdx.json",
        ];
        let manifest = |sbom: &str| format!("{hash} *{}\n{hash} *{sbom}\n", names[0]);
        check_manifest_names(&manifest(names[1]), &names).unwrap();
        assert!(check_manifest_names(&manifest("sbom-macos-arm64.spdx.json"), &names).is_err());
    }

    #[test]
    fn a_directory_must_hold_exactly_the_approved_files_and_bytes() {
        let scratch = TemporaryDirectory::new("attachments").unwrap();
        let directory = scratch.path();
        fs::write(directory.join("a.zip"), b"zip").unwrap();
        fs::write(directory.join("b.txt"), b"sums").unwrap();
        let expected = [("a.zip", b"zip".as_slice()), ("b.txt", b"sums".as_slice())]
            .into_iter()
            .map(|(name, bytes)| {
                let path = directory.join(format!("{name}.probe"));
                fs::write(&path, bytes).unwrap();
                let hash = integrity::sha256(&path).unwrap();
                fs::remove_file(path).unwrap();
                (name.to_owned(), hash)
            })
            .collect::<BTreeMap<_, _>>();
        verify_directory(directory, &expected).unwrap();
        // A stale extra file, a changed byte and a missing file are each refused.
        fs::write(directory.join("old.zip"), b"old").unwrap();
        assert!(
            verify_directory(directory, &expected)
                .unwrap_err()
                .contains("old.zip")
        );
        fs::remove_file(directory.join("old.zip")).unwrap();
        fs::write(directory.join("a.zip"), b"zap").unwrap();
        assert!(
            verify_directory(directory, &expected)
                .unwrap_err()
                .contains("a.zip")
        );
        fs::remove_file(directory.join("a.zip")).unwrap();
        assert!(
            verify_directory(directory, &expected)
                .unwrap_err()
                .contains("missing")
        );
    }
}
