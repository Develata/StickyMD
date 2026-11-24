//! The commands on the macOS host: hashes of the bundle and driver on disk, the ledger file, and
//! what is printed. An append holds an exclusive lock on a sidecar file (never replaced) across
//! read, check and publish, and rewrites the ledger atomically with the old bytes verbatim.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    append,
    decide::{self, Rows},
    options::{Command, Options},
    record::{self, Built, Identity, Record},
};
use crate::mac_common::fixture::{app_inputs, bundle_hash, contained_executable};
use std::{
    fs,
    path::{Path, PathBuf},
};

const MATRIX: &str = "docs/acceptance-cases/phase-16.md";

fn rows(root: &Path) -> Result<Rows, String> {
    decide::matrix_rows(
        &fs::read_to_string(root.join(MATRIX)).map_err(|e| format!("{MATRIX}: {e}"))?,
    )
}

pub(super) fn execute(options: &Options, root: &Path) -> Result<(), String> {
    let command = options.command.ok_or("mac-ledger needs a command")?;
    let ledger = options.ledger.clone().unwrap_or_else(|| {
        root.join(crate::qualification::reserved_paths::MAC_GUI_LEDGER)
            .join("records.jsonl")
    });
    refuse_links(&ledger)?;
    if command == Command::Plan {
        let existing = record::read_all(&read(&ledger)?)?;
        decide::check_system(&existing)?;
        return plan(options, &existing, &rows(root)?);
    }
    let _lock = lock(&ledger)?;
    let text = read(&ledger)?;
    let existing = record::read_all(&text)?;
    let new = match command {
        Command::Record => append::recorded(options)?,
        Command::Receipt => append::receipts(options, &rows(root)?, root)?,
        Command::Waive => append::waivers(options, root)?,
        Command::Revoke => append::revocation(options, &existing, root)?,
        Command::Plan => unreachable!("handled above"),
    };
    let mut all: Vec<Record> = existing;
    for record in &new {
        record::consistent_with(&all, record)?;
        all.push(record.clone());
    }
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
    if command == Command::Revoke {
        println!(
            "if docs/acceptance-cases/phase-16.md cites the withdrawn result, change that row to NOT TESTED in the same commit"
        );
    }
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
/// The ledger and its lock are read, locked and replaced by name: through a link the lock would
/// name the link and a publish would replace the link, not the ledger, and a reader could follow
/// it to another file. Every entry point refuses them. Nor may the ledger occupy the release
/// gate's own files.
fn refuse_links(ledger: &Path) -> Result<(), String> {
    for path in [ledger.to_path_buf(), lock_path(ledger)] {
        if fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
            return Err(format!(
                "{} is a symbolic link; name the ledger itself",
                path.display()
            ));
        }
    }
    let name = ledger.file_name().and_then(|name| name.to_str());
    if name.is_some_and(|name| name.starts_with("release-") || name.starts_with(".release-")) {
        return Err(format!(
            "{} would occupy a release evidence file",
            ledger.display()
        ));
    }
    Ok(())
}

/// The ledger's text for a reader that must see it whole: links refused, read under a shared
/// hold of the writers' lock (opened read-only, never created), `None` when nothing was ever
/// recorded. A writer at work makes this an error rather than a half-seen ledger. The returned
/// lock keeps writers out until the caller drops it.
pub(crate) fn read_whole(ledger: &Path) -> Result<(Option<String>, Option<fs::File>), String> {
    refuse_links(ledger)?;
    let lock = match fs::File::open(lock_path(ledger)) {
        Ok(file) => Some(file),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{}: {e}", lock_path(ledger).display())),
    };
    if let Some(file) = &lock {
        file.try_lock_shared()
            .map_err(|e| format!("a mac-ledger writer holds the ledger: {e}"))?;
    }
    match fs::read_to_string(ledger) {
        Ok(text) => Ok((Some(text), lock)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok((None, lock)),
        Err(e) => Err(format!("{}: {e}", ledger.display())),
    }
}

fn lock_path(ledger: &Path) -> PathBuf {
    let mut path = ledger.as_os_str().to_owned();
    path.push(".lock");
    PathBuf::from(path)
}

fn lock(ledger: &Path) -> Result<fs::File, String> {
    let path = lock_path(ledger);
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

fn built(app: &Path) -> Result<Built, String> {
    let app = app.canonicalize().map_err(|e| format!("app path: {e}"))?;
    Ok(Built {
        executable: crate::integrity::sha256(&contained_executable(&app)?)?,
        bundle: bundle_hash(&app)?,
    })
}

fn plan(options: &Options, existing: &[Record], rows: &Rows) -> Result<(), String> {
    let app = options.app.as_deref().ok_or("--app missing")?;
    let driver = options.driver.as_deref().ok_or("--driver missing")?;
    let system_file = options.system.as_deref().ok_or("--system missing")?;
    let built = built(app)?;
    let identity = Identity {
        inputs: app_inputs(&app.canonicalize().map_err(|e| format!("app path: {e}"))?)?,
        driver: crate::integrity::sha256(driver)?,
        image: options.image.clone().ok_or("--image missing")?,
        system: record::system(
            &fs::read_to_string(system_file)
                .map_err(|e| format!("{}: {e}", system_file.display()))?,
        ),
    };
    // W12's real logout runs one case per fresh VM boot through `mac-logout`; each case is a row
    // of its own, and W12 counts only when they all pass with mac-gui's W12.
    let logout = crate::mac_gui::logout::rows();
    let mut executable = crate::mac_gui::executable_ids();
    executable.extend(logout.iter().map(String::as_str));
    let batch = options.batch.as_deref();
    let plan = decide::plan(
        existing,
        &executable,
        &rows.manual,
        &identity,
        &built,
        batch,
    );
    println!(
        "app inputs {}\napp executable {} (audit)\ndriver {}\nimage {}\nsystem {}\nbatch {}",
        identity.inputs.as_deref().unwrap_or(
            "none: this bundle was built without its input fingerprint; rebuild it with build-app.sh to reuse anything"
        ),
        built.executable,
        identity.driver,
        identity.image,
        identity.system,
        batch.unwrap_or("none")
    );
    for reuse in &plan.reused {
        println!(
            "reuse P16-{}{}: PASS in {} (sha256 {})",
            reuse.row,
            // W12's own PASS is only its mac-gui part; the row's verdict is the composite.
            if reuse.row == decide::composite().0 {
                " (mac-gui part; the row counts only with the composite below)"
            } else {
                ""
            },
            reuse.evidence,
            reuse.evidence_sha256
        );
        if reuse.other_bytes {
            println!(
                "  audit P16-{}: that PASS was recorded for other app bytes built from the same inputs",
                reuse.row
            );
        }
    }
    for (row, reason) in &plan.rerun {
        println!("rerun P16-{row}: {reason}");
    }
    let (logout_runs, gui_runs): (Vec<&String>, Vec<&String>) =
        plan.run.iter().partition(|row| logout.contains(row));
    if plan.run.is_empty() {
        println!("run: nothing; every executable row has a PASS for this identity");
    }
    let gui: Vec<&str> = gui_runs.iter().map(|r| r.as_str()).collect();
    let cases: Vec<&str> = logout_runs.iter().map(|r| r.as_str()).collect();
    let commands = decide::commands(&gui, &cases, batch);
    let (parent, parts) = decide::composite();
    match decide::complete(&plan, parent, &parts) {
        Ok(()) => println!(
            "composite P16-{parent}: complete; {parent} and {} all have a reusable PASS",
            parts.join(", ")
        ),
        Err(missing) => println!(
            "composite P16-{parent}: incomplete; still needs {}",
            missing.join(", ")
        ),
    }
    for command in commands {
        println!("{command}");
    }
    for (row, environment) in &plan.confirmed {
        println!("manual P16-{row}: USER receipt for these inputs ({environment})");
    }
    if !plan.unconfirmed.is_empty() {
        println!(
            "manual rows needing the USER for these inputs: {} ({} count only receipts of the batch)",
            plan.unconfirmed.join(","),
            decide::NEVER_REUSED.join(",")
        );
    }
    if !rows.review.is_empty() {
        println!(
            "review rows not yet judged (receipts naming the reviewed media are not recorded yet): {}",
            rows.review.join(",")
        );
    }
    if !rows.agent.is_empty() {
        println!(
            "agent rows not yet judged (judge records are not kept yet): {}",
            rows.agent.join(",")
        );
    }
    Ok(())
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
        // Other tests fork children concurrently; one forked while the lock was open shares it
        // until its exec closes the descriptor, so the release may lag briefly.
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while super::lock(&ledger).is_err() {
            assert!(
                std::time::Instant::now() < deadline,
                "the lock was not released"
            );
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
    }
}
