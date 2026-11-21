//! Bundle copy and fixture writes happen only before the product child is started.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::session::Session;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
pub(crate) use stickymd_package::macos::bundle::{app_inputs, bundle_hash, contained_executable};
pub(crate) fn copy_fixture(
    session: &mut Session,
    app: &Path,
    note: &str,
    config: &str,
    hash: &str,
) -> Result<PathBuf, String> {
    let copied = session.path().join("StickyMD.app");
    session.spawn(
        Command::new("/usr/bin/ditto")
            .arg(app)
            .arg(&copied)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit()),
    )?;
    let status = session
        .wait_until(Instant::now() + Duration::from_secs(120))?
        .ok_or("ditto exceeded 120 seconds")?;
    session.stop()?;
    if !status.success() {
        return Err(format!("ditto failed: {status}"));
    }
    let executable = contained_executable(&copied)?;
    if crate::integrity::sha256(&executable)? != hash {
        return Err("Copied executable SHA-256 changed".into());
    }
    crate::atomic_evidence::write(&session.path().join("note/note.md"), note.as_bytes())?;
    crate::atomic_evidence::write(&session.path().join("note/config.toml"), config.as_bytes())?;
    Ok(executable)
}
