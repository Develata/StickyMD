//! Fail-closed validation shared by exact-candidate desktop evidence groups.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::path::Path;

use super::receipt::Candidate;
use super::{json, receipt};

pub(super) fn check(
    root: &Path,
    candidate: &Candidate,
    label: &str,
    relative_receipt: &str,
    expected_cases: &[&str],
    blockers: &mut Vec<String>,
) -> bool {
    let before = blockers.len();
    let document = match receipt::read_receipt(&root.join(relative_receipt)) {
        Ok(document) => document,
        Err(error) => {
            blockers.push(format!("{label} exact qualification receipt: {error}"));
            return false;
        }
    };
    expect_u64(&document, label, "schema_version", 1, blockers);
    expect_string(&document, label, "status", "PASSED", blockers);
    expect_string(
        &document,
        label,
        "source_commit",
        &candidate.source_commit,
        blockers,
    );
    expect_string(
        &document,
        label,
        "harness_commit",
        &candidate.source_commit,
        blockers,
    );
    expect_string(
        &document,
        label,
        "exe_sha256",
        &candidate.exe_sha256,
        blockers,
    );
    expect_string(
        &document,
        label,
        "zip_sha256",
        &candidate.zip_sha256,
        blockers,
    );
    expect_string(&document, label, "version", &candidate.version, blockers);
    match json::string_field(&document, "windows") {
        Ok(value) if !value.trim().is_empty() && value != "UNKNOWN" => {}
        Ok(_) => blockers.push(format!("{label} exact Windows build is unavailable")),
        Err(error) => blockers.push(format!("{label} exact Windows build: {error}")),
    }
    match json::bool_field(&document, "worktree_dirty") {
        Ok(false) => {}
        Ok(true) => blockers.push(format!(
            "{label} exact qualification was recorded from a dirty tree"
        )),
        Err(error) => blockers.push(format!("{label} exact worktree state: {error}")),
    }
    let ids = result_fields(&document, "id");
    if ids != expected_cases {
        blockers.push(format!(
            "{label} exact receipt cases are {ids:?}, expected {expected_cases:?}"
        ));
    }
    let statuses = result_fields(&document, "status");
    if statuses.len() != expected_cases.len() || statuses.iter().any(|status| status != "PASSED") {
        blockers.push(format!(
            "{label} exact receipt contains non-PASSED case results: {statuses:?}"
        ));
    }
    blockers.len() == before
}

fn expect_u64(document: &str, label: &str, key: &str, expected: u64, blockers: &mut Vec<String>) {
    match json::u64_field(document, key) {
        Ok(actual) if actual == expected => {}
        Ok(actual) => blockers.push(format!(
            "{label} exact receipt {key} is {actual}, expected {expected}"
        )),
        Err(error) => blockers.push(format!("{label} exact receipt {key}: {error}")),
    }
}

fn expect_string(
    document: &str,
    label: &str,
    key: &str,
    expected: &str,
    blockers: &mut Vec<String>,
) {
    match json::string_field(document, key) {
        Ok(actual) if actual == expected => {}
        Ok(actual) => blockers.push(format!(
            "STALE RECEIPT: {label} exact {key} is {actual}, expected {expected}"
        )),
        Err(error) => blockers.push(format!("{label} exact {key}: {error}")),
    }
}

fn result_fields(document: &str, key: &str) -> Vec<String> {
    let marker = "\"results\":[";
    let Some(results) = document.split_once(marker).map(|(_, results)| results) else {
        return Vec::new();
    };
    let field = format!("\"{key}\":\"");
    let mut values = Vec::new();
    let mut rest = results;
    while let Some((_, tail)) = rest.split_once(&field) {
        let Some((value, next)) = tail.split_once('"') else {
            break;
        };
        values.push(value.to_owned());
        rest = next;
    }
    values
}

#[cfg(test)]
mod tests {
    use super::check;
    use crate::qualification::receipt::{self, Candidate};
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn exact_receipt_requires_clean_same_commit_and_complete_ordered_cases() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("stickymd-exact-readiness-{nonce}"));
        let candidate = candidate();
        let relative = "dist/evidence/group.json";
        receipt::write_receipt(&root, relative, &document(&candidate, false, 2))
            .expect("write exact receipt");
        let mut blockers = Vec::new();
        assert!(check(
            &root,
            &candidate,
            "GX",
            relative,
            &["GX-01", "GX-02"],
            &mut blockers
        ));
        assert!(blockers.is_empty());

        receipt::write_receipt(&root, relative, &document(&candidate, true, 2))
            .expect("write dirty exact receipt");
        blockers.clear();
        assert!(!check(
            &root,
            &candidate,
            "GX",
            relative,
            &["GX-01", "GX-02"],
            &mut blockers
        ));
        assert!(
            blockers
                .iter()
                .any(|blocker| blocker.contains("dirty tree"))
        );

        receipt::write_receipt(&root, relative, &document(&candidate, false, 1))
            .expect("write incomplete exact receipt");
        blockers.clear();
        assert!(!check(
            &root,
            &candidate,
            "GX",
            relative,
            &["GX-01", "GX-02"],
            &mut blockers
        ));
        assert!(blockers.iter().any(|blocker| blocker.contains("cases are")));
        fs::remove_dir_all(root).expect("cleanup");
    }

    fn candidate() -> Candidate {
        Candidate {
            source_commit: "a".repeat(40),
            version: "0.1.0".to_owned(),
            cargo_lock_sha256: "b".repeat(64),
            exe_sha256: "c".repeat(64),
            zip_sha256: "d".repeat(64),
            sbom_sha256: "e".repeat(64),
            rustc: "rustc test".to_owned(),
            target: "x86_64-pc-windows-msvc".to_owned(),
            remote_synced: false,
        }
    }

    fn document(candidate: &Candidate, dirty: bool, count: usize) -> String {
        let results = (1..=count)
            .map(|number| {
                format!("{{\"id\":\"GX-{number:02}\",\"status\":\"PASSED\",\"detail\":null}}")
            })
            .collect::<Vec<_>>()
            .join(",");
        format!(
            concat!(
                "{{\"schema_version\":1,\"status\":\"PASSED\",",
                "\"source_commit\":\"{}\",\"harness_commit\":\"{}\",",
                "\"worktree_dirty\":{},\"version\":\"{}\",",
                "\"windows\":\"Windows test\",\"exe_sha256\":\"{}\",",
                "\"zip_sha256\":\"{}\",\"results\":[{}]}}"
            ),
            candidate.source_commit,
            candidate.source_commit,
            dirty,
            candidate.version,
            candidate.exe_sha256,
            candidate.zip_sha256,
            results,
        )
    }
}
