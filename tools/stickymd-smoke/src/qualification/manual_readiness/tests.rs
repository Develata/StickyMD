//! Readiness policy over a real observation context: tiers, waivers and their binding.

use std::fs;
use std::path::PathBuf;

use super::{Report, check, evaluate, render};
use crate::qualification::decisions::{Decision, WaiverScope};
use crate::qualification::manual_observation::log::{self, Observation, Status, fixtures};
use crate::qualification::manual_observation::registry::{CASES, InstanceRule};
use crate::qualification::manual_observation::store::{self, OBSERVATION_LOG};
use crate::qualification::manual_observation::target::Role::{self, Current, Previous};
use crate::qualification::manual_observation::target::fixtures::target;
use crate::qualification::receipt::CANDIDATE_RECEIPT;

const VERSION: &str = "0.1.4";

struct Scratch(PathBuf);

fn candidate_digest(root: &std::path::Path) -> String {
    crate::integrity::sha256_bytes(&fs::read(root.join(CANDIDATE_RECEIPT)).unwrap()).unwrap()
}

impl Scratch {
    /// A worktree with a candidate receipt and declared targets.
    fn declared() -> (Self, String) {
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let sequence = SEQUENCE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "stickymd-manual-readiness-{}-{sequence}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("dist/evidence")).unwrap();
        fs::write(root.join(CANDIDATE_RECEIPT), "{\"candidate\":1}\n").unwrap();
        let candidate = candidate_digest(&root);
        let context = store::declare(
            &root,
            store::Promoted {
                receipt_sha256: &candidate,
                exe_sha256: fixtures::EXE,
            },
            &|| Ok(candidate_digest(&root)),
            target().roles,
            &|_| Ok(false),
        )
        .unwrap()
        .context_id;
        (Self(root), context)
    }

    /// Records every scenario of every case on both roles as PASS, except `skip` and the
    /// two-copy case, which the log cannot record until the two-instance format lands.
    fn record_all_except(&self, skip: &[(&str, Role)], fail: &[(&str, Role)]) {
        let mut log = store::load(&self.0, &candidate_digest(&self.0))
            .unwrap()
            .log;
        let mut batch: Vec<Observation> = Vec::new();
        for case in CASES
            .iter()
            .filter(|case| !matches!(case.instance, InstanceRule::TwoCopies(_)))
        {
            for role in [Current, Previous] {
                if skip.contains(&(case.id, role)) {
                    continue;
                }
                let status = if fail.contains(&(case.id, role)) {
                    Status::Fail
                } else {
                    Status::Pass
                };
                for scenario in case.scenarios {
                    let seq = batch.len() as u64 + 1;
                    batch.push(fixtures::observation(
                        seq, 0, case.id, scenario, role, status,
                    ));
                }
            }
        }
        log.append(batch).unwrap();
        crate::atomic_evidence::write(&self.0.join(OBSERVATION_LOG), log::render(&log).as_bytes())
            .unwrap();
    }

    fn check(&self, decisions: &[Decision], automated_ok: bool) -> (Report, Vec<String>) {
        let mut blockers = Vec::new();
        let context = store::load(&self.0, &candidate_digest(&self.0)).unwrap();
        let report = evaluate(
            Report::detached(&self.0),
            &context,
            fixtures::EXE,
            decisions,
            VERSION,
            automated_ok,
            &mut blockers,
        );
        (report, blockers)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        // Created exclusively by this test under the temp directory.
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn waiver(key: &str, context: &str, cells: &[(&str, Role)]) -> Decision {
    Decision {
        key: key.to_owned(),
        status: "USER APPROVED".to_owned(),
        evidence: "test".to_owned(),
        scope: Some(WaiverScope {
            context_id: context.to_owned(),
            cells: cells
                .iter()
                .map(|(case, role)| ((*case).to_owned(), *role))
                .collect(),
        }),
    }
}

/// The case waiver for P12-M45, which cannot be observed until the two-instance recorder
/// lands; tests that expect READY carry it.
fn two_copies_waived(context: &str) -> Decision {
    waiver(
        "WAIVER-P12-M45",
        context,
        &[("P12-M45", Current), ("P12-M45", Previous)],
    )
}

#[test]
fn a_fully_observed_context_is_ready_and_still_reports_its_gaps() {
    let (scratch, context) = Scratch::declared();
    scratch.record_all_except(&[], &[]);
    let (report, blockers) = scratch.check(&[two_copies_waived(&context)], true);
    assert!(blockers.is_empty(), "{blockers:?}");
    assert_eq!(report.context_id.as_deref(), Some(context.as_str()));
    assert_eq!(report.rows.len(), CASES.len() * 2);
    for row in &report.rows {
        let expected = if row.case == "P12-M45" {
            "USER WAIVED"
        } else {
            "MANUAL PASS"
        };
        assert_eq!(row.status, expected, "{}", row.case);
    }
    let json = render(&report);
    crate::qualification::json::parse_object(&json).unwrap();
    assert!(json.contains("\"known_gaps\":[\"P08-"), "{json}");
    assert!(
        json.contains("\"retired\":[\"P08-M12 accessibility"),
        "{json}"
    );
}

#[test]
fn an_unrecordable_two_copy_case_blocks_without_a_waiver() {
    let (scratch, _) = Scratch::declared();
    scratch.record_all_except(&[], &[]);
    let (_, blockers) = scratch.check(&[], true);
    assert_eq!(blockers.len(), 2, "{blockers:?}");
    assert!(
        blockers
            .iter()
            .all(|blocker| blocker.contains("Tier A") && blocker.contains("P12-M45"))
    );
}

#[test]
fn no_context_blocks_and_names_the_declaration() {
    let (scratch, _) = Scratch::declared();
    fs::remove_file(
        scratch
            .0
            .join(crate::qualification::manual_observation::target::TARGET_RECEIPT),
    )
    .unwrap();
    fs::remove_file(scratch.0.join(OBSERVATION_LOG)).unwrap();
    let mut blockers = Vec::new();
    let report = check(
        &scratch.0,
        &[],
        &candidate_digest(&scratch.0),
        fixtures::EXE,
        VERSION,
        true,
        &mut blockers,
    );
    assert!(report.context_id.is_none());
    assert!(
        blockers[0].contains("acceptance manual target"),
        "{blockers:?}"
    );
}

#[test]
fn tiers_decide_what_an_untested_cell_blocks() {
    let (scratch, context) = Scratch::declared();
    scratch.record_all_except(
        &[
            ("P12-M05", Previous),
            ("P12-M35", Current),
            ("P12-M41", Current),
        ],
        &[],
    );
    let (_, blockers) = scratch.check(&[two_copies_waived(&context)], true);
    assert_eq!(blockers.len(), 2, "{blockers:?}");
    assert!(blockers[0].contains("Tier A") && blockers[0].contains("P12-M05 (previous)"));
    assert!(blockers[1].contains("Tier B") && blockers[1].contains("P12-M35 (current)"));
    // Tier C blocks only while automated coverage is incomplete.
    let (_, blockers) = scratch.check(&[], false);
    assert!(blockers.iter().any(|blocker| blocker.contains("Tier C")));
}

#[test]
fn waivers_cover_only_their_cells_in_their_context_and_never_a_failure() {
    let (scratch, context) = Scratch::declared();
    scratch.record_all_except(
        &[
            ("P12-M05", Previous),
            ("P12-M24", Previous),
            ("P12-M35", Current),
        ],
        &[("P12-M26", Previous)],
    );
    let decisions = vec![
        waiver(
            "WAIVER-PROFILE-PREVIOUS-v0.1.4",
            &context,
            &[("P12-M05", Previous), ("P12-M26", Previous)],
        ),
        // Bound to another context: ignored.
        waiver(
            "WAIVER-P12-M24",
            "ffffffffffffffffffffffffffffffff",
            &[("P12-M24", Previous)],
        ),
        waiver("WAIVER-TIER-B-v0.1.4", &context, &[("P12-M35", Current)]),
        two_copies_waived(&context),
    ];
    let (report, blockers) = scratch.check(&decisions, true);
    let status = |case: &str, role: Role| {
        report
            .rows
            .iter()
            .find(|row| row.case == case && row.role == role)
            .unwrap()
            .status
    };
    assert_eq!(status("P12-M05", Previous), "USER WAIVED");
    assert_eq!(status("P12-M35", Current), "USER WAIVED");
    assert_eq!(status("P12-M26", Previous), "MANUAL_FAIL");
    assert_eq!(status("P12-M24", Previous), "NOT TESTED");
    assert!(
        report
            .notes
            .iter()
            .any(|note| note.contains("approved release policy"))
    );
    assert_eq!(blockers.len(), 2, "{blockers:?}");
    assert!(
        blockers
            .iter()
            .any(|blocker| blocker.contains("P12-M26 (previous) failed"))
    );
    assert!(
        blockers
            .iter()
            .any(|blocker| blocker.contains("P12-M24 (previous)"))
    );
}

#[test]
fn a_waiver_edited_past_its_key_waives_nothing() {
    let (scratch, context) = Scratch::declared();
    scratch.record_all_except(&[("P12-M05", Current)], &[]);
    // A profile waiver may not cover a current cell, whatever its stored cells say.
    let decisions = vec![
        waiver(
            "WAIVER-PROFILE-PREVIOUS-v0.1.4",
            &context,
            &[("P12-M05", Current)],
        ),
        two_copies_waived(&context),
    ];
    let (_, blockers) = scratch.check(&decisions, true);
    assert_eq!(blockers.len(), 1, "{blockers:?}");
}

#[test]
fn superseded_receipts_and_waivers_are_reported_not_used() {
    let (scratch, context) = Scratch::declared();
    scratch.record_all_except(&[("P12-M05", Current)], &[]);
    fs::write(
        scratch.0.join("dist/evidence/manual-acceptance.json"),
        "{\"schema_version\":1}",
    )
    .unwrap();
    let old = Decision {
        key: "WAIVER-P12-M05".to_owned(),
        status: "USER APPROVED".to_owned(),
        evidence: "old".to_owned(),
        scope: None,
    };
    let (report, blockers) = scratch.check(&[old, two_copies_waived(&context)], true);
    assert!(report.superseded_receipt);
    assert!(report.notes[0].contains("superseded format"));
    assert_eq!(blockers.len(), 1, "{blockers:?}");
}

/// Readiness reads the decisions once, under the manual lock, and the manual policy uses
/// that snapshot: no decision receipt exists on disk here, yet the supplied waiver applies.
#[test]
fn the_manual_policy_uses_the_callers_decision_snapshot() {
    let (scratch, context) = Scratch::declared();
    scratch.record_all_except(&[("P12-M05", Previous)], &[]);
    assert!(
        !scratch
            .0
            .join(crate::qualification::decisions::DECISION_RECEIPT)
            .exists()
    );
    let decisions = vec![
        waiver(
            "WAIVER-PROFILE-PREVIOUS-v0.1.4",
            &context,
            &[("P12-M05", Previous)],
        ),
        two_copies_waived(&context),
    ];
    let mut blockers = Vec::new();
    let _guard = store::lock(&scratch.0, false).unwrap();
    check(
        &scratch.0,
        &decisions,
        &candidate_digest(&scratch.0),
        fixtures::EXE,
        VERSION,
        true,
        &mut blockers,
    );
    assert!(blockers.is_empty(), "{blockers:?}");
}
