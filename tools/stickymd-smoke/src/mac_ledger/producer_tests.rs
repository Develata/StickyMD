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

/// A finished `mac-gui` run in the VM with these passing rows (R03 NOT_TESTED with all its
/// checks, naming the ZIP when `extra` names one, and no facts, as an older driver wrote it) and
/// these extra top-level members.
fn gui_evidence(rows: &[&str], extra: &str) -> String {
    let zip = extra
        .split_once("\"zip_sha256\":\"")
        .and_then(|(_, rest)| rest.split_once('"'))
        .map(|(zip, _)| zip);
    let scenarios = rows
        .iter()
        .map(|row| {
            if *row == "R03" {
                scenario(row, "NOT_TESTED", &r03_checks(zip))
            } else {
                scenario(row, "PASS", &[("a".to_owned(), "seen".to_owned())])
            }
        })
        .collect::<Vec<_>>();
    with_scenarios(&scenarios, extra)
}

fn with_scenarios(scenarios: &[String], extra: &str) -> String {
    let scenarios = scenarios.join(",");
    format!(
        "{{\"schema_version\":2,\"finished\":true,\"batch\":\"rc1\",\"environment\":{{\"os\":\"ProductName:\\tmacOS\\nProductVersion:\\t27.2\\nBuildVersion:\\t26B123\\n\",\"hv_vmm_present\":\"1\",\"allow_host\":false,\"unix_seconds\":42}},\"app_inputs\":\"{}\",\"app_executable_sha256\":\"{}\",\"app_bundle_manifest_sha256\":\"{}\",\"driver_sha256\":\"{}\"{extra},\"scenarios\":[{scenarios}]}}",
        hex('1'),
        hex('a'),
        hex('b'),
        hex('d')
    )
}

fn record(bytes: &str) -> Result<Vec<Record>, String> {
    from_evidence(
        bytes.as_bytes(),
        "/e.json",
        "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
        3,
    )
}

fn zip_of(record: &Record) -> Option<&str> {
    match record {
        Record::Automated { zip, .. } => zip.as_ref().map(|z| z.sha256.as_str()),
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

/// The importer's own digest check, which `Record::validate` cannot stand for: without an R03
/// row no record carries the ZIP, so `validate` never sees it; with one, the importer refuses
/// it before `validate` would (pre-freeze, producers item 5 (third review)).
#[test]
fn a_zip_digest_that_is_not_a_sha256_is_refused_where_the_evidence_is_read() {
    for rows in [&["W02"][..], &["R03"], &["W02", "R03"]] {
        let error = record(&gui_evidence(rows, ",\"zip_sha256\":\"dddd\"")).unwrap_err();
        assert!(
            error.contains("the evidence names a ZIP digest that is not a SHA-256: \"dddd\""),
            "{rows:?}: {error}"
        );
    }
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
                image: "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee"
                    .into(),
                // From sw_vers, as for a mac-gui run: one image names one system across both.
                system: "ProductName: macOS ProductVersion: 27.2 BuildVersion: 26B123".into(),
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
        // Each older schema is refused for what it lacks (review F7: schema 6 records no view).
        (
            "\"schema_version\":7",
            "\"schema_version\":6",
            "mac-resources evidence of schema 6 records no view for its runs",
        ),
        // Schema 5 took no sample before 60 s (review round 3, B3), nor recorded a view.
        (
            "\"schema_version\":7",
            "\"schema_version\":5",
            "of schema 5 takes no sample before 60 s and records no view",
        ),
        (
            "\"schema_version\":7",
            "\"schema_version\":4",
            "of schema 4 names neither the bundle nor the driver",
        ),
        (
            "\"schema_version\":7",
            "\"schema_version\":8",
            "is not of schema 7, the one this reader knows",
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
        record(&old.replacen("\"schema_version\":7", "\"schema_version\":4", 1)).unwrap_err();
    assert!(error.contains("schema 7 of mac-resources"), "{error}");
    // The --wrong-view calibration is a bad sample: never recorded (review F7). As the tool
    // writes it, it is also never finished, so without its `calibration` member it is still
    // refused, as unfinished.
    let calibration = crate::mac_resources::calibration_json();
    let error = record(&calibration).unwrap_err();
    assert!(
        error.contains("calibration evidence (--wrong-view) is a bad sample"),
        "{error}"
    );
    let unnamed = calibration.replacen(",\"calibration\":\"wrong-view\"", "", 1);
    assert_ne!(unnamed, calibration);
    let error = record(&unnamed).unwrap_err();
    assert!(error.contains("not finished"), "{error}");
    // Even a calibration marked finished by hand is refused for being one.
    let finished = good.replacen(
        "\"tool\":\"mac-resources\"",
        "\"tool\":\"mac-resources\",\"calibration\":\"wrong-view\"",
        1,
    );
    let error = record(&finished).unwrap_err();
    assert!(
        error.contains("calibration evidence (--wrong-view) is a bad sample"),
        "{error}"
    );
    // Without the bundle digest or the driver it cannot be recorded.
    for member in ["\"app_bundle_manifest_sha256\"", "\"driver_sha256\""] {
        let renamed = good.replacen(member, "\"other\"", 1);
        assert!(record(&renamed).is_err(), "{member}");
    }
}

/// Review S4: only R03 opens a ZIP, so resource evidence naming one, even a well-formed digest, is
/// not this tool's and is not recorded (pre-freeze, producers item 5 (third review)).
#[test]
fn resources_evidence_naming_a_zip_is_refused() {
    let good = crate::mac_resources::evidence_json(false);
    let from = "\"tool\":\"mac-resources\",";
    let zipped = good.replacen(from, &format!("{from}\"zip_sha256\":\"{}\",", hex('f')), 1);
    assert_ne!(zipped, good);
    let error = record(&zipped).unwrap_err();
    assert!(
        error.contains("mac-resources evidence names a ZIP, which only R03 opens"),
        "{error}"
    );
}

/// R03's checks as `(name, verdict, observation)`.
type Checks = Vec<(String, String, String)>;

/// An R03 row as the driver writes it: its facts, and the checks judged from them, which
/// `tamper` may change before they are written.
fn r03_row(facts: &str, tamper: &dyn Fn(&mut Checks)) -> String {
    let mut checks = crate::mac_gui::r03_rejudge(&crate::release::json::parse(facts).unwrap())
        .unwrap()
        .checks;
    tamper(&mut checks);
    let verdicts: Vec<&str> = checks.iter().map(|(_, v, _)| v.as_str()).collect();
    let verdict = if verdicts.contains(&"FAIL") {
        "FAIL"
    } else if verdicts.contains(&"NOT_TESTED") {
        "NOT_TESTED"
    } else {
        "PASS"
    };
    let checks = checks
        .iter()
        .map(|(name, verdict, observation)| {
            format!(
                "{{\"name\":{},\"verdict\":{},\"observation\":{}}}",
                q(name),
                q(verdict),
                q(observation)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"id\":\"P16-R03\",\"verdict\":\"{verdict}\",\"checks\":[{checks}],\"facts\":{facts}}}"
    )
}

/// Review #24 B9 and S4 (USER 2026-11-24, option A): an R03 PASS is recorded only when its
/// checks follow from the facts the run recorded, write watch included, and name the ZIP the
/// evidence names; one passing check of any name, or the snapshot-only checks of an older
/// driver, is no R03.
#[test]
fn an_r03_pass_is_recomputed_from_its_facts() {
    let zip = hex('f');
    let top = format!(",\"zip_sha256\":\"{zip}\"");
    let good = crate::mac_gui::r03_sample_facts(Some(&zip), &[("StickyMD.app", 0x08)]);
    let keep = |_: &mut Checks| {};
    let records = record(&with_scenarios(&[r03_row(&good, &keep)], &top)).unwrap();
    assert!(matches!(&records[0], Record::Automated { verdict, .. } if verdict == "PASS"));
    assert_eq!(zip_of(&records[0]), Some(zip.as_str()));
    // Its facts carry the write watch, so the record says so for the gate (review round 3, B1);
    // a row without facts never does.
    assert!(records[0].line().contains(",\"write_watch\":true"));
    let unwatched = record(&with_scenarios(
        &[scenario("R03", "NOT_TESTED", &r03_checks(Some(&zip)))],
        &top,
    ))
    .unwrap();
    assert!(!unwatched[0].line().contains("write_watch"));
    // Facts present but without a watch (review fix round, 11): no marker either.
    let start = good.find("\"watch\":{").unwrap();
    let no_watch = format!("{}\"watch\":null}}", &good[..start]);
    let records = record(&with_scenarios(&[r03_row(&no_watch, &keep)], &top)).unwrap();
    assert!(matches!(&records[0], Record::Automated { verdict, .. } if verdict == "NOT_TESTED"));
    assert!(!records[0].line().contains("write_watch"));
    // A run zipped from the .app names no ZIP, in its facts or at the top.
    let unzipped = crate::mac_gui::r03_sample_facts(None, &[]);
    assert!(record(&with_scenarios(&[r03_row(&unzipped, &keep)], "")).is_ok());
    let refused = |row: String, extra: &str| record(&with_scenarios(&[row], extra)).unwrap_err();
    // An older driver's PASS: every check, snapshot-only "nothing written", no facts (m5p-2).
    let mut old = r03_checks(Some(&zip));
    old.last_mut().unwrap().0 = crate::mac_gui::R03_SNAPSHOT_ONLY_UNWRITTEN.into();
    let error = refused(scenario("R03", "PASS", &old), &top);
    assert!(error.contains("without its facts"), "{error}");
    let error = refused(scenario("R03", "PASS", &r03_checks(Some(&zip))), &top);
    assert!(error.contains("without its facts"), "{error}");
    // Facts with a write the snapshots missed, stated as a PASS.
    let written =
        crate::mac_gui::r03_sample_facts(Some(&zip), &[("StickyMD.app/Info.plist", 0x02)]);
    let as_pass = |checks: &mut Checks| {
        for check in checks {
            check.1 = "PASS".into();
        }
    };
    let error = refused(r03_row(&written, &as_pass), &top);
    assert!(error.contains("do not follow from its facts"), "{error}");
    // As the driver judged it, it is a FAIL, and recorded.
    let records = record(&with_scenarios(&[r03_row(&written, &keep)], &top)).unwrap();
    assert!(matches!(&records[0], Record::Automated { verdict, .. } if verdict == "FAIL"));
    // An observation rewritten, a check dropped, a check added.
    let rewritten = |checks: &mut Checks| checks[1].2.push('!');
    let dropped = |checks: &mut Checks| {
        checks.pop();
    };
    let added = |checks: &mut Checks| {
        checks.push(("a".into(), "PASS".into(), "seen".into()));
    };
    for tamper in [&rewritten as &dyn Fn(&mut Checks), &dropped, &added] {
        let error = refused(r03_row(&good, tamper), &top);
        assert!(error.contains("do not follow from its facts"), "{error}");
    }
    // The row's verdict not its checks'.
    let error = refused(
        r03_row(&good, &keep).replace(
            "\"verdict\":\"PASS\",\"checks\"",
            "\"verdict\":\"NOT_TESTED\",\"checks\"",
        ),
        &top,
    );
    assert!(error.contains("but its checks make it PASS"), "{error}");
    // Facts naming another ZIP than the evidence, or one the evidence does not name.
    let other = crate::mac_gui::r03_sample_facts(Some(&hex('e')), &[]);
    let error = refused(r03_row(&other, &keep), &top);
    assert!(error.contains("R03's facts name the ZIP"), "{error}");
    let error = refused(r03_row(&good, &keep), "");
    assert!(error.contains("R03's facts name the ZIP"), "{error}");
    // Facts read strictly: a member added, or of another type.
    let extra = good.replacen('{', "{\"extra\":1,", 1);
    let error = refused(r03_row(&good, &keep).replace(&good, &extra), &top);
    assert!(error.contains("R03 facts has the members"), "{error}");
    let typed = good.replace("\"exited_early\":false", "\"exited_early\":\"no\"");
    let error = refused(r03_row(&good, &keep).replace(&good, &typed), &top);
    assert!(error.contains("exited_early is not a boolean"), "{error}");
    // Facts belong to R03 alone.
    let error = refused(r03_row(&good, &keep).replace("P16-R03", "P16-W02"), &top);
    assert!(
        error.contains("records facts, which only R03 does"),
        "{error}"
    );
}

/// An R03 row without facts (a driver older than the write watch) never records a PASS, and its
/// FAIL or NOT_TESTED only with R03's own checks — today's names or the snapshot-only driver's —
/// naming the ZIP the evidence names. A row that only says its execution failed (the session
/// directory could not be made, an attachment could not be written) is the machine's, not the
/// product's, and is not recorded (review round 3, B4).
#[test]
fn an_r03_row_without_facts_holds_r03_checks_and_never_passes() {
    let zip = hex('f');
    let top = format!(",\"zip_sha256\":\"{zip}\"");
    let row = |verdict: &str, checks: &[(String, String)], extra: &str| {
        record(&with_scenarios(&[scenario("R03", verdict, checks)], extra))
    };
    let mut older = r03_checks(Some(&zip));
    older.last_mut().unwrap().0 = crate::mac_gui::R03_SNAPSHOT_ONLY_UNWRITTEN.into();
    for verdict in ["FAIL", "NOT_TESTED"] {
        assert!(
            row(verdict, &r03_checks(Some(&zip)), &top).is_ok(),
            "{verdict}"
        );
        assert!(
            row(verdict, &older, &top).is_ok(),
            "{verdict} of the older driver"
        );
        assert!(
            row(verdict, &r03_checks(None), "").is_ok(),
            "{verdict} without a ZIP"
        );
        for error in [
            "stickymd-mac-session: No space left on device (os error 28)",
            "R03-stderr.log: Permission denied (os error 13)",
        ] {
            let error = row(verdict, &[("execution".into(), error.into())], &top).unwrap_err();
            assert!(
                error.contains("an R03 row needs exactly the checks"),
                "{verdict}: {error}"
            );
        }
        // One check missing, or the ZIP not named first, or another ZIP named.
        let mut missing = r03_checks(Some(&zip));
        missing.pop();
        assert!(
            row(verdict, &missing, &top)
                .unwrap_err()
                .contains("exactly the checks")
        );
        assert!(
            row(verdict, &r03_checks(None), &top)
                .unwrap_err()
                .contains("exactly the checks")
        );
        let error = row(verdict, &r03_checks(Some(&hex('e'))), &top).unwrap_err();
        assert!(error.contains("names the ZIP it opened"), "{error}");
    }
}

/// R03's checks are judged in the driver's order: two of its own checks swapped are refused,
/// with facts (they no longer follow from them) and without (they are not exactly R03's checks,
/// the only guard on that path) (pre-freeze, producers item 5 (third review)).
#[test]
fn two_of_r03s_checks_swapped_are_refused() {
    let zip = hex('f');
    let top = format!(",\"zip_sha256\":\"{zip}\"");
    // Index 0 names the ZIP; the two swapped are R03's first two own checks.
    let mut swapped = r03_checks(Some(&zip));
    swapped.swap(1, 2);
    assert_eq!(swapped[1].0, crate::mac_gui::R03_CHECKS[1]);
    for verdict in ["FAIL", "NOT_TESTED"] {
        let error =
            record(&with_scenarios(&[scenario("R03", verdict, &swapped)], &top)).unwrap_err();
        assert!(
            error.contains("an R03 row needs exactly the checks"),
            "{verdict}: {error}"
        );
    }
    let good = crate::mac_gui::r03_sample_facts(Some(&zip), &[("StickyMD.app", 0x08)]);
    let swap = |checks: &mut Checks| checks.swap(1, 2);
    let error = record(&with_scenarios(&[r03_row(&good, &swap)], &top)).unwrap_err();
    assert!(error.contains("do not follow from its facts"), "{error}");
}

/// A calibration of R03's write watch is a bad sample of the driver, never acceptance evidence.
#[test]
fn a_calibration_run_is_not_recorded() {
    let written = crate::mac_gui::r03_sample_facts(None, &[("StickyMD.app/Info.plist", 0x02)]);
    let evidence = with_scenarios(
        &[r03_row(&written, &|_| {})],
        ",\"calibration\":\"r03-write-restore\"",
    );
    let error = record(&evidence).unwrap_err();
    assert!(
        error.contains("calibration run (r03-write-restore)"),
        "{error}"
    );
    // The driver writes calibration evidence unfinished (review fix round, B1), so a reader that
    // does not know the `calibration` member refuses it as well: here, this reader with that
    // member taken away.
    let as_written = evidence.replacen("\"finished\":true", "\"finished\":false", 1);
    let unaware = as_written.replacen(",\"calibration\":\"r03-write-restore\"", "", 1);
    assert_ne!(unaware, as_written);
    let error = record(&unaware).unwrap_err();
    assert!(error.contains("not finished"), "{error}");
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
    for row in ["W02", "W05", "K02"] {
        assert!(record(&gui_evidence(&[row], "")).is_ok(), "{row}");
    }
    let error = record(&gui_evidence(&["W02"], ",\"tool\":\"mac-latency\"")).unwrap_err();
    assert!(error.contains("tool \"mac-latency\""), "{error}");
    // Review S4: a logout row comes only from mac-logout's evidence, which names its case.
    let error = record(&gui_evidence(&["W12-logout-text"], "")).unwrap_err();
    assert!(
        error.contains("not a row mac-gui or mac-logout runs"),
        "{error}"
    );
    let logout = |case: &str, row: &str| {
        gui_evidence(&[row], "").replacen(
            "\"unix_seconds\":42",
            &format!("\"unix_seconds\":42,\"case\":\"{case}\",\"boot_session\":\"b\""),
            1,
        )
    };
    assert!(record(&logout("text", "W12-logout-text")).is_ok());
    for (case, row) in [
        ("text", "W12-logout-dialog"),
        ("text", "W02"),
        ("other", "W12-logout-other"),
    ] {
        let error = record(&logout(case, row)).unwrap_err();
        assert!(error.contains("not a row"), "{case} {row}: {error}");
    }
}

/// Review B5: resource rows are their own record kind; a line of the other kind naming one (an
/// older GUI-shaped import) is refused, and the importer writes the resource kind.
#[test]
fn resource_rows_are_recorded_only_under_their_own_kind() {
    let records = record(&crate::mac_resources::evidence_json(false)).unwrap();
    let line = records[0].line();
    assert!(line.contains("\"kind\":\"resources\""), "{line}");
    let both = format!("{line}\n{}\n", records[1].line());
    assert_eq!(read_all(&both).unwrap(), records);
    // M5 gate review C2: one evidence is R01 and R02 together; a ledger holding only the first
    // line (cut short or spliced) could let a lost R02 FAIL be waived.
    let error = read_all(&line).unwrap_err();
    assert!(error.contains("both P16-R01 and P16-R02"), "{error}");
    let error =
        read_all(&line.replace("\"kind\":\"resources\"", "\"kind\":\"automated\"")).unwrap_err();
    assert!(error.contains("not one any producer writes"), "{error}");
    let gui = record(&gui_evidence(&["W02"], "")).unwrap();
    let error = read_all(
        &gui[0]
            .line()
            .replace("\"kind\":\"automated\"", "\"kind\":\"resources\""),
    )
    .unwrap_err();
    assert!(error.contains("not one any producer writes"), "{error}");
}

/// Review B4: two spellings of one image digest are one identity, so neither a FAIL nor a system
/// can be split from a PASS by renaming the image.
#[test]
fn one_image_digest_is_one_identity_whatever_its_name() {
    let digest = "AB".repeat(32);
    let named = format!("ghcr.io/x/y@sha256:{digest}");
    let bare = format!("sha256:{}", digest.to_ascii_lowercase());
    let records = |image: &str| {
        from_evidence(gui_evidence(&["W02"], "").as_bytes(), "/e.json", image, 3).unwrap()
    };
    let identity = |records: &[Record]| match &records[0] {
        Record::Automated { identity, .. } => identity.clone(),
        other => panic!("{other:?}"),
    };
    assert_eq!(identity(&records(&named)), identity(&records(&bare)));
    assert_eq!(identity(&records(&named)).image, bare);
    // Read back from a line spelled with the name, the key is the digest.
    let line = records(&named)[0].line().replace(
        &format!("\"image\":\"{bare}\""),
        &format!("\"image\":\"{named}\""),
    );
    assert!(line.contains("ghcr.io/x/y@"));
    assert_eq!(
        identity(&read_all(&line).unwrap()),
        identity(&records(&bare))
    );
    // One digest with two systems is refused whichever spelling each record used.
    let mut both = records(&named);
    let other_system = gui_evidence(&["W03"], "").replace("27.2", "27.3");
    both.extend(from_evidence(other_system.as_bytes(), "/f.json", &bare, 4).unwrap());
    assert!(check_system(&both).is_err());
    // Old lines are read with the same grammar (review round 3, B5): a tag, or a malformed
    // reference that merely ends like a digest, keeps its bytes and names no identity.
    let digest = format!("sha256:{}", hex('e'));
    for (written, key) in [
        (
            format!("ghcr.io/x/y@{}", digest.to_uppercase()),
            Some(digest.as_str()),
        ),
        (digest.clone(), Some(digest.as_str())),
        ("mac-test:latest".to_owned(), None),
        (format!("garbage {digest}"), None),
        (format!("tag:{digest}"), None),
        // Two digests in one reference: neither is the identity (review fix round, B3).
        (format!("repo@sha256:{}@{digest}", hex('a')), None),
        ("sha256:image".to_owned(), None),
    ] {
        let found = super::record::image_key(&written);
        match key {
            Some(key) => assert_eq!(found, key, "{written}"),
            None => {
                assert_eq!(found, written, "{written} keeps its bytes");
                assert!(!super::record::image_attributable(&found), "{written}");
            }
        }
    }
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
        format!("repo@sha256:{}@{digest}", "b".repeat(64)),
        format!("name@sha512:{}", "a".repeat(64)),
    ] {
        assert!(image_digest(&bad).is_err(), "{bad}");
    }
    // A record written with another case of the same image reads as the same identity, and the
    // ledger keeps the bytes it was written with.
    let records = record(&gui_evidence(&["W02"], "")).unwrap();
    let line = records[0].line().replace(
        "\"image\":\"sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee\"",
        "\"image\":\"SHA256:EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE\"",
    );
    assert!(
        line.contains("SHA256:EEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEEE")
    );
    assert_eq!(read_all(&line).unwrap(), records);
}
