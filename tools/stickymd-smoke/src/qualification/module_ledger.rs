//! Input fingerprints and last-success authority for qualification modules.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::fs;
use std::path::{Path, PathBuf};

use super::json;
use super::module_registry::{ModuleId, module_for_receipt};
use super::receipt::{self, Candidate};

pub(super) mod fingerprint;
mod paths;
#[cfg(test)]
mod reuse_tests;
mod status;
pub(super) mod store;
#[cfg(test)]
mod tests;

pub(super) use paths::is_success_storage_path;
pub(super) use status::{print_status, print_status_for_candidate, rerun_reason};
use store::LedgerStore;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct CompatibleSuccess {
    pub(super) module: ModuleId,
    pub(super) origin_source_commit: String,
    pub(super) origin_version: String,
    pub(super) origin_exe_sha256: String,
    pub(super) origin_zip_sha256: String,
    pub(super) evidence_path: PathBuf,
    pub(super) document: String,
    pub(super) store: LedgerStore,
}

impl CompatibleSuccess {
    /// Content-addressed companion evidence archived with this success.
    pub(super) fn artifact(&self, sha256: &str) -> Result<PathBuf, String> {
        self.store.artifact(sha256)
    }
}

pub(super) fn compatible_success(
    root: &Path,
    module: ModuleId,
) -> Result<Option<CompatibleSuccess>, String> {
    with_success(
        root,
        module,
        || fingerprint::calculate(root, module),
        |success| success.cloned(),
    )
}

pub(super) fn compatible_success_for_input(
    root: &Path,
    module: ModuleId,
    current: &str,
) -> Result<Option<CompatibleSuccess>, String> {
    with_success(
        root,
        module,
        || Ok(current.to_owned()),
        |success| success.cloned(),
    )
}

/// Run `inspect` over one loaded success while the store's shared lock is held, so
/// every file the caller follows from that record (for example G5 screenshots) stays
/// in place until it returns. Identity and companion checks then see one snapshot.
pub(super) fn with_compatible_success<T>(
    root: &Path,
    module: ModuleId,
    inspect: impl FnOnce(Option<&CompatibleSuccess>) -> T,
) -> Result<T, String> {
    with_success(
        root,
        module,
        || fingerprint::calculate(root, module),
        inspect,
    )
}

fn with_success<T>(
    root: &Path,
    module: ModuleId,
    current: impl FnOnce() -> Result<String, String>,
    inspect: impl FnOnce(Option<&CompatibleSuccess>) -> T,
) -> Result<T, String> {
    let store = LedgerStore::for_repository(root)?;
    // An empty module directory cannot match; skip fingerprinting the checkout.
    if !store.has_module_records(module.as_str())? {
        return Ok(inspect(None));
    }
    let current = current()?;
    let path = store.module_record(module.as_str(), &current)?;
    let _guard = store.read_guard()?;
    let success = load_success(&store, module, &current, &path)?;
    Ok(inspect(success.as_ref()))
}

/// Read one record and its evidence; the caller holds the shared store lock.
fn load_success(
    store: &LedgerStore,
    module: ModuleId,
    current: &str,
    path: &Path,
) -> Result<Option<CompatibleSuccess>, String> {
    if !path.is_file() {
        return Ok(None);
    }
    let document = receipt::read_receipt(path)?;
    validate_success_schema(&document, module)?;
    if json::string_field(&document, "input_fingerprint")? != current {
        return Err(format!(
            "module {} ledger record {} is filed under a different fingerprint",
            module.as_str(),
            path.display()
        ));
    }
    let evidence_file = json::string_field(&document, "evidence_file")?;
    if !evidence_file.starts_with(&format!("{}-", module.as_str())) {
        return Err(format!(
            "module {} success references unexpected evidence {evidence_file}",
            module.as_str()
        ));
    }
    let evidence_path = store.evidence(&evidence_file)?;
    // Hash and parse one snapshot of the archived bytes.
    let evidence_bytes = fs::read(&evidence_path).map_err(|error| {
        format!(
            "cannot read module {} evidence {}: {error}",
            module.as_str(),
            evidence_path.display()
        )
    })?;
    let expected_evidence = json::string_field(&document, "evidence_sha256")?;
    let actual_evidence = receipt::sha256_bytes(&evidence_bytes)?;
    if expected_evidence != actual_evidence {
        return Err(format!(
            "STALE RECEIPT: module {} evidence hash is {actual_evidence}, expected {expected_evidence}",
            module.as_str()
        ));
    }
    let evidence_document = String::from_utf8(evidence_bytes)
        .map_err(|error| format!("module {} evidence is not UTF-8: {error}", module.as_str()))?;
    validate_success_evidence(&evidence_document, module)?;
    if matches!(module, ModuleId::Resource(_))
        && (json::string_field(&evidence_document, "resource_input_fingerprint")? != current
            || json::string_field(&evidence_document, "commit")?
                != json::string_field(&document, "origin_source_commit")?
            || json::string_field(&evidence_document, "executable_sha256")?
                != json::string_field(&document, "origin_exe_sha256")?)
    {
        return Err("resource evidence identity differs from its last-success entry".to_owned());
    }
    Ok(Some(CompatibleSuccess {
        module,
        origin_source_commit: json::string_field(&document, "origin_source_commit")?,
        origin_version: json::string_field(&document, "origin_version")?,
        origin_exe_sha256: json::string_field(&document, "origin_exe_sha256")?,
        origin_zip_sha256: json::string_field(&document, "origin_zip_sha256")?,
        evidence_path,
        document: evidence_document,
        store: store.clone(),
    }))
}

pub(super) fn record_success(
    root: &Path,
    module: ModuleId,
    candidate: &Candidate,
) -> Result<(), String> {
    // Read the receipt once: the same bytes are validated, hashed and archived, so a
    // concurrent rewrite of the worktree receipt cannot be published as this success.
    let source_evidence = root.join(module.receipt());
    let evidence_bytes = fs::read(&source_evidence).map_err(|error| {
        format!(
            "cannot read {} module evidence {}: {error}",
            module.as_str(),
            source_evidence.display()
        )
    })?;
    let source_document = std::str::from_utf8(&evidence_bytes)
        .map_err(|error| format!("{} module evidence is not UTF-8: {error}", module.as_str()))?;
    validate_success_evidence(source_document, module)?;
    let input_fingerprint = fingerprint::calculate(root, module)?;
    if matches!(module, ModuleId::Resource(_))
        && (json::string_field(source_document, "resource_input_fingerprint")? != input_fingerprint
            || json::string_field(source_document, "commit")? != candidate.source_commit
            || json::string_field(source_document, "executable_sha256")? != candidate.exe_sha256)
    {
        return Err("resource inputs or candidate changed during measurement".to_owned());
    }
    let artifacts = if module == ModuleId::G5 {
        super::g5_readiness::companion_artifacts(source_document)?
    } else {
        Vec::new()
    };
    let evidence_sha256 = receipt::sha256_bytes(&evidence_bytes)?;
    let evidence_file = format!("{}-{evidence_sha256}.json", module.as_str());
    let store = LedgerStore::for_repository(root)?;
    let guard = store.write_guard()?;
    // Companion files are archived before the record so a published record never
    // points at evidence that exists only in this (possibly temporary) worktree.
    for (relative, sha256) in &artifacts {
        archive_artifact(root, &store, relative, sha256)?;
    }
    crate::atomic_evidence::write(&store.evidence(&evidence_file)?, &evidence_bytes)?;
    let record_path = store.module_record(module.as_str(), &input_fingerprint)?;
    let document = format!(
        concat!(
            "{{\"schema_version\":2,\"status\":\"PASSED\",",
            "\"module_id\":\"{}\",\"input_fingerprint\":\"{}\",",
            "\"origin_source_commit\":\"{}\",\"origin_version\":\"{}\",",
            "\"origin_exe_sha256\":\"{}\",\"origin_zip_sha256\":\"{}\",",
            "\"evidence_file\":\"{}\",\"evidence_sha256\":\"{}\",",
            "\"recorded_at_unix\":{}}}\n"
        ),
        module.as_str(),
        input_fingerprint,
        json::escape(&candidate.source_commit),
        json::escape(&candidate.version),
        json::escape(&candidate.exe_sha256),
        json::escape(&candidate.zip_sha256),
        evidence_file,
        evidence_sha256,
        store::unix_seconds(),
    );
    crate::atomic_evidence::write(&record_path, document.as_bytes())?;
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

/// Copy one G5 companion file into the content-addressed store. The bytes that are
/// hashed are the bytes that are written; the caller holds the write lock.
fn archive_artifact(
    root: &Path,
    store: &LedgerStore,
    relative: &str,
    sha256: &str,
) -> Result<(), String> {
    if !relative.starts_with("dist/evidence/g5-artifacts/") || relative.contains("..") {
        return Err(format!("G5 evidence has unsafe artifact path {relative}"));
    }
    // This run's file must match its evidence even when the store already holds the
    // same content: a mismatch means the run's own output changed after capture.
    let bytes = fs::read(root.join(relative))
        .map_err(|error| format!("cannot archive G5 artifact {relative}: {error}"))?;
    let actual = receipt::sha256_bytes(&bytes)?;
    if actual != sha256 {
        return Err(format!(
            "G5 artifact {relative} hash is {actual}, expected {sha256}"
        ));
    }
    let target = store.artifact(sha256)?;
    if target.is_file() && receipt::sha256(&target)? == sha256 {
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
            super::g5_readiness::companion_artifacts(&document)?
                .into_iter()
                .map(|(_, sha256)| sha256),
        );
    }
    store.prune_artifacts(guard, &referenced)
}
pub(super) fn reuse_for_receipt(root: &Path, path: &Path) -> Result<bool, String> {
    let Some(module) = module_for_receipt(root, path) else {
        return Ok(false);
    };
    let candidate = receipt::read_candidate(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    let Some(success) = compatible_success(root, module)? else {
        return Ok(false);
    };
    println!(
        "MODULE_REUSED_PASS={} origin_source={} origin_exe={}",
        success.module.as_str(),
        success.origin_source_commit,
        success.origin_exe_sha256
    );
    Ok(true)
}

pub(super) fn record_for_receipt(root: &Path, path: &Path) -> Result<(), String> {
    let Some(module) = module_for_receipt(root, path) else {
        return Ok(());
    };
    let candidate = receipt::read_candidate(root)?;
    receipt::validate_candidate_against_repository(root, &candidate)?;
    record_success(root, module, &candidate)
}

fn validate_success_schema(document: &str, module: ModuleId) -> Result<(), String> {
    if json::u64_field(document, "schema_version")? != 2
        || json::string_field(document, "status")? != "PASSED"
        || json::string_field(document, "module_id")? != module.as_str()
    {
        return Err(format!(
            "module {} last-success receipt has invalid schema, status, or identity",
            module.as_str()
        ));
    }
    for (key, label) in [
        ("input_fingerprint", "input fingerprint"),
        ("origin_exe_sha256", "origin EXE SHA-256"),
        ("origin_zip_sha256", "origin ZIP SHA-256"),
        ("evidence_sha256", "evidence SHA-256"),
    ] {
        receipt::validate_sha256(&json::string_field(document, key)?, label)?;
    }
    if json::string_field(document, "origin_version")?.is_empty() {
        return Err(format!(
            "module {} last-success receipt has no origin version",
            module.as_str()
        ));
    }
    json::u64_field(document, "recorded_at_unix")?;
    receipt::validate_hex(
        &json::string_field(document, "origin_source_commit")?,
        40,
        "origin source commit",
    )
}

fn validate_success_evidence(document: &str, module: ModuleId) -> Result<(), String> {
    if let ModuleId::Resource(group) = module {
        return crate::resource_plan::validate_receipt(document, group);
    }
    match json::bool_field(document, "worktree_dirty") {
        Ok(false) => {}
        Ok(true) => {
            return Err(format!(
                "module {} evidence was recorded from a dirty worktree",
                module.as_str()
            ));
        }
        Err(error) => return Err(format!("module {} evidence: {error}", module.as_str())),
    }
    let statuses = json::result_status_values(document)?;
    if statuses.is_empty() || statuses.iter().any(|status| status != "PASSED") {
        return Err(format!(
            "module {} evidence does not contain an all-PASSED result set",
            module.as_str()
        ));
    }
    if matches!(module, ModuleId::Runtime | ModuleId::Performance) {
        super::smoke_scope::validate_task_coverage(document, module == ModuleId::Runtime)?;
    }
    Ok(())
}
