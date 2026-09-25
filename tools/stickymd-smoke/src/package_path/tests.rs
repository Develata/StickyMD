use super::*;
use crate::cli::CommandLine;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "stickymd-package-selection-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn repository(&self) -> (PathBuf, String) {
        let root = self.0.join("repository with spaces");
        fs::create_dir(&root).unwrap();
        fs::write(
            root.join("Cargo.toml"),
            "[workspace.package]\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        command_text(&root, "git", &["init", "--quiet"]).unwrap();
        command_text(&root, "git", &["add", "Cargo.toml"]).unwrap();
        command_text(
            &root,
            "git",
            &[
                "-c",
                "user.name=StickyMD test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "commit",
                "--quiet",
                "-m",
                "fixture",
            ],
        )
        .unwrap();
        let commit = command_text(&root, "git", &["rev-parse", "HEAD"]).unwrap();
        (root, commit)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        // Only this test's exclusively-created temporary directory is owned.
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn single_package_preserves_the_helper_behavior_without_claiming_validity() {
    let fixture = Fixture::new();
    let expected = fixture.0.join("StickyMD-old-windows-x64-portable.zip");
    fs::write(&expected, "not a verified archive").unwrap();
    fs::write(fixture.0.join("unrelated.zip"), []).unwrap();
    fs::create_dir(
        fixture
            .0
            .join("StickyMD-directory-windows-x64-portable.zip"),
    )
    .unwrap();
    assert_eq!(resolve(&fixture.0, &fixture.0).unwrap(), expected);
}

#[test]
fn multiple_packages_select_the_current_clean_then_dirty_identity() {
    let fixture = Fixture::new();
    let (root, commit) = fixture.repository();
    let clean = fixture
        .0
        .join(local_archive_name("0.1.0", &commit, false).unwrap());
    let dirty = fixture
        .0
        .join(local_archive_name("0.1.0", &commit, true).unwrap());
    fs::write(&clean, []).unwrap();
    fs::write(&dirty, []).unwrap();
    assert_eq!(resolve(&root, &fixture.0).unwrap(), clean);
    fs::write(root.join("untracked.txt"), "dirty").unwrap();
    assert_eq!(resolve(&root, &fixture.0).unwrap(), dirty);
}

#[test]
fn ambiguity_and_missing_inputs_fail_without_choosing_an_old_package() {
    let fixture = Fixture::new();
    let (root, commit) = fixture.repository();
    for version in ["old-one", "old-two"] {
        fs::write(
            fixture
                .0
                .join(format!("StickyMD-{version}-windows-x64-portable.zip")),
            [],
        )
        .unwrap();
    }
    let error = resolve(&root, &fixture.0).unwrap_err();
    assert!(error.contains(&local_archive_name("0.1.0", &commit, false).unwrap()));
    assert!(error.contains("2 package(s)"));
    assert!(resolve(&root, &fixture.0.join("missing")).is_err());
    assert!(resolve(&root, &root).unwrap_err().contains("0 package(s)"));
    assert!(resolve(&fixture.0, &fixture.0).is_err());
}

#[test]
fn local_archive_names_require_a_full_commit_and_normalize_sha_case() {
    assert_eq!(
        local_archive_name("0.1.0", &"A".repeat(40), true).unwrap(),
        "StickyMD-0.1.0-local-validation-aaaaaaaaaaaa-dirty-windows-x64-portable.zip",
    );
    for invalid in ["abc", &"z".repeat(40), &"中".repeat(14)] {
        assert!(local_archive_name("0.1.0", invalid, false).is_err());
    }
}

#[test]
fn command_line_preserves_spaces_and_rejects_missing_or_extra_arguments() {
    let args = |values: &[&str]| {
        values
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        CommandLine::parse(args(&["package-path", "--directory", "folder with spaces"])).unwrap(),
        CommandLine::PackagePath(PathBuf::from("folder with spaces")),
    );
    for input in [
        vec!["package-path"],
        vec!["package-path", "--directory"],
        vec!["package-path", "--directory", ""],
        vec!["package-path", "--directory", "path", "extra"],
        vec!["package-path", "--unknown", "path"],
    ] {
        assert!(CommandLine::parse(args(&input)).is_err());
    }
}
