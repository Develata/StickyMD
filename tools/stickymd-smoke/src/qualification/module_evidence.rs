//! Strict validation of one qualification module's evidence document.
//!
//! Registration, reuse and readiness accept a module's evidence only through
//! [`validate`], over a strict parse of the whole document: every value is read from the
//! object that owns it, never found by scanning text, so a nested or re-spaced field
//! cannot stand in for the real one, and duplicate keys or trailing input are rejected.
//! A document is therefore valid or invalid for every consumer at once.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#module-success-ledger

use super::module_registry::ModuleId;
use crate::cli::ResourceModule;
use crate::integrity::validate_sha256;
use crate::release::json::{self, Value};

/// The identity a module's evidence must carry: the candidate it ran on and, for
/// resource groups, the input fingerprint the measurement was planned for.
pub(super) struct Origin<'a> {
    pub(super) source_commit: &'a str,
    pub(super) version: &'a str,
    pub(super) exe_sha256: &'a str,
    pub(super) zip_sha256: &'a str,
    pub(super) input_fingerprint: &'a str,
}

/// One companion file listed by evidence: a workspace-relative path and its digest.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Companion {
    pub(super) path: String,
    pub(super) sha256: String,
}

const G3_CASES: [&str; 5] = ["G3-01", "G3-02", "G3-03", "G3-04", "G3-05"];
const G4_CASES: [&str; 6] = ["G4-01", "G4-02", "G4-03", "G4-04", "G4-05", "G4-06"];
/// G5 cases with the minimum number of screenshots each must archive.
const G5_SCREENSHOTS: [(&str, usize); 4] =
    [("G5-01", 1), ("G5-02", 3), ("G5-03", 13), ("G5-04", 3)];
const G5_ARTIFACT_PREFIX: &str = "dist/evidence/g5-artifacts/";

/// Validate `document` as complete, passing evidence of `module` for `origin`. Returns
/// the companion files the success depends on (G5 screenshots; empty otherwise).
pub(super) fn validate(
    module: ModuleId,
    document: &str,
    origin: &Origin<'_>,
) -> Result<Vec<Companion>, String> {
    let checked = json::parse(document).and_then(|root| match module {
        ModuleId::Runtime | ModuleId::Performance => {
            smoke(&root, module == ModuleId::Runtime, origin).map(|()| Vec::new())
        }
        ModuleId::Resource(group) => resource(&root, group, origin).map(|()| Vec::new()),
        ModuleId::G3 => exact(&root, &G3_CASES, origin).map(|_| Vec::new()),
        ModuleId::G4 => exact(&root, &G4_CASES, origin).map(|_| Vec::new()),
        ModuleId::G5 => {
            let cases = G5_SCREENSHOTS.map(|(case, _)| case);
            g5_screenshots(exact(&root, &cases, origin)?)
        }
    });
    checked.map_err(|error| format!("{} evidence: {error}", module.as_str()))
}

/// Every companion listed anywhere in a G5 document, including cases this tool does not
/// know, so cleanup run by an older tool never drops a newer case's screenshots.
/// Malformed entries are errors, never "no companions".
pub(super) fn g5_companions(document: &str) -> Result<Vec<Companion>, String> {
    let root = json::parse(document)?;
    let mut companions = Vec::new();
    for result in root.field("results")?.array()? {
        result.field("id")?.string()?;
        companions.extend(listed_companions(result)?);
    }
    Ok(companions)
}

/// Runtime and Performance: one formal Phase 14 run on the origin executable that
/// covers the runner's complete formal task plan, in order, every task PASSED.
fn smoke(root: &Value, runtime: bool, origin: &Origin<'_>) -> Result<(), String> {
    expect_unsigned(root, "schema_version", 2)?;
    expect_string(root, "suite", "phase-14")?;
    expect_string(root, "commit", origin.source_commit)?;
    expect_bool(root, "worktree_dirty", false)?;
    expect_string(root, "executable_sha256", origin.exe_sha256)?;
    let required = crate::runner::formal_task_labels(runtime)?;
    let results = root.field("results")?.array()?;
    if results.len() != required.len() {
        return Err(format!(
            "incomplete formal task coverage: {} result(s), expected {}",
            results.len(),
            required.len()
        ));
    }
    for (result, expected) in results.iter().zip(required) {
        if result.field("id")?.string()? != expected || !passed(result)? {
            return Err(format!("missing successful formal task {expected}"));
        }
    }
    Ok(())
}

/// A resource group: its own strict receipt contract plus the measured identity.
fn resource(root: &Value, group: ResourceModule, origin: &Origin<'_>) -> Result<(), String> {
    crate::resource_plan::validate_receipt_value(root, group)?;
    expect_string(root, "resource_input_fingerprint", origin.input_fingerprint)?;
    expect_string(root, "commit", origin.source_commit)?;
    expect_string(root, "executable_sha256", origin.exe_sha256)
}

/// An exact-candidate group: clean run of the origin candidate with exactly `cases`, in
/// order, every case PASSED. Returns the result objects for group-specific checks.
fn exact<'a>(root: &'a Value, cases: &[&str], origin: &Origin<'_>) -> Result<&'a [Value], String> {
    expect_unsigned(root, "schema_version", 1)?;
    expect_string(root, "status", "PASSED")?;
    expect_string(root, "source_commit", origin.source_commit)?;
    expect_string(root, "harness_commit", origin.source_commit)?;
    expect_string(root, "exe_sha256", origin.exe_sha256)?;
    expect_string(root, "zip_sha256", origin.zip_sha256)?;
    expect_string(root, "version", origin.version)?;
    let windows = root.field("windows")?.string()?;
    if !super::windows_build::is_known(windows) {
        return Err(format!(
            "exact Windows build `{windows}` names no version and build"
        ));
    }
    expect_bool(root, "worktree_dirty", false)?;
    let results = root.field("results")?.array()?;
    let ids = results
        .iter()
        .map(|result| result.field("id")?.string())
        .collect::<Result<Vec<_>, _>>()?;
    if ids != cases {
        return Err(format!("exact cases are {ids:?}, expected {cases:?}"));
    }
    for (result, case) in results.iter().zip(cases) {
        if !passed(result)? {
            return Err(format!("exact case {case} did not pass"));
        }
    }
    Ok(results)
}

/// Each G5 case's own screenshots: at least the required count, under the artifact
/// directory, with a well-formed digest.
fn g5_screenshots(results: &[Value]) -> Result<Vec<Companion>, String> {
    let mut companions = Vec::new();
    for (result, (case, minimum)) in results.iter().zip(G5_SCREENSHOTS) {
        let listed = listed_companions(result)?;
        if listed.len() < minimum {
            return Err(format!(
                "G5 {case} has {} screenshot artifact(s), expected at least {minimum}",
                listed.len()
            ));
        }
        if let Some(unsafe_path) = listed
            .iter()
            .find(|companion| !is_artifact_path(&companion.path))
        {
            return Err(format!(
                "G5 {case} has unsafe artifact path {}",
                unsafe_path.path
            ));
        }
        companions.extend(listed);
    }
    Ok(companions)
}

pub(super) fn is_artifact_path(path: &str) -> bool {
    path.starts_with(G5_ARTIFACT_PREFIX) && !path.contains("..")
}

/// The `artifacts` list of one result object; absent means none.
fn listed_companions(result: &Value) -> Result<Vec<Companion>, String> {
    let Value::Object(fields) = result else {
        return Err("evidence result is not an object".to_owned());
    };
    let Some(listed) = fields.get("artifacts") else {
        return Ok(Vec::new());
    };
    listed
        .array()?
        .iter()
        .map(|artifact| {
            let sha256 = artifact.field("sha256")?.string()?.to_owned();
            validate_sha256(&sha256, "artifact SHA-256")?;
            Ok(Companion {
                path: artifact.field("path")?.string()?.to_owned(),
                sha256,
            })
        })
        .collect()
}

fn passed(result: &Value) -> Result<bool, String> {
    Ok(result.field("status")?.string()? == "PASSED")
}

fn expect_string(root: &Value, key: &str, expected: &str) -> Result<(), String> {
    let actual = root
        .field(key)?
        .string()
        .map_err(|error| format!("{key}: {error}"))?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!(
            "STALE RECEIPT: {key} is {actual}, expected {expected}"
        ))
    }
}

fn expect_unsigned(root: &Value, key: &str, expected: u64) -> Result<(), String> {
    let actual = root
        .field(key)?
        .unsigned()
        .map_err(|error| format!("{key}: {error}"))?;
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{key} is {actual}, expected {expected}"))
    }
}

fn expect_bool(root: &Value, key: &str, expected: bool) -> Result<(), String> {
    match root.field(key)? {
        Value::Bool(actual) if *actual == expected => Ok(()),
        Value::Bool(actual) => Err(format!("{key} is {actual}, expected {expected}")),
        _ => Err(format!("{key} is not a boolean")),
    }
}

#[cfg(test)]
pub(super) mod fixtures;
#[cfg(test)]
mod tests;
