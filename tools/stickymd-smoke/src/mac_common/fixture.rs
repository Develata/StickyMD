//! Bundle copy and fixture writes happen only before the product child is started.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::session::Session;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
pub(crate) use stickymd_package::macos::bundle::{app_inputs, bundle_hash, contained_executable};

/// The bundle the evidence names: the digests taken of `--app` before the copy, which the
/// private copy must still have before the product starts (plan 11 #phase-16-macos-host).
pub(crate) struct Expected<'a> {
    pub executable: &'a str,
    pub bundle: &'a str,
    /// The product input fingerprint, `None` for a bundle built before `build-app.sh` recorded
    /// one; the copy must agree either way.
    pub inputs: Option<&'a str>,
}

/// An error before the product process was spawned: the session directory, the copy, the copy
/// not being the bundle the evidence names, or the fixture writes. It is the machine's, not a
/// product result (plan 11 #phase-16-macos-host, “记录”; gate review D2).
#[derive(Debug)]
pub(crate) struct PreLaunch(pub String);

impl From<PreLaunch> for String {
    fn from(error: PreLaunch) -> Self {
        error.0
    }
}

/// Copies `app` into the session as `StickyMD.app`, checks the copy against `expected` (its
/// executable, bundle manifest and input fingerprint), then writes the fixture note and config.
/// Every error is a [`PreLaunch`]: nothing of the product has run.
pub(crate) fn copy_fixture(
    session: &mut Session,
    app: &Path,
    note: &str,
    config: &str,
    expected: &Expected,
) -> Result<PathBuf, PreLaunch> {
    copy(session, app, note, config, expected).map_err(PreLaunch)
}

fn copy(
    session: &mut Session,
    app: &Path,
    note: &str,
    config: &str,
    expected: &Expected,
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
    if crate::integrity::sha256(&executable)? != expected.executable {
        return Err("Copied executable SHA-256 changed".into());
    }
    // Same executable, other bundle: the copy is not what the evidence names (gate review D1).
    if bundle_hash(&copied)? != expected.bundle
        || app_inputs(&copied)?.as_deref() != expected.inputs
    {
        return Err(
            "the copied bundle's manifest or input fingerprint is not the one the evidence names; refused before launch"
                .into(),
        );
    }
    crate::atomic_evidence::write(&session.path().join("note/note.md"), note.as_bytes())?;
    crate::atomic_evidence::write(&session.path().join("note/config.toml"), config.as_bytes())?;
    Ok(executable)
}

#[cfg(test)]
mod tests {
    use super::{Expected, app_inputs, bundle_hash, contained_executable, copy_fixture};
    use crate::mac_common::session::Session;
    use std::path::Path;

    const PLIST: &str = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<plist version=\"1.0\">\n<dict>\n<key>CFBundleExecutable</key>\n<string>StickyMD</string>\n</dict>\n</plist>\n";

    /// A stand-in bundle: an executable and an Info.plist without an input fingerprint.
    fn bundle(root: &Path) -> std::path::PathBuf {
        let app = root.join("StickyMD.app");
        std::fs::create_dir_all(app.join("Contents/MacOS")).unwrap();
        std::fs::write(app.join("Contents/MacOS/StickyMD"), b"#!/bin/sh\n").unwrap();
        std::fs::write(app.join("Contents/Info.plist"), PLIST).unwrap();
        app
    }

    /// Gate review D1: the digests are taken of `--app`, then the source gains an empty folder
    /// before the copy. The executable is unchanged, the manifest is not: the copy is refused
    /// before launch, as a pre-launch error, and no fixture is written.
    #[test]
    fn a_copy_whose_bundle_differs_from_the_evidence_is_refused_before_launch() {
        let source = Session::new().unwrap();
        let app = bundle(source.path());
        let executable = crate::integrity::sha256(&contained_executable(&app).unwrap()).unwrap();
        let manifest = bundle_hash(&app).unwrap();
        assert_eq!(app_inputs(&app).unwrap(), None);
        let expected = Expected {
            executable: &executable,
            bundle: &manifest,
            inputs: None,
        };
        // The same bundle copies and gets its fixture.
        let mut session = Session::new().unwrap();
        copy_fixture(&mut session, &app, "note", "config", &expected).unwrap();
        assert!(session.path().join("note/note.md").is_file());
        session.cleanup().unwrap();
        // An input fingerprint the copy lacks is refused too.
        let mut session = Session::new().unwrap();
        let fingerprint = "e".repeat(64);
        let error = copy_fixture(
            &mut session,
            &app,
            "note",
            "config",
            &Expected {
                inputs: Some(&fingerprint),
                ..expected
            },
        )
        .unwrap_err();
        assert!(error.0.contains("refused before launch"), "{error:?}");
        assert!(!session.path().join("note").exists());
        session.cleanup().unwrap();
        // One folder more after the digests were taken.
        std::fs::create_dir(app.join("Contents/Resources")).unwrap();
        let mut session = Session::new().unwrap();
        let error = copy_fixture(&mut session, &app, "note", "config", &expected).unwrap_err();
        assert!(error.0.contains("refused before launch"), "{error:?}");
        assert!(!session.path().join("note").exists());
        session.cleanup().unwrap();
        let mut source = source;
        source.cleanup().unwrap();
    }
}
