//! Headless tests of the ledger records, the reuse rule and the parser; no file is written.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    decide::{Rows, check_system, matrix_rows, plan},
    options::{Command, parse},
    record::{Built, Identity, Record, Revoked, consistent_with, from_evidence, read_all},
};

fn hex(c: char) -> String {
    std::iter::repeat_n(c, 64).collect()
}
fn identity() -> Identity {
    Identity {
        inputs: Some(hex('1')),
        driver: hex('d'),
        image: "sha256:image".into(),
        system: "ProductName: macOS ProductVersion: 27.2 BuildVersion: 26B123".into(),
    }
}
fn built() -> Built {
    Built {
        executable: hex('a'),
        bundle: hex('b'),
    }
}
fn automated(row: &str, verdict: &str, identity: Identity) -> Record {
    run(row, verdict, identity, 'e')
}
/// A record from the evidence whose digest is `digest` repeated.
fn run(row: &str, verdict: &str, identity: Identity, digest: char) -> Record {
    Record::Automated {
        row: row.into(),
        verdict: verdict.into(),
        identity,
        built: built(),
        evidence: format!("/runs/{row}-{digest}.json"),
        evidence_sha256: hex(digest),
        run_unix: None,
        batch: None,
        recorded_unix: 1,
    }
}
fn in_batch(mut record: Record, name: &str) -> Record {
    match &mut record {
        Record::Automated { batch, .. } | Record::Manual { batch, .. } => {
            *batch = Some(name.into());
        }
        Record::Revocation { .. } => unreachable!("a revocation has no batch"),
    }
    record
}
fn manual(row: &str, inputs: Option<String>) -> Record {
    Record::Manual {
        row: row.into(),
        inputs,
        app_executable: hex('a'),
        environment: "USER desktop, recorded on macOS 27.2".into(),
        quote: "都没什么问题".into(),
        batch: None,
        recorded_unix: 2,
    }
}
fn revoke(row: &str, revoked: Revoked) -> Record {
    Record::Revocation {
        row: row.into(),
        revoked,
        quote: "看录像发现不对".into(),
        recorded_unix: 3,
    }
}
fn strings(rows: &[&str]) -> Vec<String> {
    rows.iter().map(|r| (*r).to_owned()).collect()
}

#[test]
fn only_a_pass_under_the_same_identity_is_reused() {
    let changed = |f: fn(&mut Identity)| {
        let mut i = identity();
        f(&mut i);
        i
    };
    let records = [
        automated("W02", "PASS", identity()),
        automated("W03", "FAIL", identity()),
        automated("W04", "NOT_TESTED", identity()),
        automated("W07", "PASS", changed(|i| i.driver = hex('f'))),
        automated("W08", "PASS", changed(|i| i.image = "sha256:other".into())),
        automated("W09", "PASS", changed(|i| i.inputs = Some(hex('2')))),
        automated("W11", "PASS", changed(|i| i.system = "macOS 27.3".into())),
    ];
    let rows = ["W02", "W03", "W04", "W07", "W08", "W09", "W11", "K02"];
    let result = plan(&records, &rows, &[], &identity(), &built(), None);
    assert_eq!(result.reused.len(), 1);
    assert_eq!(result.reused[0].row, "W02");
    assert_eq!(
        result.run,
        ["W03", "W04", "W07", "W08", "W09", "W11", "K02"]
    );
    assert!(result.rerun.is_empty(), "no PASS was set aside");
}

/// The anchor is the product inputs, not the bytes (plan 11 #phase-16-macos-host): a PASS for
/// other bytes built from the same inputs is reused and reported for audit; a record or an app
/// without inputs reuses nothing.
#[test]
fn the_same_inputs_reuse_a_pass_for_other_bytes_and_no_inputs_reuse_nothing() {
    let mut other_bytes = automated("W02", "PASS", identity());
    if let Record::Automated { built, .. } = &mut other_bytes {
        built.executable = hex('9');
    }
    let legacy = Identity {
        inputs: None,
        ..identity()
    };
    let records = [other_bytes, automated("W03", "PASS", legacy.clone())];
    let result = plan(&records, &["W02", "W03"], &[], &identity(), &built(), None);
    assert_eq!(result.reused.len(), 1);
    assert!(result.reused[0].other_bytes);
    assert_eq!(result.run, ["W03"]);
    let result = plan(
        &records,
        &["W02", "W03"],
        &strings(&["W06"]),
        &legacy,
        &built(),
        None,
    );
    assert!(result.reused.is_empty());
    assert_eq!(result.unconfirmed, ["W06"]);
}

/// A FAIL or NOT_TESTED after the latest PASS stops its reuse, with the reason; a PASS after the
/// failure is reused; a failure under another identity changes nothing.
#[test]
fn a_later_non_pass_stops_the_reuse_of_the_latest_pass() {
    for later in ["FAIL", "NOT_TESTED"] {
        let records = [
            run("W02", "PASS", identity(), 'e'),
            run("W02", later, identity(), 'f'),
        ];
        let result = plan(&records, &["W02"], &[], &identity(), &built(), None);
        assert!(result.reused.is_empty(), "{later}");
        assert_eq!(result.run, ["W02"]);
        assert_eq!(result.rerun.len(), 1);
    }
    let records = [
        run("W02", "FAIL", identity(), 'f'),
        run("W02", "PASS", identity(), 'e'),
    ];
    let result = plan(&records, &["W02"], &[], &identity(), &built(), None);
    assert_eq!(result.reused[0].evidence_sha256, hex('e'));
    let mut other = identity();
    other.driver = hex('f');
    let records = [
        run("W02", "PASS", identity(), 'e'),
        run("W02", "FAIL", other, 'f'),
    ];
    let result = plan(&records, &["W02"], &[], &identity(), &built(), None);
    assert_eq!(result.reused.len(), 1);
}

/// A revoked latest PASS is not reused and an older PASS is not fallen back to; a real rerun
/// with other evidence after the revocation is reused again.
#[test]
fn a_revocation_withdraws_its_pass_and_a_new_run_restores_reuse() {
    let records = [
        run("W02", "PASS", identity(), 'c'),
        run("W02", "PASS", identity(), 'e'),
        revoke("W02", Revoked::Evidence(hex('e'))),
    ];
    let result = plan(&records, &["W02"], &[], &identity(), &built(), None);
    assert!(result.reused.is_empty(), "no fallback to the older PASS");
    assert_eq!(result.rerun.len(), 1);
    let rerun = [
        run("W02", "PASS", identity(), 'e'),
        revoke("W02", Revoked::Evidence(hex('e'))),
        run("W02", "PASS", identity(), 'f'),
    ];
    let result = plan(&rerun, &["W02"], &[], &identity(), &built(), None);
    assert_eq!(result.reused[0].evidence_sha256, hex('f'));
    // A revocation of another row or digest changes nothing.
    let records = [
        run("W02", "PASS", identity(), 'e'),
        run("W03", "PASS", identity(), 'e'),
        revoke("W03", Revoked::Evidence(hex('e'))),
    ];
    let result = plan(&records, &["W02"], &[], &identity(), &built(), None);
    assert_eq!(result.reused.len(), 1);
}

/// The never-reused rows count only results of the batch being planned: none without a batch, a
/// retry within the batch, and nothing from another batch under the same identity.
#[test]
fn the_never_reused_rows_count_only_their_batch() {
    let records = [
        in_batch(run("R03", "PASS", identity(), 'e'), "b1"),
        in_batch(manual("I02", Some(hex('1'))), "b1"),
    ];
    let manual_rows = strings(&["I02"]);
    let none = plan(
        &records,
        &["R03"],
        &manual_rows,
        &identity(),
        &built(),
        None,
    );
    assert!(none.reused.is_empty() && none.confirmed.is_empty());
    assert_eq!(none.rerun.len(), 1, "the plan says why without a batch");
    let same = plan(
        &records,
        &["R03"],
        &manual_rows,
        &identity(),
        &built(),
        Some("b1"),
    );
    assert_eq!(same.reused.len(), 1);
    assert_eq!(same.confirmed.len(), 1);
    let next = plan(
        &records,
        &["R03"],
        &manual_rows,
        &identity(),
        &built(),
        Some("b2"),
    );
    assert!(next.reused.is_empty() && next.confirmed.is_empty());
    assert!(next.rerun.is_empty(), "nothing of b2 exists yet");
    // An unbatched record of a never-reused row counts for no batch.
    let unbatched = [run("I03", "PASS", identity(), 'e')];
    let result = plan(&unbatched, &["I03"], &[], &identity(), &built(), Some("b1"));
    assert!(result.reused.is_empty());
    // Other rows ignore the batch.
    let other = [run("W02", "PASS", identity(), 'e')];
    let result = plan(&other, &["W02"], &[], &identity(), &built(), Some("b9"));
    assert_eq!(result.reused.len(), 1);
}

/// A revocation withdraws the receipts recorded before it; a receipt recorded after it holds.
#[test]
fn a_revocation_withdraws_earlier_receipts_only() {
    let rows = strings(&["W06"]);
    let withdrawn = [
        manual("W06", Some(hex('1'))),
        revoke("W06", Revoked::Inputs(hex('1'))),
    ];
    let result = plan(&withdrawn, &[], &rows, &identity(), &built(), None);
    assert_eq!(result.unconfirmed, ["W06"]);
    let renewed = [
        manual("W06", Some(hex('1'))),
        revoke("W06", Revoked::Inputs(hex('1'))),
        manual("W06", Some(hex('1'))),
    ];
    let result = plan(&renewed, &[], &rows, &identity(), &built(), None);
    assert_eq!(result.confirmed.len(), 1);
}

#[test]
fn a_user_receipt_holds_only_for_its_inputs() {
    let records = [
        manual("W06", Some(hex('1'))),
        manual("W11", Some(hex('2'))),
        manual("W12", None),
    ];
    let result = plan(
        &records,
        &[],
        &strings(&["W06", "W11", "W12"]),
        &identity(),
        &built(),
        None,
    );
    assert_eq!(result.confirmed.len(), 1);
    assert_eq!(result.confirmed[0].0, "W06");
    assert_eq!(result.unconfirmed, ["W11", "W12"]);
}

#[test]
fn records_survive_a_round_trip_and_bad_lines_reject_the_ledger() {
    let mut quoted = manual("W06", Some(hex('1')));
    if let Record::Manual { quote, .. } = &mut quoted {
        *quote = "引号\"、反斜杠\\、换行\n与制表\t都要保留 ✓".into();
    }
    let mut timed = run("W03", "PASS", identity(), 'c');
    if let Record::Automated { run_unix, .. } = &mut timed {
        *run_unix = Some(1_795_018_746);
    }
    let records = vec![
        automated("W02", "PASS", identity()),
        timed,
        in_batch(run("R03", "FAIL", identity(), 'f'), "batch-1"),
        quoted,
        in_batch(manual("I02", Some(hex('1'))), "batch-1"),
        manual("W11", None),
        revoke("W02", Revoked::Evidence(hex('e'))),
        revoke("W06", Revoked::Inputs(hex('1'))),
    ];
    let text: String = records.iter().map(|r| r.line() + "\n").collect();
    assert_eq!(read_all(&text).unwrap(), records);
    let bad = |line: String| read_all(&format!("{text}{line}\n")).is_err();
    assert!(bad("{\"kind\":\"other\"}".into()));
    assert!(bad("not json".into()));
    assert!(bad(run("W09", "BANANA", identity(), '7').line()));
    let mut upper = identity();
    upper.driver = hex('D');
    assert!(
        bad(run("W09", "PASS", upper, '7').line()),
        "lowercase digests only"
    );
    assert!(bad(manual("W06", Some("not-a-hash".into())).line()));
    assert!(
        bad(in_batch(run("W09", "PASS", identity(), '7'), "no spaces").line()),
        "a batch name is a plain token"
    );
    assert!(
        bad(manual("W06", Some(hex('1')))
            .line()
            .replace("MANUAL PASS", "PASS")),
        "a manual record must say MANUAL PASS"
    );
    // The same evidence twice for a row is a replay; another row of it is fine.
    assert!(bad(automated("W02", "PASS", identity()).line()));
    assert!(!bad(automated("W04", "PASS", identity()).line()));
    // A revocation names a result recorded before it, exactly one target, and the USER's words.
    assert!(bad(revoke("W07", Revoked::Evidence(hex('e'))).line()));
    assert!(
        bad(revoke("R03", Revoked::Evidence(hex('f'))).line()),
        "only a PASS"
    );
    assert!(bad(revoke("W06", Revoked::Inputs(hex('2'))).line()));
    let both = revoke("W02", Revoked::Evidence(hex('e'))).line().replace(
        ",\"quote\"",
        &format!(",\"app_inputs\":\"{}\",\"quote\"", hex('1')),
    );
    assert!(bad(both), "a revocation names one target");
    let mut silent = revoke("W02", Revoked::Evidence(hex('e')));
    if let Record::Revocation { quote, .. } = &mut silent {
        *quote = " ".into();
    }
    assert!(bad(silent.line()), "a revocation carries the USER's words");
    // JSON Lines: no byte-order mark, no blank line; a final newline is fine.
    assert!(read_all(&format!("\u{feff}{text}")).is_err());
    assert!(
        read_all(&text.replacen('\n', "\n\u{feff}", 1)).is_err(),
        "nor on a later line"
    );
    assert!(read_all(&text.replacen('\n', "\n\n", 1)).is_err());
    assert!(read_all(text.trim_end()).is_ok());
}

fn started(row: &str, digest: char, run_unix: Option<u64>) -> Record {
    let mut record = run(row, "PASS", identity(), digest);
    if let Record::Automated {
        run_unix: start, ..
    } = &mut record
    {
        *start = run_unix;
    }
    record
}

/// For one row and identity, runs follow in the order they started, so a replay of an old run,
/// even rewritten into other bytes, cannot be recorded after a newer result.
#[test]
fn runs_of_a_row_are_recorded_in_the_order_they_started() {
    let earlier = [started("W09", 'e', Some(100))];
    assert!(consistent_with(&earlier, &started("W09", 'f', Some(101))).is_ok());
    assert!(
        consistent_with(&earlier, &started("W09", 'f', Some(100))).is_err(),
        "same start"
    );
    assert!(
        consistent_with(&earlier, &started("W09", 'f', Some(99))).is_err(),
        "older run"
    );
    // Two rows of one run share its start.
    assert!(consistent_with(&earlier, &started("W02", 'e', Some(100))).is_ok());
    // Records without a start cannot be ordered against ones with it, either way.
    let legacy = [started("W09", 'e', None)];
    assert!(consistent_with(&legacy, &started("W09", 'f', Some(101))).is_err());
    assert!(consistent_with(&earlier, &started("W09", 'f', None)).is_err());
    assert!(
        consistent_with(&legacy, &started("W09", 'f', None)).is_ok(),
        "history stays readable"
    );
    // Another identity is another sequence.
    let mut other = started("W09", 'f', Some(50));
    if let Record::Automated { identity, .. } = &mut other {
        identity.driver = hex('7');
    }
    assert!(consistent_with(&earlier, &other).is_ok());
}

/// A revocation recorded before its target would otherwise reach back to it.
#[test]
fn a_revocation_cannot_precede_its_target() {
    let early = revoke("W02", Revoked::Evidence(hex('e')));
    assert!(consistent_with(&[], &early).is_err());
    let target = automated("W02", "PASS", identity());
    assert!(consistent_with(std::slice::from_ref(&target), &early).is_ok());
    assert!(
        consistent_with(std::slice::from_ref(&target), &target).is_err(),
        "a replay"
    );
}

fn evidence(scenarios: &str, environment: &str) -> String {
    evidence_marked(scenarios, environment, ",\"finished\":true")
}
fn evidence_marked(scenarios: &str, environment: &str, finished: &str) -> String {
    format!(
        "{{\"schema_version\":1{finished},\"environment\":{{\"os\":\"ProductName:\\tmacOS\\nProductVersion:\\t27.2\\nBuildVersion:\\t26B123\\n\"{environment}}},\"app_inputs\":\"{}\",\"app_executable_sha256\":\"{}\",\"app_bundle_manifest_sha256\":\"{}\",\"driver_sha256\":\"{}\",\"scenarios\":[{scenarios}]}}",
        hex('1'),
        hex('a'),
        hex('b'),
        hex('d')
    )
}
const VM: &str = ",\"hv_vmm_present\":\"1\",\"allow_host\":false,\"unix_seconds\":42";
const PASSED: &str =
    "{\"id\":\"P16-W02\",\"verdict\":\"PASS\",\"checks\":[{\"name\":\"a\",\"verdict\":\"PASS\"}]}";
const WAITED: &str = "{\"id\":\"P16-W05\",\"verdict\":\"NOT_TESTED\",\"checks\":[{\"name\":\"a\",\"verdict\":\"PASS\"},{\"name\":\"b\",\"verdict\":\"NOT_TESTED\"}]}";

#[test]
fn evidence_is_recorded_from_one_read_of_a_vm_run_with_consistent_rows() {
    let bytes = evidence(&format!("{PASSED},{WAITED}"), VM);
    let records =
        from_evidence(bytes.as_bytes(), "/e.json", "sha256:image", Some("b1"), 3).unwrap();
    assert_eq!(records.len(), 2);
    let expected_sha = crate::integrity::sha256_bytes(bytes.as_bytes()).unwrap();
    assert!(matches!(
        &records[0],
        Record::Automated { row, verdict, identity: id, evidence_sha256, run_unix: Some(42), batch: Some(b), .. }
            if row == "W02" && verdict == "PASS" && *id == identity() && *evidence_sha256 == expected_sha && b == "b1"
    ));
    let refused = |bytes: String| {
        from_evidence(bytes.as_bytes(), "/e.json", "sha256:image", None, 3).is_err()
    };
    // Not a VM run, or one that allowed a host.
    assert!(refused(evidence(PASSED, &VM.replace("\"1\"", "\"0\""))));
    assert!(refused(evidence(PASSED, &VM.replace("false", "true"))));
    assert!(refused(evidence(PASSED, "")));
    // A row twice, or a verdict its checks do not give.
    assert!(refused(evidence(&format!("{PASSED},{PASSED}"), VM)));
    assert!(refused(evidence(
        &PASSED.replace(
            "\"verdict\":\"PASS\",\"checks\"",
            "\"verdict\":\"PASS\",\"checks\":[],\"x\""
        ),
        VM
    )));
    assert!(refused(evidence(
        &WAITED.replace("NOT_TESTED\",\"checks", "PASS\",\"checks"),
        VM
    )));
    assert!(refused(evidence(
        &PASSED.replace("\"verdict\":\"PASS\"}]", "\"verdict\":\"FAIL\"}]"),
        VM
    )));
    // A checkpoint of an unfinished run, an older driver's evidence, an empty run, an unknown check.
    assert!(refused(evidence_marked(PASSED, VM, ",\"finished\":false")));
    assert!(refused(evidence_marked(PASSED, VM, "")));
    assert!(refused(evidence("", VM)));
    assert!(refused(evidence(
        &PASSED.replace("\"verdict\":\"PASS\"}]", "\"verdict\":\"MAYBE\"}]"),
        VM
    )));
    // No driver named: the evidence cannot say which driver produced it.
    assert!(refused(
        evidence(PASSED, VM).replace(&format!("\"driver_sha256\":\"{}\",", hex('d')), "")
    ));
    // Evidence from a bundle built without its inputs is recorded, as history only.
    let legacy = evidence(PASSED, VM).replace(&format!("\"app_inputs\":\"{}\",", hex('1')), "");
    let records = from_evidence(legacy.as_bytes(), "/e.json", "sha256:image", None, 3).unwrap();
    assert!(matches!(
        &records[0],
        Record::Automated {
            identity: Identity { inputs: None, .. },
            ..
        }
    ));
}

#[test]
fn one_image_digest_names_one_system_across_the_whole_ledger() {
    let mut other = identity();
    other.system = "macOS 27.3".into();
    let known = automated("W02", "PASS", identity());
    assert!(check_system(&[known.clone(), automated("W03", "FAIL", other.clone())]).is_err());
    assert!(check_system(&[known.clone(), automated("W03", "PASS", identity())]).is_ok());
    assert!(check_system(&[known, manual("W06", Some(hex('1')))]).is_ok());
    let mut new_image = other;
    new_image.image = "sha256:new".into();
    assert!(
        check_system(&[
            automated("W02", "PASS", identity()),
            automated("W03", "PASS", new_image)
        ])
        .is_ok()
    );
}

#[test]
fn the_matrix_rows_come_from_the_mode_column() {
    let matrix = "| ID | Requirement | Mode | Evidence | Status |\n\
                  | --- | --- | --- | --- | --- |\n\
                  | P16-W06 | a | Manual | e | MANUAL PASS |\n\
                  | P16-W13 | b | Automated | e | AUTOMATED PASS |\n\
                  | P16-W16 | c | — | e | USER 豁免 |\n\
                  | P16-I01 | d | Manual | e | NOT TESTED |\n\
                  | P16-V04 | f | Review | e | NOT TESTED |\n\
                  | P16-W01 | g | Agent | e | NOT TESTED |\n\
                  | P16-V05 | h | Manual | Windows report | MANUAL PASS |\n";
    assert_eq!(
        matrix_rows(matrix).unwrap(),
        Rows {
            manual: strings(&["W06", "I01"]),
            review: strings(&["V04"]),
            agent: strings(&["W01"]),
        },
        "V05 is judged on Windows"
    );
    assert!(
        matrix_rows(&format!(
            "{matrix}| P16-W02 | x | Reveiw | e | NOT TESTED |\n"
        ))
        .is_err()
    );
    assert!(matrix_rows("| P16-W06 | a | Manual |\n").is_err());
    assert!(
        matrix_rows(&format!(
            "{matrix}| P16-V04 | f | Manual | e | NOT TESTED |\n"
        ))
        .is_err(),
        "a row appears once"
    );
    assert!(
        matrix_rows(&format!(
            "{matrix}| `P16-W02` | x | Manual | e | NOT TESTED |\n"
        ))
        .is_err()
    );
    // Prose may name a row anywhere; only table rows count.
    assert_eq!(
        matrix_rows(&format!(
            "{matrix}Prose naming P16-W02 and `P16-R03` is not a row.\n"
        ))
        .unwrap()
        .manual,
        strings(&["W06", "I01"])
    );
    assert!(matrix_rows("no rows").is_err());
}

#[test]
fn the_real_matrix_has_manual_rows_and_mac_gui_has_executable_ones() {
    let matrix = std::fs::read_to_string(
        crate::governance::find_repository_root(&std::env::current_dir().unwrap())
            .unwrap()
            .join("docs/acceptance-cases/phase-16.md"),
    )
    .unwrap();
    let rows = matrix_rows(&matrix).unwrap();
    assert!(rows.manual.iter().any(|r| r == "W12") && rows.manual.iter().any(|r| r == "I01"));
    assert!(!rows.manual.iter().any(|r| r == "V05"));
    let executable = crate::mac_gui::executable_ids();
    assert!(executable.contains(&"W12") && !executable.contains(&"W05"));
}

#[test]
fn each_command_requires_its_options_and_refuses_others() {
    let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
    assert!(parse(&args("plan --app a.app --driver d --image i")).is_err());
    let plan = parse(&args(
        "plan --app a.app --driver d --image i --system s --batch b-1",
    ))
    .unwrap();
    assert_eq!(plan.command, Some(Command::Plan));
    assert_eq!(plan.batch.as_deref(), Some("b-1"));
    assert!(
        parse(&args(
            "plan --app a.app --driver d --image i --system s --batch b/1"
        ))
        .is_err()
    );
    assert!(parse(&args("record --evidence e.json")).is_err());
    assert!(parse(&args("record --evidence e.json --image i")).is_ok());
    assert!(
        parse(&args("record --evidence e.json --image i --app a.app")).is_err(),
        "not a record option"
    );
    assert!(parse(&args("receipt --app a.app --rows W06")).is_err());
    let receipt = parse(&[
        "receipt".into(),
        "--app".into(),
        "a.app".into(),
        "--rows".into(),
        "P16-W06,W11".into(),
        "--quote=--试过了，没问题".into(),
    ])
    .unwrap();
    assert_eq!(receipt.rows, ["W06", "W11"]);
    assert_eq!(
        receipt.quote.as_deref(),
        Some("--试过了，没问题"),
        "the inline form is verbatim"
    );
    // A separate value that looks like an option is refused, so a missing quote cannot swallow
    // the next option.
    assert!(parse(&args("revoke --rows W02 --app a.app --quote --help")).is_err());
    assert!(
        parse(&args("receipt --app a.app --rows W06 --quote=q --quote=r")).is_err(),
        "once"
    );
    assert!(parse(&args("receipt --app a.app --rows W06,W06 --quote q")).is_err());
    let revoke = |extra: &str| {
        let mut a = args(&format!("revoke --rows W02{extra}"));
        a.extend(["--quote".into(), "撤销".into()]);
        parse(&a)
    };
    let upper = revoke(&format!(" --evidence-sha256 {}", hex('E'))).unwrap();
    assert_eq!(upper.command, Some(Command::Revoke));
    assert_eq!(
        upper.evidence_sha256,
        Some(hex('e')),
        "a digest is lowercased"
    );
    assert!(revoke(" --app a.app").is_ok());
    assert!(revoke("").is_err(), "a revocation names a target");
    assert!(
        revoke(" --app a.app --evidence-sha256 x").is_err(),
        "one target only"
    );
    assert!(
        revoke(" --app a.app --evidence e.json").is_err(),
        "not a revoke option"
    );
    assert!(parse(&args("revoke --rows W02,W03 --app a.app --quote q")).is_err());
    assert!(parse(&args("revoke --rows W02 --app a.app")).is_err());
    assert!(parse(&args("inputs --batch b")).is_err());
    assert!(parse(&args("frobnicate")).is_err());
    assert!(parse(&[]).unwrap().help);
}

#[test]
fn public_evidence_writers_refuse_the_ledger() {
    let root = crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
    let path = std::path::Path::new("dist/evidence/mac-gui-ledger/records.jsonl");
    assert!(crate::qualification::validate_public_evidence_path(&root, path).is_err());
    let directory = std::path::Path::new("dist/evidence/mac-gui-ledger");
    assert!(crate::qualification::validate_public_output_directory(&root, directory).is_err());
    let other = std::path::Path::new("dist/evidence/mac-gui-r14/mac-gui.json");
    assert!(crate::qualification::validate_public_evidence_path(&root, other).is_ok());
}
