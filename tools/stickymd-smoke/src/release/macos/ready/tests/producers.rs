//! The gate on the two producers' evidence (plan 11 #macos-readiness-check): R03 counts only a
//! PASS that opened the candidate's ZIP, and the resource rows only records on the candidate's
//! bytes; V03 has no producer. Each sample changes one fact of the good inputs.

use super::*;

fn without(records: &mut Vec<Record>, wanted: &str) {
    records.retain(|r| !matches!(r, Record::Automated { row, .. } if row == wanted));
}

/// R03 through a ZIP the probe zipped from the `.app`: no ZIP digest.
fn repacked(verdict: &str, driver: char, evidence: char) -> Record {
    let mut record = record("R03", verdict, driver, &candidate().built, evidence);
    if let Record::Automated { zip, .. } = &mut record {
        *zip = None;
    }
    record
}

#[test]
fn r03_counts_a_clean_pass_on_the_candidate_zip_and_any_spoiler_beside_it() {
    let candidate = candidate();
    let lines = run(Read::Present(&candidate), &ledger(), &receipt());
    assert_line(&lines, "P16-R03", Verdict::Pass, "candidate ZIP dddd");
    // A NOT_TESTED through another ZIP in the same identity spoils the group: the ZIP does not
    // split groups.
    let mut records = ledger();
    records.push(repacked("NOT_TESTED", '1', '8'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-R03",
        Verdict::NotTested,
        "NOT_TESTED or revoked",
    );
    // A FAIL on these bytes through a repacked ZIP, with no clean group, is a FAIL.
    let mut records = ledger();
    without(&mut records, "R03");
    records.push(repacked("FAIL", '1', '8'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-R03", Verdict::Fail, "a FAIL of P16-R03");
    // A clean run on the candidate ZIP under another identity still holds.
    records.push(record("R03", "PASS", '2', &candidate.built, '7'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-R03", Verdict::Pass, "run-7");
}

#[test]
fn r03_on_the_candidate_zip_but_another_bundle_is_not_a_pass() {
    let candidate = candidate();
    let other = Built {
        executable: candidate.built.executable.clone(),
        bundle: hex('f'),
    };
    let mut records = ledger();
    without(&mut records, "R03");
    records.push(record("R03", "PASS", '1', &other, '9'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-R03", Verdict::NotTested, "another bundle");
}

#[test]
fn resource_rows_hold_only_on_the_candidates_bytes() {
    let candidate = candidate();
    let lines = run(Read::Present(&candidate), &ledger(), &receipt());
    for item in ["P16-R01", "P16-R02"] {
        assert_line(
            &lines,
            item,
            Verdict::Pass,
            "ledger PASS on the candidate's bytes",
        );
    }
    // Contract sample: the candidate's executable in another bundle.
    let same_executable = Built {
        executable: candidate.built.executable.clone(),
        bundle: hex('f'),
    };
    // Other bytes entirely.
    let other = Built {
        executable: hex('0'),
        bundle: hex('f'),
    };
    for (built, words) in [
        (&same_executable, "another bundle"),
        (&other, "no PASS of P16-R01"),
    ] {
        let mut records = ledger();
        without(&mut records, "R01");
        records.push(record("R01", "PASS", '1', built, '9'));
        let lines = run(Read::Present(&candidate), &records, &receipt());
        assert_line(&lines, "P16-R01", Verdict::NotTested, words);
        // NOT TESTED, so the USER may waive it for this candidate.
        records.push(release(
            ReleaseKind::Waiver,
            "R01",
            &candidate.receipt_sha256,
            "rc1",
        ));
        let lines = run(Read::Present(&candidate), &records, &receipt());
        assert_line(&lines, "P16-R01", Verdict::Waived, words);
    }
}

#[test]
fn a_resource_fail_on_the_candidates_bytes_cannot_be_waived() {
    let candidate = candidate();
    let mut records = ledger();
    without(&mut records, "R02");
    records.push(record("R02", "FAIL", '1', &candidate.built, '8'));
    records.push(release(
        ReleaseKind::Waiver,
        "R02",
        &candidate.receipt_sha256,
        "rc1",
    ));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-R02", Verdict::Fail, "a FAIL of P16-R02");
    // A revoked PASS beside it in the same identity spoils that identity too.
    let mut records = ledger();
    records.push(Record::Revocation {
        row: "R01".into(),
        revoked: Revoked::Evidence(hex('9')),
        quote: "读数不对".into(),
        recorded_unix: 3,
    });
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-R01", Verdict::NotTested, "revoked");
}

#[test]
fn v03_has_no_producer_and_holds_only_with_a_waiver() {
    let candidate = candidate();
    // Even a PASS of V03 in the ledger is no release rule for it.
    let mut records = ledger();
    records.push(record("V03", "PASS", '1', &candidate.built, '9'));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-V03", Verdict::NotTested, "no producer");
    records.push(release(
        ReleaseKind::Waiver,
        "V03",
        &candidate.receipt_sha256,
        "rc1",
    ));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-V03", Verdict::Waived, "no producer");
}
