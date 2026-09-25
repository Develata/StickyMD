//! Shared artifact hashes and strict checksum manifests.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    process::Command,
};

pub(crate) fn artifact_name(name: &str) -> Result<(), String> {
    if name.is_empty()
        || matches!(name, "." | "..")
        || name.contains(['/', '\\', ':'])
        || name.chars().any(char::is_control)
        || name.ends_with(['.', ' '])
    {
        return Err(format!("Unsafe checksum artifact name: {name}"));
    }
    Ok(())
}

pub(crate) fn parse_checksums(manifest: &str) -> Result<BTreeMap<String, String>, String> {
    let mut entries = BTreeMap::new();
    for line in manifest
        .trim_start_matches('\u{feff}')
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let (hash, name) = line
            .split_once(" *")
            .ok_or_else(|| format!("Invalid checksum line: {line}"))?;
        validate_sha256(hash, "checksum SHA-256")?;
        artifact_name(name)?;
        if entries
            .insert(name.to_ascii_lowercase(), hash.to_ascii_lowercase())
            .is_some()
        {
            return Err("Checksum manifest contains duplicate artifact names".to_owned());
        }
    }
    Ok(entries)
}

pub(crate) fn verify_manifest_text(
    manifest: &str,
    expected: &[(&str, &str)],
) -> Result<(), String> {
    let entries = parse_checksums(manifest)?;
    if entries.len() != expected.len() {
        return Err(format!(
            "Checksum manifest must contain exactly {} entries",
            expected.len()
        ));
    }
    let mut expected_names = BTreeSet::new();
    for (name, hash) in expected {
        artifact_name(name)?;
        validate_sha256(hash, "expected SHA-256")?;
        if !expected_names.insert(name.to_ascii_lowercase()) {
            return Err("Expected checksum artifacts must have distinct names".to_owned());
        }
        match entries.get(&name.to_ascii_lowercase()) {
            Some(actual) if actual.eq_ignore_ascii_case(hash) => {}
            Some(_) => return Err(format!("Checksum mismatch for {name}")),
            None => return Err(format!("Checksum manifest is missing: {name}")),
        }
    }
    Ok(())
}

pub(crate) fn verify_checksum_manifest(
    directory: &Path,
    zip_name: &str,
    zip_hash: &str,
    sbom_hash: &str,
) -> Result<(), String> {
    verify_manifest_file(
        &directory.join("SHA256SUMS.txt"),
        zip_name,
        zip_hash,
        sbom_hash,
    )
}

pub(crate) fn verify_manifest_file(
    path: &Path,
    zip_name: &str,
    zip_hash: &str,
    sbom_hash: &str,
) -> Result<(), String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    verify_manifest_text(
        &text,
        &[(zip_name, zip_hash), ("SBOM.spdx.json", sbom_hash)],
    )
}

pub(crate) fn sha256(path: &Path) -> Result<String, String> {
    let path = std::path::absolute(path)
        .map_err(|error| format!("cannot resolve hash input {}: {error}", path.display()))?;
    #[cfg(windows)]
    {
        use std::io::Read;
        // certutil rejects empty files (ERROR_FILE_INVALID). Observe EOF through an
        // opened file, not a possibly stale length, before returning SHA-256(empty).
        let mut file = fs::File::open(&path)
            .map_err(|error| format!("cannot open hash input {}: {error}", path.display()))?;
        if file
            .read(&mut [0_u8; 1])
            .map_err(|error| format!("cannot read hash input {}: {error}", path.display()))?
            == 0
        {
            return Ok(
                "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".to_owned(),
            );
        }
    }
    #[cfg(windows)]
    let output = Command::new("certutil")
        .args(["-hashfile"])
        .arg(&path)
        .arg("SHA256")
        .output();
    #[cfg(not(windows))]
    let output = Command::new("sha256sum").arg("--").arg(&path).output();
    let output = output.map_err(|error| format!("cannot hash {}: {error}", path.display()))?;
    if !output.status.success() {
        return Err(format!("SHA-256 command failed for {}", path.display()));
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let hash = digest_output(&text).map_err(|error| format!("{error} for {}", path.display()))?;
    Ok(hash.to_ascii_lowercase())
}

fn digest_output(text: &str) -> Result<&str, String> {
    #[cfg(windows)]
    let hash = {
        // certutil's localized heading contains the full filename. Only the standalone
        // digest line is authoritative; a hex-looking word in that heading is not.
        let mut lines = text
            .lines()
            .map(str::trim)
            .filter(|line| line.len() == 64 && line.bytes().all(|byte| byte.is_ascii_hexdigit()));
        let hash = lines.next().ok_or("SHA-256 output has no digest line")?;
        if lines.next().is_some() {
            return Err("SHA-256 output has ambiguous digest lines".to_owned());
        }
        hash
    };
    #[cfg(not(windows))]
    let hash = text
        // GNU sha256sum prefixes an escaped filename record with a backslash.
        .strip_prefix('\\')
        .unwrap_or(text)
        .split_once(' ')
        .map(|(hash, _)| hash)
        .ok_or("SHA-256 output has no digest field")?;
    validate_sha256(hash, "SHA-256")?;
    Ok(hash)
}

pub(crate) fn validate_sha256(value: &str, label: &str) -> Result<(), String> {
    validate_hex(value, 64, label)
}

pub(crate) fn validate_hex(value: &str, length: usize, label: &str) -> Result<(), String> {
    if value.len() == length && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(format!("{label} is not a {length}-digit hexadecimal value"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_roles_cannot_alias_the_same_artifact_and_hide_an_extra_entry() {
        let hash = "a".repeat(64);
        let manifest = format!("{hash} *SBOM.spdx.json\n{hash} *unexpected.bin\n");
        let expected = [
            ("SBOM.spdx.json", hash.as_str()),
            ("sbom.spdx.json", hash.as_str()),
        ];
        assert!(
            verify_manifest_text(&manifest, &expected)
                .unwrap_err()
                .contains("distinct names")
        );
    }

    fn hash_fixture(name: &str, contents: &[u8]) -> Result<String, String> {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "stickymd-hash-{}-{nonce} 中文 {} with spaces",
            std::process::id(),
            "a".repeat(64)
        ));
        fs::create_dir(&root).unwrap();
        let path = root.join(name);
        fs::write(&path, contents).unwrap();
        let result = sha256(&path);
        fs::remove_dir_all(root).unwrap();
        result
    }

    #[test]
    fn file_hash_uses_bytes_instead_of_hex_words_in_a_unicode_space_path() {
        assert_eq!(
            hash_fixture("abc.bin", b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn empty_file_hash_matches_the_original_powershell_adapter() {
        assert_eq!(
            hash_fixture("empty.bin", b"").unwrap(),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[cfg(not(windows))]
    #[test]
    fn hash_handles_gnu_escaped_filename_records() {
        assert_eq!(
            hash_fixture("line\nbreak\\file", b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn checksums_bind_exact_members_and_reject_duplicate_unsafe_missing_and_wrong_hashes() {
        let hash = "a".repeat(64);
        let expected = [
            ("中文 archive.zip", hash.as_str()),
            ("SBOM.spdx.json", hash.as_str()),
        ];
        let valid = format!(
            "\u{feff}{hash} *中文 archive.zip\r\n{} *SBOM.spdx.json\r\n\n",
            hash.to_uppercase()
        );
        verify_manifest_text(&valid, &expected).unwrap();
        for broken in [
            valid.replace('a', "b"),
            format!("{valid}{hash} *sbom.spdx.json\n"),
            format!("{hash} *中文 archive.zip\n"),
            format!("{hash} *extra.zip\n{hash} *SBOM.spdx.json\n"),
        ] {
            assert!(verify_manifest_text(&broken, &expected).is_err());
        }
        for name in [
            "../a", "a/b", "a\\b", "/a", "C:foo", ".", "..", "foo.", "foo ", "a\0b",
        ] {
            assert!(
                parse_checksums(&format!("{hash} *{name}\n")).is_err(),
                "{name:?}"
            );
        }
        assert!(
            parse_checksums(&format!("{hash} *a\n{hash} *A\n"))
                .unwrap_err()
                .contains("duplicate")
        );
    }
}
