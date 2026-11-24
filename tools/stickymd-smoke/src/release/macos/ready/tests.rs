//! The gate against bad samples: one good set of inputs, and samples that each change one fact.
//! The two samples the contract names (plan 11 #macos-release-gate) are tests of their own.

use super::judge::{Candidate, Inputs, Line, Read, Verdict, judge, ready};
use super::rows::{Class, ROWS};
use super::source_tests::{self, Counts, Host, Receipt, Run, SameKey, Side, Target};
use crate::mac_ledger::record::{Built, Identity, Record, Revoked};
use crate::mac_ledger::release::{DOWNLOAD_ROWS, ReleaseKind, ReleaseRecord};

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
        signing: "AD_HOC".into(),
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
        .filter(|(_, class, _)| matches!(class, Class::Gui | Class::Translocation))
        .map(|(row, _, _)| (*row).to_owned())
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
            kind: if matches!(*package, "stickymd-mac" | "stickymd-smoke") {
                "bin".into()
            } else {
                "lib".into()
            },
            name: if matches!(*package, "stickymd-mac" | "stickymd-smoke") {
                (*package).to_owned()
            } else {
                package.replace('-', "_")
            },
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
        targets: Read::Present(&receipt().expected),
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

/// The rows that hold only with a USER record (receipt, waiver, download path) or, for R03, a
/// producer that runs on the candidate ZIP (M5 report).
fn awaiting_decision(item: &str) -> bool {
    item == "download-path"
        || ROWS.iter().any(|(row, class, _)| {
            format!("P16-{row}") == item
                && matches!(
                    class,
                    Class::Manual | Class::Resources | Class::Translocation | Class::IgnoredSource
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
        targets: Read::Present(&receipt().expected),
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
        targets: Read::Present(&receipt().expected),
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
        targets: Read::Present(&receipt.expected),
    });
    assert_line(&lines, "P16-E01", Verdict::NotTested, "line 3");
    assert_line(&lines, "P16-S02", Verdict::Pass, "");
}

fn release(kind: ReleaseKind, row: &str, candidate: &str, batch: &str) -> Record {
    Record::Release(ReleaseRecord {
        kind,
        row: row.to_owned(),
        candidate: candidate.to_owned(),
        environment: "USER desktop".into(),
        quote: format!("{row} 看过了"),
        batch: Some(batch.to_owned()),
        recorded_unix: 3,
    })
}

fn download(row: &str, candidate: &str) -> Record {
    release(
        ReleaseKind::Download {
            signing: "AD_HOC".into(),
            system: "ProductName: macOS ProductVersion: 27.2".into(),
        },
        row,
        candidate,
        "rc1",
    )
}

fn withdraw(row: &str, candidate: &str) -> Record {
    Record::Revocation {
        row: row.to_owned(),
        revoked: Revoked::Candidate(candidate.to_owned()),
        quote: "撤回".into(),
        recorded_unix: 4,
    }
}

/// The ledger with every USER record a release needs: receipts for the Manual rows, waivers for
/// the resource rows, and the five download paths, all for this candidate.
fn released() -> Vec<Record> {
    let digest = candidate().receipt_sha256;
    let mut records = ledger();
    for (row, class, _) in ROWS {
        match class {
            Class::Manual => records.push(release(ReleaseKind::Receipt, row, &digest, "rc1")),
            Class::Resources => records.push(release(ReleaseKind::Waiver, row, &digest, "rc1")),
            _ => {}
        }
    }
    records.extend(DOWNLOAD_ROWS.map(|row| download(row, &digest)));
    records
}

#[test]
fn with_the_user_records_every_row_but_r03_holds() {
    let candidate = candidate();
    let lines = run(Read::Present(&candidate), &released(), &receipt());
    for line in &lines {
        let expected = match line.item.as_str() {
            "P16-R03" | "P16-S06" => Verdict::NotTested,
            "P16-W15" | "P16-W16" | "P16-V05" => Verdict::Excluded,
            "P16-R01" | "P16-R02" | "P16-V03" => Verdict::Waived,
            _ => Verdict::Pass,
        };
        assert_eq!(line.verdict, expected, "{line:?}");
    }
    assert_line(&lines, "P16-W01", Verdict::Pass, "W01 看过了");
    assert!(
        !ready(&lines),
        "R03 still needs a run on the candidate ZIP, S06 a decision"
    );
    let held: Vec<Line> = lines
        .into_iter()
        .filter(|line| line.item != "P16-R03" && line.item != "P16-S06")
        .collect();
    assert!(ready(&held));
}

#[test]
fn a_record_for_another_candidate_does_not_count() {
    let candidate = candidate();
    let other = hex('9');
    let mut records = ledger();
    records.push(release(ReleaseKind::Receipt, "W01", &other, "rc1"));
    records.push(release(ReleaseKind::Waiver, "R01", &other, "rc1"));
    records.extend(DOWNLOAD_ROWS.map(|row| download(row, &other)));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-W01",
        Verdict::NotTested,
        "no USER release receipt",
    );
    assert_line(&lines, "P16-R01", Verdict::NotTested, "mac-resources");
    assert_line(&lines, "download-path", Verdict::NotTested, "P18-D01");
}

#[test]
fn a_revocation_withdraws_the_release_records_before_it_only() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let mut records = released();
    records.push(withdraw("W05", &digest));
    records.push(withdraw("R02", &digest));
    records.push(withdraw("P18-D04", &digest));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-W05",
        Verdict::NotTested,
        "no USER release receipt",
    );
    assert_line(&lines, "P16-R02", Verdict::NotTested, "mac-resources");
    assert_line(&lines, "download-path", Verdict::NotTested, "P18-D04");
    // The USER confirms again after the withdrawal: the new record stands.
    records.push(release(ReleaseKind::Receipt, "W05", &digest, "rc1"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W05", Verdict::Pass, "W05 看过了");
}

#[test]
fn a_waiver_never_stands_for_a_fail_or_an_unwaivable_row() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let mut records = ledger();
    records.push(record("W03", "FAIL", '1', &candidate.built, '8'));
    records.retain(|r| !matches!(r, Record::Automated { row, .. } if row == "W02"));
    for row in ["W02", "W03", "R03", "S01"] {
        records.push(release(ReleaseKind::Waiver, row, &digest, "rc1"));
    }
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W03", Verdict::Fail, "a FAIL of P16-W03");
    assert_line(&lines, "P16-W02", Verdict::Waived, "no PASS of P16-W02");
    assert_line(&lines, "P16-R03", Verdict::NotTested, "candidate ZIP");
    let mut failing = receipt();
    failing.exit_code = Some(101);
    let lines = run(Read::Present(&candidate), &records, &failing);
    assert_line(&lines, "P16-S01", Verdict::Fail, "exited");
}

#[test]
fn the_download_path_needs_all_five_rows() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let mut records = ledger();
    records.extend(
        DOWNLOAD_ROWS
            .iter()
            .filter(|row| **row != "P18-D05")
            .map(|row| download(row, &digest)),
    );
    let lines = run(Read::Present(&candidate), &records, &receipt());
    let line = verdict(&lines, "download-path");
    assert_eq!(line.verdict, Verdict::NotTested);
    assert!(line.detail.ends_with("P18-D05"), "{line:?}");
    records.push(download("P18-D05", &digest));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "download-path", Verdict::Pass, "P18-D01-D05");
}

#[test]
fn ime_records_count_only_in_the_batch_being_judged() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let mut records = ledger();
    records.push(release(ReleaseKind::Receipt, "I01", &digest, "rc0"));
    records.push(release(ReleaseKind::Waiver, "I02", &digest, "rc0"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-I01",
        Verdict::NotTested,
        "no USER release receipt",
    );
    assert_line(
        &lines,
        "P16-I02",
        Verdict::NotTested,
        "no USER release receipt",
    );
    records.push(release(ReleaseKind::Receipt, "I01", &digest, "rc1"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-I01", Verdict::Pass, "I01 看过了");
}

#[test]
fn only_rows_that_depend_on_the_app_running_are_waivable() {
    use super::rows::waivable;
    for row in ["W02", "E01", "W01", "I03", "V04", "R01", "V03"] {
        assert!(waivable(row), "{row}");
    }
    for row in ["R03", "S01", "B01", "P01", "W15", "V05", "P18-D01", "X99"] {
        assert!(!waivable(row), "{row}");
    }
}

/// Review A3: a missing W12 parent must not hide a recorded logout FAIL, or a waiver could cover it.
#[test]
fn a_logout_fail_is_not_hidden_behind_a_missing_w12() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let mut records = ledger();
    records.retain(|r| !matches!(r, Record::Automated { row, .. } if row == "W12" || row == "W12-logout-fault"));
    records.push(record(
        "W12-logout-fault",
        "FAIL",
        '1',
        &candidate.built,
        '8',
    ));
    records.push(release(ReleaseKind::Waiver, "W12", &digest, "rc1"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W12", Verdict::Fail, "W12-logout-fault");
}

/// Review A4: a program FAIL of a Manual row on the candidate's bytes beats a receipt and a waiver.
#[test]
fn an_observed_failure_of_a_manual_row_beats_its_receipt() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let mut records = released();
    records.push(record("W01", "FAIL", '1', &candidate.built, '8'));
    records.push(release(ReleaseKind::Waiver, "W01", &digest, "rc1"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W01", Verdict::Fail, "a FAIL of P16-W01");
}

/// Review A6: groups are driver, image and system; the optional product inputs do not split them.
#[test]
fn a_record_without_inputs_shares_its_group() {
    let candidate = candidate();
    let mut records = ledger();
    let mut failed = record("W03", "FAIL", '1', &candidate.built, '8');
    if let Record::Automated { identity, .. } = &mut failed {
        identity.inputs = None;
    }
    records.push(failed);
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W03", Verdict::Fail, "a FAIL of P16-W03");
}

/// Review A8: each download record must name the candidate's signing, and the five paths one
/// macOS major version.
#[test]
fn download_records_must_match_the_signing_and_one_major_version() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let with = |row: &str, signing: &str, version: &str| {
        release(
            ReleaseKind::Download {
                signing: signing.into(),
                system: format!("ProductName: macOS ProductVersion: {version}"),
            },
            row,
            &digest,
            "rc1",
        )
    };
    let mut records = ledger();
    records.extend(DOWNLOAD_ROWS.map(|row| with(row, "AD_HOC", "27.2")));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "download-path", Verdict::Pass, "macOS 27");
    records.push(with("P18-D02", "DEVELOPER_ID", "27.2"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "download-path", Verdict::NotTested, "DEVELOPER_ID");
    records.push(with("P18-D02", "AD_HOC", "28.0"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "download-path",
        Verdict::NotTested,
        "different macOS major",
    );
    records.push(with("P18-D02", "AD_HOC", "unknown"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "download-path",
        Verdict::NotTested,
        "no macOS version",
    );
}

/// Review A1: S06's test is `#[ignore]`, so a green default run does not prove it.
#[test]
fn s06_is_not_proven_by_the_default_run() {
    let candidate = candidate();
    let lines = run(Read::Present(&candidate), &released(), &receipt());
    assert_line(&lines, "source-tests", Verdict::Pass, "");
    assert_line(&lines, "P16-S06", Verdict::NotTested, "#[ignore]");
}

/// Review A7: without R's own target list the receipt proves nothing.
#[test]
fn without_r_s_target_list_the_source_rows_stay_open() {
    let candidate = candidate();
    let receipt = receipt();
    let lines = judge(&Inputs {
        release: R,
        batch: Some("rc1"),
        candidate: Read::Present(&candidate),
        ledger: Read::Present(&ledger()),
        source_tests: Read::Present(&receipt),
        targets: Read::Invalid("cannot list R's test targets: cargo metadata failed".into()),
    });
    assert_line(&lines, "source-tests", Verdict::NotTested, "cargo metadata");
    assert_line(&lines, "P16-S01", Verdict::NotTested, "cargo metadata");
}
