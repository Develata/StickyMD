//! Portable package verification; no candidate resolver, ledger update or publication.
//! plan_ref: docs/plan/11_testing_and_release.md#portable-windows-runtime

use super::{
    cli::PackageOptions, identity, json, notices, package_rules, package_runtime, sbom,
    temporary::TemporaryDirectory, windows,
};
use crate::{integrity, package_path, pe_dependencies, repository};
use std::{fs, path::Path};

pub(super) fn verify(root: &Path, options: &PackageOptions) -> Result<(), String> {
    let directory = options
        .directory
        .clone()
        .unwrap_or_else(|| root.join("dist"));
    let zip = match &options.zip {
        Some(path) => path.clone(),
        None => package_path::resolve(root, &directory)?,
    };
    let zip = std::path::absolute(&zip).map_err(|error| error.to_string())?;
    let checksums = options
        .checksums
        .clone()
        .unwrap_or_else(|| directory.join("SHA256SUMS.txt"));
    let version = repository::workspace_version(root)?;
    let source = identity::source(root)?;
    let zip_name = zip
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("ZIP name is not Unicode")?;
    integrity::artifact_name(zip_name)?;
    // An explicit ZipPath must be the very file authenticated by PackageDirectory's manifest.
    verify_zip_location(&zip, &directory.join(zip_name))?;
    package_rules::size(fs::metadata(&zip).map_err(|error| error.to_string())?.len())?;
    let temporary = TemporaryDirectory::new("verify")?;
    let copied_zip = temporary.path().join("input.zip");
    fs::copy(&zip, &copied_zip).map_err(|error| format!("cannot snapshot ZIP: {error}"))?;
    let zip_hash = integrity::sha256(&copied_zip)?;
    let copied_sbom = temporary.path().join("SBOM.spdx.json");
    fs::copy(directory.join("SBOM.spdx.json"), &copied_sbom)
        .map_err(|error| format!("cannot snapshot SBOM: {error}"))?;
    let sbom_hash = integrity::sha256(&copied_sbom)?;
    integrity::verify_manifest_file(&checksums, zip_name, &zip_hash, &sbom_hash)?;
    sbom::validate_file(&copied_sbom)?;
    package_rules::size(
        fs::metadata(&copied_zip)
            .map_err(|error| error.to_string())?
            .len(),
    )?;
    package_rules::members(&windows::names(root, &copied_zip)?)?;
    windows::archive(
        root,
        "Extract",
        &copied_zip,
        &[
            "-DestinationPath",
            temporary
                .path()
                .to_str()
                .ok_or("scratch path is not Unicode")?,
        ],
    )?;
    let package = temporary.path().join("StickyMD");
    let readme = fs::read_to_string(package.join("README.txt"))
        .map_err(|error| format!("cannot read packaged README: {error}"))?;
    identity::readme_source(&readme, &source)?;
    let expected_notices = temporary.path().join("expected-third-party-notices.txt");
    notices::generate(root, &expected_notices)?;
    if integrity::sha256(&package.join("THIRD_PARTY_NOTICES.txt"))?
        != integrity::sha256(&expected_notices)?
    {
        return Err(
            "Packaged third-party notices do not match the frozen Windows runtime dependency graph"
                .to_owned(),
        );
    }
    let exe = package.join("StickyMD.exe");
    pe_dependencies::verify_package_image(&exe)?;
    let resources = windows::script(
        root,
        "resource-facts.ps1",
        &[
            "-ExePath",
            exe.to_str().ok_or("executable path is not Unicode")?,
        ],
    )?;
    package_rules::resources(&json::parse(&resources)?, &version)?;
    if options.runtime {
        package_runtime::verify(&exe, &temporary.path().join("runtime"))?;
    }
    temporary.close()?;
    println!("PACKAGE_VERIFY=PASS\nPACKAGE_PATH={}", zip.display());
    Ok(())
}

fn verify_zip_location(zip: &Path, manifest_zip: &Path) -> Result<(), String> {
    let resolve = |path: &Path| {
        path.canonicalize()
            .map_err(|error| format!("cannot resolve {}: {error}", path.display()))
    };
    if resolve(zip)? != resolve(manifest_zip)? {
        return Err(
            "ZipPath is not the artifact bound by PackageDirectory's checksum manifest".to_owned(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_zip_cannot_authenticate_a_different_same_named_file() {
        let scratch = TemporaryDirectory::new("package-binding-test").unwrap();
        let first = scratch.path().join("archive.zip");
        let directory = scratch.path().join("中文 with spaces");
        fs::create_dir(&directory).unwrap();
        let second = directory.join("archive.zip");
        fs::write(&first, b"unverified").unwrap();
        fs::write(&second, b"verified").unwrap();
        verify_zip_location(&second, &second).unwrap();
        assert!(verify_zip_location(&first, &second).is_err());
    }
}
