//! Input fingerprints and last-success authority for qualification modules.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::fs;
use std::path::{Path, PathBuf};

use super::json;
use super::receipt::{self, Candidate};
use crate::cli::ResourceModule;

pub(super) mod fingerprint;
#[cfg(test)]
mod reuse_tests;
pub(super) mod store;
#[cfg(test)]
mod tests;

use store::LedgerStore;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ModuleId {
    Runtime,
    Performance,
    Resource(ResourceModule),
    G3,
    G4,
    G5,
}

fn modules() -> impl Iterator<Item = ModuleId> {
    [ModuleId::Runtime, ModuleId::Performance]
        .into_iter()
        .chain(
            crate::resource_plan::GROUPS
                .into_iter()
                .map(ModuleId::Resource),
        )
        .chain([ModuleId::G3, ModuleId::G4, ModuleId::G5])
}

impl ModuleId {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::Performance => "performance",
            Self::Resource(group) => group.ledger_id(),
            Self::G3 => "g3",
            Self::G4 => "g4",
            Self::G5 => "g5",
        }
    }

    pub(super) const fn receipt(self) -> &'static str {
        match self {
            Self::Runtime => "dist/evidence/runtime-qualification.json",
            Self::Performance => "dist/evidence/performance-qualification.json",
            Self::Resource(group) => group.receipt(),
            Self::G3 => "dist/evidence/g3-exact-qualification.json",
            Self::G4 => "dist/evidence/g4-exact-qualification.json",
            Self::G5 => "dist/evidence/g5-exact-qualification.json",
        }
    }
}

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

pub(super) fn module_for_receipt(root: &Path, path: &Path) -> Option<ModuleId> {
    modules().find(|module| matches_receipt(root, path, module.receipt()))
}

pub(super) fn matches_receipt(root: &Path, path: &Path, expected: &str) -> bool {
    normalize(&root.join(path)) == normalize(&root.join(expected))
}

pub(super) fn is_success_storage_path(root: &Path, path: &Path) -> bool {
    is_within(root, path, "dist/evidence/module-success")
}

pub(super) fn is_within(root: &Path, path: &Path, directory: &str) -> bool {
    let path = normalize(&root.join(path));
    let directory = normalize(&root.join(directory));
    path == directory
        || path
            .strip_prefix(&directory)
            .is_some_and(|suffix| suffix.starts_with('/'))
}

pub(super) fn compatible_success(
    root: &Path,
    module: ModuleId,
) -> Result<Option<CompatibleSuccess>, String> {
    compatible_success_using(root, module, || fingerprint::calculate(root, module))
}

pub(super) fn compatible_success_for_input(
    root: &Path,
    module: ModuleId,
    current: &str,
) -> Result<Option<CompatibleSuccess>, String> {
    compatible_success_using(root, module, || Ok(current.to_owned()))
}

fn compatible_success_using(
    root: &Path,
    module: ModuleId,
    current: impl FnOnce() -> Result<String, String>,
) -> Result<Option<CompatibleSuccess>, String> {
    let store = LedgerStore::for_repository(root)?;
    // An empty module directory cannot match; skip fingerprinting the checkout.
    if !store.has_module_records(module.as_str())? {
        return Ok(None);
    }
    let current = current()?;
    let path = store.module_record(module.as_str(), &current)?;
    if !path.is_file() {
        return Ok(None);
    }
    let document = receipt::read_receipt(&path)?;
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
    let expected_evidence = json::string_field(&document, "evidence_sha256")?;
    let actual_evidence = receipt::sha256(&evidence_path)?;
    if expected_evidence != actual_evidence {
        return Err(format!(
            "STALE RECEIPT: module {} evidence hash is {actual_evidence}, expected {expected_evidence}",
            module.as_str()
        ));
    }
    let evidence_document = receipt::read_receipt(&evidence_path)?;
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
        store,
    }))
}

pub(super) fn record_success(
    root: &Path,
    module: ModuleId,
    candidate: &Candidate,
) -> Result<(), String> {
    let source_evidence = root.join(module.receipt());
    let source_document = receipt::read_receipt(&source_evidence)?;
    validate_success_evidence(&source_document, module)?;
    let input_fingerprint = fingerprint::calculate(root, module)?;
    if matches!(module, ModuleId::Resource(_))
        && (json::string_field(&source_document, "resource_input_fingerprint")?
            != input_fingerprint
            || json::string_field(&source_document, "commit")? != candidate.source_commit
            || json::string_field(&source_document, "executable_sha256")? != candidate.exe_sha256)
    {
        return Err("resource inputs or candidate changed during measurement".to_owned());
    }
    let store = LedgerStore::for_repository(root)?;
    // Companion files are archived before the record so a published record never
    // points at evidence that exists only in this (possibly temporary) worktree.
    let artifacts = if module == ModuleId::G5 {
        super::g5_readiness::companion_artifacts(&source_document)
    } else {
        Vec::new()
    };
    for (relative, sha256) in &artifacts {
        archive_artifact(root, &store, relative, sha256)?;
    }
    let evidence_sha256 = receipt::sha256(&source_evidence)?;
    let evidence_file = format!("{}-{evidence_sha256}.json", module.as_str());
    let evidence_path = store.evidence(&evidence_file)?;
    let evidence_bytes = fs::read(&source_evidence).map_err(|error| {
        format!(
            "cannot archive {} module evidence: {error}",
            module.as_str()
        )
    })?;
    crate::atomic_evidence::write(&evidence_path, &evidence_bytes)?;
    let record_path = store.module_record(module.as_str(), &input_fingerprint)?;
    let replaced_evidence = receipt::read_receipt(&record_path)
        .ok()
        .and_then(|previous| json::string_field(&previous, "evidence_file").ok())
        .filter(|previous| previous != &evidence_file);
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
    // A same-input rerun supersedes its own earlier evidence immediately.
    if let Some(previous) = replaced_evidence
        && let Ok(previous) = store.evidence(&previous)
    {
        let _ = fs::remove_file(previous);
    }
    store.prune_module(module.as_str(), &record_path);
    if module == ModuleId::G5 {
        prune_g5_artifacts(&store);
    }
    Ok(())
}

/// Copy one G5 companion file into the content-addressed store after verifying it.
fn archive_artifact(
    root: &Path,
    store: &LedgerStore,
    relative: &str,
    sha256: &str,
) -> Result<(), String> {
    if !relative.starts_with("dist/evidence/g5-artifacts/") || relative.contains("..") {
        return Err(format!("G5 evidence has unsafe artifact path {relative}"));
    }
    let source = root.join(relative);
    let actual = receipt::sha256(&source)?;
    if actual != sha256 {
        return Err(format!(
            "G5 artifact {relative} hash is {actual}, expected {sha256}"
        ));
    }
    let target = store.artifact(sha256)?;
    if target.is_file() && receipt::sha256(&target)? == sha256 {
        return Ok(());
    }
    let bytes = fs::read(&source)
        .map_err(|error| format!("cannot archive G5 artifact {relative}: {error}"))?;
    crate::atomic_evidence::write(&target, &bytes)
}

fn prune_g5_artifacts(store: &LedgerStore) {
    let Ok(documents) = store.module_evidence_documents(ModuleId::G5.as_str()) else {
        return;
    };
    let referenced = documents
        .iter()
        .flat_map(|document| super::g5_readiness::companion_artifacts(document))
        .map(|(_, sha256)| sha256)
        .collect::<Vec<_>>();
    store.prune_artifacts(&referenced);
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

pub(super) fn print_status(root: &Path) -> Result<(), String> {
    let candidate = receipt::read_candidate(root).ok();
    print_status_for_candidate(root, candidate.as_ref())
}

pub(super) fn print_status_for_candidate(
    root: &Path,
    candidate: Option<&Candidate>,
) -> Result<(), String> {
    let store = LedgerStore::for_repository(root)?;
    println!("LEDGER_STORE={}", store.root().display());
    // One planning pass reads each shared input once for every module.
    let all = modules().collect::<Vec<_>>();
    let digests = fingerprint::PlanningInputs::read(root)?
        .calculate_many(root, &all)?
        .digests;
    for (module, current) in all.into_iter().zip(digests) {
        match compatible_success_for_input(root, module, &current) {
            Ok(Some(success)) => println!(
                "MODULE={} STATUS={} ORIGIN_SOURCE={} ORIGIN_VERSION={} ORIGIN_EXE={} EVIDENCE={}",
                module.as_str(),
                success_status(&success, candidate),
                success.origin_source_commit,
                success.origin_version,
                success.origin_exe_sha256,
                success.evidence_path.display()
            ),
            Ok(None) => println!(
                "MODULE={} STATUS=RUN_REQUIRED REASON={}",
                module.as_str(),
                rerun_reason(root, module)
            ),
            Err(error) => println!(
                "MODULE={} STATUS=RUN_REQUIRED REASON={}",
                module.as_str(),
                error.replace(['\r', '\n'], " ")
            ),
        }
    }
    Ok(())
}

fn success_status(success: &CompatibleSuccess, candidate: Option<&Candidate>) -> &'static str {
    if candidate.is_some_and(|candidate| {
        success.origin_source_commit == candidate.source_commit
            && success.origin_exe_sha256 == candidate.exe_sha256
            && success.origin_zip_sha256 == candidate.zip_sha256
    }) {
        "RAN_PASS"
    } else {
        "REUSED_PASS"
    }
}

pub(super) fn rerun_reason(root: &Path, module: ModuleId) -> &'static str {
    match LedgerStore::for_repository(root)
        .and_then(|store| store.has_module_records(module.as_str()))
    {
        Ok(true) => "INPUT_FINGERPRINT_CHANGED",
        Ok(false) => "NO_LAST_SUCCESS",
        Err(_) => "LEDGER_STORE_UNAVAILABLE",
    }
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

fn normalize(path: &Path) -> String {
    #[cfg(windows)]
    let win32_path = win32_spelling(path);
    #[cfg(windows)]
    let path = win32_path.as_path();
    // Resolve existing filesystem identity before lexical cleanup: Windows verbatim/8.3
    // paths and directory junctions can name the same reserved evidence destination.
    // New output files inherit the identity of their nearest existing ancestor.
    let resolved = path.ancestors().find_map(|ancestor| {
        ancestor
            .canonicalize()
            .ok()
            .map(|existing| existing.join(path.strip_prefix(ancestor).expect("ancestor prefix")))
    });
    let path = resolved.as_deref().unwrap_or(path);
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                normalized.pop();
            }
            component => normalized.push(component.as_os_str()),
        }
    }
    let normalized = normalized.to_string_lossy().replace('\\', "/");
    if cfg!(windows) {
        normalized.to_lowercase()
    } else {
        normalized
    }
}

#[cfg(windows)]
fn win32_spelling(path: &Path) -> PathBuf {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};

    if is_verbatim(path) {
        return path.to_path_buf();
    }
    // MoveFileExW receives ordinary Win32 paths for ordinary CLI output paths.
    // Only the final component is folded; verbatim paths keep their literal name.
    let Some(name) = path.file_name() else {
        return path.to_path_buf();
    };
    let mut units = name.encode_wide().collect::<Vec<_>>();
    while units
        .last()
        .is_some_and(|unit| matches!(*unit, 0x20 | 0x2e))
    {
        units.pop();
    }
    path.with_file_name(OsString::from_wide(&units))
}

#[cfg(windows)]
fn is_verbatim(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(component, std::path::Component::Prefix(prefix) if prefix.kind().is_verbatim())
    })
}

#[cfg(windows)]
pub(super) fn validate_output_spelling(path: &Path) -> Result<(), String> {
    use std::os::windows::ffi::OsStrExt;
    if !is_verbatim(path) {
        let mut components = path.components();
        components.next_back();
        if components.any(|component| {
            matches!(component, std::path::Component::Normal(name)
                if name.encode_wide().last().is_some_and(|unit| matches!(unit, 0x20 | 0x2e)))
        }) {
            // An ancestor becomes the final component while canonicalizing. Do not
            // let that fold an intermediate directory that Win32 would keep literal.
            return Err("ordinary Windows evidence paths cannot contain a directory ending in a dot or space; use a verbatim path or a diagnostic path without ambiguous directories".into());
        }
    }
    Ok(())
}
