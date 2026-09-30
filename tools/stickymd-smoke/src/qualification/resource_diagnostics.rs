//! Diagnostic-only complete cohorts; no formal receipts, promotion or last-success writes.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

#![cfg_attr(not(windows), allow(dead_code))]
mod batch;
#[cfg(test)]
mod batch_tests;
mod digest;
#[cfg(test)]
mod equivalence_tests;
#[cfg(test)]
mod group_tests;
mod identity;
mod lookup;
pub(crate) mod plan;
mod record;
#[cfg(test)]
mod tests;

use crate::{evidence::EvidenceResult, resource_plan::diagnostic::Unit};
use identity::Identity;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const DIRECTORY: &str = "target/resource-diagnostics/v2";
const MAX_BYTES: u64 = 1024 * 1024;
const MAX_AGE: u64 = 24 * 60 * 60;

pub(crate) struct Store {
    identity: Identity,
    host: String,
}

impl Store {
    pub(crate) fn open(root: &Path) -> Result<Self, String> {
        let host = identity::host()?;
        let identity = identity::capture(root, &host)?;
        // Establish the command identity. Every load/plan/save has its own fresh bracket;
        // checking again here would immediately repeat the first operation's pre-check.
        Ok(Self { identity, host })
    }

    pub(crate) fn verify(&self, root: &Path) -> Result<(), String> {
        self.matches(identity::capture(root, &self.host)?)
    }

    fn matches(&self, current: Identity) -> Result<(), String> {
        if current == self.identity {
            Ok(())
        } else {
            Err("diagnostic resource identity changed during execution; rerun the command".into())
        }
    }

    pub(crate) fn load(&self, root: &Path, case: Unit) -> Result<Option<EvidenceResult>, String> {
        self.load_using(root, case, || identity::capture(root, &self.host))
    }

    fn load_using(
        &self,
        root: &Path,
        case: Unit,
        mut fresh: impl FnMut() -> Result<Identity, String>,
    ) -> Result<Option<EvidenceResult>, String> {
        self.matches(fresh()?)?;
        let cached = self.inspect(root, case)?;
        self.matches(fresh()?)?;
        eprintln!(
            "RESOURCE_RESUME unit={} status={} reason=\"{}\"",
            case.key(),
            if cached.result.is_some() {
                "DIAGNOSTIC_REUSED"
            } else {
                "MISS"
            },
            crate::evidence::escape_json(&cached.reason)
        );
        Ok(cached.result)
    }

    pub(crate) fn save(
        &self,
        root: &Path,
        case: Unit,
        result: &EvidenceResult,
        elapsed: f64,
    ) -> Result<(), String> {
        self.save_using(root, case, result, elapsed, || {
            identity::capture(root, &self.host)
        })
    }

    fn save_using(
        &self,
        root: &Path,
        case: Unit,
        result: &EvidenceResult,
        elapsed: f64,
        mut fresh: impl FnMut() -> Result<Identity, String>,
    ) -> Result<(), String> {
        self.matches(fresh()?)?;
        let document = record::encode(&self.identity, case, result, elapsed, now()?)?;
        let path = self.path(root, case)?;
        fs::create_dir_all(path.parent().ok_or("missing cache parent")?)
            .map_err(|e| e.to_string())?;
        // Advisory only. Publishing the validated record remains the final operation.
        self.save_hint(root, case)?;
        self.matches(fresh()?)?;
        // Re-resolve junctions after directory creation, immediately before atomic publication.
        let path = self.path(root, case)?;
        crate::atomic_evidence::write(&path, document.as_bytes())?;
        eprintln!(
            "RESOURCE_RESUME case={} status=SAVED_COMPLETE_DIAGNOSTIC",
            case.key()
        );
        Ok(())
    }

    fn path(&self, root: &Path, case: Unit) -> Result<PathBuf, String> {
        lookup::cache_path(root, &self.identity.fingerprint, case)
    }
}

fn read_bounded(path: &Path) -> Result<String, String> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_BYTES {
        return Err("diagnostic cache too large".into());
    }
    String::from_utf8(bytes).map_err(|e| e.to_string())
}

fn now() -> Result<u64, String> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_secs())
        .map_err(|e| e.to_string())
}
