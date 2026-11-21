//! The identity of a macOS bundle: its deterministic content manifest, the product input
//! fingerprint `build-app.sh` records in its Info.plist, and its executable.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use std::path::{Path, PathBuf};

pub const EXECUTABLE: &str = "Contents/MacOS/StickyMD";
/// Deterministic content manifest; paths are length-prefixed, symlinks are not followed.
/// Includes Info.plist/resources/signature bytes, but deliberately not quarantine/xattrs.
pub fn bundle_hash(app: &Path) -> Result<String, String> {
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
/// The Info.plist key `build-app.sh` writes the product input fingerprint under.
pub const INPUTS_KEY: &str = "StickyMDInputs";

/// The product input fingerprint the bundle was built from (plan 11 #phase-16-macos-host), or
/// `None` for a bundle built before `build-app.sh` recorded it. A key present with anything but
/// one SHA-256 is an error, not an absent fingerprint.
pub fn app_inputs(app: &Path) -> Result<Option<String>, String> {
    let plist = app.join("Contents/Info.plist");
    let bytes = std::fs::read(&plist).map_err(|e| format!("{}: {e}", plist.display()))?;
    inputs_in_plist_bytes(&bytes)
}

/// [`app_inputs`] of a property list given as text.
pub fn inputs_in_plist(text: &str) -> Result<Option<String>, String> {
    inputs_in_plist_bytes(text.as_bytes())
}

/// Every reading below is of one private copy, so the file cannot change between them.
fn inputs_in_plist_bytes(bytes: &[u8]) -> Result<Option<String>, String> {
    let directory = crate::temporary::TemporaryDirectory::new("info-plist")?;
    let path = directory.path().join("Info.plist");
    std::fs::write(&path, bytes).map_err(|e| format!("{}: {e}", path.display()))?;
    let value = plist_inputs(&path);
    directory.close()?;
    value
}

/// The value of `INPUTS_KEY` in the top-level dictionary of the property list at `path`, read the
/// way the system reads it (`plutil`): a commented-out key or one in a nested dictionary is not
/// the bundle's value. A list `plutil` cannot lint, the key element written twice anywhere, or a
/// value that is not one SHA-256 string is an error.
fn plist_inputs(path: &Path) -> Result<Option<String>, String> {
    // Only the form `build-app.sh` writes, UTF-8 XML: in another encoding or the binary form a
    // repeated key could not be seen below, and plutil would pick one of the values.
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !bytes.starts_with(b"<?xml") || std::str::from_utf8(&bytes).is_err() {
        return Err(format!(
            "{} is not an XML property list in UTF-8, the form build-app.sh writes",
            path.display()
        ));
    }
    let key = format!("<key>{INPUTS_KEY}</key>");
    if bytes
        .windows(key.len())
        .filter(|window| *window == key.as_bytes())
        .count()
        > 1
    {
        return Err(format!("Info.plist names {INPUTS_KEY} twice"));
    }
    let plutil = |arguments: &[&str]| {
        std::process::Command::new("/usr/bin/plutil")
            .args(arguments)
            .arg(path)
            .output()
            .map_err(|error| format!("cannot run plutil: {error}"))
    };
    if !plutil(&["-lint", "-s"])?.status.success() {
        return Err(format!("{} is not a valid property list", path.display()));
    }
    // `-n`: no newline after the value, so it is compared exactly as stored.
    let typed = plutil(&[
        "-extract", INPUTS_KEY, "raw", "-expect", "string", "-n", "-o", "-",
    ])?;
    if !typed.status.success() {
        return if plutil(&["-extract", INPUTS_KEY, "raw", "-o", "-"])?
            .status
            .success()
        {
            Err(format!("Info.plist {INPUTS_KEY} is not a string"))
        } else {
            Ok(None)
        };
    }
    let value = String::from_utf8(typed.stdout)
        .map_err(|_| format!("Info.plist {INPUTS_KEY} is not UTF-8"))?;
    if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(Some(value.to_ascii_lowercase()))
    } else {
        Err(format!(
            "Info.plist {INPUTS_KEY} is not a SHA-256: {value:?}"
        ))
    }
}

pub fn contained_executable(app: &Path) -> Result<PathBuf, String> {
    let executable = app
        .join(EXECUTABLE)
        .canonicalize()
        .map_err(|e| format!("bundle executable: {e}"))?;
    if !executable.starts_with(app) || !executable.is_file() {
        return Err("Bundle executable must be a file inside the bundle".into());
    }
    Ok(executable)
}

#[cfg(test)]
mod tests {
    #[test]
    fn manifest_covers_metadata_and_symlinks_without_following_them() {
        let directory = crate::temporary::TemporaryDirectory::new("bundle-hash").unwrap();
        let root = directory.path().join("bundle");
        std::fs::create_dir(&root).unwrap();
        crate::atomic::write(&root.join("Info.plist"), b"UIElement").unwrap();
        std::os::unix::fs::symlink("../missing", root.join("link")).unwrap();
        let before = super::bundle_hash(&root).unwrap();
        assert_eq!(before, super::bundle_hash(&root).unwrap());
        crate::atomic::write(&root.join("Info.plist"), b"Regular").unwrap();
        assert_ne!(before, super::bundle_hash(&root).unwrap());
        directory.close().unwrap();
    }

    #[test]
    fn the_input_fingerprint_is_read_from_the_info_plist_or_absent() {
        let digest = "A".repeat(64);
        let document = |dict: &str| {
            format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE plist PUBLIC \"-//Apple//DTD PLIST 1.0//EN\" \"http://www.apple.com/DTDs/PropertyList-1.0.dtd\">\n<plist version=\"1.0\">\n{dict}\n</plist>\n"
            )
        };
        let plist = |value: &str| {
            document(&format!(
                "<dict>\n\t<key>CFBundleVersion</key>\n\t<string>0.2.0</string>\n\t<key>StickyMDInputs</key>\n\t<string>{value}</string>\n</dict>"
            ))
        };
        assert_eq!(
            super::inputs_in_plist(&plist(&digest)).unwrap(),
            Some("a".repeat(64))
        );
        assert_eq!(
            super::inputs_in_plist(&document(
                "<dict><key>CFBundleVersion</key><string>1</string></dict>"
            ))
            .unwrap(),
            None
        );
        assert!(super::inputs_in_plist(&plist("@INPUTS@")).is_err());
        assert!(super::inputs_in_plist(&plist(&"a".repeat(63))).is_err());
        assert!(
            super::inputs_in_plist(&document("<dict><key>StickyMDInputs</key><true/></dict>"))
                .is_err()
        );
        assert!(super::inputs_in_plist("not a property list").is_err());
        // The value as stored: padding around a digest is not a digest.
        assert!(super::inputs_in_plist(&plist(&format!(" {digest} "))).is_err());
        let twice = document(&format!(
            "<dict><key>StickyMDInputs</key><string>{digest}</string><key>StickyMDInputs</key><string>{digest}</string></dict>"
        ));
        assert!(super::inputs_in_plist(&twice).is_err());
        // A commented-out pair, and the key in a nested dictionary (written `<dict >` too), are not
        // the bundle's value: the system reads none at the top level.
        let commented = document(&format!(
            "<dict>\n<!-- <key>StickyMDInputs</key><string>{digest}</string> -->\n</dict>"
        ));
        assert_eq!(super::inputs_in_plist(&commented).unwrap(), None);
        let nested = document(&format!(
            "<dict><key>Other</key><dict ><key>StickyMDInputs</key><string>{digest}</string></dict></dict>"
        ));
        assert_eq!(super::inputs_in_plist(&nested).unwrap(), None);
        // Only the UTF-8 XML that build-app.sh writes: in UTF-16 or the binary form a repeated
        // key would go unseen.
        let utf16: Vec<u8> = twice.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let error = super::inputs_in_plist_bytes(&utf16).unwrap_err();
        assert!(error.contains("UTF-8"), "{error}");
        let error = super::inputs_in_plist_bytes(b"bplist00\x00").unwrap_err();
        assert!(error.contains("UTF-8"), "{error}");
        let mut latin1 = plist(&digest).into_bytes();
        latin1.extend_from_slice(b"<!-- \xe9 -->");
        assert!(super::inputs_in_plist_bytes(&latin1).is_err());
    }
}
