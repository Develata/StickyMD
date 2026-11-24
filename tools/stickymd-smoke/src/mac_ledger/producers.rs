//! Which producer may write which row (plan 11 #phase-16-macos-host): a `mac-gui` or `mac-logout`
//! run only the rows they run, and an R03 PASS only with every check R03 makes — the
//! quarantine preset, translocation, the prompt, the exit and its status, and the observation
//! that nothing was written — plus, when the run opened a candidate ZIP, the check naming that
//! ZIP's digest. R01 and R02 come only from `mac-resources` evidence (`resources`).
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use crate::release::json::Value;

/// Whether a `mac-gui` or `mac-logout` run can produce `row` (stored without its prefix).
pub(super) fn gui_row(row: &str) -> bool {
    crate::mac_gui::IDS.contains(&row) || crate::mac_gui::logout::rows().iter().any(|r| r == row)
}

/// An R03 PASS holds exactly R03's checks, in order, and with a ZIP the first names it.
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
    Ok(())
}
