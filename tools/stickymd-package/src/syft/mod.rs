//! Reviewed Syft identity and local cache decisions; network transport remains in PowerShell.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

mod cache;
mod plan;
pub(super) use plan::{DOWNLOAD_ATTEMPTS, Download, Plan};
#[cfg(test)]
mod tests;

use crate::evidence::escape_json;
use std::path::{Path, PathBuf};

struct Pin<'a> {
    version: &'a str,
    archive_hash: &'a str,
    checksums_hash: &'a str,
}

// Migrated verbatim from generate-sbom.ps1; updating the tool is a separate change.
const PIN: Pin<'static> = Pin {
    version: "1.50.0",
    archive_hash: "815ee6973ec5dff6a671d7f41b0e78835a8c45b91d5a39f4743ea1cee833d3be",
    checksums_hash: "bb8824a06c27c625fc103db5d7e9d7131ba2cc6e7c7a79318ee71686ede3c3f0",
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    Archive,
    Checksums,
}

impl Kind {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value {
            "archive" => Ok(Self::Archive),
            "checksums" => Ok(Self::Checksums),
            _ => Err("Syft kind must be archive or checksums".into()),
        }
    }
    fn key(self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::Checksums => "checksums",
        }
    }
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Archive => "Syft archive",
            Self::Checksums => "Syft checksum manifest",
        }
    }
}

impl Pin<'_> {
    fn name(&self, kind: Kind) -> String {
        let suffix = match kind {
            Kind::Archive => "windows_amd64.zip",
            Kind::Checksums => "checksums.txt",
        };
        format!("syft_{}_{suffix}", self.version)
    }
    fn hash(&self, kind: Kind) -> &str {
        match kind {
            Kind::Archive => self.archive_hash,
            Kind::Checksums => self.checksums_hash,
        }
    }
    fn path(&self, root: &Path, kind: Kind) -> PathBuf {
        root.join("target/release-tools/syft")
            .join(self.version)
            .join(self.name(kind))
    }
}

pub(super) fn plan(root: &Path, provided: Option<&Path>) -> Result<(), String> {
    println!("{}", plan_json(root, provided, &PIN)?);
    Ok(())
}

fn plan_json(root: &Path, provided: Option<&Path>, pin: &Pin<'_>) -> Result<String, String> {
    Ok(Plan::new(
        root,
        provided.is_some(),
        &pending(root, provided, pin)?,
        pin,
    )
    .json())
}

fn pending(root: &Path, provided: Option<&Path>, pin: &Pin<'_>) -> Result<Vec<Kind>, String> {
    if let Some(path) = provided {
        if !path.is_file() {
            return Err(format!(
                "Syft executable does not exist: {}",
                path.display()
            ));
        }
        Ok(Vec::new())
    } else {
        cache::pending(root, pin)
    }
}

pub(super) fn prepare(
    root: &Path,
    provided: Option<&Path>,
    directory: &Path,
) -> Result<(Plan, Option<PathBuf>), String> {
    prepare_with_pin(root, provided, directory, &PIN)
}

fn prepare_with_pin(
    root: &Path,
    provided: Option<&Path>,
    directory: &Path,
    pin: &Pin<'_>,
) -> Result<(Plan, Option<PathBuf>), String> {
    let pending = pending(root, provided, pin)?;
    let plan = Plan::new(root, provided.is_some(), &pending, pin);
    let verified = if provided.is_none() && pending.is_empty() {
        Some(cache::snapshot(
            &pin.path(root, Kind::Archive),
            &pin.path(root, Kind::Checksums),
            directory,
            pin,
        )?)
    } else {
        None
    };
    Ok((plan, verified))
}

pub(super) fn publish(root: &Path, kind: Kind, input: &Path) -> Result<(), String> {
    publish_cached(root, kind, input)?;
    println!("SYFT_CACHE=VERIFIED");
    Ok(())
}

pub(super) fn publish_cached(root: &Path, kind: Kind, input: &Path) -> Result<(), String> {
    cache::publish(root, kind, input, &PIN)
}

pub(super) fn snapshot(
    archive: &Path,
    checksums: &Path,
    directory: &Path,
) -> Result<PathBuf, String> {
    cache::snapshot(archive, checksums, directory, &PIN)
}

pub(super) fn verify(archive: &Path, checksums: &Path, directory: &Path) -> Result<(), String> {
    let path = snapshot(archive, checksums, directory)?;
    println!(
        "{{\"archive_path\":\"{}\"}}",
        escape_json(&path.to_string_lossy())
    );
    Ok(())
}
