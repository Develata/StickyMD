//! Last-success authority for qualification modules: record a passing run under its
//! input fingerprint, and look up whether the current inputs have a reusable success.
//!
//! A success is reusable exactly when readiness accepts it. Lookup validates the
//! record, its archived evidence and every companion file under one shared store lock,
//! through the same evidence contract registration used, so `qualification modules`,
//! a formal rerun decision and release readiness can never disagree.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::fs;
use std::path::{Path, PathBuf};

use super::module_evidence::{self, Companion, Origin};
use super::module_registry::{self, ModuleId, module_for_receipt};
use super::receipt::{self, Candidate};
use crate::integrity;

pub(super) mod fingerprint;
mod paths;
mod record;
#[cfg(test)]
mod reuse_tests;
mod status;
mod store;
#[cfg(test)]
mod tests;

pub(super) use paths::is_success_storage_path;
use record::LedgerRecord;
pub(super) use status::{print_snapshot, print_status};
use store::LedgerStore;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompatibleSuccess {
    pub(super) module: ModuleId,
    pub(super) origin_source_commit: String,
    pub(super) origin_version: String,
    pub(super) origin_exe_sha256: String,
    pub(super) origin_zip_sha256: String,
    pub(super) evidence_path: PathBuf,
}

/// What the ledger holds for one module's current inputs.
#[derive(Debug)]
pub(super) enum Lookup {
    /// No record for these inputs; `other_inputs` when records for other inputs exist.
    Missing {
        other_inputs: bool,
    },
    /// A record exists, but it, its evidence or a companion file fails validation.
    /// Reuse is refused; a formal rerun replaces the record and re-archives companions.
    Invalid(String),
    Valid(CompatibleSuccess),
}

impl Lookup {
    /// The stable reason a module without a valid success must run.
    pub(super) const fn rerun_reason(&self) -> &'static str {
        match self {
            Self::Missing {
                other_inputs: false,
            } => "NO_LAST_SUCCESS",
            Self::Missing { other_inputs: true } => "INPUT_FINGERPRINT_CHANGED",
            Self::Invalid(_) => "INVALID_LAST_SUCCESS",
            Self::Valid(_) => "REUSABLE",
        }
    }

    /// `Some` for a valid success, `None` when there is none, the reason when invalid.
    pub(super) fn into_result(self) -> Result<Option<CompatibleSuccess>, String> {
        match self {
            Self::Valid(success) => Ok(Some(success)),
            Self::Missing { .. } => Ok(None),
            Self::Invalid(reason) => Err(reason),
        }
    }
}

/// Every registered module's lookup from one pass.
pub(super) struct Snapshot {
    pub(super) store_root: PathBuf,
    pub(super) modules: Vec<(ModuleId, Lookup)>,
}

/// Look up every registered module at once: a fresh fingerprint batch reads the shared
/// inputs once for the modules that have records, and one shared lock (one store scan)
/// covers every validation. Modules without records are not fingerprinted, and an
/// empty store is not created.
pub(super) fn lookup_all(root: &Path) -> Result<Snapshot, String> {
    let store = LedgerStore::for_repository(root)?;
    let mut modules = Vec::new();
    let mut recorded = Vec::new();
    for module in module_registry::modules() {
        if store.has_module_records(module.as_str())? {
            recorded.push(module);
        }
        modules.push((
            module,
            Lookup::Missing {
                other_inputs: false,
            },
        ));
    }
    if !recorded.is_empty() {
        let digests = fingerprint::PlanningInputs::read(root)?
            .calculate_many(root, &recorded)?
            .digests;
        let _guard = store.read_guard()?;
        for (module, current) in recorded.into_iter().zip(digests) {
            let lookup = load(&store, module, &current)?;
            if let Some(entry) = modules.iter_mut().find(|(id, _)| *id == module) {
                entry.1 = lookup;
            }
        }
    }
    Ok(Snapshot {
        store_root: store.root().to_path_buf(),
        modules,
    })
}

/// The current inputs' success for `module`, fingerprinting the checkout only when the
/// module has records at all; an empty store is never created by a lookup.
pub(super) fn lookup(root: &Path, module: ModuleId) -> Result<Lookup, String> {
    lookup_with(root, module, || fingerprint::calculate(root, module))
}

/// As [`lookup`], for an input digest the caller has just computed.
pub(super) fn lookup_for_input(
    root: &Path,
    module: ModuleId,
    current: &str,
) -> Result<Lookup, String> {
    lookup_with(root, module, || Ok(current.to_owned()))
}

#[cfg(test)]
pub(super) fn compatible_success(
    root: &Path,
    module: ModuleId,
) -> Result<Option<CompatibleSuccess>, String> {
    lookup(root, module)?.into_result()
}

fn lookup_with(
    root: &Path,
    module: ModuleId,
    current: impl FnOnce() -> Result<String, String>,
) -> Result<Lookup, String> {
    let store = LedgerStore::for_repository(root)?;
    if !store.has_module_records(module.as_str())? {
        return Ok(Lookup::Missing {
            other_inputs: false,
        });
    }
    let current = current()?;
    let _guard = store.read_guard()?;
    load(&store, module, &current)
}

/// Validate one module's record for `current` and everything it references. The caller
/// holds the shared store lock, so no writer can replace or prune what is checked.
/// Content problems are `Invalid`; only store-level failures are errors.
fn load(store: &LedgerStore, module: ModuleId, current: &str) -> Result<Lookup, String> {
    let path = store.module_record(module.as_str(), current)?;
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Lookup::Missing { other_inputs: true });
        }
        Err(error) => {
            return Ok(Lookup::Invalid(format!(
                "cannot read ledger record {}: {error}",
                path.display()
            )));
        }
    };
    Ok(match verify(store, module, current, &text) {
        Ok(success) => Lookup::Valid(success),
        Err(reason) => Lookup::Invalid(format!("ledger record {}: {reason}", path.display())),
    })
}

fn verify(
    store: &LedgerStore,
    module: ModuleId,
    current: &str,
    text: &str,
) -> Result<CompatibleSuccess, String> {
    let record = LedgerRecord::parse(text)?;
    if record.module_id != module.as_str() || record.input_fingerprint != current {
        return Err("record is filed under another module or fingerprint".to_owned());
    }
    let evidence_path = store.evidence(&record.evidence_file())?;
    // Hash and validate one snapshot of the archived bytes.
    let bytes = fs::read(&evidence_path)
        .map_err(|error| format!("cannot read evidence {}: {error}", evidence_path.display()))?;
    let actual = integrity::sha256_bytes(&bytes)?;
    if actual != record.evidence_sha256 {
        return Err(format!(
            "STALE RECEIPT: evidence hash is {actual}, expected {}",
            record.evidence_sha256
        ));
    }
    let document =
        std::str::from_utf8(&bytes).map_err(|error| format!("evidence is not UTF-8: {error}"))?;
    let origin = Origin {
        source_commit: &record.origin_source_commit,
        version: &record.origin_version,
        exe_sha256: &record.origin_exe_sha256,
        zip_sha256: &record.origin_zip_sha256,
        input_fingerprint: current,
    };
    for companion in module_evidence::validate(module, document, &origin)? {
        let archived = store.artifact(&companion.sha256)?;
        let actual = integrity::sha256(&archived)
            .map_err(|error| format!("archived companion {}: {error}", companion.path))?;
        if actual != companion.sha256 {
            return Err(format!(
                "STALE RECEIPT: archived companion {} hash is {actual}, expected {}",
                companion.path, companion.sha256
            ));
        }
    }
    Ok(CompatibleSuccess {
        module,
        origin_source_commit: record.origin_source_commit,
        origin_version: record.origin_version,
        origin_exe_sha256: record.origin_exe_sha256,
        origin_zip_sha256: record.origin_zip_sha256,
        evidence_path,
    })
}

pub(super) fn record_success(
    root: &Path,
    module: ModuleId,
    candidate: &Candidate,
) -> Result<(), String> {
    record_success_for_input(
        root,
        module,
        candidate,
        &fingerprint::calculate(root, module)?,
    )
}

/// Record the worktree receipt of `module` as the success of `candidate` for inputs
/// `current`, a digest the caller computed after the run finished. Registration applies
/// the same evidence contract as lookup, so an incomplete or foreign receipt fails here.
pub(super) fn record_success_for_input(
    root: &Path,
    module: ModuleId,
    candidate: &Candidate,
    current: &str,
) -> Result<(), String> {
    // Read the receipt once: the same bytes are validated, hashed and archived, so a
    // concurrent rewrite of the worktree receipt cannot be published as this success.
    let source = root.join(module.receipt());
    let bytes = fs::read(&source).map_err(|error| {
        format!(
            "cannot read {} module evidence {}: {error}",
            module.as_str(),
            source.display()
        )
    })?;
    let document = std::str::from_utf8(&bytes)
        .map_err(|error| format!("{} module evidence is not UTF-8: {error}", module.as_str()))?;
    let origin = Origin {
        source_commit: &candidate.source_commit,
        version: &candidate.version,
        exe_sha256: &candidate.exe_sha256,
        zip_sha256: &candidate.zip_sha256,
        input_fingerprint: current,
    };
    let companions = module_evidence::validate(module, document, &origin)?;
    let record = LedgerRecord {
        module_id: module.as_str().to_owned(),
        input_fingerprint: current.to_owned(),
        origin_source_commit: candidate.source_commit.clone(),
        origin_version: candidate.version.clone(),
        origin_exe_sha256: candidate.exe_sha256.clone(),
        origin_zip_sha256: candidate.zip_sha256.clone(),
        evidence_sha256: integrity::sha256_bytes(&bytes)?,
        recorded_at_unix: store::unix_seconds(),
    };
    let store = LedgerStore::for_repository(root)?;
    let guard = store.write_guard()?;
    // Companion files are archived before the record so a published record never
    // points at evidence that exists only in this (possibly temporary) worktree.
    for companion in &companions {
        archive_artifact(root, &store, companion)?;
    }
    crate::atomic_evidence::write(&store.evidence(&record.evidence_file())?, &bytes)?;
    let record_path = store.module_record(module.as_str(), current)?;
    crate::atomic_evidence::write(&record_path, record.render().as_bytes())?;
    // The success is durable from here; cleanup problems are reported, not fatal.
    let cleanup = store
        .prune_module(&guard, module.as_str(), &record_path)
        .and_then(|()| {
            if module == ModuleId::G5 {
                prune_g5_artifacts(&store, &guard)
            } else {
                Ok(())
            }
        });
    if let Err(error) = cleanup {
        eprintln!("LEDGER_PRUNE_SKIPPED={}: {error}", module.as_str());
    }
    Ok(())
}

/// Copy one companion file into the content-addressed store. The bytes that are hashed
/// are the bytes that are written; the caller holds the write lock. A missing or
/// damaged archive copy is rewritten, which is how a rerun repairs an invalid success.
fn archive_artifact(root: &Path, store: &LedgerStore, companion: &Companion) -> Result<(), String> {
    let Companion { path, sha256 } = companion;
    if !module_evidence::is_artifact_path(path) {
        return Err(format!("evidence has unsafe artifact path {path}"));
    }
    // This run's file must match its evidence even when the store already holds the
    // same content: a mismatch means the run's own output changed after capture.
    let bytes = fs::read(root.join(path))
        .map_err(|error| format!("cannot archive artifact {path}: {error}"))?;
    let actual = integrity::sha256_bytes(&bytes)?;
    if &actual != sha256 {
        return Err(format!(
            "artifact {path} hash is {actual}, expected {sha256}"
        ));
    }
    let target = store.artifact(sha256)?;
    if target.is_file() && integrity::sha256(&target)? == *sha256 {
        return Ok(());
    }
    crate::atomic_evidence::write(&target, &bytes)
}

/// Artifacts are pruned only against a complete, structurally valid scan of every
/// retained G5 evidence document; any failure keeps every artifact.
fn prune_g5_artifacts(store: &LedgerStore, guard: &store::StoreGuard) -> Result<(), String> {
    let mut referenced = Vec::new();
    for document in store.module_evidence_documents(ModuleId::G5.as_str())? {
        referenced.extend(
            module_evidence::g5_companions(&document)?
                .into_iter()
                .map(|companion| companion.sha256),
        );
    }
    store.prune_artifacts(guard, &referenced)
}

/// Whether a formal run may skip `path`'s module. An invalid success is reported and
/// rerun: the new run's registration replaces it.
pub(super) fn reuse_for_receipt(root: &Path, path: &Path) -> Result<bool, String> {
    let Some(module) = module_for_receipt(root, path) else {
        return Ok(false);
    };
    let candidate = receipt::read_candidate(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    match lookup(root, module)? {
        Lookup::Valid(success) => {
            println!(
                "MODULE_REUSED_PASS={} origin_source={} origin_exe={}",
                success.module.as_str(),
                success.origin_source_commit,
                success.origin_exe_sha256
            );
            Ok(true)
        }
        Lookup::Invalid(reason) => {
            println!(
                "MODULE_RUN_REQUIRED={} REASON=INVALID_LAST_SUCCESS DETAIL={}",
                module.as_str(),
                reason.replace(['\r', '\n'], " ")
            );
            Ok(false)
        }
        Lookup::Missing { .. } => Ok(false),
    }
}

pub(super) fn record_for_receipt(root: &Path, path: &Path) -> Result<(), String> {
    let Some(module) = module_for_receipt(root, path) else {
        return Ok(());
    };
    let candidate = receipt::read_candidate(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    record_success(root, module, &candidate)
}

/// The clone-wide record path of `module` for the checkout's current inputs.
#[cfg(test)]
pub(super) fn record_path(root: &Path, module: ModuleId) -> PathBuf {
    LedgerStore::for_repository(root)
        .unwrap()
        .module_record(
            module.as_str(),
            &fingerprint::calculate(root, module).unwrap(),
        )
        .unwrap()
}

/// The archived copy of one companion file.
#[cfg(test)]
pub(super) fn archived_artifact(root: &Path, sha256: &str) -> PathBuf {
    LedgerStore::for_repository(root)
        .unwrap()
        .artifact(sha256)
        .unwrap()
}
