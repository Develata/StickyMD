//! The gate against bad samples: one good set of inputs, and samples that each change one fact.
//! The two samples the contract names (plan 11 #macos-release-gate) are tests of their own; the
//! producers' samples (R03's ZIP, the resource rows) are in `producers`.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

mod producers;

use super::judge::{Candidate, Inputs, Line, Read, Verdict, judge, ready};
use super::rows::{Class, ROWS};
use super::source_tests::{self, Receipt, Target};
use crate::mac_ledger::record::{Built, Identity, OpenedZip, Record, Revoked};
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
        image: "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee".into(),
        system: "ProductName: macOS ProductVersion: 27.2".into(),
    }
}

/// A record under another VM image than `record`'s, same driver.
fn record_on_image(row: &str, verdict: &str, image: &str, built: &Built, evidence: char) -> Record {
    let mut found = record(row, verdict, '1', built, evidence);
    if let Record::Automated { identity, .. } = &mut found {
        identity.image = image.to_owned();
    }
    found
}

fn record(row: &str, verdict: &str, driver: char, built: &Built, evidence: char) -> Record {
    // R03 opened the candidate's ZIP (`mac-gui --zip`), "nothing written" read through the
    // write watch.
    let zip = (row == "R03").then(|| OpenedZip {
        sha256: candidate().zip_sha256,
        write_watch: true,
    });
    Record::Automated {
        zip,
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

/// A PASS on the candidate's bytes for every row `mac-gui` runs, W12's logout cases, R03 on the
/// candidate's ZIP, and the `mac-resources` rows.
fn ledger() -> Vec<Record> {
    let built = candidate().built;
    let mut rows: Vec<String> = ROWS
        .iter()
        .filter(|(_, class, _)| {
            matches!(class, Class::Gui | Class::Translocation | Class::Resources)
        })
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
    source_tests::fixture(R, expected)
}

fn run(candidate: Read<&Candidate>, records: &[Record], tests: &Receipt) -> Vec<Line> {
    judge(&Inputs {
        release: R,
        batch: Some("rc1"),
        macos_major: 27,
        driver: &hex('1'),
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

/// The rows that hold only with a USER record (receipt, waiver, download path); V03 has no
/// producer and holds only with a waiver.
fn awaiting_decision(item: &str) -> bool {
    item == "download-path"
        || ROWS.iter().any(|(row, class, _)| {
            format!("P16-{row}") == item && matches!(class, Class::Manual | Class::Comparison)
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
/// when the bundle inside is byte-identical: zipped by the probe it names no ZIP, and given as
/// `--zip` it names another digest.
#[test]
fn an_r03_pass_without_the_candidate_zip_is_not_evidence() {
    let candidate = candidate();
    for repacked in [None, Some(hex('f'))] {
        let records: Vec<Record> = ledger()
            .into_iter()
            .map(|mut record| {
                if let Record::Automated { row, zip, .. } = &mut record
                    && row == "R03"
                {
                    *zip = repacked.clone().map(|sha256| OpenedZip {
                        sha256,
                        write_watch: true,
                    });
                }
                record
            })
            .collect();
        let lines = run(Read::Present(&candidate), &records, &receipt());
        assert_line(
            &lines,
            "P16-R03",
            Verdict::NotTested,
            "no clean group of P16-R03 on the candidate's bytes holds a PASS that opened the candidate ZIP dddd",
        );
    }
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
    // A clean run of R's driver in another VM image on the same bytes holds, as in the ledger's
    // own rule.
    records.push(record_on_image(
        "W03",
        "PASS",
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        &built,
        '7',
    ));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W03", Verdict::Pass, "run-7");
}

/// Review round 3, B5: a record whose VM image names no digest (an old line's tag) cannot be
/// placed in or apart from a group, so beside it no group is shown clean; its FAIL still counts.
#[test]
fn a_record_of_unknown_image_blocks_the_row_and_its_fail_still_counts() {
    let candidate = candidate();
    let built = candidate.built.clone();
    let mut records = ledger();
    records.push(record_on_image(
        "W03",
        "PASS",
        "mac-test:latest",
        &built,
        '8',
    ));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-W03",
        Verdict::NotTested,
        "its identity is unknown",
    );
    records.pop();
    records.push(record_on_image(
        "W03",
        "FAIL",
        "mac-test:latest",
        &built,
        '8',
    ));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W03", Verdict::Fail, "a FAIL of P16-W03");
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
    records.push(record_on_image(
        "W12-logout-dialog",
        "PASS",
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        &built,
        '6',
    ));
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
    assert_line(&lines, "P16-R03", Verdict::Pass, "candidate ZIP dddd");
    // Without a batch nothing counts.
    let lines = judge(&Inputs {
        release: R,
        batch: None,
        macos_major: 27,
        driver: &hex('1'),
        candidate: Read::Present(&candidate),
        ledger: Read::Present(&records),
        source_tests: Read::Present(&receipt()),
        targets: Read::Present(&receipt().expected),
    });
    assert_line(
        &lines,
        "P16-R03",
        Verdict::NotTested,
        "counts only the batch named with --batch",
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
        macos_major: 27,
        driver: &hex('1'),
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
        macos_major: 27,
        driver: &hex('1'),
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
            outcome: crate::mac_ledger::release::outcomes(row).map(|o| o[0].to_owned()),
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

/// The ledger with every USER record a release needs: receipts for the Manual rows, the waiver
/// for V03, and the five download paths, all for this candidate.
fn released() -> Vec<Record> {
    let digest = candidate().receipt_sha256;
    let mut records = ledger();
    for (row, class, _) in ROWS {
        match class {
            Class::Manual => records.push(release(ReleaseKind::Receipt, row, &digest, "rc1")),
            Class::Comparison => records.push(release(ReleaseKind::Waiver, row, &digest, "rc1")),
            _ => {}
        }
    }
    records.extend(DOWNLOAD_ROWS.map(|row| download(row, &digest)));
    records
}

#[test]
fn with_the_user_records_every_row_holds() {
    let candidate = candidate();
    let lines = run(Read::Present(&candidate), &released(), &receipt());
    for line in &lines {
        let expected = match line.item.as_str() {
            "P16-W15" | "P16-W16" | "P16-V05" => Verdict::Excluded,
            "P16-V03" => Verdict::Waived,
            _ => Verdict::Pass,
        };
        assert_eq!(line.verdict, expected, "{line:?}");
    }
    assert_line(&lines, "P16-W01", Verdict::Pass, "W01 看过了");
    assert_line(&lines, "P16-R03", Verdict::Pass, "under the write watch");
    assert_line(&lines, "P16-S06", Verdict::Pass, "explicit run");
    assert!(ready(&lines));
    // Without R03's run on the candidate ZIP the gate does not hold.
    let mut records = released();
    records.retain(|r| !matches!(r, Record::Automated { row, .. } if row == "R03"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-R03", Verdict::NotTested, "no PASS of P16-R03");
    assert!(!ready(&lines));
}

#[test]
fn a_record_for_another_candidate_does_not_count() {
    let candidate = candidate();
    let other = hex('9');
    let mut records = ledger();
    records.push(release(ReleaseKind::Receipt, "W01", &other, "rc1"));
    records.push(release(ReleaseKind::Waiver, "V03", &other, "rc1"));
    records.extend(DOWNLOAD_ROWS.map(|row| download(row, &other)));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-W01",
        Verdict::NotTested,
        "no USER release receipt",
    );
    assert_line(&lines, "P16-V03", Verdict::NotTested, "no producer");
    assert_line(&lines, "download-path", Verdict::NotTested, "P18-D01");
}

#[test]
fn a_revocation_withdraws_the_release_records_before_it_only() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let mut records = released();
    records.push(withdraw("W05", &digest));
    records.push(withdraw("V03", &digest));
    records.push(withdraw("P18-D04", &digest));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-W05",
        Verdict::NotTested,
        "no USER release receipt",
    );
    assert_line(&lines, "P16-V03", Verdict::NotTested, "no producer");
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
    records.retain(|r| !matches!(r, Record::Automated { row, .. } if row == "W02" || row == "R03"));
    for row in ["W02", "W03", "R03", "S01"] {
        records.push(release(ReleaseKind::Waiver, row, &digest, "rc1"));
    }
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W03", Verdict::Fail, "a FAIL of P16-W03");
    assert_line(&lines, "P16-W02", Verdict::Waived, "no PASS of P16-W02");
    assert_line(&lines, "P16-R03", Verdict::NotTested, "no PASS of P16-R03");
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
                outcome: crate::mac_ledger::release::outcomes(row).map(|o| o[0].to_owned()),
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
        "ran on macOS 28",
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

/// Review A1 and decision (a): S06's test is `#[ignore]`, so the default run does not prove it;
/// the receipt's explicit run does, and without it S06 is not tested.
#[test]
fn s06_holds_on_its_explicit_run_and_not_on_the_default_run() {
    let candidate = candidate();
    let lines = run(Read::Present(&candidate), &released(), &receipt());
    assert_line(&lines, "P16-S06", Verdict::Pass, "explicit run");
    let mut without = receipt();
    without.explicit.pop();
    let lines = run(Read::Present(&candidate), &released(), &without);
    assert_line(
        &lines,
        "source-tests",
        Verdict::NotTested,
        "explicit runs are",
    );
    assert_line(&lines, "P16-S06", Verdict::NotTested, "explicit runs are");
}

/// Review A7: without R's own target list the receipt proves nothing.
#[test]
fn without_r_s_target_list_the_source_rows_stay_open() {
    let candidate = candidate();
    let receipt = receipt();
    let lines = judge(&Inputs {
        release: R,
        batch: Some("rc1"),
        macos_major: 27,
        driver: &hex('1'),
        candidate: Read::Present(&candidate),
        ledger: Read::Present(&ledger()),
        source_tests: Read::Present(&receipt),
        targets: Read::Invalid("cannot list R's test targets: cargo metadata failed".into()),
    });
    assert_line(&lines, "source-tests", Verdict::NotTested, "cargo metadata");
    assert_line(&lines, "P16-S01", Verdict::NotTested, "cargo metadata");
}

/// Every row of the table, given a FAIL under one identity, a clean PASS under another, a waiver
/// and a receipt for this candidate: no row is ever WAIVED over the FAIL, and no Manual row
/// PASSes (plan 11 #macos-readiness-check: a waiver stands for evidence not taken, a program FAIL
/// beats a receipt). A row that can PASS here does so only through the clean identity.
#[test]
fn over_every_row_a_recorded_fail_is_never_waived_and_never_hidden_from_a_manual_row() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    for (row, class, _) in ROWS {
        let mut records = ledger();
        let rows: Vec<String> = if *row == "W12" {
            std::iter::once("W12".to_owned())
                .chain(crate::mac_gui::logout::rows())
                .collect()
        } else {
            vec![(*row).to_owned()]
        };
        for member in &rows {
            records.retain(|r| !matches!(r, Record::Automated { row: rr, .. } if rr == member));
            records.push(record(member, "FAIL", '1', &candidate.built, '8'));
            records.push(record_on_image(
                member,
                "PASS",
                "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
                &candidate.built,
                '7',
            ));
        }
        if super::rows::waivable(row) {
            records.push(release(ReleaseKind::Waiver, row, &digest, "rc1"));
        }
        if super::rows::receipt_row(row) {
            records.push(release(ReleaseKind::Receipt, row, &digest, "rc1"));
        }
        let lines = run(Read::Present(&candidate), &records, &receipt());
        let line = verdict(&lines, &format!("P16-{row}"));
        assert_ne!(line.verdict, Verdict::Waived, "{line:?}");
        match class {
            Class::Manual => assert_eq!(line.verdict, Verdict::Fail, "{line:?}"),
            // A resource row is judged from the ledger as a mac-gui row is (#24).
            Class::Gui | Class::Resources => assert_eq!(line.verdict, Verdict::Pass, "{line:?}"),
            _ => assert_ne!(line.verdict, Verdict::Waived, "{line:?}"),
        }
    }
}

/// Review round 2, finding 6: a Manual row's FAIL under one driver is not hidden by another
/// driver's PASS.
#[test]
fn a_manual_row_fail_under_one_driver_is_not_hidden_by_another_driver_s_pass() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let mut records = released();
    records.push(release(ReleaseKind::Receipt, "I03", &digest, "rc1"));
    records.push(record("I03", "FAIL", '1', &candidate.built, '8'));
    records.push(record("I03", "PASS", '2', &candidate.built, '7'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-I03", Verdict::Fail, "a FAIL of P16-I03");
}

/// Review round 2, finding 7: W12 fails when every complete identity has a failed logout case,
/// even though each case alone has a clean identity somewhere; a waiver cannot cover it.
#[test]
fn w12_fails_when_every_identity_has_a_failed_logout_case() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let mut records: Vec<Record> = ledger()
        .into_iter()
        .filter(|r| !matches!(r, Record::Automated { row, .. } if row.starts_with("W12")))
        .collect();
    for (driver, evidence, failing) in [
        ('1', '5', "W12-logout-fault"),
        ('2', '6', "W12-logout-dialog"),
    ] {
        records.push(record("W12", "PASS", driver, &candidate.built, evidence));
        for case in crate::mac_gui::logout::rows() {
            let verdict = if case == failing { "FAIL" } else { "PASS" };
            records.push(record(&case, verdict, driver, &candidate.built, evidence));
        }
    }
    records.push(release(ReleaseKind::Waiver, "W12", &digest, "rc1"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-W12",
        Verdict::Fail,
        "no identity holds a clean run",
    );
}

/// Review round 2, finding 8: a recorded FAIL of a resource row is not waived.
#[test]
fn a_resource_row_fail_is_not_waived() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let mut records = ledger();
    records.push(record("V03", "FAIL", '1', &candidate.built, '8'));
    records.push(release(ReleaseKind::Waiver, "V03", &digest, "rc1"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-V03", Verdict::Fail, "a FAIL of P16-V03");
}

/// The resource rows' own guard, apart from the waiver's: a recorded FAIL alone is a FAIL.
#[test]
fn a_resource_row_fail_alone_is_a_fail() {
    let candidate = candidate();
    let mut records = ledger();
    records.push(record("R01", "FAIL", '1', &candidate.built, '8'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-R01", Verdict::Fail, "a FAIL of P16-R01");
}

/// Review round 2, finding 4, and decision (b): a download path whose D04 or D05 result is not
/// recorded has no path to PASS, and all five rows must have run on the major version checked.
#[test]
fn the_download_path_needs_its_outcomes_and_the_checked_major_version() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let mut records = ledger();
    records.extend(DOWNLOAD_ROWS.map(|row| download(row, &digest)));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "download-path", Verdict::Pass, "macOS 27");
    // The same five rows without the D05 outcome (a line written before decision (b)).
    let mut silent = records.clone();
    if let Some(Record::Release(release)) = silent
        .iter_mut()
        .rev()
        .find(|r| matches!(r, Record::Release(x) if x.row == "P18-D05"))
        && let ReleaseKind::Download { outcome, .. } = &mut release.kind
    {
        *outcome = None;
    }
    let lines = run(Read::Present(&candidate), &silent, &receipt());
    assert_line(
        &lines,
        "download-path",
        Verdict::NotTested,
        "P18-D05 records no valid outcome",
    );
    // Another major version than the one the gate is told to check.
    let lines = judge(&Inputs {
        release: R,
        batch: Some("rc1"),
        macos_major: 28,
        driver: &hex('1'),
        candidate: Read::Present(&candidate),
        ledger: Read::Present(&records),
        source_tests: Read::Present(&receipt()),
        targets: Read::Present(&receipt().expected),
    });
    assert_line(
        &lines,
        "download-path",
        Verdict::NotTested,
        "checked on macOS 28",
    );
}

/// Review round 2, SHOULD_FIX 8: each byte identity alone decides; another executable in the
/// candidate's bundle digest does not count either.
#[test]
fn another_executable_alone_does_not_count() {
    let candidate = candidate();
    let other = Built {
        executable: hex('0'),
        bundle: candidate.built.bundle.clone(),
    };
    let mut records = ledger();
    records.retain(|r| !matches!(r, Record::Automated { row, .. } if row == "K02"));
    records.push(record_on("K02", &other));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-K02", Verdict::NotTested, "no PASS of P16-K02");
}

/// USER 2026-11-24 (`--driver`): a clean run of an older driver does not count for a
/// `mac-gui` row; only R's driver's runs can hold a PASS.
#[test]
fn a_pass_of_another_driver_does_not_count() {
    let candidate = candidate();
    let mut records = ledger();
    records.retain(|r| !matches!(r, Record::Automated { row, .. } if row == "W02"));
    records.push(record("W02", "PASS", '2', &candidate.built, '7'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-W02",
        Verdict::NotTested,
        "only under another driver",
    );
}

/// USER 2026-11-24 (`--driver`): a FAIL counts whatever the driver, and keeps a waiver off.
#[test]
fn a_fail_of_another_driver_still_counts() {
    let candidate = candidate();
    let digest = candidate.receipt_sha256.clone();
    let mut records = ledger();
    records.retain(|r| !matches!(r, Record::Automated { row, .. } if row == "W02"));
    records.push(record("W02", "FAIL", '2', &candidate.built, '7'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W02", Verdict::Fail, "a FAIL of P16-W02");
    records.push(release(ReleaseKind::Waiver, "W02", &digest, "rc1"));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-W02", Verdict::Fail, "a FAIL of P16-W02");
}

/// Review round 3, S1: R03's FAIL in this batch stays a FAIL beside another group's clean run, in
/// either order of import.
#[test]
fn an_r03_fail_beside_a_clean_group_is_a_fail_in_either_order() {
    let candidate = candidate();
    let fail = record("R03", "FAIL", '1', &candidate.built, '8');
    let pass = record_on_image(
        "R03",
        "PASS",
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc",
        &candidate.built,
        '7',
    );
    for (first, second) in [(fail.clone(), pass.clone()), (pass, fail)] {
        let mut records: Vec<Record> = ledger()
            .into_iter()
            .filter(|r| !matches!(r, Record::Automated { row, .. } if row == "R03"))
            .collect();
        records.push(first);
        records.push(second);
        let lines = run(Read::Present(&candidate), &records, &receipt());
        assert_line(&lines, "P16-R03", Verdict::Fail, "a FAIL of P16-R03");
    }
}

/// Review round 3, S2: a failure the receipt records is a FAIL even when R's target list
/// cannot be read.
#[test]
fn a_recorded_failure_beats_a_missing_target_list() {
    let candidate = candidate();
    let mut failing = receipt();
    failing.exit_code = Some(101);
    let lines = judge(&Inputs {
        release: R,
        batch: Some("rc1"),
        macos_major: 27,
        driver: &hex('1'),
        candidate: Read::Present(&candidate),
        ledger: Read::Present(&ledger()),
        source_tests: Read::Present(&failing),
        targets: Read::Invalid("cannot list R's test targets".into()),
    });
    assert_line(&lines, "source-tests", Verdict::Fail, "exited");
}

/// Review round 3, S7: one complete ProductVersion, or no major version at all.
#[test]
fn the_major_version_needs_one_complete_product_version() {
    use super::receipts::major_version;
    assert_eq!(
        major_version("ProductName: macOS ProductVersion: 27.2.1"),
        Some(27)
    );
    assert_eq!(major_version("ProductVersion: 27"), Some(27));
    assert_eq!(major_version("ProductVersion: 27.bad"), None);
    assert_eq!(
        major_version("ProductVersion: 27.2 ProductVersion: 28.0"),
        None
    );
    assert_eq!(major_version("ProductVersion:"), None);
}
