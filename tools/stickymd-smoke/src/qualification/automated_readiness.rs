//! Source-only automated receipt validation: the headless CI run bound to Source Freeze.
//! Artifact-bound modules are judged by the module ledger.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::path::Path;

use super::source_freeze::SourceFreeze;
use super::{json, receipt};

const HEADLESS_CI_RECEIPT: &str = "dist/evidence/headless-ci-qualification.json";

pub(super) fn check(root: &Path, source: &SourceFreeze, blockers: &mut Vec<String>) -> bool {
    let checked = receipt::read_receipt(&root.join(HEADLESS_CI_RECEIPT)).and_then(|text| {
        check_document(
            &text,
            &source.source_commit,
            &crate::runner::headless_task_labels()?,
        )
    });
    match checked {
        Ok(()) => true,
        Err(error) => {
            blockers.push(format!("headless CI qualification receipt: {error}"));
            false
        }
    }
}

/// A clean, complete headless run of the frozen source: one PASSED result for every
/// task the runner plans for `all --ci`, in order, ending with the task-set marker.
fn check_document(document: &str, source_commit: &str, expected: &[&str]) -> Result<(), String> {
    let root = json::parse_object(document)?;
    if json::u64_field(&root, "schema_version")? != 2 {
        return Err("schema is not 2".to_owned());
    }
    if json::string_field(&root, "suite")? != "all" {
        return Err("suite is not all".to_owned());
    }
    let commit = json::string_field(&root, "commit")?;
    if commit != source_commit {
        return Err(format!(
            "STALE RECEIPT: source commit is {commit}, expected {source_commit}"
        ));
    }
    if json::bool_field(&root, "worktree_dirty")? {
        return Err("recorded from a dirty tree".to_owned());
    }
    let results = json::objects(&root, "results")?;
    let ids = results
        .iter()
        .map(|result| json::string_field(result, "id"))
        .collect::<Result<Vec<_>, _>>()?;
    if ids != expected {
        return Err(format!(
            "results are {ids:?}, expected the complete headless task plan {expected:?}"
        ));
    }
    for (result, id) in results.iter().zip(ids) {
        let status = json::string_field(result, "status")?;
        if status != "PASSED" {
            return Err(format!("task `{id}` is {status}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_document;
    use crate::runner::{HEADLESS_TASK_SET, headless_task_labels};

    fn document(commit: &str, ids: &[&str]) -> String {
        let results = ids
            .iter()
            .map(|id| format!("{{\"id\":\"{id}\",\"status\":\"PASSED\"}}"))
            .collect::<Vec<_>>()
            .join(",");
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
    fn headless_receipt_binds_the_frozen_source_and_the_complete_task_plan() {
        let commit = "a".repeat(40);
        let plan = headless_task_labels().unwrap();
        assert!(plan.len() > 1, "{plan:?}");
        assert_eq!(plan.last(), Some(&HEADLESS_TASK_SET));
        let complete = document(&commit, &plan);
        // The headless run may use any local executable; only the source is bound.
        check_document(&complete, &commit, &plan).unwrap();
        assert!(check_document(&complete, &"b".repeat(40), &plan).is_err());
        // Only the completion marker, a missing task or a repeated task is incomplete.
        for ids in [
            vec![HEADLESS_TASK_SET],
            plan[1..].to_vec(),
            [plan.as_slice(), &plan[..1]].concat(),
        ] {
            assert!(
                check_document(&document(&commit, &ids), &commit, &plan).is_err(),
                "{ids:?}"
            );
        }
        let first = format!("{{\"id\":\"{}\",\"status\":\"PASSED\"}}", plan[0]);
        let decoy = format!(
            "{{\"id\":\"{}\",\"status\" : \"FAILED\",\"extra\":{{\"status\":\"PASSED\"}}}}",
            plan[0]
        );
        assert!(check_document(&complete.replacen(&first, &decoy, 1), &commit, &plan).is_err());
        let dirty = complete.replace(
            "\"worktree_dirty\":false",
            "\"worktree_dirty\" : true,\"x\":{\"worktree_dirty\":false}",
        );
        assert!(check_document(&dirty, &commit, &plan).is_err());
    }
}
