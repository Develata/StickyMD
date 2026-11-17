//! Headless tests of the ledger records, the reuse rule and the parser; no file is written.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    decide::{check_system, manual_rows, plan},
    options::{Command, parse},
    record::{Built, Identity, Record, from_evidence, read_all},
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
    built_as(row, verdict, identity, built())
}
fn built_as(row: &str, verdict: &str, identity: Identity, built: Built) -> Record {
    Record::Automated {
        row: row.into(),
        verdict: verdict.into(),
        identity,
        built,
        evidence: format!("/runs/{row}.json"),
        evidence_sha256: hex('e'),
        recorded_unix: 1,
    }
}
fn manual(row: &str, inputs: Option<String>) -> Record {
    Record::Manual {
        row: row.into(),
        inputs,
        app_executable: hex('a'),
        environment: "USER desktop, recorded on macOS 27.2".into(),
        quote: "都没什么问题".into(),
        recorded_unix: 2,
    }
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
    let result = plan(&records, &rows, &[], &identity(), &built());
    assert_eq!(result.reused.len(), 1);
    assert_eq!(result.reused[0].row, "W02");
    assert_eq!(
        result.run,
        ["W03", "W04", "W07", "W08", "W09", "W11", "K02"]
    );
}

/// The anchor is the product inputs, not the bytes (plan 11 #phase-16-macos-host): a PASS for
/// other bytes built from the same inputs is reused and reported for audit; a record or an app
/// without inputs reuses nothing.
#[test]
fn the_same_inputs_reuse_a_pass_for_other_bytes_and_no_inputs_reuse_nothing() {
    let other_bytes = Built {
        executable: hex('9'),
        bundle: hex('8'),
    };
    let legacy = Identity {
        inputs: None,
        ..identity()
    };
    let records = [
        built_as("W02", "PASS", identity(), other_bytes),
        automated("W03", "PASS", legacy.clone()),
    ];
    let result = plan(&records, &["W02", "W03"], &[], &identity(), &built());
    assert_eq!(result.reused.len(), 1);
    assert_eq!(result.reused[0].row, "W02");
    assert!(result.reused[0].other_bytes);
    assert_eq!(result.run, ["W03"]);
    let same_bytes = [automated("W02", "PASS", identity())];
    assert!(!plan(&same_bytes, &["W02"], &[], &identity(), &built()).reused[0].other_bytes);
    // An app built without its inputs matches nothing, a legacy record included.
    let result = plan(
        &records,
        &["W02", "W03"],
        &["W06".into()],
        &legacy,
        &built(),
    );
    assert!(result.reused.is_empty());
    assert_eq!(result.unconfirmed, ["W06"]);
}

#[test]
fn the_latest_pass_is_cited_and_a_later_non_pass_is_reported() {
    let records = [
        automated("W02", "PASS", identity()),
        automated("W02", "FAIL", identity()),
    ];
    let result = plan(&records, &["W02"], &[], &identity(), &built());
    assert!(result.reused[0].later_non_pass);
    let records = [
        automated("W02", "FAIL", identity()),
        automated("W02", "PASS", identity()),
    ];
    assert!(!plan(&records, &["W02"], &[], &identity(), &built()).reused[0].later_non_pass);
}

#[test]
fn a_user_receipt_holds_only_for_its_inputs_and_never_for_the_never_reused_rows() {
    let records = [
        manual("W06", Some(hex('1'))),
        manual("W11", Some(hex('2'))),
        manual("W12", None),
        manual("I01", Some(hex('1'))),
        manual("R03", Some(hex('1'))),
    ];
    let rows: Vec<String> = ["W06", "W11", "W12", "I01", "R03"].map(String::from).into();
    let result = plan(&records, &[], &rows, &identity(), &built());
    assert_eq!(result.confirmed.len(), 1);
    assert_eq!(result.confirmed[0].0, "W06");
    assert_eq!(result.unconfirmed, ["W11", "W12", "I01", "R03"]);
}

#[test]
fn records_survive_a_round_trip_and_bad_lines_reject_the_ledger() {
    let mut quoted = manual("W06", Some(hex('1')));
    if let Record::Manual { quote, .. } = &mut quoted {
        *quote = "引号\"、反斜杠\\、换行\n与制表\t都要保留 ✓".into();
    }
    let legacy = Identity {
        inputs: None,
        ..identity()
    };
    let records = vec![
        automated("W02", "PASS", identity()),
        automated("W03", "PASS", legacy),
        quoted,
        manual("W11", None),
    ];
    let text: String = records.iter().map(|r| r.line() + "\n").collect();
    assert_eq!(read_all(&text).unwrap(), records);
    let bad = |line: String| read_all(&format!("{text}{line}\n")).is_err();
    assert!(bad("{\"kind\":\"other\"}".into()));
    assert!(bad("not json".into()));
    assert!(bad(automated("W02", "BANANA", identity()).line()));
    let mut empty_image = identity();
    empty_image.image = " ".into();
    assert!(bad(automated("W02", "PASS", empty_image).line()));
    let mut short_hash = identity();
    short_hash.driver = "abc".into();
    assert!(bad(automated("W02", "PASS", short_hash).line()));
    assert!(bad(manual("W06", Some("not-a-hash".into())).line()));
    let mut bad_inputs = identity();
    bad_inputs.inputs = Some("abc".into());
    assert!(bad(automated("W02", "PASS", bad_inputs).line()));
    assert!(
        bad(manual("W06", Some(hex('1')))
            .line()
            .replace("MANUAL PASS", "PASS")),
        "a manual record must say MANUAL PASS"
    );
}

#[test]
fn evidence_is_recorded_from_one_read_and_must_name_its_driver() {
    let evidence = |driver: &str| {
        format!(
            "{{\"environment\":{{\"os\":\"ProductName:\\tmacOS\\nProductVersion:\\t27.2\\nBuildVersion:\\t26B123\\n\"}},\"app_inputs\":\"{}\",\"app_executable_sha256\":\"{}\",\"app_bundle_manifest_sha256\":\"{}\",{driver}\"scenarios\":[{{\"id\":\"P16-W02\",\"verdict\":\"PASS\"}},{{\"id\":\"P16-W05\",\"verdict\":\"NOT_TESTED\"}}]}}",
            hex('1'),
            hex('a'),
            hex('b')
        )
    };
    assert!(from_evidence(evidence("").as_bytes(), "/e.json", "sha256:image", 3).is_err());
    let bytes = evidence(&format!("\"driver_sha256\":\"{}\",", hex('d')));
    let records = from_evidence(bytes.as_bytes(), "/e.json", "sha256:image", 3).unwrap();
    assert_eq!(records.len(), 2);
    let expected_sha = crate::integrity::sha256_bytes(bytes.as_bytes()).unwrap();
    assert!(matches!(
        &records[0],
        Record::Automated { row, verdict, identity: id, built: b, evidence_sha256, .. }
            if row == "W02" && verdict == "PASS" && *id == identity() && *b == built()
                && *evidence_sha256 == expected_sha
    ));
    // Evidence from a bundle built without its inputs is recorded, as history only.
    let legacy = bytes.replace(&format!("\"app_inputs\":\"{}\",", hex('1')), "");
    let records = from_evidence(legacy.as_bytes(), "/e.json", "sha256:image", 3).unwrap();
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
fn the_manual_rows_come_from_the_matrix_mode_column() {
    let matrix = "| ID | Requirement | Mode | Evidence | Status |\n\
                  | --- | --- | --- | --- | --- |\n\
                  | P16-W06 | a | Manual | e | MANUAL PASS |\n\
                  | P16-W13 | b | Automated | e | AUTOMATED PASS |\n\
                  | P16-W16 | c | — | e | USER 豁免 |\n\
                  | P16-I01 | d | Manual | e | NOT TESTED |\n";
    assert_eq!(manual_rows(matrix).unwrap(), ["W06", "I01"]);
    assert!(manual_rows("| P16-W06 | a | Manual |\n").is_err());
    assert!(manual_rows("no rows").is_err());
}

#[test]
fn the_real_matrix_has_manual_rows_and_mac_gui_has_executable_ones() {
    let matrix = std::fs::read_to_string(
        crate::governance::find_repository_root(&std::env::current_dir().unwrap())
            .unwrap()
            .join("docs/acceptance-cases/phase-16.md"),
    )
    .unwrap();
    let rows = manual_rows(&matrix).unwrap();
    assert!(rows.iter().any(|r| r == "W12") && rows.iter().any(|r| r == "I01"));
    let executable = crate::mac_gui::executable_ids();
    assert!(executable.contains(&"W12") && !executable.contains(&"W05"));
}

#[test]
fn each_command_requires_its_options() {
    let args = |s: &str| s.split(' ').map(String::from).collect::<Vec<_>>();
    assert!(parse(&args("plan --app a.app --driver d --image i")).is_err());
    let plan = parse(&args("plan --app a.app --driver d --image i --system s")).unwrap();
    assert_eq!(plan.command, Some(Command::Plan));
    assert!(parse(&args("record --evidence e.json")).is_err());
    assert!(parse(&args("record --evidence e.json --image i")).is_ok());
    assert!(parse(&args("receipt --app a.app --rows W06")).is_err());
    let receipt = parse(&[
        "receipt".into(),
        "--app".into(),
        "a.app".into(),
        "--rows".into(),
        "P16-W06,W11".into(),
        "--quote".into(),
        "试过了".into(),
    ])
    .unwrap();
    assert_eq!(receipt.rows, ["W06", "W11"]);
    assert!(parse(&args("receipt --app a.app --rows W06,W06 --quote q")).is_err());
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
