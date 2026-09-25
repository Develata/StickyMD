//! SPDX output validation and local publication; never promotes a release candidate.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use super::{checksums, cli::SbomOptions, json, temporary::TemporaryDirectory};
use crate::{atomic_evidence, integrity};
use std::{fs, path::Path};

const REQUIRED_FILES: &[&str] = &[
    r"\package\StickyMD\StickyMD.exe",
    r"\package\StickyMD\THIRD_PARTY_NOTICES.txt",
    r"\package\StickyMD\licenses\SIL-OFL-1.1.txt",
    r"\package\StickyMD\licenses\KaTeX-fonts-NOTICE.txt",
];

pub(super) fn validate(text: &str) -> Result<(), String> {
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
    for required in REQUIRED_FILES {
        if !names.iter().any(|name| name.eq_ignore_ascii_case(required)) {
            return Err(format!(
                "Generated SBOM does not cover packaged file {required}"
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_file(path: &Path) -> Result<(), String> {
    validate(
        &fs::read_to_string(path)
            .map_err(|error| format!("cannot read SBOM {}: {error}", path.display()))?,
    )
}

pub(super) fn publish(options: &SbomOptions) -> Result<(), String> {
    let output = checksums::destination(&options.output)?;
    let checksum = checksums::destination(&options.checksums)?;
    checksums::distinct_paths(&[&options.input, &options.zip, &output, &checksum])?;
    // Snapshot the untrusted adapter output so validation, hashing and publication use one input.
    let temp = TemporaryDirectory::new("sbom-verify")?;
    let snapshot = temp.path().join("SBOM.spdx.json");
    fs::copy(&options.input, &snapshot)
        .map_err(|error| format!("cannot snapshot SBOM: {error}"))?;
    let bytes = fs::read(&snapshot).map_err(|error| error.to_string())?;
    validate(std::str::from_utf8(&bytes).map_err(|error| format!("SBOM is not UTF-8: {error}"))?)?;
    let sbom_hash = integrity::sha256(&snapshot)?;
    let zip_hash = integrity::sha256(&options.zip)?;
    let manifest = checksums::manifest(&options.zip, &zip_hash, Some((&output, &sbom_hash)))?;
    temp.close()?;
    // Each file is atomically replaced. Publish the manifest last; a partial pair cannot verify.
    // This is not a multi-file transaction and does not write qualification evidence.
    atomic_evidence::write(&output, &bytes)?;
    atomic_evidence::write(&checksum, manifest.as_bytes())?;
    println!("SBOM_PATH={}\nSBOM_SHA256={sbom_hash}", output.display());
    Ok(())
}

#[cfg(test)]
mod tests;
