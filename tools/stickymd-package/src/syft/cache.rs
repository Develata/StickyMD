//! Hash-verified cache publication and immutable inputs for the extraction adapter.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use super::{Kind, Pin};
use crate::{atomic_evidence, integrity, release::temporary::TemporaryDirectory};
use std::{
    fs,
    path::{Path, PathBuf},
};

pub(super) fn pending(root: &Path, pin: &Pin<'_>) -> Result<Vec<Kind>, String> {
    let mut pending = Vec::new();
    for kind in [Kind::Archive, Kind::Checksums] {
        let path = pin.path(root, kind);
        // A matching filename/version directory is not proof of cached bytes.
        if !path.is_file() || integrity::sha256(&path)? != pin.hash(kind) {
            pending.push(kind);
        }
    }
    Ok(pending)
}

fn verify_file(path: &Path, kind: Kind, pin: &Pin<'_>) -> Result<(), String> {
    let actual = integrity::sha256(path)?;
    if actual != pin.hash(kind) {
        return Err(format!("{} checksum mismatch: {actual}", kind.label()));
    }
    Ok(())
}

pub(super) fn publish(root: &Path, kind: Kind, input: &Path, pin: &Pin<'_>) -> Result<(), String> {
    let temporary = TemporaryDirectory::new("syft-download")?;
    let snapshot = temporary.path().join(pin.name(kind));
    fs::copy(input, &snapshot)
        .map_err(|error| format!("cannot snapshot Syft download: {error}"))?;
    verify_file(&snapshot, kind, pin)?;
    let bytes = fs::read(&snapshot).map_err(|error| error.to_string())?;
    temporary.close()?;
    // Cache replacement is atomic; failed downloads/checks never replace old bytes.
    atomic_evidence::write(&pin.path(root, kind), &bytes)
}

pub(super) fn snapshot(
    archive: &Path,
    checksums: &Path,
    directory: &Path,
    pin: &Pin<'_>,
) -> Result<PathBuf, String> {
    let directory = std::path::absolute(directory).map_err(|error| error.to_string())?;
    let temporary = TemporaryDirectory::create(directory)?;
    let archive_copy = temporary.path().join(pin.name(Kind::Archive));
    let checksums_copy = temporary.path().join(pin.name(Kind::Checksums));
    for (input, output, kind) in [
        (archive, &archive_copy, Kind::Archive),
        (checksums, &checksums_copy, Kind::Checksums),
    ] {
        fs::copy(input, output)
            .map_err(|error| format!("cannot snapshot {}: {error}", kind.label()))?;
        verify_file(output, kind, pin)?;
    }
    let manifest = fs::read_to_string(&checksums_copy)
        .map_err(|error| format!("cannot read Syft checksum manifest: {error}"))?;
    verify_manifest(&manifest, pin)?;
    temporary.keep();
    Ok(archive_copy)
}

pub(super) fn verify_manifest(text: &str, pin: &Pin<'_>) -> Result<(), String> {
    // Upstream uses SHA256SUMS text mode (two spaces); share digest/name/duplicate
    // validation with repository manifests after normalizing only that separator.
    let mut normalized = String::new();
    for line in text
        .trim_start_matches('\u{feff}')
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let (hash, name) = line
            .split_once(char::is_whitespace)
            .ok_or("Invalid Syft checksum line")?;
        let name = name
            .trim_start()
            .strip_prefix('*')
            .unwrap_or(name.trim_start());
        normalized.push_str(&format!("{hash} *{name}\n"));
    }
    let entries = integrity::parse_checksums(&normalized)?;
    if entries
        .get(&pin.name(Kind::Archive).to_ascii_lowercase())
        .map(String::as_str)
        != Some(pin.archive_hash)
    {
        return Err(
            "Pinned Syft archive hash is not present in the verified upstream checksum manifest"
                .to_owned(),
        );
    }
    Ok(())
}
