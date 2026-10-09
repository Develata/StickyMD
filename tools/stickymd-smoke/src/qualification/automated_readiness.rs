//! Source-only automated receipt validation: the headless CI run bound to Source Freeze.
//! Artifact-bound modules are judged by the module ledger.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::path::Path;

use super::receipt;
use super::source_freeze::SourceFreeze;
use crate::release::json::{self, Value};

const HEADLESS_CI_RECEIPT: &str = "dist/evidence/headless-ci-qualification.json";
const HEADLESS_CI_TASK: &str = "requested headless CI task set";

pub(super) fn check(root: &Path, source: &SourceFreeze, blockers: &mut Vec<String>) -> bool {
    let checked = receipt::read_receipt(&root.join(HEADLESS_CI_RECEIPT))
        .and_then(|document| check_document(&document, &source.source_commit));
    match checked {
        Ok(()) => true,
        Err(error) => {
            blockers.push(format!("headless CI qualification receipt: {error}"));
            false
        }
    }
}

/// A clean full headless run of the frozen source: the requested task set exactly once
/// and every result PASSED, read from a strict parse of the receipt.
fn check_document(document: &str, source_commit: &str) -> Result<(), String> {
    let root = json::parse(document)?;
    if root.field("schema_version")?.unsigned()? != 2 {
        return Err("schema is not 2".to_owned());
    }
    if root.field("suite")?.string()? != "all" {
        return Err("suite is not all".to_owned());
    }
    let commit = root.field("commit")?.string()?;
    if commit != source_commit {
        return Err(format!(
            "STALE RECEIPT: source commit is {commit}, expected {source_commit}"
        ));
    }
    if root.field("worktree_dirty")? != &Value::Bool(false) {
        return Err("recorded from a dirty tree".to_owned());
    }
    let results = root.field("results")?.array()?;
    let mut required = 0;
    for result in results {
        if result.field("id")?.string()? == HEADLESS_CI_TASK {
            required += 1;
        }
        let status = result.field("status")?.string()?;
        if status != "PASSED" {
            return Err(format!("contains a {status} result"));
        }
    }
    if required != 1 {
        return Err(format!(
            "contains required task `{HEADLESS_CI_TASK}` {required} times, expected exactly once"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{HEADLESS_CI_TASK, check_document};

    fn document(commit: &str, results: &str) -> String {
        format!(
            concat!(
                "{{\"schema_version\":2,\"commit\":\"{}\",\"worktree_dirty\":false,",
                "\"artifact_sha256\":null,\"executable_sha256\":\"{}\",",
                "\"suite\":\"all\",\"qualification_environment\":{{\"status\":\"VALID\"}},",
                "\"results\":[{}]}}"
            ),
            commit,
            "f".repeat(64),
            results
        )
    }

    #[test]
    fn source_only_headless_receipt_binds_the_frozen_source_and_passes_once() {
        let commit = "a".repeat(40);
        let task = format!("{{\"id\":\"{HEADLESS_CI_TASK}\",\"status\":\"PASSED\"}}");
        // The headless run may use any local executable; only the source is bound.
        check_document(&document(&commit, &task), &commit).unwrap();
        assert!(check_document(&document(&commit, &task), &"b".repeat(40)).is_err());
        assert!(check_document(&document(&commit, &format!("{task},{task}")), &commit).is_err());
        assert!(check_document(&document(&commit, ""), &commit).is_err());
        let decoy = format!(
            "{{\"id\":\"{HEADLESS_CI_TASK}\",\"status\" : \"FAILED\",\"extra\":{{\"status\":\"PASSED\"}}}}"
        );
        assert!(check_document(&document(&commit, &decoy), &commit).is_err());
        let dirty = document(&commit, &task).replace(
            "\"worktree_dirty\":false",
            "\"worktree_dirty\" : true,\"x\":{\"worktree_dirty\":false}",
        );
        assert!(check_document(&dirty, &commit).is_err());
    }
}
