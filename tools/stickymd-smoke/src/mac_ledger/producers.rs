//! Which producer may write which row (plan 11 #phase-16-macos-host): a `mac-gui` or `mac-logout`
//! run only the rows they run, and an R03 PASS only with every check R03 makes — the
//! quarantine preset, translocation, the prompt, the exit and its status, and the observation
//! that nothing was written — plus, when the run opened a candidate ZIP, the check naming that
//! ZIP's digest, all recomputed from the facts R03 recorded. R01 and R02 come only from
//! `mac-resources` evidence (`resources`).
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use crate::release::json::Value;

/// The rows only `mac-resources` writes.
pub(super) const RESOURCE_ROWS: [&str; 2] = ["R01", "R02"];

/// The ledger record kind of an automated result of `row`: resource rows have their own, so a
/// line of the other kind cannot stand for them (review B5).
pub(super) fn kind_of(row: &str) -> &'static str {
    if RESOURCE_ROWS.contains(&row) {
        "resources"
    } else {
        "automated"
    }
}

/// Whether the producer can write `row` (stored without its prefix): `mac-logout`, whose
/// evidence names its `case`, only that case's row; `mac-gui` (no case) only the rows it runs.
pub(super) fn gui_row(row: &str, case: Option<&str>) -> bool {
    match case {
        Some(case) => {
            let row_of_case = format!("W12-logout-{case}");
            row == row_of_case && crate::mac_gui::logout::rows().contains(&row_of_case)
        }
        None => crate::mac_gui::IDS.contains(&row),
    }
}

/// R03 judged again from its facts (review #24 S4): the checks the evidence states must be the
/// ones the driver's rule gives for the facts it recorded — name, verdict and observation
/// alike — and the row's verdict theirs; the facts name the same ZIP as the evidence. A row
/// without facts (a driver older than the write watch) is recorded only when it does not PASS
/// and holds R03's own checks: it can then only keep the row from holding. A row whose only
/// product result (review round 3, B4). Returns whether the facts carried the write watch.
pub(super) fn r03(
    scenario: &Value,
    verdict: &str,
    checks: &[Value],
    zip: Option<&str>,
) -> Result<bool, String> {
    let stated = checks
        .iter()
        .map(|check| {
            let text = |key: &str| check.field(key).and_then(Value::string).map(str::to_owned);
            Ok((text("name")?, text("verdict")?, text("observation")?))
        })
        .collect::<Result<Vec<_>, String>>()?;
    let Ok(facts) = scenario.field("facts") else {
        if verdict == "PASS" {
            return Err("an R03 PASS without its facts is not recorded: its checks cannot be recomputed (a driver older than the write watch; review #24 B9, S4)".into());
        }
        return r03_checks(checks, zip, true).map(|()| false);
    };
    let rejudged = crate::mac_gui::r03_rejudge(facts)?;
    if rejudged.zip.as_deref() != zip {
        return Err(format!(
            "R03's facts name the ZIP {:?}, the evidence {zip:?}",
            rejudged.zip
        ));
    }
    if rejudged.checks != stated {
        return Err(format!(
            "R03's checks do not follow from its facts: recomputed {:?}, stated {stated:?}",
            rejudged.checks
        ));
    }
    let verdicts: Vec<&str> = stated.iter().map(|(_, v, _)| v.as_str()).collect();
    let aggregate = if verdicts.contains(&"FAIL") {
        "FAIL"
    } else if verdicts.is_empty() || verdicts.contains(&"NOT_TESTED") {
        "NOT_TESTED"
    } else {
        "PASS"
    };
    if aggregate != verdict {
        return Err(format!(
            "R03 is stated {verdict} but its checks make it {aggregate}"
        ));
    }
    r03_checks(checks, zip, false)?;
    Ok(rejudged.watched)
}

/// An R03 row holds exactly R03's checks, in order, and with a ZIP the first names it. A row
/// without facts (`legacy`) may name "nothing written" as the snapshot-only driver did.
fn r03_checks(checks: &[Value], zip: Option<&str>, legacy: bool) -> Result<(), String> {
    let mut expected: Vec<&str> = Vec::new();
    if zip.is_some() {
        expected.push(crate::mac_gui::R03_ZIP_CHECK);
    }
    expected.extend(crate::mac_gui::R03_CHECKS);
    let older: Vec<&str> = expected
        .iter()
        .map(|name| {
            if *name == crate::mac_gui::R03_UNWRITTEN {
                crate::mac_gui::R03_SNAPSHOT_ONLY_UNWRITTEN
            } else {
                name
            }
        })
        .collect();
    let names = checks
        .iter()
        .map(|check| check.field("name").and_then(Value::string))
        .collect::<Result<Vec<_>, String>>()?;
    if names != expected && !(legacy && names == older) {
        return Err(format!(
            "an R03 row needs exactly the checks {expected:?}; the evidence has {names:?}"
        ));
    }
    if let Some(digest) = zip {
        let observed = checks[0].field("observation")?.string()?;
        if observed != format!("sha256={digest}") {
            return Err(format!(
                "R03 names the ZIP it opened as {observed:?}, the evidence {digest}"
            ));
        }
    }
    Ok(())
}
