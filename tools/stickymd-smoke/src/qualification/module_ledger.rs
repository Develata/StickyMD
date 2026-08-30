//! Input fingerprints and last-success authority for qualification modules.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use std::fs;
use std::path::{Path, PathBuf};

use super::json;
use super::receipt::{self, Candidate};

mod fingerprint;
#[cfg(test)]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ModuleId {
    Runtime,
    Performance,
    Resources,
    G3,
    G4,
    G5,
}

const MODULES: [ModuleId; 6] = [
    ModuleId::Runtime,
    ModuleId::Performance,
    ModuleId::Resources,
    ModuleId::G3,
    ModuleId::G4,
    ModuleId::G5,
];

impl ModuleId {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Runtime => "runtime",
            Self::Performance => "performance",
            Self::Resources => "resources",
            Self::G3 => "g3",
            Self::G4 => "g4",
            Self::G5 => "g5",
        }
    }

    pub(super) const fn receipt(self) -> &'static str {
        match self {
            Self::Runtime => "dist/evidence/runtime-qualification.json",
            Self::Performance => "dist/evidence/performance-qualification.json",
            Self::Resources => "dist/evidence/resources-qualification.json",
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
    pub(super) origin_exe_sha256: String,
    pub(super) origin_zip_sha256: String,
    pub(super) evidence_path: PathBuf,
    pub(super) document: String,
}

pub(super) fn module_for_receipt(root: &Path, path: &Path) -> Option<ModuleId> {
    let relative = if path.is_absolute() {
        path.strip_prefix(root).ok()?
    } else {
        path
    };
    let normalized = normalize(relative);
    MODULES
        .into_iter()
        .find(|module| module.receipt() == normalized)
}

pub(super) fn compatible_success(
    root: &Path,
    module: ModuleId,
) -> Result<Option<CompatibleSuccess>, String> {
    let path = success_path(root, module);
    if !path.is_file() {
        return Ok(None);
    }
    let document = receipt::read_receipt(&path)?;
    validate_success_schema(&document, module)?;
    let current = fingerprint::calculate(root, module)?;
    if json::string_field(&document, "input_fingerprint")? != current {
        return Ok(None);
    }
    let evidence_relative = json::string_field(&document, "evidence_path")?;
    let expected_prefix = format!("dist/evidence/module-success/evidence/{}-", module.as_str());
    if !evidence_relative.starts_with(&expected_prefix)
        || !evidence_relative.ends_with(".json")
        || evidence_relative.contains("..")
    {
        return Err(format!(
            "module {} success references unexpected evidence path {evidence_relative}",
            module.as_str()
        ));
    }
    let evidence_path = root.join(&evidence_relative);
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
    Ok(Some(CompatibleSuccess {
        module,
        origin_source_commit: json::string_field(&document, "origin_source_commit")?,
        origin_exe_sha256: json::string_field(&document, "origin_exe_sha256")?,
        origin_zip_sha256: json::string_field(&document, "origin_zip_sha256")?,
        evidence_path,
        document: evidence_document,
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
    let evidence_sha256 = receipt::sha256(&source_evidence)?;
    let evidence_relative = format!(
        "dist/evidence/module-success/evidence/{}-{evidence_sha256}.json",
        module.as_str()
    );
    let evidence_path = root.join(&evidence_relative);
    let evidence_bytes = fs::read(&source_evidence).map_err(|error| {
        format!(
            "cannot archive {} module evidence: {error}",
            module.as_str()
        )
    })?;
    crate::atomic_evidence::write(&evidence_path, &evidence_bytes)?;
    let input_fingerprint = fingerprint::calculate(root, module)?;
    let previous_evidence = previous_evidence_path(root, module);
    let document = format!(
        concat!(
            "{{\"schema_version\":1,\"status\":\"PASSED\",",
            "\"module_id\":\"{}\",\"input_fingerprint\":\"{}\",",
            "\"origin_source_commit\":\"{}\",",
            "\"origin_exe_sha256\":\"{}\",\"origin_zip_sha256\":\"{}\",",
            "\"evidence_path\":\"{}\",\"evidence_sha256\":\"{}\"}}\n"
        ),
        module.as_str(),
        input_fingerprint,
        json::escape(&candidate.source_commit),
        json::escape(&candidate.exe_sha256),
        json::escape(&candidate.zip_sha256),
        evidence_relative,
        evidence_sha256,
    );
    crate::atomic_evidence::write(&success_path(root, module), document.as_bytes())?;
    if let Some(previous) = previous_evidence
        && previous != evidence_path
    {
        let _ = fs::remove_file(previous);
    }
    Ok(())
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
    for module in MODULES {
        match compatible_success(root, module) {
            Ok(Some(success)) => println!(
                "MODULE={} STATUS={} ORIGIN_SOURCE={} ORIGIN_EXE={} EVIDENCE={}",
                module.as_str(),
                success_status(&success, candidate),
                success.origin_source_commit,
                success.origin_exe_sha256,
                success.evidence_path.display()
            ),
            Ok(None) => println!("MODULE={} STATUS=RUN_REQUIRED", module.as_str()),
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

fn success_path(root: &Path, module: ModuleId) -> PathBuf {
    root.join(format!(
        "dist/evidence/module-success/{}.json",
        module.as_str()
    ))
}

fn previous_evidence_path(root: &Path, module: ModuleId) -> Option<PathBuf> {
    let document = receipt::read_receipt(&success_path(root, module)).ok()?;
    let relative = json::string_field(&document, "evidence_path").ok()?;
    (!relative.contains("..") && relative.starts_with("dist/evidence/module-success/evidence/"))
        .then(|| root.join(relative))
}

fn validate_success_schema(document: &str, module: ModuleId) -> Result<(), String> {
    if json::u64_field(document, "schema_version")? != 1
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
    receipt::validate_hex(
        &json::string_field(document, "origin_source_commit")?,
        40,
        "origin source commit",
    )
}

fn validate_success_evidence(document: &str, module: ModuleId) -> Result<(), String> {
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
    Ok(())
}

fn normalize(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}
