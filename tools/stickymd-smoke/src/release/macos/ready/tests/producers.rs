//! The gate on the two producers' evidence (plan 11 #macos-readiness-check): R03 counts only a
//! PASS that opened the candidate's ZIP with "nothing written" read through the write watch
//! (review #24 B9, round 3 B1); the resource rows only records on the candidate's bytes; V03 has
//! no producer. Each sample changes one fact of the good inputs.

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
fn r03_counts_a_watched_pass_on_the_candidate_zip_and_a_fail_beside_it_counts_first() {
    let candidate = candidate();
    let lines = run(Read::Present(&candidate), &ledger(), &receipt());
    assert_line(
        &lines,
        "P16-R03",
        Verdict::Pass,
        "ledger PASS on the candidate ZIP dddd",
    );
    assert_line(&lines, "P16-R03", Verdict::Pass, "under the write watch");
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
    // A clean run on the candidate ZIP by R's driver in another VM image, a group of its own,
    // does not hide that FAIL (review round 3, S1).
    records.push(record_on_image(
        "R03",
        "PASS",
        &format!("sha256:{}", hex('c')),
        &candidate.built,
        '7',
    ));
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(&lines, "P16-R03", Verdict::Fail, "a FAIL of P16-R03");
}

/// Review round 3, BLOCKER 1: an R03 PASS already in a ledger, written as a line the way an
/// older importer wrote it (the candidate ZIP but no write watch: a driver that compared
/// snapshots only), read back through the ledger reader, does not make the gate ready; the same
/// line with the watch the current importer records does.
#[test]
fn an_r03_pass_already_in_a_ledger_does_not_pass_the_gate() {
    let candidate = candidate();
    let text: String = released().iter().map(|r| r.line() + "\n").collect();
    let r03 = text
        .lines()
        .find(|line| line.contains("\"row\":\"R03\""))
        .unwrap();
    assert!(r03.contains("\"verdict\":\"PASS\""), "{r03}");
    assert!(
        r03.contains("\"zip_sha256\":\"dddd") && r03.contains(",\"write_watch\":true"),
        "{r03}"
    );
    let watched = crate::mac_ledger::record::read_all(&text).unwrap();
    let lines = run(Read::Present(&candidate), &watched, &receipt());
    assert_line(&lines, "P16-R03", Verdict::Pass, "under the write watch");
    let older = text.replace(",\"write_watch\":true", "");
    let records = crate::mac_ledger::record::read_all(&older).unwrap();
    let lines = run(Read::Present(&candidate), &records, &receipt());
    assert_line(
        &lines,
        "P16-R03",
        Verdict::NotTested,
        "a PASS recorded without the watch",
    );
    assert!(!ready(&lines));
    // The marker is only ever written as true, and only beside the ZIP R03 opened.
    for bad in [
        text.replace(",\"write_watch\":true", ",\"write_watch\":false"),
        older.replace(
            "\"row\":\"W02\",\"verdict\":\"PASS\"",
            "\"row\":\"W02\",\"verdict\":\"PASS\",\"write_watch\":true",
        ),
    ] {
        assert_ne!(bad, text);
        assert!(crate::mac_ledger::record::read_all(&bad).is_err());
    }
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

/// The layers that keep a resource row's FAIL from a USER waiver: the groups' verdict FAILs the
/// row (no identity holds a clean run), and a line that is not NOT TESTED is never waived. The
/// waiver's own failure check cannot be reached for these rows, since a FAIL on the candidate's
/// bytes always makes the groups' verdict FAIL or a clean PASS, so the detail names the groups'
/// verdict (pre-freeze, producers item 5 (third review)).
#[test]
fn a_resource_row_fail_under_a_waiver_stays_the_groups_fail() {
    let candidate = candidate();
    for row in ["R01", "R02"] {
        let mut records = ledger();
        without(&mut records, row);
        records.push(record(row, "FAIL", '1', &candidate.built, '8'));
        records.push(release(
            ReleaseKind::Waiver,
            row,
            &candidate.receipt_sha256,
            "rc1",
        ));
        let lines = run(Read::Present(&candidate), &records, &receipt());
        let line = verdict(&lines, &format!("P16-{row}"));
        assert_eq!(line.verdict, Verdict::Fail, "{line:?}");
        assert!(
            line.detail.contains(&format!(
                "a FAIL of P16-{row} is recorded on the candidate's bytes, and no identity holds a clean run of every row it needs"
            )),
            "{line:?}"
        );
        assert!(!line.detail.contains("USER waiver"), "{line:?}");
    }
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
