use super::*;

const VALID: &str = r#"{"spdxVersion":"SPDX-2.3","packages":[{}],"files":[{"fileName":"\\package\\StickyMD\\StickyMD.exe"},{"fileName":"\\package\\StickyMD\\THIRD_PARTY_NOTICES.txt"},{"fileName":"\\package\\StickyMD\\licenses\\SIL-OFL-1.1.txt"},{"fileName":"\\package\\StickyMD\\licenses\\KaTeX-fonts-NOTICE.txt"}]}"#;

#[test]
fn sbom_requires_spdx_packages_and_each_packaged_file() {
    validate(VALID).unwrap();
    validate(
        &VALID
            .replace("SPDX-2.3", "spdx-2.2")
            .replace("StickyMD.exe", "stickymd.EXE"),
    )
    .unwrap();
    for invalid in [
        "{}".to_owned(),
        "{".to_owned(),
        VALID.replace("SPDX-2.3", "SPDX-3.0"),
        VALID.replace("\"packages\":[{}]", "\"packages\":[]"),
        VALID.replace("\"packages\":[{}]", "\"packages\":null"),
        VALID.replace("\"spdxVersion\"", "\"missing\""),
        VALID.replace("\"fileName\"", "\"missing\""),
    ] {
        assert!(validate(&invalid).is_err(), "{invalid}");
    }
    for name in [
        "StickyMD.exe",
        "THIRD_PARTY_NOTICES.txt",
        "SIL-OFL-1.1.txt",
        "KaTeX-fonts-NOTICE.txt",
    ] {
        assert!(validate(&VALID.replace(name, "missing")).is_err(), "{name}");
    }
}

#[test]
fn publication_validates_every_input_before_changing_existing_files() {
    let temp = TemporaryDirectory::new("sbom-publish-test").unwrap();
    let root = temp.path();
    let options = SbomOptions {
        input: root.join("staged.json"),
        output: root.join("中文 SBOM.json"),
        zip: root.join("中文 archive.zip"),
        checksums: root.join("SHA256SUMS.txt"),
    };
    fs::write(&options.zip, b"zip bytes").unwrap();
    fs::write(&options.output, b"previous-sbom").unwrap();
    fs::write(&options.checksums, b"previous-checksums").unwrap();
    for contents in [b"{".as_slice(), b"{}", &[0xff]] {
        fs::write(&options.input, contents).unwrap();
        assert!(publish(&options).is_err());
        assert_eq!(fs::read(&options.output).unwrap(), b"previous-sbom");
        assert_eq!(fs::read(&options.checksums).unwrap(), b"previous-checksums");
    }
    fs::write(&options.input, VALID).unwrap();
    for invalid in [
        SbomOptions {
            checksums: options.zip.clone(),
            ..options.clone()
        },
        SbomOptions {
            output: options.input.clone(),
            ..options.clone()
        },
        SbomOptions {
            output: options.checksums.clone(),
            ..options.clone()
        },
        SbomOptions {
            zip: root.join("missing.zip"),
            ..options.clone()
        },
        SbomOptions {
            output: root.join("missing parent/SBOM.json"),
            ..options.clone()
        },
        SbomOptions {
            checksums: root.to_path_buf(),
            ..options.clone()
        },
    ] {
        assert!(publish(&invalid).is_err());
        assert_eq!(fs::read(&options.output).unwrap(), b"previous-sbom");
        assert_eq!(fs::read(&options.checksums).unwrap(), b"previous-checksums");
    }
    publish(&options).unwrap();
    assert_eq!(fs::read_to_string(&options.output).unwrap(), VALID);
    let zip_hash = integrity::sha256(&options.zip).unwrap();
    let sbom_hash = integrity::sha256(&options.output).unwrap();
    integrity::verify_manifest_text(
        &fs::read_to_string(&options.checksums).unwrap(),
        &[
            ("中文 archive.zip", &zip_hash),
            ("中文 SBOM.json", &sbom_hash),
        ],
    )
    .unwrap();
    assert_eq!(fs::read_dir(root).unwrap().count(), 4);
}

#[cfg(windows)]
#[test]
fn sbom_replace_failure_preserves_previous_outputs() {
    use std::os::windows::fs::OpenOptionsExt;
    let temp = TemporaryDirectory::new("sbom-locked-test").unwrap();
    let options = SbomOptions {
        input: temp.path().join("staged.json"),
        output: temp.path().join("SBOM.spdx.json"),
        zip: temp.path().join("archive.zip"),
        checksums: temp.path().join("SHA256SUMS.txt"),
    };
    fs::write(&options.input, VALID).unwrap();
    fs::write(&options.zip, b"zip bytes").unwrap();
    fs::write(&options.output, b"old").unwrap();
    fs::write(&options.checksums, b"old-checksums").unwrap();
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&options.output)
        .unwrap();
    assert!(publish(&options).is_err());
    drop(locked);
    assert_eq!(fs::read(&options.output).unwrap(), b"old");
    assert_eq!(fs::read(&options.checksums).unwrap(), b"old-checksums");
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 4);
}

#[cfg(windows)]
#[test]
fn checksum_replace_failure_leaves_a_pair_that_fails_verification() {
    use std::os::windows::fs::OpenOptionsExt;
    let temp = TemporaryDirectory::new("sbom-manifest-locked-test").unwrap();
    let options = SbomOptions {
        input: temp.path().join("staged.json"),
        output: temp.path().join("SBOM.spdx.json"),
        zip: temp.path().join("archive.zip"),
        checksums: temp.path().join("SHA256SUMS.txt"),
    };
    fs::write(&options.input, VALID).unwrap();
    fs::write(&options.zip, b"zip bytes").unwrap();
    fs::write(&options.output, b"old-sbom").unwrap();
    let zip_hash = integrity::sha256(&options.zip).unwrap();
    let old_hash = integrity::sha256(&options.output).unwrap();
    let old_manifest = format!("{zip_hash} *archive.zip\n{old_hash} *SBOM.spdx.json\n");
    fs::write(&options.checksums, &old_manifest).unwrap();
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(&options.checksums)
        .unwrap();
    assert!(publish(&options).is_err());
    drop(locked);
    assert_eq!(
        fs::read_to_string(&options.checksums).unwrap(),
        old_manifest
    );
    let new_hash = integrity::sha256(&options.output).unwrap();
    assert!(
        integrity::verify_checksum_manifest(temp.path(), "archive.zip", &zip_hash, &new_hash)
            .is_err()
    );
    assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 4);
}
