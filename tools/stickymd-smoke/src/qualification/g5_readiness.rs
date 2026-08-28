//! G5 exact-candidate readiness projection.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::path::Path;

use super::exact_readiness;
use super::receipt;
use super::receipt::Candidate;

pub(super) const G5_RECEIPT: &str = "dist/evidence/g5-exact-qualification.json";
const EXPECTED_CASES: [&str; 4] = ["G5-01", "G5-02", "G5-03", "G5-04"];

pub(super) fn check(root: &Path, candidate: &Candidate, blockers: &mut Vec<String>) -> bool {
    let before = blockers.len();
    if exact_readiness::check(root, candidate, "G5", G5_RECEIPT, &EXPECTED_CASES, blockers) {
        verify_artifacts(root, blockers);
    }
    blockers.len() == before
}

fn verify_artifacts(root: &Path, blockers: &mut Vec<String>) {
    let document = match receipt::read_receipt(&root.join(G5_RECEIPT)) {
        Ok(document) => document,
        Err(error) => {
            blockers.push(format!("G5 screenshot evidence: {error}"));
            return;
        }
    };
    for (case, minimum) in [("G5-01", 1), ("G5-02", 3), ("G5-03", 13), ("G5-04", 3)] {
        let artifacts = artifacts_for_case(&document, case);
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
            let absolute = root.join(&path);
            match receipt::sha256(&absolute) {
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
