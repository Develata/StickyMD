//! Reading a finished `mac-gui`, `mac-logout` or `mac-resources` run's evidence into ledger records.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use super::{Built, Identity, OpenedZip, Record, VERDICTS, is_sha256, optional_text, system};
use crate::release::json::{self, Value};

/// The fixed texts a driver built before gate review D2 (59f38e1) wrote as an `execution` FAIL,
/// in a run it marked finished, for an error before the product was spawned (plan 11
/// #phase-16-macos-host "记录", gate review round 2 R2-1): `ditto` failing or running out of
/// time, the copy's executable or bundle manifest differing from the evidence. Each opens the
/// row's errors (`one` joins later ones after it). Its other errors before the spawn (the session
/// directory, the fixture writes) share their wording with errors after it, so none is named.
const OLD_PRE_LAUNCH: [&str; 4] = [
    "ditto failed: ",
    "ditto exceeded 120 seconds",
    "Copied executable SHA-256 changed",
    "copied bundle manifest changed; refused before launch",
];

/// The fixed text of [`OLD_PRE_LAUNCH`] a check's observation opens with, if any (no driver
/// writes one in a check of a finished run but as that FAIL).
fn old_pre_launch(check: &Value) -> Option<&'static str> {
    let observation = check.field("observation").and_then(Value::string).ok()?;
    OLD_PRE_LAUNCH
        .into_iter()
        .find(|text| observation.starts_with(text))
}

/// The records of a finished `mac-gui` run, one per row, from one read of its bytes (the hash
/// and the parsed verdicts describe the same file). Evidence written before the driver hash was
/// recorded cannot say which driver produced it, so it is refused.
pub(in crate::mac_ledger) fn from_evidence(
    bytes: &[u8],
    evidence: &str,
    image: &str,
    recorded_unix: u64,
) -> Result<Vec<Record>, String> {
    let text = std::str::from_utf8(bytes).map_err(|e| format!("evidence is not UTF-8: {e}"))?;
    let value = json::parse(text)?;
    // Each producer has its own schema and rows: mac-resources writes R01 and R02, mac-gui and
    // mac-logout (no `tool` member) only the rows they run.
    match optional_text(&value, "tool")?.as_deref() {
        Some(crate::mac_ledger::resources::TOOL) => {
            return crate::mac_ledger::resources::from_evidence(
                &value,
                bytes,
                evidence,
                image,
                recorded_unix,
            );
        }
        Some(other) => return Err(format!("evidence of tool {other:?} cannot be recorded")),
        None => {}
    }
    // Schema 2 is the first whose `finished` only the current rules set and that names the batch
    // the run was started for; an older file cannot say either, so it is rerun, not recorded.
    if !matches!(
        value.field("schema_version").and_then(Value::unsigned),
        Ok(2)
    ) {
        return Err("the evidence is not schema 2 of mac-gui (an older driver) nor schema 7 of mac-resources; it cannot be recorded".into());
    }
    // A calibration run (`mac-gui --r03-write-restore`) is a bad sample of the driver itself.
    if let Ok(calibration) = value.field("calibration") {
        return Err(format!(
            "the evidence is a calibration run ({}); it cannot be recorded",
            calibration.string().unwrap_or("of an unknown kind")
        ));
    }
    // The release ZIP R03 opened, kept with R03 alone.
    let zip = optional_text(&value, "zip_sha256")?;
    if let Some(digest) = zip.as_deref().filter(|d| !is_sha256(d)) {
        return Err(format!(
            "the evidence names a ZIP digest that is not a SHA-256: {digest:?}"
        ));
    }
    let batch = optional_text(&value, "batch")?;
    if let Some(name) = batch
        .as_deref()
        .filter(|b| !crate::mac_ledger::options::is_batch(b))
    {
        return Err(format!("the evidence names an invalid batch {name:?}"));
    }
    let field =
        |v: &Value, key: &str| -> Result<String, String> { Ok(v.field(key)?.string()?.to_owned()) };
    let driver = field(&value, "driver_sha256")
        .map_err(|e| format!("evidence names no driver ({e}); it cannot be recorded"))?;
    // Only a run in the dedicated VM is acceptance evidence (plan 11 #phase-16-macos-host).
    let environment = value.field("environment")?;
    if field(environment, "hv_vmm_present")? != "1" {
        return Err("the run was not in a virtual machine; it cannot be recorded".into());
    }
    if !matches!(environment.field("allow_host")?, Value::Bool(false)) {
        return Err("the run allowed a host; it cannot be recorded".into());
    }
    let run_unix = environment.field("unix_seconds")?.unsigned()?;
    // The driver rewrites the evidence after every row and marks it finished only after the last.
    if !matches!(value.field("finished"), Ok(Value::Bool(true))) {
        return Err(
            "the run is not finished (a checkpoint or an older driver); it cannot be recorded"
                .into(),
        );
    }
    // mac-logout's evidence names its case (and boot); mac-gui's names none (review S4).
    let logout_case = optional_text(environment, "case")?;
    let identity = Identity {
        inputs: optional_text(&value, "app_inputs")?,
        driver,
        image: image.to_owned(),
        system: system(&field(value.field("environment")?, "os")?),
    };
    let built = Built {
        executable: field(&value, "app_executable_sha256")?,
        bundle: field(&value, "app_bundle_manifest_sha256")?,
    };
    let evidence_sha256 = crate::integrity::sha256_bytes(bytes)?;
    let mut records: Vec<Record> = Vec::new();
    for scenario in value.field("scenarios")?.array()? {
        let id = field(scenario, "id")?;
        let row = id.strip_prefix("P16-").unwrap_or(&id).to_owned();
        if records
            .iter()
            .any(|r| matches!(r, Record::Automated { row: seen, .. } if *seen == row))
        {
            return Err(format!("P16-{row} appears twice in the evidence"));
        }
        if !crate::mac_ledger::producers::gui_row(&row, logout_case.as_deref()) {
            return Err(format!(
                "P16-{row} is not a row mac-gui or mac-logout runs; it cannot be recorded from this evidence"
            ));
        }
        let verdict = field(scenario, "verdict")?;
        let checks = scenario.field("checks")?.array()?;
        // Whether R03's "nothing written" was read through the write watch (its facts say so).
        let mut write_watch = false;
        if row == "R03" {
            write_watch =
                crate::mac_ledger::producers::r03(scenario, &verdict, checks, zip.as_deref())?;
        } else if scenario.field("facts").is_ok() {
            return Err(format!(
                "P16-{row} records facts, which only R03 does; it cannot be recorded"
            ));
        }
        // A row stopped by the run's shared stop, or never started, means the run was cut short;
        // an older driver could still mark such a run finished, so the rows themselves decide.
        for check in checks {
            let name = field(check, "name")?;
            let unstarted = name == "execution"
                && check
                    .field("observation")
                    .and_then(|o| Ok(o.string()? == "not started"))
                    .unwrap_or(false);
            if name == "shared stop" || unstarted {
                return Err(format!(
                    "P16-{row} was not run (check {name:?}); the run was cut short and cannot be recorded"
                ));
            }
            if name == "execution"
                && let Some(text) = old_pre_launch(check)
            {
                return Err(format!(
                    "P16-{row} failed before the product was spawned ({text:?}), which a driver older than gate review D2 wrote as a FAIL; it is not a product result and cannot be recorded"
                ));
            }
        }
        let checks = checks
            .iter()
            .map(|check| field(check, "verdict"))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(unknown) = checks.iter().find(|c| !VERDICTS.contains(&c.as_str())) {
            return Err(format!(
                "P16-{row} has a check with unknown verdict {unknown:?}"
            ));
        }
        // The driver's aggregation: FAIL first, then NOT_TESTED, then PASS; no check is not a PASS.
        let expected = if checks.iter().any(|c| c == "FAIL") {
            "FAIL"
        } else if checks.is_empty() || checks.iter().any(|c| c != "PASS") {
            "NOT_TESTED"
        } else {
            "PASS"
        };
        if verdict != expected {
            return Err(format!(
                "P16-{row} says {verdict} but its checks give {expected}"
            ));
        }
        let record = Record::Automated {
            zip: zip
                .clone()
                .filter(|_| row == "R03")
                .map(|sha256| OpenedZip {
                    sha256,
                    write_watch,
                }),
            row,
            verdict,
            identity: identity.clone(),
            built: built.clone(),
            evidence: evidence.to_owned(),
            evidence_sha256: evidence_sha256.clone(),
            run_unix: Some(run_unix),
            batch: batch.clone(),
            recorded_unix,
        };
        record.validate()?;
        // The ledger stores digests in lower case whatever case the evidence used.
        records.push(record.normalized());
    }
    if records.is_empty() {
        return Err("the evidence has no rows".into());
    }
    if zip.is_some()
        && !records
            .iter()
            .any(|r| matches!(r, Record::Automated { row, .. } if row == "R03"))
    {
        return Err(
            "the evidence names a ZIP but has no R03 row, the only row that opens one".into(),
        );
    }
    Ok(records)
}
