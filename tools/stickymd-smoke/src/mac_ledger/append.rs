//! The records an append adds, built from what is on disk: a finished `mac-gui` run, USER
//! receipts, and the USER's revocation of a recorded result. Each is checked against the ledger
//! it joins before the caller publishes it.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    decide::{self, Rows},
    options::Options,
    record::{self, Record, Revoked},
    release::{DOWNLOAD_ROWS, ReleaseKind, ReleaseRecord},
};
use crate::mac_common::fixture::app_inputs;
use std::{
    fs,
    path::Path,
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

/// Every row of a finished run; the caller checks each against the ledger (`consistent_with`),
/// which refuses the same evidence recorded twice for a row.
pub(super) fn recorded(options: &Options) -> Result<Vec<Record>, String> {
    let evidence = options.evidence.as_deref().ok_or("--evidence missing")?;
    let evidence = evidence
        .canonicalize()
        .map_err(|e| format!("{}: {e}", evidence.display()))?;
    let bytes = fs::read(&evidence).map_err(|e| format!("{}: {e}", evidence.display()))?;
    record::from_evidence(
        &bytes,
        &evidence.display().to_string(),
        options.image.as_deref().ok_or("--image missing")?,
        now()?,
    )
}

/// The promoted candidate for `--release-source`, resolved and checked against R as the gate
/// resolves it (plan 11 #macos-readiness-check); a record names its receipt's digest.
fn candidate(
    options: &Options,
    root: &Path,
) -> Result<crate::release::macos::promote::Resolved, String> {
    let release = options
        .release
        .as_deref()
        .ok_or("--release-source missing")?;
    crate::release::macos::promote::resolve(root, release)
        .map_err(|e| format!("the macOS candidate for {release} does not resolve: {e}"))
}

fn release_record(
    kind: ReleaseKind,
    row: &str,
    candidate: &str,
    environment: &str,
    options: &Options,
    recorded_unix: u64,
) -> Result<Record, String> {
    let record = ReleaseRecord {
        kind,
        row: row.to_owned(),
        candidate: candidate.to_owned(),
        environment: environment.to_owned(),
        quote: options.quote.clone().ok_or("--quote missing")?,
        batch: options.batch.clone(),
        recorded_unix,
    };
    record.validate()?;
    Ok(Record::Release(record))
}

/// Release receipts for R's candidate: a Manual Phase 16 row (checked as `receipts` checks it)
/// or a download path, which carries the candidate's signing and the system it ran on.
fn release_receipts(options: &Options, rows: &Rows, root: &Path) -> Result<Vec<Record>, String> {
    let is_download = |row: &String| DOWNLOAD_ROWS.contains(&row.as_str());
    let downloads = options.rows.iter().any(is_download);
    let system = match (&options.system, downloads) {
        (Some(path), true) => Some(record::system(
            &fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?,
        )),
        (None, true) => return Err("a download-path record needs --system".into()),
        (Some(_), false) => return Err("--system is only for download-path records".into()),
        (None, false) => None,
    };
    let p16: Vec<String> = options
        .rows
        .iter()
        .filter(|row| !is_download(row))
        .cloned()
        .collect();
    manual_rows(&p16, rows)?;
    let resolved = candidate(options, root)?;
    let environment = desktop()?;
    let recorded_unix = now()?;
    options
        .rows
        .iter()
        .map(|row| {
            let kind = match &system {
                Some(system) if is_download(row) => ReleaseKind::Download {
                    signing: resolved.signing.clone(),
                    system: system.clone(),
                },
                _ => ReleaseKind::Receipt,
            };
            release_record(
                kind,
                row,
                &resolved.receipt_sha256,
                &environment,
                options,
                recorded_unix,
            )
        })
        .collect()
}

/// The USER's waivers for R's candidate, only for rows the gate lets the USER waive.
pub(super) fn waivers(options: &Options, root: &Path) -> Result<Vec<Record>, String> {
    if let Some(row) = options
        .rows
        .iter()
        .find(|row| !crate::release::macos::waivable(row))
    {
        return Err(format!(
            "{} cannot be waived for a release (plan 11 #macos-release-gate)",
            record::label(row)
        ));
    }
    let resolved = candidate(options, root)?;
    let environment = desktop()?;
    let recorded_unix = now()?;
    options
        .rows
        .iter()
        .map(|row| {
            release_record(
                ReleaseKind::Waiver,
                row,
                &resolved.receipt_sha256,
                &environment,
                options,
                recorded_unix,
            )
        })
        .collect()
}

/// Every row must be a Manual row: a Review receipt must name the media the USER judged and an
/// Agent row needs judge records, and the ledger keeps neither yet; a Windows-only row is judged
/// on Windows.
fn manual_rows(requested: &[String], rows: &Rows) -> Result<(), String> {
    for row in requested {
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
    Ok(())
}

/// Receipts for Manual rows: for the app's product inputs, or with `--release-source` for R's
/// promoted candidate.
pub(super) fn receipts(options: &Options, rows: &Rows, root: &Path) -> Result<Vec<Record>, String> {
    if options.release.is_some() {
        return release_receipts(options, rows, root);
    }
    manual_rows(&options.rows, rows)?;
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
/// recorded from the named evidence, the row's receipts for the app's product inputs, or the
/// row's release records for R's candidate. The ledger read checks the target again, so a
/// hand-edited revocation cannot name a future result.
pub(super) fn revocation(
    options: &Options,
    existing: &[Record],
    root: &Path,
) -> Result<Vec<Record>, String> {
    let row = options.rows.first().ok_or("--rows missing")?.clone();
    let revoked = match (&options.evidence_sha256, &options.app, &options.release) {
        (Some(digest), None, None) => Revoked::Evidence(digest.clone()),
        (None, Some(app), None) => Revoked::Inputs(
            app_inputs(&app.canonicalize().map_err(|e| format!("app path: {e}"))?)?
                .ok_or("this bundle carries no input fingerprint, so no receipt can name it")?,
        ),
        (None, None, Some(_)) => Revoked::Candidate(candidate(options, root)?.receipt_sha256),
        _ => {
            return Err(
                "revoke needs exactly one of --evidence-sha256, --app and --release-source".into(),
            );
        }
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
