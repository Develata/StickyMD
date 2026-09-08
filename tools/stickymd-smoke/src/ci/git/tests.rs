use super::*;
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct Repository(PathBuf);

impl Repository {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("stickymd-ci-git-{}-{nonce}", std::process::id()));
        fs::create_dir(&root).unwrap();
        let repo = Self(root);
        repo.git(&["init", "--quiet"]);
        repo.git(&["config", "user.name", "StickyMD test"]);
        repo.git(&["config", "user.email", "test@example.invalid"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo
    }
    fn git(&self, args: &[&str]) -> String {
        crate::repository::command_text(&self.0, "git", args).unwrap()
    }
    fn write(&self, path: &str, content: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }
    fn commit(&self) -> String {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "-m", "fixture"]);
        self.git(&["rev-parse", "HEAD"])
    }
}

impl Drop for Repository {
    fn drop(&mut self) {
        // Only this test's exclusively created temporary Git repository is owned.
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn ci_git_detects_add_delete_and_both_sides_of_a_rename() {
    let repo = Repository::new();
    repo.write("crates/stickymd-render/old file.md", "unchanged content");
    repo.write("README.md", "documentation");
    let base = repo.commit();
    fs::remove_file(repo.0.join("crates/stickymd-render/old file.md")).unwrap();
    repo.write(
        "experiments/phase-01/persistence/new file.md",
        "unchanged content",
    );
    repo.write("crates/stickymd-core/新增.rs", "new source");
    let head = repo.commit();
    let facts = inspect(&repo.0, Some(&base)).unwrap();
    assert_eq!(facts.head, head);
    assert!(!facts.dirty);
    let paths = facts.paths.unwrap();
    assert_eq!(
        paths,
        [
            "crates/stickymd-core/新增.rs",
            "crates/stickymd-render/old file.md",
            "experiments/phase-01/persistence/new file.md"
        ]
    );
    let selection = super::super::selection::select(&paths);
    assert_eq!(
        selection.modules,
        [
            crate::headless::Module::Core,
            crate::headless::Module::Render,
            crate::headless::Module::Windows,
            crate::headless::Module::Persistence
        ]
    );
    assert!(changed_paths(&repo.0, &head, &head).unwrap().is_empty());
}

#[test]
fn ci_git_missing_zero_invalid_or_uncommitted_inputs_request_full_checks() {
    let repo = Repository::new();
    repo.write("README.md", "one");
    let head = repo.commit();
    for base in ["", "--help", &"0".repeat(40), &"f".repeat(40)] {
        assert!(inspect(&repo.0, Some(base)).unwrap().paths.is_err());
    }
    assert!(inspect(&repo.0, None).unwrap().paths.is_err());
    repo.write("untracked.rs", "new input");
    let dirty = inspect(&repo.0, Some(&head)).unwrap();
    assert!(dirty.dirty && dirty.paths.is_err());
}

#[test]
fn ci_git_path_transport_is_nul_delimited_and_fails_closed_on_unknown_encoding() {
    assert_eq!(
        decode_paths(b"crates/stickymd-render/a\nline.md\0README.md\0")
            .unwrap()
            .len(),
        2
    );
    assert!(decode_paths(b"\xff\0").is_err());
    assert!(decode_paths(b"README.md").is_err());
    assert!(decode_paths(b"../README.md\0").is_err());
    assert!(decode_paths(b"a\\b\0").is_err());
}
