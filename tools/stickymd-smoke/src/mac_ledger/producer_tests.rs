//! The ledger records of the two producers the macOS release gate needed (plan 11
//! #phase-16-macos-host): R03's release ZIP and `mac-resources` runs, with bad samples.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    decide::check_system,
    record::{Built, Identity, Record, from_evidence, read_all},
};

fn hex(c: char) -> String {
    std::iter::repeat_n(c, 64).collect()
}

/// The checks a passing R03 records: its six, after the one naming `zip` when it opened one.
fn r03_checks(zip: Option<&str>) -> Vec<(String, String)> {
    let mut checks: Vec<(String, String)> = zip
        .map(|z| {
            (
                crate::mac_gui::R03_ZIP_CHECK.to_owned(),
                format!("sha256={z}"),
            )
        })
        .into_iter()
        .collect();
    checks.extend(
        crate::mac_gui::R03_CHECKS
            .iter()
            .map(|name| ((*name).to_owned(), "seen".to_owned())),
    );
    checks
}

fn q(text: &str) -> String {
    format!("\"{}\"", crate::evidence::escape_json(text))
}

fn scenario(row: &str, verdict: &str, checks: &[(String, String)]) -> String {
    let checks = checks
        .iter()
        .map(|(name, observation)| {
            format!(
                "{{\"name\":{},\"verdict\":\"{verdict}\",\"observation\":{}}}",
                q(name),
                q(observation)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("{{\"id\":\"P16-{row}\",\"verdict\":\"{verdict}\",\"checks\":[{checks}]}}")
}

/// A finished `mac-gui` run in the VM with these passing rows (R03 with all its checks, naming
/// the ZIP when `extra` names one) and these extra top-level members.
fn gui_evidence(rows: &[&str], extra: &str) -> String {
    let zip = extra
        .split_once("\"zip_sha256\":\"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(zip, _)| zip);
    let scenarios = rows
        .iter()
        .map(|row| {
            let checks = if *row == "R03" {
                r03_checks(zip)
            } else {
                vec![("a".to_owned(), "seen".to_owned())]
            };
            scenario(row, "PASS", &checks)
        })
        .collect::<Vec<_>>();
    with_scenarios(&scenarios, extra)
}

fn with_scenarios(scenarios: &[String], extra: &str) -> String {
    let scenarios = scenarios.join(",");
    format!(
        "{{\"schema_version\":2,\"finished\":true,\"batch\":\"rc1\",\"environment\":{{\"os\":\"ProductName:\\tmacOS\\nProductVersion:\\t27.2\\n\",\"hv_vmm_present\":\"1\",\"allow_host\":false,\"unix_seconds\":42}},\"app_inputs\":\"{}\",\"app_executable_sha256\":\"{}\",\"app_bundle_manifest_sha256\":\"{}\",\"driver_sha256\":\"{}\"{extra},\"scenarios\":[{scenarios}]}}",
        hex('1'),
        hex('a'),
        hex('b'),
        hex('d')
    )
}

fn record(bytes: &str) -> Result<Vec<Record>, String> {
    from_evidence(bytes.as_bytes(), "/e.json", "sha256:image", 3)
}

fn zip_of(record: &Record) -> Option<&str> {
    match record {
        Record::Automated { zip, .. } => zip.as_deref(),
        _ => None,
    }
}

/// Each record written as a ledger line and read back.
fn round_trip(records: &[Record]) -> (String, Vec<Record>) {
    let text: String = records.iter().map(|r| r.line() + "\n").collect();
    let read = read_all(&text).unwrap();
    (text, read)
}

#[test]
fn r03_keeps_the_zip_it_opened_and_records_without_one_read_as_before() {
    let zip = "F".repeat(64);
    let records = record(&gui_evidence(
        &["W02", "R03"],
        &format!(",\"zip_sha256\":\"{zip}\""),
    ))
    .unwrap();
    // Only R03 opens the ZIP; its digest is kept lowercased like every other.
    assert_eq!(zip_of(&records[0]), None);
    assert_eq!(zip_of(&records[1]), Some(hex('f').as_str()));
    let (text, read) = round_trip(&records);
    assert_eq!(read, records);
    assert_eq!(text.matches("\"zip_sha256\"").count(), 1);
    // A run zipped from the .app names no ZIP, and its records are written as before.
    let records = record(&gui_evidence(&["R03"], "")).unwrap();
    assert_eq!(zip_of(&records[0]), None);
    let (text, read) = round_trip(&records);
    assert!(!text.contains("zip_sha256"));
    assert_eq!(read, records);
    assert_eq!(
        read.iter().map(|r| r.line() + "\n").collect::<String>(),
        text,
        "a record without a ZIP keeps its bytes"
    );
}

#[test]
fn a_zip_without_r03_or_not_a_digest_is_refused() {
    let error = record(&gui_evidence(
        &["W02"],
        &format!(",\"zip_sha256\":\"{}\"", hex('f')),
    ))
    .unwrap_err();
    assert!(error.contains("no R03 row"), "{error}");
    let error = record(&gui_evidence(&["R03"], ",\"zip_sha256\":\"dddd\"")).unwrap_err();
    assert!(error.contains("not a SHA-256"), "{error}");
    // A hand-written line naming a ZIP on another row rejects the ledger.
    let records = record(&gui_evidence(
        &["R03"],
        &format!(",\"zip_sha256\":\"{}\"", hex('f')),
    ))
    .unwrap();
    let line = records[0]
        .line()
        .replace("\"row\":\"R03\"", "\"row\":\"W02\"");
    let error = read_all(&line).unwrap_err();
    assert!(error.contains("only R03 opens a ZIP"), "{error}");
}

#[test]
fn a_finished_resources_run_in_the_vm_records_r01_and_r02_with_both_digests() {
    let json = crate::mac_resources::evidence_json(false);
    let records = record(&json).unwrap();
    let digest = crate::integrity::sha256_bytes(json.as_bytes()).unwrap();
    let rows: Vec<(&str, &str)> = records
        .iter()
        .map(|r| match r {
            Record::Automated { row, verdict, .. } => (row.as_str(), verdict.as_str()),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(rows, [("R01", "PASS"), ("R02", "PASS")]);
    for r in &records {
        let Record::Automated {
            identity,
            built,
            evidence_sha256,
            run_unix,
            batch,
            zip,
            ..
        } = r
        else {
            unreachable!()
        };
        assert_eq!(
            *identity,
            Identity {
                inputs: Some(hex('e')),
                driver: hex('d'),
                image: "sha256:image".into(),
                // From sw_vers, as for a mac-gui run: one image names one system across both.
                system: "ProductName: macOS ProductVersion: 27.2".into(),
            }
        );
        assert_eq!(
            *built,
            Built {
                executable: hex('a'),
                bundle: hex('c'),
            }
        );
        assert_eq!(*evidence_sha256, digest);
        assert_eq!((*run_unix, batch, zip), (Some(7), &None, &None));
    }
    let (_, read) = round_trip(&records);
    assert_eq!(read, records);
    let mut all = records.clone();
    all.extend(record(&gui_evidence(&["W02"], "")).unwrap());
    check_system(&all).unwrap();
    // An R02 FAIL is recorded as a FAIL.
    let busy = record(&crate::mac_resources::evidence_json(true)).unwrap();
    assert!(
        matches!(&busy[1], Record::Automated { row, verdict, .. } if row == "R02" && verdict == "FAIL")
    );
}

#[test]
fn resources_evidence_outside_the_vm_unfinished_old_or_inconsistent_is_refused() {
    let good = crate::mac_resources::evidence_json(false);
    let samples = [
        (
            "\"hv_vmm_present\":\"1\"",
            "\"hv_vmm_present\":\"0\"",
            "not in a virtual machine",
        ),
        ("\"finished\":true", "\"finished\":false", "not finished"),
        (
            "\"schema_version\":5",
            "\"schema_version\":4",
            "before schema 5",
        ),
        // A run's status changed while the rows still say PASS.
        (
            "\"status\":\"VALID\"",
            "\"status\":\"ERROR\"",
            "its runs give",
        ),
    ];
    for (from, to, words) in samples {
        assert!(good.contains(from), "{from}");
        let error = record(&good.replacen(from, to, 1)).unwrap_err();
        assert!(error.contains(words), "{from}: {error}");
    }
    // Schema 4 evidence carried no tool member: it is neither mac-gui's nor this one's schema.
    let old = good.replacen("\"tool\":\"mac-resources\",", "", 1);
    let error =
        record(&old.replacen("\"schema_version\":5", "\"schema_version\":4", 1)).unwrap_err();
    assert!(error.contains("schema 5 of mac-resources"), "{error}");
    // Without the bundle digest or the driver it cannot be recorded.
    for member in ["\"app_bundle_manifest_sha256\"", "\"driver_sha256\""] {
        let renamed = good.replacen(member, "\"other\"", 1);
        assert!(record(&renamed).is_err(), "{member}");
    }
}

/// Review A5/F3: an R03 PASS counts only with every observation R03 makes, and a ZIP only when
/// R03 names the same one; one passing check of any name is no R03.
#[test]
fn an_r03_pass_needs_every_r03_check_and_the_zip_it_names() {
    let zip = hex('f');
    let top = format!(",\"zip_sha256\":\"{zip}\"");
    let pass = |checks: &[(String, String)], extra: &str| {
        record(&with_scenarios(&[scenario("R03", "PASS", checks)], extra))
    };
    assert!(pass(&r03_checks(Some(&zip)), &top).is_ok());
    let error = pass(&[("a".into(), "seen".into())], &top).unwrap_err();
    assert!(error.contains("exactly the checks"), "{error}");
    // One observation missing: here, that nothing was written.
    let mut missing = r03_checks(Some(&zip));
    missing.pop();
    assert!(
        pass(&missing, &top)
            .unwrap_err()
            .contains("exactly the checks")
    );
    // The ZIP given, but R03 does not say it opened it; or says it opened another.
    assert!(
        pass(&r03_checks(None), &top)
            .unwrap_err()
            .contains("exactly the checks")
    );
    let other = r03_checks(Some(&hex('e')));
    assert!(
        pass(&other, &top)
            .unwrap_err()
            .contains("names the ZIP it opened")
    );
    // R03 claims a ZIP the evidence does not name.
    assert!(
        pass(&r03_checks(Some(&zip)), "")
            .unwrap_err()
            .contains("exactly the checks")
    );
    // A run zipped from the .app passes with the six checks and names no ZIP.
    assert!(pass(&r03_checks(None), "").is_ok());
    // A FAIL is recorded whatever checks it holds: it can only keep the row from holding.
    let failed = with_scenarios(
        &[scenario(
            "R03",
            "FAIL",
            &[("execution".into(), "refused".into())],
        )],
        &top,
    );
    assert!(record(&failed).is_ok());
}

/// Review A5/F3: each producer writes only its own rows.
#[test]
fn only_the_rows_a_producer_runs_are_recorded() {
    for row in ["R01", "R02", "V03", "S01", "X99"] {
        let error = record(&gui_evidence(&[row], "")).unwrap_err();
        assert!(
            error.contains("not a row mac-gui or mac-logout runs"),
            "{row}: {error}"
        );
    }
    for row in ["W02", "W05", "W12-logout-text", "K02"] {
        assert!(record(&gui_evidence(&[row], "")).is_ok(), "{row}");
    }
    let error = record(&gui_evidence(&["W02"], ",\"tool\":\"mac-latency\"")).unwrap_err();
    assert!(error.contains("tool \"mac-latency\""), "{error}");
}

/// Review F10: one image, one spelling, so its records group together.
#[test]
fn image_digests_are_validated_and_kept_in_one_spelling() {
    use super::options::image_digest;
    let digest = format!("sha256:{}", "AB".repeat(32));
    assert_eq!(
        image_digest(&format!("GHCR.io/x/y@{digest}")).unwrap(),
        format!("ghcr.io/x/y@sha256:{}", "ab".repeat(32))
    );
    assert_eq!(image_digest(&digest).unwrap(), digest.to_ascii_lowercase());
    for bad in [
        "i".to_owned(),
        "sha256:abc".to_owned(),
        format!("@{digest}"),
        format!("name@sha256:{}", "a".repeat(63)),
        format!("name @{digest}"),
        format!("name@sha512:{}", "a".repeat(64)),
    ] {
        assert!(image_digest(&bad).is_err(), "{bad}");
    }
    // A record written with another case of the same image reads as the same identity, and the
    // ledger keeps the bytes it was written with.
    let records = record(&gui_evidence(&["W02"], "")).unwrap();
    let line = records[0]
        .line()
        .replace("\"image\":\"sha256:image\"", "\"image\":\"SHA256:IMAGE\"");
    assert!(line.contains("SHA256:IMAGE"));
    assert_eq!(read_all(&line).unwrap(), records);
}
