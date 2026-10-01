//! Local image admission and native no-reparse integration checks.
//!
//! plan_ref: docs/plan/08_assets_and_export.md#local-image-read-boundary

use super::*;
use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::os::windows::process::CommandExt;
use std::path::PathBuf;
use std::process::Command;

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = crate::test_support::unique_temp_path("local-image-中文 空格");
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // This unique directory was created exclusively by this fixture.
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn phase7_native_local_images_keep_seekable_unicode_and_parent_reads() {
    let fixture = Fixture::new();
    let child = fixture.0.join("note");
    fs::create_dir(&child).unwrap();
    let source = fixture.0.join("数学 图.png");
    fs::write(&source, b"0123456789").unwrap();
    for path in [
        source.clone(),
        child.join("../数学 图.png"),
        fs::canonicalize(&source).unwrap(),
    ] {
        let mut file = open(&path).unwrap();
        file.seek(SeekFrom::Start(4)).unwrap();
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"456789");
    }
    assert_eq!(
        open(&fixture.0.join("missing.png")).unwrap_err().kind(),
        io::ErrorKind::NotFound
    );
    assert!(open(&fixture.0).is_err());
    assert_eq!(fs::read(source).unwrap(), b"0123456789");
}

#[test]
fn phase7_network_device_paths_are_rejected_before_mapping_lookup() {
    for path in [
        r"\\server\share\a.png",
        r"\\?\UNC\server\share\a.png",
        r"\\.\pipe\image",
        r"\\?\GLOBALROOT\Device\Mup\a.png",
        r"C:a.png",
    ] {
        assert!(path::native_path(Path::new(path), |_| panic!("must not query {path}")).is_err());
    }
}

#[test]
fn phase7_drive_observations_reject_remote_unknown_and_cyclic_aliases() {
    for target in [
        r"\Device\Mup\server\share",
        r"\Device\LanmanRedirector\server",
        r"\??\UNC\server\share",
        r"\Device\HarddiskVolume1\redirect",
        r"\Device\NamedPipe",
        r"\Device\Unknown",
        r"\??\C:\cycle",
    ] {
        assert!(
            path::native_path(Path::new(r"C:\a.png"), |_| Ok(target.into())).is_err(),
            "{target}"
        );
    }
    assert!(path::native_path(Path::new(r"C:\a.png:stream"), |_| panic!("invalid name")).is_err());
    assert!(
        path::native_path(Path::new(r"C:\a.png"), |_| Err(io::Error::new(
            io::ErrorKind::NotFound,
            "unmapped"
        )))
        .is_err()
    );
}

#[test]
fn phase7_local_subst_aliases_normalize_without_following_filesystem_paths() {
    let native = path::native_path(Path::new(r"S:\note\..\图.png"), |drive| {
        Ok(match drive {
            b'S' => OsString::from(r"\??\E:\中文 空格"),
            b'E' => OsString::from(r"\Device\HarddiskVolume12"),
            _ => panic!("unexpected drive"),
        })
    })
    .unwrap();
    assert_eq!(
        native,
        OsString::from(r"\Device\HarddiskVolume12\中文 空格\图.png")
    );
}

#[test]
fn phase7_native_local_images_reject_reparse_ancestors_and_keep_open_identity() {
    let fixture = Fixture::new();
    let target = fixture.0.join("target");
    let link = fixture.0.join("junction");
    fs::create_dir(&target).unwrap();
    let source = target.join("a.png");
    fs::write(&source, b"original bytes").unwrap();
    let mut handle = open(&source).unwrap();
    fs::rename(&source, target.join("old.png")).unwrap();
    fs::write(&source, b"replacement bytes").unwrap();
    let mut bytes = Vec::new();
    handle.read_to_end(&mut bytes).unwrap();
    assert_eq!(bytes, b"original bytes");

    // A local junction needs no symlink privilege and never targets a network.
    // Paths travel in the child environment rather than interpolated script text.
    let result = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:STICKY_IMAGE_LINK -Target $env:STICKY_IMAGE_TARGET -ErrorAction Stop | Out-Null"])
        .env("STICKY_IMAGE_LINK", &link)
        .env("STICKY_IMAGE_TARGET", &target)
        .creation_flags(0x0800_0000)
        .output().unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert!(
        open(&link.join("a.png")).is_err(),
        "must not follow the junction"
    );
    assert!(
        open(&link).is_err(),
        "must not follow a final reparse point"
    );
    assert_eq!(fs::read(&source).unwrap(), b"replacement bytes");
}
