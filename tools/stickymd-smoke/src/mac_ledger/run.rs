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
    refuse_links(root, &ledger)?;
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
fn refuse_links(root: &Path, ledger: &Path) -> Result<(), String> {
    for path in [ledger.to_path_buf(), lock_path(ledger)] {
        match fs::symlink_metadata(&path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(format!(
                    "{} is a symbolic link; name the ledger itself",
                    path.display()
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            // Anything but absence leaves the path unproven; nothing is read or written.
            Err(e) => return Err(format!("cannot inspect {}: {e}", path.display())),
        }
        // The repository the command runs for, not one guessed from the path (review round 3, B6).
        if crate::qualification::reserved_paths::is_macos_source_test_path(root, &path) {
            return Err(format!(
                "{} would occupy the macOS source-test receipt or its lock",
                path.display()
            ));
        }
    }
    Ok(())
}

/// The ledger's text for a reader that must see it whole: links refused, read under a shared
/// hold of the writers' lock (opened read-only, never created), `None` when nothing was ever
/// recorded. A writer at work makes this an error rather than a half-seen ledger. The returned
/// lock keeps writers out until the caller drops it.
pub(crate) fn read_whole(
    root: &Path,
    ledger: &Path,
) -> Result<(Option<String>, Option<fs::File>), String> {
    refuse_links(root, ledger)?;
    let lock = match fs::File::open(lock_path(ledger)) {
        Ok(file) => Some(file),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => return Err(format!("{}: {e}", lock_path(ledger).display())),
    };
    if let Some(file) = &lock {
        file.try_lock_shared()
            .map_err(|e| format!("a mac-ledger writer holds the ledger: {e}"))?;
    }
    match (fs::read_to_string(ledger), lock) {
        (Ok(text), Some(lock)) => Ok((Some(text), Some(lock))),
        // Every append creates the lock first; a ledger without it was not written under it
        // (copied, or a writer starting), so it is not read unlocked (review round 3, B5).
        (Ok(_), None) => Err(format!(
            "{} has no lock beside it; create the empty file {} (or record through mac-ledger) and check again",
            ledger.display(),
            lock_path(ledger).display()
        )),
        (Err(e), lock) if e.kind() == std::io::ErrorKind::NotFound => Ok((None, lock)),
        (Err(e), _) => Err(format!("{}: {e}", ledger.display())),
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
        // Grouped as the ledger's records are: by the image's content digest (review B4).
        image: record::image_key(options.image.as_deref().ok_or("--image missing")?),
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

#[cfg(test)]
mod reading_tests {
    use super::{lock_path, read_whole, refuse_links};

    /// Review round 3, B5: a ledger without its lock is not read; with the lock it is; absent
    /// both, nothing was recorded.
    #[test]
    fn a_ledger_is_read_only_with_its_lock() {
        let scratch = stickymd_package::temporary::TemporaryDirectory::new("ledger-lock").unwrap();
        let root = scratch.path();
        let ledger = root.join("records.jsonl");
        assert!(matches!(read_whole(root, &ledger), Ok((None, None))));
        std::fs::write(&ledger, "").unwrap();
        let error = read_whole(root, &ledger).unwrap_err();
        assert!(error.contains("has no lock"), "{error}");
        std::fs::write(lock_path(&ledger), "").unwrap();
        assert!(matches!(read_whole(root, &ledger), Ok((Some(_), Some(_)))));
    }

    /// Review round 3, B6: the destination is checked against the command's own root, and any
    /// error but absence stops it.
    #[test]
    fn a_ledger_cannot_take_the_source_test_lock_or_an_unreadable_path() {
        let scratch =
            stickymd_package::temporary::TemporaryDirectory::new("ledger-reserved").unwrap();
        let root = scratch.path();
        let reserved = root.join(crate::qualification::reserved_paths::MACOS_SOURCE_TEST_LOCK);
        std::fs::create_dir_all(reserved.parent().unwrap()).unwrap();
        let error = refuse_links(root, &reserved).unwrap_err();
        assert!(error.contains("source-test receipt or its lock"), "{error}");
        // A path through a regular file is ENOTDIR on Unix, an error other than absence.
        #[cfg(unix)]
        {
            std::fs::write(root.join("a-file"), "").unwrap();
            let error = refuse_links(root, &root.join("a-file/records.jsonl")).unwrap_err();
            assert!(error.contains("cannot inspect"), "{error}");
        }
        assert!(refuse_links(root, &root.join("records.jsonl")).is_ok());
    }
}
