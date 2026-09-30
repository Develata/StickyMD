//! Publish a completed local ZIP without replacing different bytes; this is not package acceptance.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use super::{checksums, cli::PackagePublishOptions, temporary::TemporaryDirectory};
use crate::{atomic_evidence, integrity};
use std::{fs, path::Path};

pub(super) fn execute(options: &PackagePublishOptions) -> Result<(), String> {
    let hash = publish(options)?;
    println!("{{\"zip_sha256\":\"{hash}\",\"sbom_sha256\":null}}");
    Ok(())
}

pub(super) fn publish(options: &PackagePublishOptions) -> Result<String, String> {
    let output = checksums::destination(&options.output)?;
    let manifest = checksums::destination(&options.checksums)?;
    checksums::distinct_paths(&[&options.input, &output, &manifest])?;
    let temporary = TemporaryDirectory::new("package-publish")?;
    let snapshot = temporary.path().join("input.zip");
    fs::copy(&options.input, &snapshot)
        .map_err(|error| format!("cannot snapshot completed ZIP: {error}"))?;
    let hash = integrity::sha256(&snapshot)?;
    let text = checksums::manifest(&output, &hash, None)?;
    if !matches_existing(&output, &hash)? {
        let bytes = fs::read(&snapshot).map_err(|error| error.to_string())?;
        if let Err(error) = atomic_evidence::write_new(&output, &bytes) {
            // Another producer may have won CreateNew. Accept only identical complete bytes.
            if !matches_existing(&output, &hash)? {
                return Err(error);
            }
        }
    }
    temporary.close()?;
    // As for SBOM publication, this is not a multi-file transaction. A manifest failure
    // leaves the complete ZIP available for retry, returns failure, and emits no success.
    atomic_evidence::write(&manifest, text.as_bytes())?;
    Ok(hash)
}

fn matches_existing(path: &Path, expected: &str) -> Result<bool, String> {
    checksums::destination(path)?;
    if !path.exists() {
        return Ok(false);
    }
    if integrity::sha256(path)? != expected {
        return Err(format!(
            "Refusing to overwrite a different existing package: {}",
            path.display()
        ));
    }
    Ok(true)
}

#[cfg(test)]
mod tests;
