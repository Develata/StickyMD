//! Evidence is judged from a strict parse of the object that owns each value.

use std::path::PathBuf;

use super::fixtures::{checks, exact, valid_document};
use super::{Companion, G5_SCREENSHOTS, Origin, g5_companions, validate};
use crate::qualification::exact_checks::{DeclaredCase, G3, G4, G5};
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

/// Passing results for `cases`, each with exactly its declared checks.
fn passing_cases<'a>(cases: impl IntoIterator<Item = &'a DeclaredCase>) -> String {
    cases
        .into_iter()
        .map(|case| {
            format!(
                "{{\"id\":\"{}\",\"status\":\"PASSED\",\"detail\":null,\"checks\":[{}]}}",
                case.id,
                checks(case.checks)
            )
        })
        .collect::<Vec<_>>()
        .join(",")
}

/// A schema 2 result passes only with exactly its declared checks: a missing, extra,
/// repeated or out-of-order label, another case's label, an absent or non-string list and a
/// schema 1 document are all invalid, and a G5 case declares no labels.
#[test]
fn exact_results_need_exactly_the_declared_checks() {
    let scratch = Scratch::new();
    let candidate = candidate();
    let complete = exact(&candidate, &passing_cases(G3));
    validate(ModuleId::G3, &complete, &origin(&candidate)).unwrap();
    let first = format!("\"checks\":[{}]", checks(G3[0].checks));
    let mut labels = G3[0].checks.to_vec();
    let variants =
        |labels: &[&str]| complete.replacen(&first, &format!("\"checks\":[{}]", checks(labels)), 1);
    let mut broken = vec![
        variants(&labels[1..]),
        variants(&[&labels[..], &["zz-extra"]].concat()),
    ];
    broken.push(variants(&[&labels[..1], &labels[..]].concat()));
    labels.swap(0, 1);
    broken.push(variants(&labels));
    broken.push(variants(&[&G3[0].checks[1..], &G3[1].checks[..1]].concat()));
    broken.push(complete.replacen(&format!(",{first}"), "", 1));
    broken.push(complete.replacen(&first, "\"checks\":[1]", 1));
    broken.push(complete.replacen("\"schema_version\":2", "\"schema_version\":1", 1));
    for document in &broken {
        assert_ne!(document, &complete);
        assert!(
            validate(ModuleId::G3, document, &origin(&candidate)).is_err(),
            "{document}"
        );
    }
    let error = validate(ModuleId::G3, &broken[0], &origin(&candidate)).unwrap_err();
    assert!(
        error.contains("checks differ from the declaration: missing"),
        "{error}"
    );

    let g5 = valid_document(&scratch.0, ModuleId::G5, &candidate, INPUT, "run");
    validate(ModuleId::G5, &g5, &origin(&candidate)).unwrap();
    let labelled = g5.replacen("\"checks\":[]", "\"checks\":[\"dock-snap\"]", 1);
    assert!(validate(ModuleId::G5, &labelled, &origin(&candidate)).is_err());
}

#[test]
fn g5_screenshot_minimums_follow_the_declared_g5_cases() {
    let ids = G5_SCREENSHOTS.map(|(case, _)| case);
    assert_eq!(
        ids.as_slice(),
        G5.iter().map(|case| case.id).collect::<Vec<_>>()
    );
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
    // Legal JSON: whitespace around the real status, a PASSED decoy nested deeper.
    let failing = passing_cases(G3).replacen(
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

    let dirty = exact(&candidate, &passing_cases(G3)).replace(
        "\"worktree_dirty\":false",
        "\"worktree_dirty\" : true,\"nested\":{\"worktree_dirty\":false}",
    );
    assert!(validate(ModuleId::G3, &dirty, &origin(&candidate)).is_err());
}

#[test]
fn malformed_documents_are_invalid_rather_than_partially_read() {
    let candidate = candidate();
    let document = exact(&candidate, &passing_cases(G4));
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
    let complete = exact(&candidate, &passing_cases(G4));
    validate(ModuleId::G4, &complete, &origin(&candidate)).unwrap();
    let incomplete = exact(&candidate, &passing_cases(&G4[..5]));
    let error = validate(ModuleId::G4, &incomplete, &origin(&candidate)).unwrap_err();
    assert!(error.contains("exact cases are"), "{error}");
    let reordered = [&G4[1], &G4[0]].into_iter().chain(&G4[2..]);
    assert!(
        validate(
            ModuleId::G4,
            &exact(&candidate, &passing_cases(reordered)),
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
        (
            "\"windows\":\"Microsoft Windows 10.0.26200.9457\"",
            "\"windows\":\"UNKNOWN\"",
        ),
        (
            "\"windows\":\"Microsoft Windows 10.0.26200.9457\"",
            "\"windows\":\"Windows_NT\"",
        ),
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
    let g5_04 =
        "\"id\":\"G5-04\",\"status\":\"PASSED\",\"detail\":\"run\",\"checks\":[],\"artifacts\":[";
    assert_eq!(document.matches(g5_04).count(), 1);
    let moved = document
        .replacen(&entry, "", 1)
        .replace(g5_04, &format!("{g5_04}{entry},"));
    assert_eq!(g5_companions(&moved).unwrap().len(), companions.len());
    let error = validate(ModuleId::G5, &moved, &origin(&candidate)).unwrap_err();
    assert!(error.contains("G5-01 has 0 screenshot"), "{error}");
    let escaped = document.replacen(
        "dist/evidence/g5-artifacts/",
        "dist/evidence/g5-artifacts/../",
        1,
    );
    assert!(validate(ModuleId::G5, &escaped, &origin(&candidate)).is_err());
}

/// Schema 3 smoke and resource results list no owners: a missing, non-array or non-empty
/// `owners` field makes the document invalid for every reader.
#[test]
fn smoke_and_resource_results_must_list_no_owners() {
    let scratch = Scratch::new();
    let candidate = candidate();
    for module in [
        ModuleId::Runtime,
        ModuleId::Performance,
        ModuleId::Resource(crate::resource_plan::GROUPS[0]),
    ] {
        let document = valid_document(&scratch.0, module, &candidate, INPUT, "run");
        validate(module, &document, &origin(&candidate)).unwrap();
        assert!(document.contains("\"owners\":[]"), "{module:?}");
        for broken in [
            document.replacen("\"owners\":[]", "\"owners\":[\"p/lib/x\"]", 1),
            document.replacen(",\"owners\":[]", "", 1),
            document.replacen("\"owners\":[]", "\"owners\":{}", 1),
        ] {
            assert_ne!(broken, document);
            assert!(
                validate(module, &broken, &origin(&candidate)).is_err(),
                "{module:?}: {broken}"
            );
        }
    }
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
