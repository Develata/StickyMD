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
    let text = std::fs::read_to_string(&plist).map_err(|e| format!("{}: {e}", plist.display()))?;
    inputs_in_plist(&text)
}

/// The value of `INPUTS_KEY` in an XML property list as `build-app.sh` writes it: the key element
/// followed, after whitespace, by one `<string>` element.
pub fn inputs_in_plist(text: &str) -> Result<Option<String>, String> {
    let key = format!("<key>{INPUTS_KEY}</key>");
    let Some(at) = text.find(&key) else {
        return Ok(None);
    };
    if text[at + key.len()..].contains(&key) {
        return Err(format!("Info.plist names {INPUTS_KEY} twice"));
    }
    let value = text[at + key.len()..]
        .trim_start()
        .strip_prefix("<string>")
        .and_then(|rest| rest.split_once("</string>"))
        .map(|(value, _)| value)
        .ok_or_else(|| format!("Info.plist {INPUTS_KEY} is not followed by a string"))?;
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
        let plist = |value: &str| {
            format!(
                "<dict>\n\t<key>CFBundleVersion</key>\n\t<string>0.2.0</string>\n\t<key>StickyMDInputs</key>\n\t<string>{value}</string>\n</dict>"
            )
        };
        assert_eq!(
            super::inputs_in_plist(&plist(&digest)).unwrap(),
            Some("a".repeat(64))
        );
        assert_eq!(
            super::inputs_in_plist("<dict><key>CFBundleVersion</key></dict>").unwrap(),
            None
        );
        assert!(super::inputs_in_plist(&plist("@INPUTS@")).is_err());
        assert!(super::inputs_in_plist(&plist(&"a".repeat(63))).is_err());
        let twice = plist(&digest) + &plist(&digest);
        assert!(super::inputs_in_plist(&twice).is_err());
        assert!(super::inputs_in_plist("<key>StickyMDInputs</key><true/>").is_err());
    }
}
