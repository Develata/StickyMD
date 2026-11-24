//! Where `mac-gui`, `mac-logout` and `mac-resources` may write their evidence (review F6, #24
//! B6): never inside the bundle under test, the note directory a portable bundle uses beside
//! itself, the installed version's Note Roots (`~/Library/Application Support/StickyMD/`, plan
//! 05 #note-root, whichever key), or the mac-gui ledger, which only `mac-ledger` writes (plan 11
//! #phase-16-macos-host). Both sides are resolved through their existing ancestors (links
//! included), and paths are compared without regard to case, as a case-insensitive volume does;
//! refusing more on a case-sensitive one is the safe direction. The drivers run in the VM,
//! outside any checkout, so the ledger is recognised by its path's last components.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use std::path::{Component, Path, PathBuf};

/// `path` made absolute with its existing part resolved (links and all), without `..`.
pub(crate) fn resolve(path: &Path) -> Result<PathBuf, String> {
    let absolute = std::path::absolute(path).map_err(|e| e.to_string())?;
    if absolute.components().any(|c| c == Component::ParentDir) {
        return Err("--out must not contain '..'".into());
    }
    let mut ancestor = absolute.as_path();
    let mut missing = Vec::new();
    while !ancestor.exists() {
        missing.push(
            ancestor
                .file_name()
                .ok_or("invalid output path")?
                .to_owned(),
        );
        ancestor = ancestor.parent().ok_or("output has no existing ancestor")?;
    }
    let mut resolved = ancestor.canonicalize().map_err(|e| e.to_string())?;
    for name in missing.into_iter().rev() {
        resolved.push(name);
    }
    Ok(resolved)
}

/// A path's components, lowercased, for a comparison that ignores case.
fn folded(path: &Path) -> Vec<String> {
    path.components()
        .map(|c| c.as_os_str().to_string_lossy().to_lowercase())
        .collect()
}

/// Whether `out` lies in `protected` (both resolved), ignoring case.
fn within(out: &Path, protected: &Path) -> bool {
    folded(out).starts_with(&folded(protected))
}

/// Whether `out` (resolved) lies in a directory named like the mac-gui ledger's, ignoring case.
fn in_ledger(out: &Path) -> bool {
    let ledger = folded(Path::new(
        crate::qualification::reserved_paths::MAC_GUI_LEDGER,
    ));
    out.ancestors()
        .any(|ancestor| folded(ancestor).ends_with(&ledger))
}

/// The evidence directory for `out`, refused when it reaches the canonical bundle `app`, a Note
/// Directory the product may use for it, or the mac-gui ledger.
pub(crate) fn guarded(out: &Path, app: &Path) -> Result<PathBuf, String> {
    let home = std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .ok_or("HOME is unset; the installed version's Note Roots cannot be protected")?;
    guarded_in(out, app, Path::new(&home))
}

fn guarded_in(out: &Path, app: &Path, home: &Path) -> Result<PathBuf, String> {
    let out = resolve(out)?;
    let protected = [
        (app.to_path_buf(), "the input bundle"),
        (
            app.parent().ok_or("bundle has no parent")?.join("note"),
            "the portable note directory",
        ),
        (
            home.join("Library/Application Support/StickyMD"),
            "the installed version's Note Roots",
        ),
    ];
    for (path, what) in protected {
        if within(&out, &resolve(&path)?) {
            return Err(format!(
                "evidence cannot be inside {what} ({})",
                path.display()
            ));
        }
    }
    if in_ledger(&out) {
        return Err(
            "evidence cannot be inside the mac-gui ledger; only mac-ledger writes there".into(),
        );
    }
    Ok(out)
}

/// Readies the checked `out` for one run (review round 3, B2): creates it on a first run, and
/// refuses it when an input lies inside it or when it already holds a file this tool may write
/// (`taken`, asked with the name lowercased, as a case-insensitive volume compares names): an
/// earlier run's evidence, or any file named like one, such as an input placed there. Each file
/// is then created exclusively, so nothing in `out` is replaced but this run's own checkpoint.
pub(crate) fn prepare(
    out: &Path,
    taken: &dyn Fn(&str) -> bool,
    inputs: &[&Path],
) -> Result<(), String> {
    for input in inputs {
        if within(&resolve(input)?, out) {
            return Err(format!(
                "{} lies inside the evidence directory {}; give each run an --out of its own",
                input.display(),
                out.display()
            ));
        }
    }
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", out.display()))?;
    for entry in std::fs::read_dir(out).map_err(|e| format!("{}: {e}", out.display()))? {
        let name = entry.map_err(|e| e.to_string())?.file_name();
        let name = name.to_string_lossy();
        if taken(&name.to_lowercase()) {
            return Err(format!(
                "{} already holds {name}; each run needs an --out of its own",
                out.display()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Review round 3, B2: a file a tool may write, left alone in --out by an earlier run (F01's
    /// screenshot, a logout log), or an input there, is never replaced: the run is refused and
    /// the bytes stay. A first run creates its directory.
    #[test]
    fn an_out_holding_any_file_the_run_may_write_or_an_input_is_refused() {
        let session = crate::mac_common::session::Session::new().unwrap();
        let root = session.path();
        let gui = crate::mac_gui::written_here;
        let logout = |name: &str| name == "mac-gui.json" || name.starts_with("w12-logout-");
        for (name, taken) in [
            ("F01.ppm", &gui as &dyn Fn(&str) -> bool),
            ("W02-stderr.log", &gui),
            ("R03-system.log", &gui),
            ("MAC-GUI.JSON", &gui),
            ("W12-logout-normal-system.log", &logout),
        ] {
            let out = root.join(format!("out-{name}"));
            std::fs::create_dir(&out).unwrap();
            std::fs::write(out.join(name), b"earlier").unwrap();
            let error = prepare(&out, taken, &[]).unwrap_err();
            assert!(error.contains("already holds"), "{name}: {error}");
            assert_eq!(std::fs::read(out.join(name)).unwrap(), b"earlier");
        }
        // The --zip input inside --out, under whatever name.
        let out = root.join("with-input");
        std::fs::create_dir(&out).unwrap();
        let zip = out.join("candidate.zip");
        std::fs::write(&zip, b"zip").unwrap();
        let error = prepare(&out, &gui, &[&zip]).unwrap_err();
        assert!(
            error.contains("lies inside the evidence directory"),
            "{error}"
        );
        assert_eq!(std::fs::read(&zip).unwrap(), b"zip");
        // A first run: the directory does not exist yet, and is made.
        let fresh = root.join("first/run");
        prepare(&fresh, &gui, &[]).unwrap();
        assert!(fresh.is_dir());
        // Files of no tool's are left alone and do not refuse the run.
        std::fs::write(fresh.join("notes.txt"), b"mine").unwrap();
        prepare(&fresh, &gui, &[]).unwrap();
    }

    #[test]
    fn the_bundle_its_note_directory_and_the_ledger_are_refused() {
        let session = crate::mac_common::session::Session::new().unwrap();
        let root = session.path();
        let app = root.join("StickyMD.app");
        std::fs::create_dir_all(&app).unwrap();
        for bad in [
            app.join("Contents/out"),
            root.join("note"),
            root.join("note/evidence"),
            root.join("repo/dist/evidence/mac-gui-ledger"),
            root.join("repo/dist/evidence/mac-gui-ledger/run-1"),
        ] {
            assert!(guarded(&bad, &app).is_err(), "{}", bad.display());
        }
        // A link that spells the ledger another way resolves to it.
        let ledger = root.join("repo/dist/evidence/mac-gui-ledger");
        std::fs::create_dir_all(&ledger).unwrap();
        std::os::unix::fs::symlink(&ledger, root.join("alias")).unwrap();
        assert!(guarded(&root.join("alias/run"), &app).is_err());
        for good in [
            root.join("out"),
            root.join("repo/dist/evidence/mac-gui"),
            root.join("repo/dist/evidence/mac-gui-ledger-copy"),
        ] {
            assert!(guarded(&good, &app).is_ok(), "{}", good.display());
        }
        assert!(guarded(&root.join("a/../b"), &app).is_err());
    }

    /// Review B6: the installed Note Root, a protected directory that is itself a link, and a
    /// spelling in another case are all refused.
    #[test]
    fn the_installed_note_root_a_linked_note_and_another_case_are_refused() {
        let session = crate::mac_common::session::Session::new().unwrap();
        let root = session.path();
        let home = root.join("home");
        let installed = home.join("Library/Application Support/StickyMD/StickyMD/note");
        std::fs::create_dir_all(&installed).unwrap();
        let applications = root.join("Applications");
        let app = applications.join("StickyMD.app");
        std::fs::create_dir_all(&app).unwrap();
        for bad in [
            installed.join("out"),
            home.join("Library/Application Support/StickyMD/Other/note/out"),
            home.join("library/application support/stickymd/StickyMD/note/out"),
            root.join("repo/dist/EVIDENCE/Mac-GUI-Ledger/run"),
        ] {
            let error = guarded_in(&bad, &app, &home).unwrap_err();
            assert!(
                error.contains("cannot be inside"),
                "{}: {error}",
                bad.display()
            );
        }
        // A portable bundle whose note directory is a link to elsewhere: the target is protected.
        let elsewhere = root.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        let portable = root.join("portable");
        std::fs::create_dir_all(portable.join("StickyMD.app")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, portable.join("note")).unwrap();
        let error = guarded_in(
            &elsewhere.join("out"),
            &portable.join("StickyMD.app"),
            &home,
        )
        .unwrap_err();
        assert!(error.contains("portable note directory"), "{error}");
        assert!(guarded_in(&root.join("out"), &app, &home).is_ok());
    }
}
