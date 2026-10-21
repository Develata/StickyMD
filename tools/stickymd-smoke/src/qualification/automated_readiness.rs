//! Source-only automated receipt validation: the headless CI run bound to Source Freeze.
//! Artifact-bound modules are judged by the module ledger.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::path::Path;

use super::source_freeze::SourceFreeze;
use super::{json, receipt};
use crate::runner::UNIT_TEST_OWNERS_TASK;

const HEADLESS_CI_RECEIPT: &str = "dist/evidence/headless-ci-qualification.json";

/// Whether the headless CI receipt is a clean, complete run of the frozen source that also
/// verified exactly the mapping's unit-test owners. The same answer decides whether those
/// owners count (plan 11): a failure blocks and leaves every unit-test owner unproven.
pub(super) fn check(root: &Path, source: &SourceFreeze, blockers: &mut Vec<String>) -> bool {
    // Sorted as strings, the order the owner verification writes them in.
    let mut owners = super::unit_test_owners()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    owners.sort();
    let checked = receipt::read_receipt(&root.join(HEADLESS_CI_RECEIPT)).and_then(|text| {
        check_document(
            &text,
            &source.source_commit,
            &crate::runner::headless_task_labels()?,
            &owners,
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
/// task the runner plans for `all --ci`, in order, ending with the task-set marker; the
/// owner verification lists exactly `owners` (ascending) and no other result lists any.
fn check_document(
    document: &str,
    source_commit: &str,
    expected: &[&str],
    owners: &[String],
) -> Result<(), String> {
    let root = json::parse_object(document)?;
    if json::u64_field(&root, "schema_version")? != 3 {
        return Err("schema is not 3".to_owned());
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
    for (result, id) in results.iter().zip(&ids) {
        let status = json::string_field(result, "status")?;
        if status != "PASSED" {
            return Err(format!("task `{id}` is {status}"));
        }
        let listed = json::strings(result, "owners")?;
        let required: &[String] = if *id == UNIT_TEST_OWNERS_TASK {
            owners
        } else {
            &[]
        };
        if listed != required {
            return Err(format!(
                "task `{id}` verified unit-test owners {listed:?}, expected {required:?}"
            ));
        }
    }
    if !owners.is_empty() && !ids.iter().any(|id| id == UNIT_TEST_OWNERS_TASK) {
        return Err("the headless task plan verifies no unit-test owners".to_owned());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::check_document;
    use crate::runner::{HEADLESS_TASK_SET, UNIT_TEST_OWNERS_TASK, headless_task_labels};

    const OWNERS: [&str; 2] = ["p/bin:p/a::b", "p/lib/c::d"];

    fn owners() -> Vec<String> {
        OWNERS.map(str::to_owned).to_vec()
    }

    /// Results for `ids`; the owner verification lists `OWNERS`.
    fn document(commit: &str, ids: &[&str]) -> String {
        let results = ids
            .iter()
            .map(|id| {
                let listed = if *id == UNIT_TEST_OWNERS_TASK {
                    OWNERS.map(|owner| format!("\"{owner}\"")).join(",")
                } else {
                    String::new()
                };
                format!("{{\"id\":\"{id}\",\"status\":\"PASSED\",\"owners\":[{listed}]}}")
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            concat!(
                "{{\"schema_version\":3,\"commit\":\"{}\",\"worktree_dirty\":false,",
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
        let expected = if plan.contains(&UNIT_TEST_OWNERS_TASK) {
            owners()
        } else {
            Vec::new()
        };
        // The headless run may use any local executable; only the source is bound.
        check_document(&complete, &commit, &plan, &expected).unwrap();
        assert!(check_document(&complete, &"b".repeat(40), &plan, &expected).is_err());
        assert!(
            check_document(
                &complete.replace("\"schema_version\":3", "\"schema_version\":2"),
                &commit,
                &plan,
                &expected
            )
            .is_err()
        );
        // Only the completion marker, a missing task or a repeated task is incomplete.
        for ids in [
            vec![HEADLESS_TASK_SET],
            plan[1..].to_vec(),
            [plan.as_slice(), &plan[..1]].concat(),
        ] {
            assert!(
                check_document(&document(&commit, &ids), &commit, &plan, &expected).is_err(),
                "{ids:?}"
            );
        }
        let first = format!(
            "{{\"id\":\"{}\",\"status\":\"PASSED\",\"owners\":[]}}",
            plan[0]
        );
        let decoy = format!(
            "{{\"id\":\"{}\",\"status\" : \"FAILED\",\"extra\":{{\"status\":\"PASSED\"}},\"owners\":[]}}",
            plan[0]
        );
        assert!(
            check_document(
                &complete.replacen(&first, &decoy, 1),
                &commit,
                &plan,
                &expected
            )
            .is_err()
        );
        let dirty = complete.replace(
            "\"worktree_dirty\":false",
            "\"worktree_dirty\" : true,\"x\":{\"worktree_dirty\":false}",
        );
        assert!(check_document(&dirty, &commit, &plan, &expected).is_err());
        // Owners listed outside the owner verification never count.
        let elsewhere = complete.replacen(
            &first,
            &format!(
                "{{\"id\":\"{}\",\"status\":\"PASSED\",\"owners\":[\"x/lib/y\"]}}",
                plan[0]
            ),
            1,
        );
        assert!(check_document(&elsewhere, &commit, &plan, &expected).is_err());
    }

    /// The owner verification must list exactly the mapping's owners: one missing, one extra
    /// or a plan without the verification leaves them unproven.
    #[test]
    fn the_receipt_must_verify_exactly_the_mapping_owners() {
        let commit = "a".repeat(40);
        let plan = [
            "governance contracts",
            UNIT_TEST_OWNERS_TASK,
            HEADLESS_TASK_SET,
        ];
        let complete = document(&commit, &plan);
        check_document(&complete, &commit, &plan, &owners()).unwrap();
        for expected in [
            vec![OWNERS[0].to_owned()],
            [owners(), vec!["p/lib/e::f".to_owned()]].concat(),
            Vec::new(),
        ] {
            assert!(check_document(&complete, &commit, &plan, &expected).is_err());
        }
        let without = ["governance contracts", HEADLESS_TASK_SET];
        assert!(
            check_document(&document(&commit, &without), &commit, &without, &owners()).is_err()
        );
        check_document(&document(&commit, &without), &commit, &without, &[]).unwrap();
    }
}
