//! Shared artifact hashes and strict checksum manifests.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
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

/// Generation and verification share the same name, digest and duplicate rules.
pub(crate) fn render_checksums(artifacts: &[(&str, &str)]) -> Result<String, String> {
    let text = artifacts
        .iter()
        .map(|(name, hash)| format!("{} *{name}\n", hash.to_ascii_lowercase()))
        .collect::<String>();
    verify_manifest_text(&text, artifacts)?;
    Ok(text)
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

#[cfg(not(windows))]
mod portable;
#[cfg(windows)]
mod windows;

pub(crate) fn sha256(path: &Path) -> Result<String, String> {
    let path = std::path::absolute(path)
        .map_err(|error| format!("cannot resolve hash input {}: {error}", path.display()))?;
    #[cfg(windows)]
    {
        windows::sha256(&path)
    }
    #[cfg(not(windows))]
    {
        portable::sha256(&path)
    }
}
/// SHA-256 of bytes already in memory; callers that validate and archive the same
/// bytes use this instead of re-reading a file that another process may replace.
pub(crate) fn sha256_bytes(bytes: &[u8]) -> Result<String, String> {
    #[cfg(windows)]
    {
        windows::sha256_bytes(bytes)
    }
    #[cfg(not(windows))]
    {
        portable::sha256_bytes(bytes)
    }
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
    fn in_memory_hash_matches_file_hash_beyond_one_pipe_buffer() {
        assert_eq!(
            sha256_bytes(b"abc").unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        // Larger than a pipe buffer, so the portable adapter must stream while waiting.
        let large = vec![0x5a_u8; 3 * 1024 * 1024 + 7];
        assert_eq!(
            sha256_bytes(&large).unwrap(),
            hash_fixture("large.bin", &large).unwrap()
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
