//! R03's download directory before the product starts: the package a browser would leave there,
//! quarantined as the USER's approval leaves it, and the bundle inside it. With `--zip` the
//! package is the candidate ZIP itself, never repacked from the `.app` (plan 11
//! #macos-release-gate); its bundle must be the app under test in both byte identities, checked
//! before any row runs and again on the extracted copy. Without `--zip` the `.app` under test is
//! packed here, as Phase 16 batches do, and the evidence names no ZIP.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

/// The two byte identities of a bundle (plan 11 #macos-release-gate binds evidence to both).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Bytes {
    pub executable: String,
    pub bundle: String,
}

/// Whether `found`, the bundle inside a ZIP, is `expected`, the app under test. The same
/// executable in another bundle is another bundle.
pub(super) fn same_bundle(expected: &Bytes, found: &Bytes) -> Result<(), String> {
    let mut differ = Vec::new();
    if found.bundle != expected.bundle {
        differ.push(format!(
            "bundle manifest {} (the app under test has {})",
            found.bundle, expected.bundle
        ));
    }
    if found.executable != expected.executable {
        differ.push(format!(
            "executable {} (the app under test has {})",
            found.executable, expected.executable
        ));
    }
    if differ.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "the ZIP's bundle is not the app under test: {}",
            differ.join("; ")
        ))
    }
}

#[cfg(target_os = "macos")]
pub(super) use live::{Candidate, check, stage};

#[cfg(target_os = "macos")]
mod live {
    use super::{Bytes, same_bundle};
    use crate::mac_common::fixture::{bundle_hash, contained_executable};
    use crate::mac_gui::live::command_within;
    use std::{
        fs,
        path::{Path, PathBuf},
        time::Duration,
    };
    use stickymd_package::macos::content::APP;

    /// A candidate ZIP as checked before any row ran: where it is and the SHA-256 of its bytes.
    #[derive(Clone, Debug)]
    pub struct Candidate {
        pub path: PathBuf,
        pub sha256: String,
    }

    /// The download directory as R03 opens it.
    #[derive(Debug)]
    pub struct Staged {
        /// The quarantined bundle LaunchServices opens.
        pub copy: PathBuf,
        /// The folder holding the bundle, where a portable `note/` would be written.
        pub watched: PathBuf,
        /// The SHA-256 of the ZIP extracted here; `None` when the `.app` was packed here.
        pub zip_sha256: Option<String>,
    }

    fn text(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    /// ditto, xattr and the like on a release-sized archive; 10 s is not always enough in the VM.
    fn run(program: &str, args: &[&str]) -> Result<String, String> {
        command_within(program, args, Duration::from_secs(120))
    }

    fn bytes_of(app: &Path) -> Result<Bytes, String> {
        Ok(Bytes {
            executable: crate::integrity::sha256(&contained_executable(app)?)?,
            bundle: bundle_hash(app)?,
        })
    }

    /// Checks before any row runs that `zip` holds the app under test in the release layout
    /// (`StickyMD/StickyMD.app`), extracting it into `scratch`, which must be empty.
    pub fn check(zip: &Path, expected: &Bytes, scratch: &Path) -> Result<Candidate, String> {
        let path = zip.canonicalize().map_err(|e| format!("--zip: {e}"))?;
        if !path.is_file() {
            return Err("--zip must name a ZIP file".into());
        }
        let sha256 = crate::integrity::sha256(&path)?;
        run(
            "/usr/bin/ditto",
            &["-x", "-k", &text(&path), &text(scratch)],
        )?;
        let app = scratch.join(APP);
        if !app.is_dir() {
            return Err(format!(
                "--zip has no {APP}: not a StickyMD release ZIP (plan 11 #macos-release-artifact)"
            ));
        }
        same_bundle(expected, &bytes_of(&app)?)?;
        Ok(Candidate { path, sha256 })
    }

    /// Lays out `download` (created here): the package, quarantined with `quarantine` as a
    /// browser download approved by the USER is, extracted beside itself, every extracted file
    /// carrying the attribute as Archive Utility leaves it. The extracted bundle must be
    /// `expected` in both identities; a candidate ZIP must still have the bytes it was checked
    /// with.
    pub fn stage(
        download: &Path,
        app: &Path,
        expected: &Bytes,
        zip: Option<&Candidate>,
        quarantine: &str,
    ) -> Result<Staged, String> {
        fs::create_dir(download).map_err(|e| e.to_string())?;
        let download = fs::canonicalize(download).map_err(|e| e.to_string())?;
        let (archive, extracted, top) = match zip {
            Some(candidate) => {
                let name = candidate.path.file_name().ok_or("--zip has no file name")?;
                let archive = download.join(name);
                fs::copy(&candidate.path, &archive).map_err(|e| format!("copy --zip: {e}"))?;
                if crate::integrity::sha256(&archive)? != candidate.sha256 {
                    return Err("the ZIP changed after it was checked; refused before R03".into());
                }
                let top = APP.split('/').next().ok_or("release layout")?;
                (archive, download.join(APP), download.join(top))
            }
            None => {
                let archive = download.join("StickyMD.zip");
                run(
                    "/usr/bin/ditto",
                    &["-c", "-k", "--keepParent", &text(app), &text(&archive)],
                )?;
                let copy = download.join("StickyMD.app");
                (archive, copy.clone(), copy)
            }
        };
        run(
            "/usr/bin/xattr",
            &["-w", "com.apple.quarantine", quarantine, &text(&archive)],
        )?;
        run(
            "/usr/bin/ditto",
            &["-x", "-k", &text(&archive), &text(&download)],
        )?;
        run(
            "/usr/bin/xattr",
            &["-w", "-r", "com.apple.quarantine", quarantine, &text(&top)],
        )?;
        if !extracted.is_dir() {
            return Err(format!("no bundle at {}", extracted.display()));
        }
        same_bundle(expected, &bytes_of(&extracted)?)?;
        Ok(Staged {
            watched: extracted
                .parent()
                .ok_or("the bundle has no parent")?
                .to_path_buf(),
            copy: extracted,
            zip_sha256: zip.map(|candidate| candidate.sha256.clone()),
        })
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use crate::mac_common::session::Session;

        /// A bundle with an executable and a property list, enough for both identities.
        fn bundle(at: &Path, executable: &[u8]) {
            fs::create_dir_all(at.join("Contents/MacOS")).unwrap();
            fs::write(at.join("Contents/MacOS/StickyMD"), executable).unwrap();
            fs::write(at.join("Contents/Info.plist"), b"<plist/>").unwrap();
        }

        /// A file of the bundle to overwrite, and its new bytes.
        type Change = Option<(&'static str, &'static [u8])>;

        /// A ZIP in the release layout around a copy of `app`, with `change` made in it.
        fn release_zip(at: &Path, app: &Path, work: &Path, change: Change) {
            let root = work.join("StickyMD");
            fs::create_dir_all(&root).unwrap();
            run(
                "/usr/bin/ditto",
                &[&text(app), &text(&root.join("StickyMD.app"))],
            )
            .unwrap();
            fs::write(root.join("README.txt"), b"readme").unwrap();
            if let Some((file, bytes)) = change {
                fs::write(root.join("StickyMD.app").join(file), bytes).unwrap();
            }
            run(
                "/usr/bin/ditto",
                &[
                    "-c",
                    "-k",
                    "--norsrc",
                    "--keepParent",
                    &text(&root),
                    &text(at),
                ],
            )
            .unwrap();
        }

        fn fixture(session: &Session) -> (PathBuf, Bytes) {
            let app = session.path().join("StickyMD.app");
            bundle(&app, b"product");
            let expected = bytes_of(&app).unwrap();
            (app, expected)
        }

        const QUARANTINE: &str = "00c3;0;Safari;";

        #[test]
        fn the_candidate_zip_is_used_as_given_and_its_digest_kept() {
            let session = Session::new().unwrap();
            let (app, expected) = fixture(&session);
            let zip = session
                .path()
                .join("StickyMD-0.0.0-macos-arm64-portable.zip");
            release_zip(&zip, &app, &session.path().join("work"), None);
            let scratch = session.path().join("scratch");
            fs::create_dir(&scratch).unwrap();
            let candidate = check(&zip, &expected, &scratch).unwrap();
            assert_eq!(candidate.sha256, crate::integrity::sha256(&zip).unwrap());
            let download = session.path().join("download");
            let staged = stage(&download, &app, &expected, Some(&candidate), QUARANTINE).unwrap();
            let download = fs::canonicalize(&download).unwrap();
            assert_eq!(staged.copy, download.join("StickyMD/StickyMD.app"));
            // The folder a portable note/ would land in, not the top of the download directory.
            assert_eq!(staged.watched, download.join("StickyMD"));
            assert_eq!(
                staged.zip_sha256.as_deref(),
                Some(candidate.sha256.as_str())
            );
            for file in [&staged.copy, &staged.watched.join("README.txt")] {
                let read = run(
                    "/usr/bin/xattr",
                    &["-p", "com.apple.quarantine", &text(file)],
                );
                assert_eq!(read.unwrap().trim(), QUARANTINE, "{}", file.display());
            }
        }

        #[test]
        fn without_a_zip_the_app_is_packed_and_no_zip_is_named() {
            let session = Session::new().unwrap();
            let (app, expected) = fixture(&session);
            let download = session.path().join("download");
            let staged = stage(&download, &app, &expected, None, QUARANTINE).unwrap();
            let download = fs::canonicalize(&download).unwrap();
            assert_eq!(staged.copy, download.join("StickyMD.app"));
            assert_eq!(staged.watched, download);
            assert_eq!(staged.zip_sha256, None);
        }

        /// Bad samples: a ZIP whose bundle differs from `--app` (another executable; the same
        /// executable in another bundle), a ZIP not in the release layout, and a ZIP replaced
        /// after its check.
        #[test]
        fn a_zip_with_another_bundle_or_layout_is_refused() {
            let session = Session::new().unwrap();
            let (app, expected) = fixture(&session);
            let cases: [(&str, Change, &str); 2] = [
                (
                    "other-executable",
                    Some(("Contents/MacOS/StickyMD", b"other")),
                    "executable",
                ),
                (
                    "other-bundle",
                    Some(("Contents/Info.plist", b"<plist>x</plist>")),
                    "bundle manifest",
                ),
            ];
            for (name, change, words) in cases {
                let zip = session.path().join(format!("{name}.zip"));
                release_zip(
                    &zip,
                    &app,
                    &session.path().join(format!("work-{name}")),
                    change,
                );
                let scratch = session.path().join(format!("scratch-{name}"));
                fs::create_dir(&scratch).unwrap();
                let error = check(&zip, &expected, &scratch).unwrap_err();
                assert!(
                    error.contains("not the app under test") && error.contains(words),
                    "{error}"
                );
                if name == "other-bundle" {
                    assert!(!error.contains("executable"), "{error}");
                }
            }
            // The .app packed alone is not the release layout.
            let flat = session.path().join("flat.zip");
            run(
                "/usr/bin/ditto",
                &["-c", "-k", "--keepParent", &text(&app), &text(&flat)],
            )
            .unwrap();
            let scratch = session.path().join("scratch-flat");
            fs::create_dir(&scratch).unwrap();
            assert!(
                check(&flat, &expected, &scratch)
                    .unwrap_err()
                    .contains("release ZIP")
            );
            // Checked, then replaced by other bytes before R03 copied it.
            let zip = session.path().join("replaced.zip");
            release_zip(&zip, &app, &session.path().join("work-replaced"), None);
            let scratch = session.path().join("scratch-replaced");
            fs::create_dir(&scratch).unwrap();
            let candidate = check(&zip, &expected, &scratch).unwrap();
            fs::remove_file(&zip).unwrap();
            release_zip(
                &zip,
                &app,
                &session.path().join("work-again"),
                Some(("Contents/Info.plist", b"y")),
            );
            let error = stage(
                &session.path().join("download"),
                &app,
                &expected,
                Some(&candidate),
                QUARANTINE,
            )
            .unwrap_err();
            assert!(error.contains("changed after it was checked"), "{error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Bytes, same_bundle};

    fn bytes(executable: char, bundle: char) -> Bytes {
        Bytes {
            executable: executable.to_string().repeat(64),
            bundle: bundle.to_string().repeat(64),
        }
    }

    #[test]
    fn the_bundle_in_the_zip_must_match_both_identities() {
        let expected = bytes('a', 'b');
        same_bundle(&expected, &bytes('a', 'b')).unwrap();
        // The same executable in another bundle is another bundle.
        let error = same_bundle(&expected, &bytes('a', 'c')).unwrap_err();
        assert!(
            error.contains("bundle manifest") && !error.contains("executable"),
            "{error}"
        );
        let error = same_bundle(&expected, &bytes('d', 'b')).unwrap_err();
        assert!(
            error.contains("executable") && !error.contains("manifest"),
            "{error}"
        );
        let error = same_bundle(&expected, &bytes('d', 'c')).unwrap_err();
        assert!(
            error.contains("bundle manifest") && error.contains("executable"),
            "{error}"
        );
    }
}
