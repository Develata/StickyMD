//! G5 exact-candidate readiness projection.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::path::Path;

use super::exact_readiness;
use super::module_ledger;
use super::module_registry::ModuleId;

pub(super) const G5_RECEIPT: &str = "dist/evidence/g5-exact-qualification.json";
const EXPECTED_CASES: [&str; 4] = ["G5-01", "G5-02", "G5-03", "G5-04"];

pub(super) fn check(root: &Path, blockers: &mut Vec<String>) -> bool {
    let before = blockers.len();
    if ModuleId::G5.receipt() != G5_RECEIPT {
        blockers.push(format!(
            "G5 module registry expects {}, not {G5_RECEIPT}",
            ModuleId::G5.receipt()
        ));
        return false;
    }
    // One success snapshot, validated under one shared store lock: identity and every
    // screenshot come from the same record, and no writer can prune them meanwhile.
    let found = module_ledger::with_compatible_success(root, ModuleId::G5, |success| {
        let Some(success) = success else {
            blockers.push(
                "G5 has no compatible last-success receipt for current module inputs".to_owned(),
            );
            return;
        };
        if exact_readiness::check_success(success, "G5", &EXPECTED_CASES, blockers) {
            verify_artifacts(success, blockers);
        }
    });
    if let Err(error) = found {
        blockers.push(format!("G5 last-success receipt: {error}"));
    }
    blockers.len() == before
}

/// Every `(path, sha256)` companion file in a G5 evidence document, taken from every
/// result rather than the current case list, so cleanup run by an older tool never
/// misses a newer case's screenshots. Malformed entries are errors, never "absent".
pub(super) fn companion_artifacts(document: &str) -> Result<Vec<Artifact>, String> {
    Ok(results_with_artifacts(document)?
        .into_iter()
        .flat_map(|(_, artifacts)| artifacts)
        .collect())
}

/// Each result's id with its own artifacts, from a strict JSON parse: a case can only
/// count screenshots inside its own object, whatever the document's whitespace.
/// `(path, sha256)` of one companion file.
type Artifact = (String, String);

fn results_with_artifacts(document: &str) -> Result<Vec<(String, Vec<Artifact>)>, String> {
    use crate::release::json::{self, Value};
    let parsed = json::parse(document)?;
    let mut results = Vec::new();
    for result in parsed.field("results")?.array()? {
        let Value::Object(fields) = result else {
            return Err("G5 evidence result is not an object".to_owned());
        };
        let id = result.field("id")?.string()?.to_owned();
        let mut artifacts = Vec::new();
        if let Some(listed) = fields.get("artifacts") {
            for artifact in listed.array()? {
                let path = artifact.field("path")?.string()?.to_owned();
                let sha256 = artifact.field("sha256")?.string()?.to_owned();
                super::receipt::validate_sha256(&sha256, "G5 artifact SHA-256")?;
                artifacts.push((path, sha256));
            }
        }
        results.push((id, artifacts));
    }
    Ok(results)
}

fn verify_artifacts(success: &module_ledger::CompatibleSuccess, blockers: &mut Vec<String>) {
    let results = match results_with_artifacts(&success.document) {
        Ok(results) => results,
        Err(error) => {
            blockers.push(format!("G5 exact screenshot list: {error}"));
            return;
        }
    };
    for (case, minimum) in [("G5-01", 1), ("G5-02", 3), ("G5-03", 13), ("G5-04", 3)] {
        let mut owned = results.iter().filter(|(id, _)| id == case);
        let (Some((_, artifacts)), None) = (owned.next(), owned.next()) else {
            blockers.push(format!("G5 exact {case} must appear exactly once"));
            continue;
        };
        if artifacts.len() < minimum {
            blockers.push(format!(
                "G5 exact {case} has {} screenshot artifact(s), expected at least {minimum}",
                artifacts.len()
            ));
            continue;
        }
        for (path, expected) in artifacts {
            if !path.starts_with("dist/evidence/g5-artifacts/") || path.contains("..") {
                blockers.push(format!("G5 exact {case} has unsafe artifact path {path}"));
                continue;
            }
            // Screenshots are archived with the success, so a fresh release
            // worktree verifies them from the clone-wide store.
            let archived = match success.artifact(expected) {
                Ok(archived) => archived,
                Err(error) => {
                    blockers.push(format!("G5 artifact {path}: {error}"));
                    continue;
                }
            };
            match super::receipt::sha256(&archived) {
                Ok(actual) if &actual == expected => {}
                Ok(actual) => blockers.push(format!(
                    "STALE RECEIPT: G5 artifact {path} hash is {actual}, expected {expected}"
                )),
                Err(error) => blockers.push(format!("G5 artifact {path}: {error}")),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::results_with_artifacts;

    fn artifact(name: &str) -> String {
        format!(
            "{{\"path\":\"dist/evidence/g5-artifacts/{name}.png\",\"sha256\":\"{}\"}}",
            "a".repeat(64)
        )
    }

    #[test]
    fn each_case_counts_only_its_own_screenshots_whatever_the_whitespace() {
        // Legal JSON with spaces between objects; only G5-04 has screenshots.
        let late = (0..13)
            .map(|i| artifact(&format!("late-{i}")))
            .collect::<Vec<_>>();
        let document = format!(
            "{{\"results\": [ {{\"id\":\"G5-01\",\"artifacts\":[]}}, {{\"id\":\"G5-02\",\"artifacts\":[]}},\n {{\"id\":\"G5-03\"}}, {{\"id\":\"G5-04\",\"artifacts\":[{}]}} ]}}",
            late.join(", ")
        );
        let counts = results_with_artifacts(&document)
            .unwrap()
            .into_iter()
            .map(|(id, artifacts)| (id, artifacts.len()))
            .collect::<Vec<_>>();
        assert_eq!(
            counts,
            [
                ("G5-01".to_owned(), 0),
                ("G5-02".to_owned(), 0),
                ("G5-03".to_owned(), 0),
                ("G5-04".to_owned(), 13)
            ]
        );
    }

    #[test]
    fn malformed_artifact_lists_are_errors_not_empty() {
        for document in [
            "{\"results\":[{\"id\":\"G5-01\",\"artifacts\":{}}]}",
            "{\"results\":[{\"id\":\"G5-01\",\"artifacts\":[{\"path\":\"x\"}]}]}",
            "{\"results\":[{\"id\":\"G5-01\",\"artifacts\":[{\"path\":\"x\",\"sha256\":\"short\"}]}]}",
            "{\"results\":[{\"artifacts\":[]}]}",
            "{\"results\":[{\"id\":\"G5-01\",\"artifacts\":[]}]",
        ] {
            assert!(results_with_artifacts(document).is_err(), "{document}");
        }
    }
}
