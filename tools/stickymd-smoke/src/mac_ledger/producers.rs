//! Which producer may write which row (plan 11 #phase-16-macos-host): a `mac-gui` or `mac-logout`
//! run only the rows they run, and an R03 PASS only with every check R03 makes — the
//! quarantine preset, translocation, the prompt, the exit and its status, and the observation
//! that nothing was written — plus, when the run opened a candidate ZIP, the check naming that
//! ZIP's digest. R01 and R02 come only from `mac-resources` evidence (`resources`).
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

/// An R03 PASS holds exactly R03's checks, in order, and with a ZIP the first names it. Even
/// then it is not recorded while "nothing written" cannot be proven (review #24 B9): the driver
/// writes that check NOT_TESTED, so an R03 PASS is an older driver's or none at all.
pub(super) fn r03_pass(checks: &[Value], zip: Option<&str>) -> Result<(), String> {
    let mut expected: Vec<&str> = Vec::new();
    if zip.is_some() {
        expected.push(crate::mac_gui::R03_ZIP_CHECK);
    }
    expected.extend(crate::mac_gui::R03_CHECKS);
    let names = checks
        .iter()
        .map(|check| check.field("name").and_then(Value::string))
        .collect::<Result<Vec<_>, String>>()?;
    if names != expected {
        return Err(format!(
            "an R03 PASS needs exactly the checks {expected:?}; the evidence has {names:?}"
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
    Err(format!(
        "an R03 PASS is not recorded: {}",
        crate::mac_gui::R03_WRITE_UNPROVEN
    ))
}
