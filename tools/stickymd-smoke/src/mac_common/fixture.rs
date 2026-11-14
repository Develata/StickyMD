//! Bundle copy and fixture writes happen only before the product child is started.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use super::session::Session;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
pub(crate) const EXECUTABLE: &str = "Contents/MacOS/StickyMD";
/// Deterministic content manifest; paths are length-prefixed, symlinks are not followed.
/// Includes Info.plist/resources/signature bytes, but deliberately not quarantine/xattrs.
pub(crate) fn bundle_hash(app: &Path) -> Result<String, String> {
    fn walk(
        root: &Path,
        directory: &Path,
        entries: &mut Vec<(Vec<u8>, Vec<u8>)>,
    ) -> Result<(), String> {
        use std::os::unix::ffi::OsStrExt;
        for entry in std::fs::read_dir(directory).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let meta = std::fs::symlink_metadata(&path).map_err(|e| e.to_string())?;
            let name = path
                .strip_prefix(root)
                .map_err(|e| e.to_string())?
                .as_os_str()
                .as_bytes()
                .to_vec();
            let value = if meta.file_type().is_symlink() {
                let mut value = b"symlink:".to_vec();
                value.extend_from_slice(
                    std::fs::read_link(&path)
                        .map_err(|e| e.to_string())?
                        .as_os_str()
                        .as_bytes(),
                );
                value
            } else if meta.is_dir() {
                walk(root, &path, entries)?;
                b"directory".to_vec()
            } else if meta.is_file() {
                format!("file:{}", crate::integrity::sha256(&path)?).into_bytes()
            } else {
                return Err(format!("unsupported bundle entry: {}", path.display()));
            };
            entries.push((name, value));
        }
        Ok(())
    }
    let mut entries = Vec::new();
    walk(app, app, &mut entries)?;
    entries.sort_by(|a, b| a.0.cmp(&b.0));
    let mut manifest = Vec::new();
    for (name, value) in entries {
        manifest.extend_from_slice(&(name.len() as u64).to_le_bytes());
        manifest.extend(name);
        manifest.extend_from_slice(&(value.len() as u64).to_le_bytes());
        manifest.extend(value);
    }
    crate::integrity::sha256_bytes(&manifest)
}
pub(crate) fn contained_executable(app: &Path) -> Result<PathBuf, String> {
    let executable = app
        .join(EXECUTABLE)
        .canonicalize()
        .map_err(|e| format!("bundle executable: {e}"))?;
    if !executable.starts_with(app) || !executable.is_file() {
        return Err("Bundle executable must be a file inside the bundle".into());
    }
    Ok(executable)
}
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

#[cfg(test)]
mod tests {
    #[test]
    fn manifest_covers_metadata_and_symlinks_without_following_them() {
        let mut session = super::Session::new().unwrap();
        let root = session.path().join("bundle");
        std::fs::create_dir(&root).unwrap();
        crate::atomic_evidence::write(&root.join("Info.plist"), b"UIElement").unwrap();
        std::os::unix::fs::symlink("../missing", root.join("link")).unwrap();
        let before = super::bundle_hash(&root).unwrap();
        assert_eq!(before, super::bundle_hash(&root).unwrap());
        crate::atomic_evidence::write(&root.join("Info.plist"), b"Regular").unwrap();
        assert_ne!(before, super::bundle_hash(&root).unwrap());
        session.cleanup().unwrap();
    }
}
