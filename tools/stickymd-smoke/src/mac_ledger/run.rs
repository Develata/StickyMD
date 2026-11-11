//! The commands on the macOS host: hashes of the bundle and driver on disk, the ledger file, and
//! what is printed. An append holds an exclusive lock on a sidecar file (never replaced) across
//! read, check and publish, and rewrites the ledger atomically with the old bytes verbatim.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    decide,
    options::{Command, Options},
    record::{self, Identity, Record},
};
use crate::mac_common::fixture::{bundle_hash, contained_executable};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

const MATRIX: &str = "docs/acceptance-cases/phase-16.md";

pub(super) fn execute(options: &Options, root: &Path) -> Result<(), String> {
    let ledger = options.ledger.clone().unwrap_or_else(|| {
        root.join(crate::qualification::reserved_paths::MAC_GUI_LEDGER)
            .join("records.jsonl")
    });
    let manual = decide::manual_rows(
        &fs::read_to_string(root.join(MATRIX)).map_err(|e| format!("{MATRIX}: {e}"))?,
    )?;
    let command = options.command.ok_or("mac-ledger needs a command")?;
    if command == Command::Plan {
        let existing = record::read_all(&read(&ledger)?)?;
        decide::check_system(&existing)?;
        return plan(options, &existing, &manual);
    }
    let _lock = lock(&ledger)?;
    let text = read(&ledger)?;
    let existing = record::read_all(&text)?;
    let new = match command {
        Command::Record => recorded(options)?,
        Command::Receipt => receipts(options, &manual)?,
        Command::Plan => unreachable!("planned above"),
    };
    let all: Vec<Record> = existing.iter().chain(&new).cloned().collect();
    decide::check_system(&all)?;
    let mut out = text;
    if !out.is_empty() && !out.ends_with('\n') {
        out.push('\n');
    }
    for record in &new {
        out.push_str(&record.line());
        out.push('\n');
    }
    crate::atomic_evidence::write(&ledger, out.as_bytes())?;
    println!("recorded {} rows in {}", new.len(), ledger.display());
    Ok(())
}

fn read(ledger: &Path) -> Result<String, String> {
    match fs::read_to_string(ledger) {
        Ok(text) => Ok(text),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(format!("{}: {e}", ledger.display())),
    }
}

/// Another writer holding the lock refuses this one; the lock is released when the file closes,
/// at exit too, so a crash leaves nothing to clean up.
fn lock(ledger: &Path) -> Result<fs::File, String> {
    let mut path = ledger.as_os_str().to_owned();
    path.push(".lock");
    let path = PathBuf::from(path);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.try_lock()
        .map_err(|e| format!("another mac-ledger writer holds {}: {e}", path.display()))?;
    Ok(file)
}

fn app_hashes(app: &Path) -> Result<(String, String), String> {
    let app = app.canonicalize().map_err(|e| format!("app path: {e}"))?;
    Ok((
        crate::integrity::sha256(&contained_executable(&app)?)?,
        bundle_hash(&app)?,
    ))
}

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

fn plan(options: &Options, existing: &[Record], manual: &[String]) -> Result<(), String> {
    let app = options.app.as_deref().ok_or("--app missing")?;
    let driver = options.driver.as_deref().ok_or("--driver missing")?;
    let system_file = options.system.as_deref().ok_or("--system missing")?;
    let (app_executable, app_bundle) = app_hashes(app)?;
    let identity = Identity {
        app_executable,
        app_bundle,
        driver: crate::integrity::sha256(driver)?,
        image: options.image.clone().ok_or("--image missing")?,
        system: record::system(
            &fs::read_to_string(system_file)
                .map_err(|e| format!("{}: {e}", system_file.display()))?,
        ),
    };
    let executable = crate::mac_gui::executable_ids();
    let plan = decide::plan(existing, &executable, manual, &identity);
    println!(
        "app executable {}\ndriver {}\nimage {}\nsystem {}",
        identity.app_executable, identity.driver, identity.image, identity.system
    );
    for reuse in &plan.reused {
        println!(
            "reuse P16-{}: PASS in {} (sha256 {}){}",
            reuse.row,
            reuse.evidence,
            reuse.evidence_sha256,
            if reuse.later_non_pass {
                "; note: a later FAIL or NOT_TESTED was recorded under the same identity"
            } else {
                ""
            }
        );
    }
    if plan.run.is_empty() {
        println!("run: nothing; every executable row has a PASS for this identity");
    } else {
        println!(
            "run: stickymd-smoke mac-gui --app <the same app> --only {}",
            plan.run.join(",")
        );
    }
    for (row, environment) in &plan.confirmed {
        println!("manual P16-{row}: USER receipt for this app ({environment})");
    }
    if !plan.unconfirmed.is_empty() {
        println!(
            "manual rows needing the USER for this app: {} ({} are never carried over)",
            plan.unconfirmed.join(","),
            decide::NEVER_REUSED.join(",")
        );
    }
    Ok(())
}

fn recorded(options: &Options) -> Result<Vec<Record>, String> {
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

fn receipts(options: &Options, manual: &[String]) -> Result<Vec<Record>, String> {
    if let Some(row) = options.rows.iter().find(|r| !manual.contains(r)) {
        return Err(format!(
            "P16-{row} is not a Manual row of the Phase 16 matrix"
        ));
    }
    let (app_executable, _) = app_hashes(options.app.as_deref().ok_or("--app missing")?)?;
    let environment = desktop()?;
    let quote = options.quote.clone().ok_or("--quote missing")?;
    let recorded_unix = now()?;
    let records: Vec<Record> = options
        .rows
        .iter()
        .map(|row| Record::Manual {
            row: row.clone(),
            app_executable: app_executable.clone(),
            environment: environment.clone(),
            quote: quote.clone(),
            recorded_unix,
        })
        .collect();
    for record in &records {
        record.validate()?;
    }
    Ok(records)
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_second_writer_is_refused_until_the_first_lock_closes() {
        let directory = crate::mac_common::session::Session::new().unwrap();
        let ledger = directory.path().join("records.jsonl");
        let first = super::lock(&ledger).unwrap();
        assert!(
            super::lock(&ledger)
                .unwrap_err()
                .contains("another mac-ledger writer")
        );
        drop(first);
        assert!(super::lock(&ledger).is_ok());
    }
}
