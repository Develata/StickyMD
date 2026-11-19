//! The commands on the macOS host: hashes of the bundle and driver on disk, the ledger file, and
//! what is printed. An append holds an exclusive lock on a sidecar file (never replaced) across
//! read, check and publish, and rewrites the ledger atomically with the old bytes verbatim.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::{
    append, build,
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
    if command == Command::Inputs {
        println!("{}", inputs(root, options.home.as_deref())?);
        return Ok(());
    }
    let ledger = options.ledger.clone().unwrap_or_else(|| {
        root.join(crate::qualification::reserved_paths::MAC_GUI_LEDGER)
            .join("records.jsonl")
    });
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
        Command::Receipt => append::receipts(options, &rows(root)?)?,
        Command::Revoke => append::revocation(options, &existing)?,
        Command::Plan | Command::Inputs => unreachable!("handled above"),
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

fn built(app: &Path) -> Result<Built, String> {
    let app = app.canonicalize().map_err(|e| format!("app path: {e}"))?;
    Ok(Built {
        executable: crate::integrity::sha256(&contained_executable(&app)?)?,
        bundle: bundle_hash(&app)?,
    })
}

/// The product input fingerprint of this checkout as `build-app.sh` builds it: the tracked inputs
/// of the macOS app, then the build identity (the Cargo and Rust toolchain, the Xcode tools). A
/// variable or a Cargo configuration outside the repository that could change the build
/// unrecorded is refused; `build-app.sh` runs this with a clean environment.
fn inputs(root: &Path, home: Option<&Path>) -> Result<String, String> {
    let refused = build::refused(std::env::vars());
    if !refused.is_empty() {
        return Err(format!(
            "unset {}: they change the app's build but are not part of its input fingerprint",
            refused.join(", ")
        ));
    }
    // Both streams: `ld -v` reports its version on stderr.
    let output = |program: &str, args: &[&str]| -> Result<String, String> {
        let output = std::process::Command::new(program)
            .args(args)
            .current_dir(root)
            .output()
            .map_err(|e| format!("{program}: {e}"))?;
        if !output.status.success() {
            return Err(format!("{program} {} failed", args.join(" ")));
        }
        let mut text = String::from_utf8(output.stdout).map_err(|e| format!("{program}: {e}"))?;
        text.push_str(&String::from_utf8(output.stderr).map_err(|e| format!("{program}: {e}"))?);
        Ok(text)
    };
    let xcrun = "/usr/bin/xcrun";
    // A home given by the caller wins over this process's environment, which a Cargo
    // configuration's `[env]` can rewrite before this program starts.
    let cargo_home = match home {
        Some(home) => home.join(".cargo"),
        None => build::cargo_home(
            std::env::var_os("CARGO_HOME"),
            std::env::var_os("HOME"),
            &std::env::current_dir().map_err(|e| format!("current directory: {e}"))?,
        )
        .ok_or("neither CARGO_HOME nor a non-empty HOME is set")?,
    };
    let outside = build::outside_cargo_configs(root, &cargo_home);
    if !outside.is_empty() {
        return Err(format!(
            "remove or move {}: Cargo configuration outside the repository can change the build but is not part of its input fingerprint",
            outside
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let tools = [
        ("cargo -V", output("cargo", &["-V"])?),
        ("rustc -vV", output("rustc", &["-vV"])?),
        ("xcode-select -p", output("/usr/bin/xcode-select", &["-p"])?),
        ("sdk path", output(xcrun, &["--show-sdk-path"])?),
        ("sdk version", output(xcrun, &["--show-sdk-version"])?),
        ("clang --version", output(xcrun, &["clang", "--version"])?),
        ("ld -v", output(xcrun, &["ld", "-v"])?),
    ];
    crate::qualification::mac_app_inputs(root, &build::identity(&tools))
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
