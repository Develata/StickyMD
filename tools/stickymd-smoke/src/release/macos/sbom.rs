//! The macOS SBOM and checksum file: the reviewed Syft release for darwin-arm64 scans the
//! extracted package with the workspace manifest and lock, the SPDX output must cover the
//! packaged files, and the checksum file is written last, over the ZIP and the SBOM.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-release-artifact

use super::content::{APP_FILES, FILES};
use crate::release::platform::Platform;
use crate::release::temporary::TemporaryDirectory;
use crate::release::{json, macos::package::run};
use crate::{atomic_evidence, integrity, repository};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const SYFT_VERSION: &str = "1.50.0";
/// The release's checksum manifest; the same pin the Windows SBOM uses.
const CHECKSUMS_SHA256: &str = "bb8824a06c27c625fc103db5d7e9d7131ba2cc6e7c7a79318ee71686ede3c3f0";
/// `syft_1.50.0_darwin_arm64.tar.gz` as that manifest lists it (read 2026-11-19).
const ARCHIVE_SHA256: &str = "e32fdb9d47823fa633748a1efca2528fd77c37469ea93c9e40ab835da44e4cce";

pub(crate) struct Published {
    pub sbom_sha256: String,
    pub checksums_sha256: String,
}

/// A pinned release file in the tool cache, downloaded when absent and always checked.
fn pinned(root: &Path, name: &str, sha256: &str) -> Result<PathBuf, String> {
    let directory = root.join("target/release-tools/syft").join(SYFT_VERSION);
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let path = directory.join(name);
    if !path.is_file() {
        let partial = directory.join(format!("{name}.partial-{}", std::process::id()));
        let url =
            format!("https://github.com/anchore/syft/releases/download/v{SYFT_VERSION}/{name}");
        let status = Command::new("/usr/bin/curl")
            .args(["-fsSL", "--proto", "=https", "--retry", "3", "-o"])
            .arg(&partial)
            .arg(&url)
            .status()
            .map_err(|error| format!("cannot run curl: {error}"))?;
        if !status.success() {
            let _ = fs::remove_file(&partial);
            return Err(format!("cannot download {url}"));
        }
        fs::rename(&partial, &path).map_err(|error| error.to_string())?;
    }
    let actual = integrity::sha256(&path)?;
    if actual != sha256 {
        return Err(format!(
            "{} has SHA-256 {actual}, the reviewed pin is {sha256}",
            path.display()
        ));
    }
    Ok(path)
}

/// The reviewed `syft` executable, extracted next to its archive.
fn syft(root: &Path) -> Result<PathBuf, String> {
    let checksums = pinned(
        root,
        &format!("syft_{SYFT_VERSION}_checksums.txt"),
        CHECKSUMS_SHA256,
    )?;
    let archive_name = format!("syft_{SYFT_VERSION}_darwin_arm64.tar.gz");
    let listed = fs::read_to_string(&checksums)
        .map_err(|error| error.to_string())?
        .lines()
        .any(|line| line == format!("{ARCHIVE_SHA256}  {archive_name}"));
    if !listed {
        return Err("the pinned darwin archive is not in the pinned Syft manifest".to_owned());
    }
    let archive = pinned(root, &archive_name, ARCHIVE_SHA256)?;
    let directory = archive.parent().ok_or("the Syft cache has no parent")?;
    let executable = directory.join("syft-darwin-arm64");
    let extracted = TemporaryDirectory::new("syft-extract")?;
    run(
        "/usr/bin/tar",
        &[
            "-xzf".as_ref(),
            archive.as_os_str(),
            "-C".as_ref(),
            extracted.path().as_os_str(),
            "syft".as_ref(),
        ],
    )?;
    fs::rename(extracted.path().join("syft"), &executable).map_err(|error| error.to_string())?;
    extracted.close()?;
    Ok(executable)
}

/// The SPDX document must be SPDX 2.x, list packages, and name every packaged file the notices
/// and executable depend on, as Syft names files of a directory source (`/package/...`).
pub(crate) fn validate(text: &str) -> Result<(), String> {
    let document = json::parse(text)?;
    if !document
        .field("spdxVersion")?
        .string()?
        .to_ascii_uppercase()
        .starts_with("SPDX-2.")
    {
        return Err("Generated document is not an SPDX 2.x JSON SBOM".to_owned());
    }
    if document.field("packages")?.array()?.is_empty() {
        return Err("Generated SBOM contains no packages".to_owned());
    }
    let names = document
        .field("files")?
        .array()?
        .iter()
        .map(|file| file.field("fileName")?.string())
        .collect::<Result<Vec<_>, _>>()?;
    let required = FILES
        .iter()
        .map(|(name, _)| *name)
        .filter(|name| !name.ends_with("README.txt") && !name.ends_with("LICENSE.txt"))
        .chain(
            APP_FILES
                .iter()
                .copied()
                .filter(|name| name.contains("/MacOS/")),
        );
    for member in required {
        let required = format!("/package/{member}");
        if !names.contains(&required.as_str()) {
            return Err(format!(
                "Generated SBOM does not cover packaged file {required}"
            ));
        }
    }
    Ok(())
}

/// Scans the package ZIP and publishes `SBOM-macos-arm64.spdx.json`, then
/// `SHA256SUMS-macos-arm64.txt`, in the ZIP's directory; neither may exist yet.
pub(crate) fn publish(root: &Path, zip: &Path) -> Result<Published, String> {
    let platform = Platform::MacOsArm64;
    let directory = zip.parent().ok_or("the ZIP has no directory")?;
    let zip_name = zip
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("the ZIP name is not UTF-8")?;
    let version = repository::workspace_version(root)?;
    if zip_name != platform.archive_name(&version) {
        return Err(format!("expected {}", platform.archive_name(&version)));
    }
    let syft = syft(root)?;
    let context = TemporaryDirectory::new("macos-sbom")?;
    for file in ["Cargo.lock", "Cargo.toml"] {
        fs::copy(root.join(file), context.path().join(file)).map_err(|error| error.to_string())?;
    }
    run(
        "/usr/bin/ditto",
        &[
            "-x".as_ref(),
            "-k".as_ref(),
            zip.as_os_str(),
            context.path().join("package").as_os_str(),
        ],
    )?;
    let output = TemporaryDirectory::new("macos-sbom-output")?;
    let generated = output.path().join("sbom.json");
    let status = Command::new(&syft)
        .arg(format!("dir:{}", context.path().display()))
        .args(["--source-name", "StickyMD", "--source-version", &version])
        .arg("--output")
        .arg(format!("spdx-json={}", generated.display()))
        .env("SYFT_FILE_METADATA_SELECTION", "all")
        .env("SYFT_CHECK_FOR_APP_UPDATE", "false")
        .status()
        .map_err(|error| format!("cannot run Syft: {error}"))?;
    if !status.success() {
        return Err(format!("Syft {SYFT_VERSION} failed with {status}"));
    }
    let bytes = fs::read(&generated).map_err(|error| error.to_string())?;
    validate(std::str::from_utf8(&bytes).map_err(|_| "the SBOM is not UTF-8")?)?;
    let sbom = directory.join(platform.sbom_file());
    atomic_evidence::write_new(&sbom, &bytes)?;
    let sbom_sha256 = integrity::sha256(&sbom)?;
    let zip_sha256 = integrity::sha256(zip)?;
    let manifest = integrity::render_checksums(&[
        (zip_name, zip_sha256.as_str()),
        (platform.sbom_file(), sbom_sha256.as_str()),
    ])?;
    let checksums = directory.join(platform.checksum_file());
    atomic_evidence::write_new(&checksums, manifest.as_bytes())?;
    context.close()?;
    output.close()?;
    Ok(Published {
        sbom_sha256,
        checksums_sha256: integrity::sha256(&checksums)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document(files: &[&str]) -> String {
        let files = files
            .iter()
            .map(|name| format!("{{\"fileName\":\"{name}\"}}"))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"spdxVersion\":\"SPDX-2.3\",\"packages\":[{{\"name\":\"x\"}}],\"files\":[{files}]}}"
        )
    }

    #[test]
    fn the_sbom_must_cover_the_executable_notices_and_licenses() {
        let covered = [
            "/package/StickyMD/StickyMD.app/Contents/MacOS/StickyMD",
            "/package/StickyMD/THIRD_PARTY_NOTICES.txt",
            "/package/StickyMD/licenses/SIL-OFL-1.1.txt",
            "/package/StickyMD/licenses/KaTeX-fonts-NOTICE.txt",
        ];
        validate(&document(&covered)).unwrap();
        for missing in covered {
            let rest = covered
                .iter()
                .copied()
                .filter(|name| *name != missing)
                .collect::<Vec<_>>();
            let error = validate(&document(&rest)).unwrap_err();
            assert!(error.contains(missing), "{error}");
        }
        assert!(validate(&document(&covered).replace("SPDX-2.3", "SPDX-3.0")).is_err());
    }
}
