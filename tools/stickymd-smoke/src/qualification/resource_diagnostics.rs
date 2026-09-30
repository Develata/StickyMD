//! Diagnostic-only complete cohorts; no formal receipts, promotion or last-success writes.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

#![cfg_attr(not(windows), allow(dead_code))]
mod digest;
#[cfg(test)]
mod group_tests;
mod identity;
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
        let store = Self { identity, host };
        store.verify(root)?;
        Ok(store)
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
        let path = self.path(root, case)?;
        let cached = read_bounded(&path)
            .and_then(|document| record::decode(&document, &self.identity, case, now()?));
        self.matches(fresh()?)?;
        match cached {
            Ok(result) => {
                eprintln!(
                    "RESOURCE_RESUME case={} status=DIAGNOSTIC_REUSED",
                    case.key()
                );
                Ok(Some(result))
            }
            Err(_) => {
                eprintln!(
                    "RESOURCE_RESUME case={} status=MISS (missing, expired or invalid record)",
                    case.key()
                );
                Ok(None)
            }
        }
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
        super::receipt::validate_sha256(&self.identity.fingerprint, "diagnostic key")?;
        case.registered()?;
        let path = root
            .join(DIRECTORY)
            .join(&self.identity.fingerprint)
            .join(format!("{}.json", case.key()));
        if !super::module_ledger::is_within(root, &path, "target")
            || super::module_ledger::is_within(root, &path, "dist")
        {
            return Err("diagnostic cache path escaped its target directory".into());
        }
        Ok(path)
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
