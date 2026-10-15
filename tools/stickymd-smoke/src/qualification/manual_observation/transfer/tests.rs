//! Task and bundle codecs: round trips, the binding of each part, and strict rejection.

use super::{
    Bundle, Task, Unresolved, parse_bundle, parse_task, render_bundle, render_task, unresolved,
};
use crate::cli::{GuidedSession, ManualSelection};
use crate::integrity::sha256_bytes;
use crate::qualification::manual_observation::log::{Carried, Note, Status, fixtures};
use crate::qualification::manual_observation::reduce::Binding;
use crate::qualification::manual_observation::target::{self, Role, fixtures::target};
use crate::qualification::receipt::{Candidate, RELEASE_ARTIFACT_NAME, render_candidate};

/// A labelled way to break one part of a task or bundle.
type Change<T> = Box<dyn Fn(&mut T)>;

fn candidate() -> Candidate {
    Candidate {
        source_commit: "a".repeat(40),
        version: "0.1.3".to_owned(),
        cargo_lock_sha256: "b".repeat(64),
        exe_sha256: "c".repeat(64),
        zip_sha256: "d".repeat(64),
        sbom_sha256: "e".repeat(64),
        target: "x86_64-pc-windows-msvc".to_owned(),
        workflow_run_id: 7,
        workflow_attempt: 1,
        artifact_id: 9,
        artifact_name: RELEASE_ARTIFACT_NAME.to_owned(),
        zip_name: "StickyMD-0.1.3-windows-x64-portable.zip".to_owned(),
    }
}

/// A task whose parts belong together, with one unresolved FAIL.
fn task() -> Task {
    let candidate_receipt = render_candidate(&candidate());
    let candidate_receipt_sha256 = sha256_bytes(candidate_receipt.as_bytes()).unwrap();
    let mut declared = target();
    declared.candidate_receipt_sha256 = candidate_receipt_sha256.clone();
    let target_receipt = target::render(&declared);
    Task {
        context_id: declared.context_id.clone(),
        base: 3,
        selection: ManualSelection::Guided(Some(GuidedSession::G2)),
        registry_sha256: "f".repeat(64),
        target_sha256: sha256_bytes(target_receipt.as_bytes()).unwrap(),
        candidate_receipt,
        candidate_receipt_sha256,
        candidate: candidate(),
        target_receipt,
        target: declared,
        unresolved: vec![Unresolved {
            case: "P12-M25".to_owned(),
            role: Role::Current,
            scenario: "formulas".to_owned(),
            session: fixtures::SESSION.to_owned(),
            seq: 2,
            position: 1,
            note: Some("matrix \"misaligned\"\nbadly".to_owned()),
        }],
        exported_at_unix: 1_760_000_000,
    }
}

#[test]
fn a_task_round_trips_with_its_receipts_verbatim() {
    let task = task();
    let parsed = parse_task(&render_task(&task)).unwrap();
    assert_eq!(parsed.candidate_receipt, task.candidate_receipt);
    assert_eq!(parsed.target_receipt, task.target_receipt);
    assert_eq!(parsed.candidate, task.candidate);
    assert_eq!(parsed.target, task.target);
    assert_eq!(parsed.unresolved, task.unresolved);
    assert_eq!(parsed.selection, task.selection);
    assert_eq!((parsed.base, parsed.exported_at_unix), (3, 1_760_000_000));
    assert_eq!(render_task(&parsed), render_task(&task));
}

#[test]
fn a_task_whose_parts_do_not_belong_together_is_refused() {
    let cases: Vec<(&str, Change<Task>)> = vec![
        (
            "candidate digest",
            Box::new(|task| task.candidate_receipt_sha256 = "0".repeat(64)),
        ),
        (
            "candidate text",
            Box::new(|task| {
                task.candidate_receipt = task.candidate_receipt.replace("0.1.3", "0.1.4")
            }),
        ),
        (
            "target text",
            Box::new(|task| task.target_receipt = task.target_receipt.replace("25H2", "26H2")),
        ),
        (
            "target of another candidate",
            Box::new(|task| {
                let mut other = task.target.clone();
                other.candidate_receipt_sha256 = "9".repeat(64);
                task.target_receipt = target::render(&other);
                task.target_sha256 = sha256_bytes(task.target_receipt.as_bytes()).unwrap();
            }),
        ),
        ("context", Box::new(|task| task.context_id = "1".repeat(32))),
        (
            "unknown scenario",
            Box::new(|task| task.unresolved[0].scenario = "nowhere".to_owned()),
        ),
        (
            "FAIL after base",
            Box::new(|task| task.unresolved[0].position = 3),
        ),
        (
            "empty note",
            Box::new(|task| task.unresolved[0].note = Some(String::new())),
        ),
    ];
    for (label, change) in cases {
        let mut task = task();
        change(&mut task);
        assert!(parse_task(&render_task(&task)).is_err(), "{label}");
    }
    let text = render_task(&task());
    for tampered in [
        text.replacen(
            "{\"schema_version\":1,",
            "{\"schema_version\":1,\"extra\":0,",
            1,
        ),
        text.replace("observation-task", "observation-bundle"),
        text.replace("\"selection\":\"G2\"", "\"selection\":\"G9\""),
        text.replace("\"schema_version\":1", "\"schema_version\":2"),
    ] {
        assert!(parse_task(&tampered).is_err(), "{tampered}");
    }
}

fn bundle() -> Bundle {
    let task = task();
    let mut bundle = Bundle::start(&task, "a".repeat(64), fixtures::SESSION.to_owned());
    bundle.observations = vec![
        fixtures::observation(1, 3, "P12-M25", "formulas", Role::Current, Status::Pass),
        fixtures::observation(
            2,
            3,
            "P12-M34",
            "clean-vm-first-run",
            Role::Previous,
            Status::Fail,
        ),
    ];
    bundle.notes = vec![Note {
        session: fixtures::SESSION.to_owned(),
        seq: 2,
        text: "SmartScreen text differs".to_owned(),
    }];
    bundle
}

#[test]
fn a_bundle_round_trips() {
    let bundle = bundle();
    assert_eq!(parse_bundle(&render_bundle(&bundle)).unwrap(), bundle);
    let empty = Bundle::start(&task(), "a".repeat(64), fixtures::SESSION.to_owned());
    assert_eq!(parse_bundle(&render_bundle(&empty)).unwrap(), empty);
    assert_eq!(empty.base, 3);
    assert_eq!(empty.target_sha256, task().target_sha256);
}

#[test]
fn a_bundle_is_one_session_in_order_from_its_base() {
    let cases: Vec<(&str, Change<Bundle>)> = vec![
        ("gap", Box::new(|bundle| bundle.observations[1].seq = 3)),
        (
            "reordered",
            Box::new(|bundle| bundle.observations.swap(0, 1)),
        ),
        (
            "other base",
            Box::new(|bundle| bundle.observations[0].base = 2),
        ),
        (
            "other session",
            Box::new(|bundle| bundle.observations[1].session = "1".repeat(32)),
        ),
        (
            "carried",
            Box::new(|bundle| {
                bundle.observations[1].carried_from = vec![Carried {
                    context_id: "2".repeat(32),
                    index: 0,
                }];
            }),
        ),
        (
            "note on nothing",
            Box::new(|bundle| bundle.notes[0].seq = 5),
        ),
        (
            "two notes",
            Box::new(|bundle| {
                let mut second = bundle.notes[0].clone();
                second.text = "other".to_owned();
                bundle.notes.push(second);
            }),
        ),
        (
            "unknown case",
            Box::new(|bundle| bundle.observations[0].case = "P12-M99".to_owned()),
        ),
        (
            "bad digest",
            Box::new(|bundle| bundle.target_sha256 = "xyz".to_owned()),
        ),
    ];
    for (label, change) in cases {
        let mut bundle = bundle();
        change(&mut bundle);
        assert!(parse_bundle(&render_bundle(&bundle)).is_err(), "{label}");
    }
    let mut empty = Bundle::start(&task(), "a".repeat(64), fixtures::SESSION.to_owned());
    empty.base = u64::MAX;
    assert!(
        parse_bundle(&render_bundle(&empty)).is_err(),
        "base beyond any log"
    );
    let text = render_bundle(&bundle());
    assert!(parse_bundle(&text.replace("observation-bundle", "observation-task")).is_err());
    assert!(parse_bundle(&text.replacen("\"base\":3,", "\"base\":3,\"x\":1,", 1)).is_err());
}

#[test]
fn unresolved_fails_carry_their_notes_until_a_later_session_passes() {
    let mut log = fixtures::empty();
    log.append(vec![fixtures::observation(
        1,
        0,
        "P12-M25",
        "formulas",
        Role::Current,
        Status::Fail,
    )])
    .unwrap();
    log.annotate(vec![Note {
        session: fixtures::SESSION.to_owned(),
        seq: 1,
        text: "matrix misaligned".to_owned(),
    }])
    .unwrap();
    let target = target();
    let binding = Binding {
        target: &target,
        exe_sha256: fixtures::EXE,
    };
    let entries = unresolved(&log, binding);
    assert_eq!(entries.len(), 1);
    assert_eq!(
        (entries[0].position, entries[0].note.as_deref()),
        (0, Some("matrix misaligned"))
    );
    log.append(vec![fixtures::observation(
        2,
        1,
        "P12-M25",
        "formulas",
        Role::Current,
        Status::Pass,
    )])
    .unwrap();
    assert!(unresolved(&log, binding).is_empty());
}
