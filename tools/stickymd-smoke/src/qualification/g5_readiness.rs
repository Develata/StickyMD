//! G5 exact-candidate readiness projection.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::path::Path;

use super::exact_readiness;
use super::module_ledger::{self, ModuleId};

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
pub(super) fn companion_artifacts(document: &str) -> Result<Vec<(String, String)>, String> {
    use crate::release::json::{self, Value};
    let parsed = json::parse(document)?;
    let mut artifacts = Vec::new();
    for result in parsed.field("results")?.array()? {
        let Value::Object(fields) = result else {
            return Err("G5 evidence result is not an object".to_owned());
        };
        let Some(listed) = fields.get("artifacts") else {
            continue;
        };
        for artifact in listed.array()? {
            let path = artifact.field("path")?.string()?.to_owned();
            let sha256 = artifact.field("sha256")?.string()?.to_owned();
            super::receipt::validate_sha256(&sha256, "G5 artifact SHA-256")?;
            artifacts.push((path, sha256));
        }
    }
    Ok(artifacts)
}

fn verify_artifacts(success: &module_ledger::CompatibleSuccess, blockers: &mut Vec<String>) {
    for (case, minimum) in [("G5-01", 1), ("G5-02", 3), ("G5-03", 13), ("G5-04", 3)] {
        let artifacts = artifacts_for_case(&success.document, case);
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
            let archived = match success.artifact(&expected) {
                Ok(archived) => archived,
                Err(error) => {
                    blockers.push(format!("G5 artifact {path}: {error}"));
                    continue;
                }
            };
            match super::receipt::sha256(&archived) {
                Ok(actual) if actual == expected => {}
                Ok(actual) => blockers.push(format!(
                    "STALE RECEIPT: G5 artifact {path} hash is {actual}, expected {expected}"
                )),
                Err(error) => blockers.push(format!("G5 artifact {path}: {error}")),
            }
        }
    }
}

fn artifacts_for_case(document: &str, case: &str) -> Vec<(String, String)> {
    let marker = format!("{{\"id\":\"{case}\"");
    let Some((_, tail)) = document.split_once(&marker) else {
        return Vec::new();
    };
    let case_end = tail.find("},{\"id\":\"G5-").unwrap_or(tail.len());
    let mut rest = &tail[..case_end];
    let mut artifacts = Vec::new();
    while let Some((_, after_path)) = rest.split_once("\"path\":\"") {
        let Some((path, after_path)) = after_path.split_once('"') else {
            break;
        };
        let Some((_, after_hash)) = after_path.split_once("\"sha256\":\"") else {
            break;
        };
        let Some((sha256, next)) = after_hash.split_once('"') else {
            break;
        };
        artifacts.push((path.to_owned(), sha256.to_owned()));
        rest = next;
    }
    artifacts
}

#[cfg(test)]
mod tests {
    use super::artifacts_for_case;

    #[test]
    fn g5_artifact_parser_is_case_bounded() {
        let document = concat!(
            "{\"results\":[",
            "{\"id\":\"G5-01\",\"artifacts\":[{\"path\":\"dist/evidence/g5-artifacts/a.png\",\"sha256\":\"aa\"}]},",
            "{\"id\":\"G5-02\",\"artifacts\":[{\"path\":\"dist/evidence/g5-artifacts/b.png\",\"sha256\":\"bb\"}]}",
            "]}"
        );
        assert_eq!(
            artifacts_for_case(document, "G5-01"),
            vec![(
                "dist/evidence/g5-artifacts/a.png".to_owned(),
                "aa".to_owned()
            )]
        );
    }
}
