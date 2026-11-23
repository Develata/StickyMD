//! The gate against bad samples: one good set of inputs, and samples that each change one fact.
//! The two samples the contract names (plan 11 #macos-release-gate) are tests of their own.

use super::judge::{Candidate, Inputs, Line, Read, Verdict, judge, ready};
use super::rows::{Class, ROWS};
use super::source_tests::{self, Counts, Host, Receipt, Run, SameKey, Side, Target};
use crate::mac_ledger::record::{Built, Identity, Record, Revoked};

const R: &str = "1111111111111111111111111111111111111111";

fn hex(digit: char) -> String {
    std::iter::repeat_n(digit, 64).collect()
}

fn candidate() -> Candidate {
    Candidate {
        receipt_sha256: hex('c'),
        zip_sha256: hex('d'),
        built: Built {
            executable: hex('e'),
            bundle: hex('b'),
        },
    }
}

fn identity(driver: char) -> Identity {
    Identity {
        inputs: Some(hex('a')),
        driver: hex(driver),
        image: "sha256:image".into(),
        system: "ProductName: macOS ProductVersion: 27.2".into(),
    }
}

fn record(row: &str, verdict: &str, driver: char, built: &Built, evidence: char) -> Record {
    Record::Automated {
        row: row.to_owned(),
        verdict: verdict.to_owned(),
        identity: identity(driver),
        built: built.clone(),
        evidence: format!("dist/evidence/run-{evidence}/mac-gui.json"),
        evidence_sha256: hex(evidence),
        run_unix: Some(1),
        batch: Some("rc1".into()),
        recorded_unix: 2,
    }
}

/// A PASS on the candidate's bytes for every row `mac-gui` runs, W12's logout cases and R03.
fn ledger() -> Vec<Record> {
    let built = candidate().built;
    let mut rows: Vec<String> = ROWS
        .iter()
        .filter(|(_, class)| matches!(class, Class::Gui | Class::Translocation))
        .map(|(row, _)| (*row).to_owned())
        .collect();
    rows.extend(crate::mac_gui::logout::rows());
    rows.iter()
        .map(|row| record(row, "PASS", '1', &built, '9'))
        .collect()
}

fn receipt() -> Receipt {
    let expected: Vec<Target> = source_tests::PACKAGES
        .iter()
        .map(|package| Target {
            package: (*package).to_owned(),
            kind: "lib".into(),
            name: package.replace('-', "_"),
        })
        .collect();
    Receipt {
        release_source: R.into(),
        host: Host {
            product_name: "macOS".into(),
            product_version: "27.2".into(),
            build_version: "27C55".into(),
            arch: "arm64".into(),
            virtual_machine: "0".into(),
            cargo: "cargo 1.97.1".into(),
            rustc: "rustc 1.97.1".into(),
        },
        start: Side {
            head: R.into(),
            clean: true,
        },
        end: Side {
            head: R.into(),
            clean: true,
        },
        command: source_tests::command(),
        exit_code: Some(0),
        binaries: expected
            .iter()
            .map(|target| Run {
                target: target.clone(),
                counts: Counts {
                    ran: 3,
                    passed: 3,
                    ..Counts::default()
                },
            })
            .collect(),
        expected,
        same_key: SameKey {
            command: source_tests::same_key_command(),
            tests: source_tests::SAME_KEY_TESTS.map(str::to_owned).to_vec(),
            counts: Some(Counts {
                ran: 2,
                passed: 2,
                filtered_out: 9,
                ..Counts::default()
            }),
            exit_code: Some(0),
        },
        problems: Vec::new(),
    }
}

fn run(candidate: Read<&Candidate>, records: &[Record], tests: &Receipt) -> Vec<Line> {
    judge(&Inputs {
        release: R,
        batch: Some("rc1"),
        candidate,
        ledger: Read::Present(records),
        source_tests: Read::Present(tests),
    })
}

fn verdict<'a>(lines: &'a [Line], item: &str) -> &'a Line {
    lines
        .iter()
        .find(|line| line.item == item)
        .unwrap_or_else(|| panic!("no line {item}"))
}

fn assert_line(lines: &[Line], item: &str, expected: Verdict, words: &str) {
    let line = verdict(lines, item);
    assert_eq!(line.verdict, expected, "{line:?}");
    assert!(line.detail.contains(words), "{line:?}");
}

/// The rows that cannot hold until their formats and producers are decided (M5 report).
fn awaiting_decision(item: &str) -> bool {
    item == "download-path"
        || ROWS.iter().any(|(row, class)| {
            format!("P16-{row}") == item
                && matches!(
                    class,
                    Class::Manual | Class::Resources | Class::Translocation
                )
        })
}

#[test]
fn good_inputs_pass_every_row_that_has_an_evidence_source() {
    let candidate = candidate();
    let lines = run(Read::Present(&candidate), &ledger(), &receipt());
    assert_eq!(lines.len(), 4 + ROWS.len());
    for line in &lines {
        let expected = if awaiting_decision(&line.item) {
            Verdict::NotTested
        } else if ["P16-W15", "P16-W16", "P16-V05"].contains(&line.item.as_str()) {
            Verdict::Excluded
        } else {
            Verdict::Pass
        };
        assert_eq!(line.verdict, expected, "{line:?}");
    }
    assert!(
        !ready(&lines),
        "rows awaiting a decision keep the gate closed"
    );
    let held: Vec<Line> = lines
        .into_iter()
        .filter(|line| !awaiting_decision(&line.item))
        .collect();
    assert!(ready(&held));
}

/// Contract sample: the evidence of a ZIP rebuilt from the `.app` is not the release ZIP, even
/// when the bundle inside is byte-identical.
#[test]
fn a_zip_repacked_from_the_app_is_not_r03_evidence() {
    let candidate = candidate();
    let lines = run(Read::Present(&candidate), &ledger(), &receipt());
    assert_line(
        &lines,
        "P16-R03",
        Verdict::NotTested,
        "a PASS on the candidate's bundle exists, but R03 must run on the candidate ZIP dddd",
    );
}

/// Contract sample: a PASS for the candidate's executable inside another bundle does not count.
#[test]
fn the_same_executable_in_another_bundle_is_not_a_pass() {
    let candidate = candidate();
    let other = Built {
        executable: candidate.built.executable.clone(),
        bundle: hex('f'),
    };
    let records: Vec<Record> = ledger()
        .into_iter()
        .map(|record| match record {
            Record::Automated { row, .. } if row == "W02" => record_on("W02", &other),
            record => record,
        })
        .collect();
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W02", Verdict::NotTested, "another bundle");
}

fn record_on(row: &str, built: &Built) -> Record {
    record(row, "PASS", '1', built, '9')
}

#[test]
fn a_pass_on_other_bytes_entirely_does_not_count() {
    let candidate = candidate();
    let other = Built {
        executable: hex('0'),
        bundle: hex('f'),
    };
    let mut records = ledger();
    records.retain(|r| !matches!(r, Record::Automated { row, .. } if row == "K02"));
    records.push(record_on("K02", &other));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-K02", Verdict::NotTested, "no PASS of P16-K02");
}

#[test]
fn a_fail_spoils_its_identity_and_another_identity_can_still_hold() {
    let candidate = candidate();
    let built = candidate.built.clone();
    let mut records = ledger();
    records.push(record("W03", "FAIL", '1', &built, '8'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W03", Verdict::Fail, "a FAIL of P16-W03");
    // A clean run under another driver on the same bytes holds, as in the ledger's own rule.
    records.push(record("W03", "PASS", '2', &built, '7'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W03", Verdict::Pass, "run-7");
}

#[test]
fn a_not_tested_or_revoked_record_beside_the_pass_spoils_it() {
    let candidate = candidate();
    let built = candidate.built.clone();
    let mut records = ledger();
    records.push(record("W04", "NOT_TESTED", '1', &built, '8'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-W04",
        Verdict::NotTested,
        "NOT_TESTED or revoked",
    );
    let mut records = ledger();
    records.push(Record::Revocation {
        row: "W07".into(),
        revoked: Revoked::Evidence(hex('9')),
        quote: "no".into(),
        recorded_unix: 3,
    });
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W07", Verdict::NotTested, "revoked");
}

#[test]
fn w12_needs_its_three_logout_cases_in_one_identity() {
    let candidate = candidate();
    let built = candidate.built.clone();
    let mut records = ledger();
    records.retain(|r| !matches!(r, Record::Automated { row, .. } if row == "W12-logout-dialog"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-W12",
        Verdict::NotTested,
        "no PASS of P16-W12-logout-dialog",
    );
    records.push(record("W12-logout-dialog", "PASS", '2', &built, '6'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-W12",
        Verdict::NotTested,
        "no clean identity in common",
    );
}

#[test]
fn r03_counts_only_this_batch_and_a_fail_on_these_bytes_blocks() {
    let candidate = candidate();
    let built = candidate.built.clone();
    let mut records = ledger();
    records.push(record("R03", "FAIL", '1', &built, '8'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-R03", Verdict::Fail, "a FAIL of P16-R03");
    // The same FAIL in another batch is not this batch's.
    if let Some(Record::Automated { batch, .. }) = records.last_mut() {
        *batch = Some("rc0".into());
    }
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-R03", Verdict::NotTested, "candidate ZIP");
    // Without a batch nothing counts.
    let lines = judge(&Inputs {
        release: R,
        batch: None,
        candidate: Read::Present(&candidate),
        ledger: Read::Present(&records),
        source_tests: Read::Present(&receipt()),
    });
    assert_line(
        &lines,
        "P16-R03",
        Verdict::NotTested,
        "no evidence binds it",
    );
}

#[test]
fn without_a_resolved_candidate_nothing_binds_to_bytes() {
    let receipt = receipt();
    let lines = run(Read::Missing("no candidate".into()), &ledger(), &receipt);
    assert_line(&lines, "package", Verdict::NotTested, "no candidate");
    assert_line(&lines, "P16-P01", Verdict::NotTested, "no candidate");
    assert_line(
        &lines,
        "P16-W02",
        Verdict::NotTested,
        "no resolved candidate",
    );
    let lines = run(
        Read::Invalid("the staged ZIP does not match the receipt".into()),
        &ledger(),
        &receipt,
    );
    assert_line(&lines, "package", Verdict::Fail, "staged ZIP");
    assert!(!ready(&lines));
}

#[test]
fn the_source_rows_follow_the_receipt_on_r() {
    let candidate = candidate();
    let mut other = receipt();
    other.release_source = "2".repeat(40);
    let lines = run(Read::Present(&candidate), &ledger(), &other);
    assert_line(&lines, "source-tests", Verdict::NotTested, "not 1111");
    assert_line(&lines, "P16-S01", Verdict::NotTested, "not 1111");
    assert_line(&lines, "same-key", Verdict::NotTested, "not 1111");
    let mut failing = receipt();
    failing.exit_code = Some(101);
    let lines = run(Read::Present(&candidate), &ledger(), &failing);
    assert_line(&lines, "P16-S01", Verdict::Fail, "exited");
    let lines = judge(&Inputs {
        release: R,
        batch: Some("rc1"),
        candidate: Read::Present(&candidate),
        ledger: Read::Present(&ledger()),
        source_tests: Read::Missing("no source-test receipt".into()),
    });
    assert_line(
        &lines,
        "P16-B01",
        Verdict::NotTested,
        "no source-test receipt",
    );
}

#[test]
fn an_unreadable_ledger_leaves_the_ledger_rows_open() {
    let candidate = candidate();
    let receipt = receipt();
    let lines = judge(&Inputs {
        release: R,
        batch: Some("rc1"),
        candidate: Read::Present(&candidate),
        ledger: Read::Invalid("the ledger is unreadable: line 3".into()),
        source_tests: Read::Present(&receipt),
    });
    assert_line(&lines, "P16-E01", Verdict::NotTested, "line 3");
    assert_line(&lines, "P16-S02", Verdict::Pass, "");
}
