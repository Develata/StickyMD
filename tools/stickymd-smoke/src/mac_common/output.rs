//! Where `mac-gui`, `mac-logout` and `mac-resources` may write their evidence (review F6): never
//! inside the bundle under test or its note directory, and never inside the mac-gui ledger,
//! which only `mac-ledger` writes (plan 11 #phase-16-macos-host). The drivers run in the VM,
//! outside any checkout, so the ledger is recognised by its path's last components rather than
//! by a repository root; aliases are resolved first, so a symlinked spelling is caught too.
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

/// Whether `out` (resolved) lies in a directory named like the mac-gui ledger's.
fn in_ledger(out: &Path) -> bool {
    let ledger = Path::new(crate::qualification::reserved_paths::MAC_GUI_LEDGER);
    out.ancestors().any(|ancestor| ancestor.ends_with(ledger))
}

/// The evidence directory for `out`, refused when it reaches the canonical bundle `app`, its
/// note directory or the mac-gui ledger.
pub(crate) fn guarded(out: &Path, app: &Path) -> Result<PathBuf, String> {
    let out = resolve(out)?;
    let note = app.parent().ok_or("bundle has no parent")?.join("note");
    if out.starts_with(app) || out.starts_with(&note) {
        return Err("evidence cannot be inside the input bundle or its note directory".into());
    }
    if in_ledger(&out) {
        return Err(
            "evidence cannot be inside the mac-gui ledger; only mac-ledger writes there".into(),
        );
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
