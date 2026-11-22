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

/// The value of `INPUTS_KEY` in the property list at `path`.
///
/// Only the form `build-app.sh` writes is accepted, so that every key can be read off the
/// bytes: UTF-8 XML in which each key element is `<key>`, a plain name (ASCII letters, digits,
/// `.`, `_`, `-`) and `</key>`, with no entity, CDATA section, comment, attribute or space
/// inside. A plain name decodes to itself, so `INPUTS_KEY` occurs in the bytes exactly as often
/// as in the document:
/// - never: a bundle built before `build-app.sh` recorded the fingerprint (`None`);
/// - once: it must be one SHA-256 string of the top-level dictionary, read by `plutil`; a key
///   that is commented out, nested or not a string is an error, not an absent fingerprint;
/// - more than once, anywhere (a comment included): an error.
fn plist_inputs(path: &Path) -> Result<Option<String>, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let text = std::str::from_utf8(&bytes)
        .ok()
        .filter(|text| text.starts_with("<?xml"))
        .ok_or_else(|| {
            format!(
                "{} is not an XML property list in UTF-8, the form build-app.sh writes",
                path.display()
            )
        })?;
    let mut occurrences = 0;
    for (at, _) in text.match_indices("<key") {
        let name = text[at + "<key".len()..]
            .strip_prefix('>')
            .and_then(|rest| rest.split_once("</key>"))
            .map(|(name, _)| name)
            .filter(|name| {
                !name.is_empty()
                    && name
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
            })
            .ok_or_else(|| {
                format!(
                    "Info.plist writes a key element other than <key>plain-name</key>, the form build-app.sh writes, near byte {at}"
                )
            })?;
        occurrences += usize::from(name == INPUTS_KEY);
    }
    if occurrences > 1 {
        return Err(format!("Info.plist names {INPUTS_KEY} {occurrences} times"));
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
    if occurrences == 0 {
        return Ok(None);
    }
    // `-n`: no newline after the value, so it is compared exactly as stored.
    let typed = plutil(&[
        "-extract", INPUTS_KEY, "raw", "-expect", "string", "-n", "-o", "-",
    ])?;
    if !typed.status.success() {
        return Err(format!(
            "Info.plist writes {INPUTS_KEY}, but plutil reads no string under it in the top-level dictionary"
        ));
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
        let refused = |bytes: &[u8], reason: &str| {
            let error = super::inputs_in_plist_bytes(bytes).unwrap_err();
            assert!(error.contains(reason), "{reason}: {error}");
        };
        refused(plist("@INPUTS@").as_bytes(), "not a SHA-256");
        refused(plist(&"a".repeat(63)).as_bytes(), "not a SHA-256");
        // The value as stored: padding around a digest is not a digest.
        refused(plist(&format!(" {digest} ")).as_bytes(), "not a SHA-256");
        refused(
            document("<dict><key>StickyMDInputs</key><true/></dict>").as_bytes(),
            "no string",
        );
        // The key written but not as the bundle's value: commented out, or in a nested
        // dictionary (written `<dict >` too).
        let commented = document(&format!(
            "<dict>\n<!-- <key>StickyMDInputs</key><string>{digest}</string> -->\n</dict>"
        ));
        refused(commented.as_bytes(), "no string");
        let nested = document(&format!(
            "<dict><key>Other</key><dict ><key>StickyMDInputs</key><string>{digest}</string></dict></dict>"
        ));
        refused(nested.as_bytes(), "no string");
        // Twice: equal or different values, or the real key and a comment that names it.
        let other = "b".repeat(64);
        for second in [
            format!("<key>StickyMDInputs</key><string>{digest}</string>"),
            format!("<key>StickyMDInputs</key><string>{other}</string>"),
            "<!-- <key>StickyMDInputs</key> -->".to_owned(),
        ] {
            let twice = document(&format!(
                "<dict><key>StickyMDInputs</key><string>{digest}</string>{second}</dict>"
            ));
            refused(twice.as_bytes(), "2 times");
        }
        // A key spelled so that its bytes differ from its decoded name would hide a repeat.
        for spelling in [
            "<key>StickyMD&#73;nputs</key>",
            "<key><![CDATA[StickyMDInputs]]></key>",
            "<key >StickyMDInputs</key>",
            "<key>Sticky<!-- -->MDInputs</key>",
        ] {
            let hidden = document(&format!(
                "<dict><key>StickyMDInputs</key><string>{digest}</string>{spelling}<string>{other}</string></dict>"
            ));
            refused(hidden.as_bytes(), "other than <key>plain-name</key>");
        }
        refused(
            b"<?xml version=\"1.0\"?><plist><dict>",
            "not a valid property list",
        );
        // Only UTF-8 XML: UTF-16 (declared as such), a binary property list, other bytes.
        let utf16: Vec<u8> = [0xFF, 0xFE]
            .into_iter()
            .chain(
                plist(&digest)
                    .replace("UTF-8", "UTF-16")
                    .encode_utf16()
                    .flat_map(u16::to_le_bytes),
            )
            .collect();
        refused(&utf16, "UTF-8");
        let directory = crate::temporary::TemporaryDirectory::new("binary-plist").unwrap();
        let xml = directory.path().join("Info.plist");
        std::fs::write(&xml, plist(&digest)).unwrap();
        let binary = std::process::Command::new("/usr/bin/plutil")
            .args(["-convert", "binary1", "-o", "-"])
            .arg(&xml)
            .output()
            .unwrap();
        assert!(binary.status.success() && binary.stdout.starts_with(b"bplist"));
        refused(&binary.stdout, "UTF-8");
        directory.close().unwrap();
        refused(b"not a property list", "UTF-8");
        let mut latin1 = plist(&digest).into_bytes();
        latin1.extend_from_slice(b"<!-- \xe9 -->");
        refused(&latin1, "UTF-8");
    }
}
