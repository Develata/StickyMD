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
    write_manifest(&manifest, text.as_bytes())?;
    Ok(hash)
}

/// Publishers of identical bytes race to replace the same manifest, and Windows can
/// reject a replace while the other publisher's rename is still completing. Retry
/// briefly and accept a manifest that already holds exactly this text; any other
/// failure is reported.
fn write_manifest(manifest: &Path, text: &[u8]) -> Result<(), String> {
    let mut delay = std::time::Duration::from_millis(10);
    let mut last_error = String::new();
    for _ in 0..5 {
        match atomic_evidence::write(manifest, text) {
            Ok(()) => return Ok(()),
            Err(error) => last_error = error,
        }
        if fs::read(manifest).is_ok_and(|existing| existing == text) {
            return Ok(());
        }
        std::thread::sleep(delay);
        delay *= 2;
    }
    Err(last_error)
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
