//! R03's download directory before the product starts: the package a browser would leave there,
//! quarantined as the USER's approval leaves it, and the bundle inside it. With `--zip` the
//! package is the candidate ZIP itself, never repacked from the `.app` (plan 11
//! #macos-release-gate); its bundle must be the app under test in both byte identities, checked
//! before any row runs and again on the extracted copy. The check reads one private copy of the
//! ZIP (its digest, its member list, its extraction), so a file replaced meanwhile cannot mix in;
//! the ZIP must hold exactly the release inventory, and every extracted entry must be a regular
//! file or directory before a quarantine attribute is written to the tree, so no link can lead it
//! outside the session. Without `--zip` the `.app` under test is packed here, as Phase 16 batches
//! do, and the evidence names no ZIP.
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
pub(super) use live::{Candidate, check, matches, stage};

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

    /// Whether the bundle at `app` is still `expected` in both identities.
    pub fn matches(app: &Path, expected: &Bytes) -> Result<(), String> {
        same_bundle(expected, &bytes_of(app)?)
    }

    /// The most a release ZIP may expand to; the package is a few megabytes.
    const MAX_EXPANDED_BYTES: u64 = 256 * 1024 * 1024;

    /// Refuses an archive with a member that is not a plain file or folder, or that expands past
    /// [`MAX_EXPANDED_BYTES`], from its listing before anything is extracted (review #24 S2).
    fn members_are_plain(zip: &Path) -> Result<(), String> {
        let listing = run("/usr/bin/tar", &["-tvf", &text(zip)])?;
        let mut expanded: u64 = 0;
        for line in listing.lines().filter(|line| !line.is_empty()) {
            if !matches!(line.chars().next(), Some('-' | 'd')) {
                return Err(format!(
                    "the ZIP holds a link or special member, refused before extracting: {line}"
                ));
            }
            let size: u64 = line
                .split_whitespace()
                .nth(4)
                .and_then(|size| size.parse().ok())
                .ok_or_else(|| format!("the ZIP listing has no size: {line}"))?;
            expanded = expanded.saturating_add(size);
        }
        if expanded > MAX_EXPANDED_BYTES {
            return Err(format!("the ZIP expands to {expanded} bytes"));
        }
        Ok(())
    }

    /// Refuses a tree holding anything but regular files and directories; links are not
    /// followed.
    fn regular_tree(directory: &Path) -> Result<(), String> {
        for entry in fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let kind = fs::symlink_metadata(&path)
                .map_err(|e| format!("{}: {e}", path.display()))?
                .file_type();
            if kind.is_dir() {
                regular_tree(&path)?;
            } else if !kind.is_file() {
                return Err(format!(
                    "{} is a link or a special file; the release ZIP holds only files and folders",
                    path.display()
                ));
            }
        }
        Ok(())
    }

    /// Checks before any row runs that `zip` holds the app under test in the release layout
    /// (`StickyMD/StickyMD.app`), from one private copy in `scratch`, which must be empty: its
    /// digest, its member list (exactly the release inventory, before anything is extracted)
    /// and its extraction (only files and folders) all describe the same bytes.
    pub fn check(zip: &Path, expected: &Bytes, scratch: &Path) -> Result<Candidate, String> {
        let path = zip.canonicalize().map_err(|e| format!("--zip: {e}"))?;
        if !path.is_file() {
            return Err("--zip must name a ZIP file".into());
        }
        let snapshot = scratch.join("candidate.zip");
        fs::copy(&path, &snapshot).map_err(|e| format!("copy --zip: {e}"))?;
        let sha256 = crate::integrity::sha256(&snapshot)?;
        stickymd_package::macos::listing::check_archive(&snapshot).map_err(|e| {
            format!("--zip is not a StickyMD release ZIP (plan 11 #macos-release-artifact): {e}")
        })?;
        members_are_plain(&snapshot)?;
        let extracted = scratch.join("extracted");
        run(
            "/usr/bin/ditto",
            &["-x", "-k", &text(&snapshot), &text(&extracted)],
        )?;
        regular_tree(&extracted)?;
        same_bundle(expected, &bytes_of(&extracted.join(APP))?)?;
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
                // The bytes are the ones checked; their members are checked again where they are
                // extracted.
                members_are_plain(&archive)?;
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
        if !extracted.is_dir() {
            return Err(format!("no bundle at {}", extracted.display()));
        }
        // The attribute is written to the tree only after it is known to hold no link, and `-s`
        // acts on a link itself in any case. The root itself must be a folder too.
        let root = fs::symlink_metadata(&top).map_err(|e| format!("{}: {e}", top.display()))?;
        if !root.file_type().is_dir() {
            return Err(format!("{} is not a folder", top.display()));
        }
        regular_tree(&top)?;
        run(
            "/usr/bin/xattr",
            &[
                "-w",
                "-r",
                "-s",
                "com.apple.quarantine",
                quarantine,
                &text(&top),
            ],
        )?;
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

        /// A bundle with the release inventory's three files, enough for both identities.
        fn bundle(at: &Path, executable: &[u8]) {
            fs::create_dir_all(at.join("Contents/MacOS")).unwrap();
            fs::create_dir_all(at.join("Contents/_CodeSignature")).unwrap();
            fs::write(at.join("Contents/MacOS/StickyMD"), executable).unwrap();
            fs::write(at.join("Contents/Info.plist"), b"<plist/>").unwrap();
            fs::write(at.join("Contents/_CodeSignature/CodeResources"), b"seal").unwrap();
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
            // The rest of the release inventory beside the bundle.
            for (name, _) in stickymd_package::macos::content::FILES {
                let file = work.join(name);
                fs::create_dir_all(file.parent().unwrap()).unwrap();
                fs::write(file, name.as_bytes()).unwrap();
            }
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

        /// Review F4: a link in the ZIP, under an inventory name, could lead the quarantine
        /// attribute to a file outside the session.
        #[test]
        fn a_zip_holding_a_link_is_refused_before_any_attribute_is_written() {
            let session = Session::new().unwrap();
            let (app, expected) = fixture(&session);
            let outside = session.path().join("outside.txt");
            fs::write(&outside, b"a file of the USER").unwrap();
            let work = session.path().join("work-link");
            release_zip(&session.path().join("first.zip"), &app, &work, None);
            let readme = work.join("StickyMD/README.txt");
            fs::remove_file(&readme).unwrap();
            std::os::unix::fs::symlink(&outside, &readme).unwrap();
            let zip = session.path().join("linked.zip");
            run(
                "/usr/bin/ditto",
                &[
                    "-c",
                    "-k",
                    "--norsrc",
                    "--keepParent",
                    &text(&work.join("StickyMD")),
                    &text(&zip),
                ],
            )
            .unwrap();
            let scratch = session.path().join("scratch-link");
            fs::create_dir(&scratch).unwrap();
            let error = check(&zip, &expected, &scratch).unwrap_err();
            assert!(error.contains("link or special member"), "{error}");
            assert!(
                run(
                    "/usr/bin/xattr",
                    &["-p", "com.apple.quarantine", &text(&outside)]
                )
                .is_err(),
                "the file outside was given a quarantine attribute"
            );
            // Review #24 S5: where R03 actually extracts and quarantines, a linked ZIP whose
            // digest was somehow taken as checked is refused before anything is extracted.
            let forged = Candidate {
                path: zip.clone(),
                sha256: crate::integrity::sha256(&zip).unwrap(),
            };
            let download = session.path().join("download-link");
            let error = stage(&download, &app, &expected, Some(&forged), QUARANTINE).unwrap_err();
            assert!(error.contains("link or special member"), "{error}");
            assert!(!download.join("StickyMD").exists(), "nothing was extracted");
            assert!(
                run(
                    "/usr/bin/xattr",
                    &["-p", "com.apple.quarantine", &text(&outside)]
                )
                .is_err()
            );
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
