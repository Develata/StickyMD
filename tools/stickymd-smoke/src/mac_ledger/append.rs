//! The records an append adds, built from what is on disk: a finished `mac-gui` run, USER
//! receipts, and the USER's revocation of a recorded result. Each is checked against the ledger
//! it joins before the caller publishes it.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    decide::{self, Rows},
    options::Options,
    record::{self, Record, Revoked},
};
use crate::mac_common::fixture::app_inputs;
use std::{
    fs,
    time::{SystemTime, UNIX_EPOCH},
};

fn now() -> Result<u64, String> {
    Ok(SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_secs())
}

/// The machine a receipt is recorded on: this host's system name, version and build. It does
/// not prove the USER tested here; the USER's words in the receipt say what was tested.
fn desktop() -> Result<String, String> {
    let output = std::process::Command::new("/usr/bin/sw_vers")
        .output()
        .map_err(|e| format!("sw_vers: {e}"))?;
    if !output.status.success() {
        return Err("sw_vers failed".into());
    }
    Ok(format!(
        "USER desktop, recorded on {}",
        record::system(&String::from_utf8_lossy(&output.stdout))
    ))
}

/// Every row of a finished run. A run older than one already recorded for the same row and
/// identity is refused: appended after it, it would read as the newer result.
pub(super) fn recorded(options: &Options, existing: &[Record]) -> Result<Vec<Record>, String> {
    let evidence = options.evidence.as_deref().ok_or("--evidence missing")?;
    let evidence = evidence
        .canonicalize()
        .map_err(|e| format!("{}: {e}", evidence.display()))?;
    let bytes = fs::read(&evidence).map_err(|e| format!("{}: {e}", evidence.display()))?;
    let new = record::from_evidence(
        &bytes,
        &evidence.display().to_string(),
        options.image.as_deref().ok_or("--image missing")?,
        options.batch.as_deref(),
        now()?,
    )?;
    for record in &new {
        let Record::Automated {
            row,
            identity,
            run_unix: Some(run_unix),
            ..
        } = record
        else {
            continue;
        };
        let newer = existing.iter().any(|r| {
            matches!(r, Record::Automated { row: r_row, identity: r_identity, run_unix: Some(other), .. }
                if r_row == row && r_identity == identity && other > run_unix)
        });
        if newer {
            return Err(format!(
                "P16-{row}: a later run is already recorded for this identity; record runs in the order they ran"
            ));
        }
    }
    Ok(new)
}

/// Receipts for Manual rows only: a Review receipt must name the media the USER judged and an
/// Agent row needs judge records, and the ledger keeps neither yet; a Windows-only row is judged
/// on Windows.
pub(super) fn receipts(options: &Options, rows: &Rows) -> Result<Vec<Record>, String> {
    for row in &options.rows {
        if rows.manual.contains(row) {
            continue;
        }
        let why = if rows.review.contains(row) {
            "is a Review row; its receipt must name the screenshots or video the USER judged, which mac-ledger does not record yet"
        } else if rows.agent.contains(row) {
            "is an Agent row; agents judge it, not a USER receipt"
        } else if decide::WINDOWS_ONLY.contains(&row.as_str()) {
            "is judged on Windows; its receipt is the Windows regression report"
        } else {
            "is not a Manual row of the Phase 16 matrix"
        };
        return Err(format!("P16-{row} {why}"));
    }
    let app = options.app.as_deref().ok_or("--app missing")?;
    let app = app.canonicalize().map_err(|e| format!("app path: {e}"))?;
    let app_executable =
        crate::integrity::sha256(&crate::mac_common::fixture::contained_executable(&app)?)?;
    let inputs = app_inputs(&app)?
        .ok_or("this bundle carries no input fingerprint; rebuild it with build-app.sh before recording a receipt")?;
    let environment = desktop()?;
    let quote = options.quote.clone().ok_or("--quote missing")?;
    let recorded_unix = now()?;
    let records: Vec<Record> = options
        .rows
        .iter()
        .map(|row| Record::Manual {
            row: row.clone(),
            inputs: Some(inputs.clone()),
            app_executable: app_executable.clone(),
            environment: environment.clone(),
            quote: quote.clone(),
            batch: options.batch.clone(),
            recorded_unix,
        })
        .collect();
    for record in &records {
        record.validate()?;
    }
    Ok(records)
}

/// The USER's withdrawal of one result that the ledger holds: an automated PASS of the row
/// recorded from the named evidence, or the row's receipts for the app's product inputs. The
/// ledger read checks the target again, so a hand-edited revocation cannot name a future result.
pub(super) fn revocation(options: &Options, existing: &[Record]) -> Result<Vec<Record>, String> {
    let row = options.rows.first().ok_or("--rows missing")?.clone();
    let revoked = match (&options.evidence_sha256, &options.app) {
        (Some(digest), None) => Revoked::Evidence(digest.clone()),
        (None, Some(app)) => Revoked::Inputs(
            app_inputs(&app.canonicalize().map_err(|e| format!("app path: {e}"))?)?
                .ok_or("this bundle carries no input fingerprint, so no receipt can name it")?,
        ),
        _ => return Err("revoke needs exactly one of --evidence-sha256 and --app".into()),
    };
    let record = Record::Revocation {
        row,
        revoked,
        quote: options.quote.clone().ok_or("--quote missing")?,
        recorded_unix: now()?,
    };
    record.validate()?;
    record::consistent_with(existing, &record)?;
    Ok(vec![record])
}
