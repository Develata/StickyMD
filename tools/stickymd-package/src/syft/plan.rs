//! Typed Syft transport facts shared by JSON clients and in-process workflows.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use super::{Kind, Pin};
use crate::evidence::escape_json;
use std::path::{Path, PathBuf};

pub(in crate::release) const DOWNLOAD_ATTEMPTS: u32 = 3;

pub(in crate::release) struct Download {
    pub kind: Kind,
    pub path: PathBuf,
    pub uri: String,
}

pub(in crate::release) struct Plan {
    pub version: String,
    pub archive: PathBuf,
    pub checksums: PathBuf,
    pub downloads: Vec<Download>,
    external: bool,
}

impl Plan {
    pub(super) fn new(root: &Path, external: bool, pending: &[Kind], pin: &Pin<'_>) -> Self {
        Self {
            version: pin.version.into(),
            archive: pin.path(root, Kind::Archive),
            checksums: pin.path(root, Kind::Checksums),
            downloads: pending
                .iter()
                .map(|&kind| Download {
                    kind,
                    path: pin.path(root, kind),
                    uri: format!(
                        "https://github.com/anchore/syft/releases/download/v{}/{}",
                        pin.version,
                        pin.name(kind)
                    ),
                })
                .collect(),
            external,
        }
    }

    pub(in crate::release) fn json(&self) -> String {
        let downloads = self
            .downloads
            .iter()
            .map(|d| {
                format!(
                    "{{\"kind\":\"{}\",\"label\":\"{}\",\"path\":\"{}\",\"uri\":\"{}\"}}",
                    d.kind.key(),
                    d.kind.label(),
                    escape_json(&d.path.to_string_lossy()),
                    escape_json(&d.uri)
                )
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"version\":\"{}\",\"external\":{},\"download_attempts\":{DOWNLOAD_ATTEMPTS},\"archive_path\":\"{}\",\"checksums_path\":\"{}\",\"downloads\":[{downloads}]}}",
            self.version,
            self.external,
            escape_json(&self.archive.to_string_lossy()),
            escape_json(&self.checksums.to_string_lossy())
        )
    }
}
