//! Evidence is judged from a strict parse of the object that owns each value.

use std::path::PathBuf;

use super::fixtures::{exact, valid_document};
use super::{Companion, Origin, g5_companions, validate};
use crate::qualification::module_registry::{ModuleId, modules};
use crate::qualification::receipt::{Candidate, RELEASE_ARTIFACT_NAME};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Self(std::env::temp_dir().join(format!(
            "stickymd-module-evidence-{}-{sequence}",
            std::process::id()
        )))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Created exclusively by this test under the temp directory.
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn candidate() -> Candidate {
    Candidate {
        source_commit: "a".repeat(40),
        version: "0.1.0".to_owned(),
        cargo_lock_sha256: "b".repeat(64),
        exe_sha256: "c".repeat(64),
        zip_sha256: "d".repeat(64),
        sbom_sha256: "e".repeat(64),
        target: "x86_64-pc-windows-msvc".to_owned(),
        workflow_run_id: 1,
        workflow_attempt: 1,
        artifact_id: 2,
        artifact_name: RELEASE_ARTIFACT_NAME.to_owned(),
        zip_name: "StickyMD-0.1.0-windows-x64-portable.zip".to_owned(),
    }
}

const INPUT: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn origin(candidate: &Candidate) -> Origin<'_> {
    Origin {
        source_commit: &candidate.source_commit,
        version: &candidate.version,
        exe_sha256: &candidate.exe_sha256,
        zip_sha256: &candidate.zip_sha256,
        input_fingerprint: INPUT,
    }
}

fn passing_cases(ids: &[&str]) -> String {
    ids.iter()
        .map(|id| format!("{{\"id\":\"{id}\",\"status\":\"PASSED\",\"detail\":null}}"))
        .collect::<Vec<_>>()
        .join(",")
}

#[test]
fn every_module_accepts_its_complete_passing_document() {
    let scratch = Scratch::new();
    let candidate = candidate();
    for module in modules() {
        let document = valid_document(&scratch.0, module, &candidate, INPUT, "run");
        let companions = validate(module, &document, &origin(&candidate))
            .unwrap_or_else(|error| panic!("{module:?}: {error}"));
        assert_eq!(companions.is_empty(), module != ModuleId::G5, "{module:?}");
    }
}

#[test]
fn a_nested_or_respaced_field_cannot_stand_in_for_the_real_one() {
    let candidate = candidate();
    let ids = ["G3-01", "G3-02", "G3-03", "G3-04", "G3-05"];
    // Legal JSON: whitespace around the real status, a PASSED decoy nested deeper.
    let failing = passing_cases(&ids).replacen(
        "\"status\":\"PASSED\"",
        "\"status\" : \"FAILED\",\"extra\":{\"status\":\"PASSED\"}",
        1,
    );
    let error = validate(
        ModuleId::G3,
        &exact(&candidate, &failing),
        &origin(&candidate),
    )
    .unwrap_err();
    assert!(error.contains("did not pass"), "{error}");

    let dirty = exact(&candidate, &passing_cases(&ids)).replace(
        "\"worktree_dirty\":false",
        "\"worktree_dirty\" : true,\"nested\":{\"worktree_dirty\":false}",
    );
    assert!(validate(ModuleId::G3, &dirty, &origin(&candidate)).is_err());
}

#[test]
fn malformed_documents_are_invalid_rather_than_partially_read() {
    let candidate = candidate();
    let ids = ["G4-01", "G4-02", "G4-03", "G4-04", "G4-05", "G4-06"];
    let document = exact(&candidate, &passing_cases(&ids));
    // The baseline is complete, so each rejection below comes from its one mutation.
    validate(ModuleId::G4, &document, &origin(&candidate)).unwrap();
    for broken in [
        document.trim_end().trim_end_matches('}').to_owned(),
        document.replacen(
            "\"status\":\"PASSED\",",
            "\"status\":\"PASSED\",\"status\":\"PASSED\",",
            1,
        ),
        format!("{document} {{}}"),
    ] {
        assert!(
            validate(ModuleId::G4, &broken, &origin(&candidate)).is_err(),
            "{broken}"
        );
    }
}

#[test]
fn exact_groups_need_every_case_in_order_and_the_origin_identity() {
    let candidate = candidate();
    let ids = ["G4-01", "G4-02", "G4-03", "G4-04", "G4-05", "G4-06"];
    let complete = exact(&candidate, &passing_cases(&ids));
    validate(ModuleId::G4, &complete, &origin(&candidate)).unwrap();
    let incomplete = exact(&candidate, &passing_cases(&ids[..5]));
    let error = validate(ModuleId::G4, &incomplete, &origin(&candidate)).unwrap_err();
    assert!(error.contains("exact cases are"), "{error}");
    let mut reordered = ids;
    reordered.swap(0, 1);
    assert!(
        validate(
            ModuleId::G4,
            &exact(&candidate, &passing_cases(&reordered)),
            &origin(&candidate)
        )
        .is_err()
    );
    let mut other = candidate.clone();
    other.version = "9.9.9".to_owned();
    let error = validate(ModuleId::G4, &complete, &origin(&other)).unwrap_err();
    assert!(error.contains("STALE RECEIPT: version"), "{error}");
    for (from, to) in [
        ("\"worktree_dirty\":false", "\"worktree_dirty\":true"),
        ("\"windows\":\"Windows test\"", "\"windows\":\"UNKNOWN\""),
        ("\"harness_commit\":\"aaaa", "\"harness_commit\":\"baaa"),
    ] {
        assert!(
            validate(
                ModuleId::G4,
                &complete.replace(from, to),
                &origin(&candidate)
            )
            .is_err(),
            "{to}"
        );
    }
}

#[test]
fn g5_needs_each_case_to_own_its_minimum_screenshots_at_safe_paths() {
    let scratch = Scratch::new();
    let candidate = candidate();
    let document = valid_document(&scratch.0, ModuleId::G5, &candidate, INPUT, "run");
    let companions = validate(ModuleId::G5, &document, &origin(&candidate)).unwrap();
    assert_eq!(companions.len(), 1 + 3 + 13 + 3);
    // Moving one G5-01 screenshot under G5-04 keeps the total but not the per-case count.
    let first = &companions[0];
    let entry = format!(
        "{{\"path\":\"{}\",\"sha256\":\"{}\"}}",
        first.path, first.sha256
    );
    // G5-01 lists exactly one screenshot, so removing it leaves `"artifacts":[]`.
    let moved = document.replacen(&entry, "", 1).replace(
        "\"id\":\"G5-04\",\"status\":\"PASSED\",\"detail\":\"run\",\"artifacts\":[",
        &format!(
            "\"id\":\"G5-04\",\"status\":\"PASSED\",\"detail\":\"run\",\"artifacts\":[{entry},"
        ),
    );
    let error = validate(ModuleId::G5, &moved, &origin(&candidate)).unwrap_err();
    assert!(error.contains("G5-01 has 0 screenshot"), "{error}");
    let escaped = document.replacen(
        "dist/evidence/g5-artifacts/",
        "dist/evidence/g5-artifacts/../",
        1,
    );
    assert!(validate(ModuleId::G5, &escaped, &origin(&candidate)).is_err());
}

#[test]
fn smoke_coverage_requires_the_complete_formal_plan_on_the_origin_executable() {
    let candidate = candidate();
    for module in [ModuleId::Runtime, ModuleId::Performance] {
        let document = valid_document(std::path::Path::new("."), module, &candidate, INPUT, "run");
        validate(module, &document, &origin(&candidate)).unwrap();
        let unused_build = document.replace(
            "promoted candidate identity and artifact verification",
            "Release Windows app build",
        );
        assert!(validate(module, &unused_build, &origin(&candidate)).is_err());
        let mut other = candidate.clone();
        other.exe_sha256 = "0".repeat(64);
        assert!(validate(module, &document, &origin(&other)).is_err());
    }
    let sentinel = r#"{"schema_version":2,"suite":"phase-14","commit":"COMMIT","worktree_dirty":false,"executable_sha256":"EXE","results":[{"id":"copied Release Phase 8 close-to-tray/show lifecycle","status":"PASSED"}]}"#
        .replace("COMMIT", &candidate.source_commit)
        .replace("EXE", &candidate.exe_sha256);
    assert!(validate(ModuleId::Runtime, &sentinel, &origin(&candidate)).is_err());
    // The tasks readiness used to require by name stay part of the formal plans.
    assert!(
        crate::runner::formal_task_labels(true)
            .unwrap()
            .contains(&"copied Release Phase 8 close-to-tray/show lifecycle")
    );
    assert!(
        crate::runner::formal_task_labels(false)
            .unwrap()
            .contains(&"copied Release Phase 9 editor-ready cold/warm startup matrix")
    );
}

#[test]
fn resource_evidence_must_carry_its_planned_input_and_candidate() {
    let scratch = Scratch::new();
    let candidate = candidate();
    for group in crate::resource_plan::GROUPS {
        let module = ModuleId::Resource(group);
        let document = valid_document(&scratch.0, module, &candidate, INPUT, "run");
        validate(module, &document, &origin(&candidate)).unwrap();
        let drifted = Origin {
            input_fingerprint: &"f".repeat(64),
            ..origin(&candidate)
        };
        assert!(validate(module, &document, &drifted).is_err());
    }
}

#[test]
fn companion_lists_are_read_per_case_whatever_the_whitespace() {
    let artifact = |name: &str| {
        format!(
            "{{\"path\":\"dist/evidence/g5-artifacts/{name}.png\",\"sha256\":\"{}\"}}",
            "a".repeat(64)
        )
    };
    let late = (0..13)
        .map(|i| artifact(&format!("late-{i}")))
        .collect::<Vec<_>>();
    let document = format!(
        "{{\"results\": [ {{\"id\":\"G5-01\",\"artifacts\":[]}}, {{\"id\":\"G5-99\",\"artifacts\":[{}]}},\n {{\"id\":\"G5-03\"}} ]}}",
        late.join(", ")
    );
    let companions = g5_companions(&document).unwrap();
    assert_eq!(companions.len(), 13);
    assert!(
        companions
            .iter()
            .all(|Companion { path, .. }| path.contains("late-"))
    );
    for broken in [
        "{\"results\":[{\"id\":\"G5-01\",\"artifacts\":{}}]}",
        "{\"results\":[{\"id\":\"G5-01\",\"artifacts\":[{\"path\":\"x\"}]}]}",
        "{\"results\":[{\"id\":\"G5-01\",\"artifacts\":[{\"path\":\"x\",\"sha256\":\"short\"}]}]}",
        "{\"results\":[{\"artifacts\":[]}]}",
        "{\"results\":[{\"id\":\"G5-01\",\"artifacts\":[]}]",
    ] {
        assert!(g5_companions(broken).is_err(), "{broken}");
    }
}
