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
        let root = std::env::temp_dir().join(format!(
            "stickymd-local-git 中文 space-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        let repo = Self(root);
        repo.git(&["init", "--quiet"]);
        repo.git(&["config", "user.name", "StickyMD local test"]);
        repo.git(&["config", "user.email", "test@example.invalid"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo.git(&["config", "core.autocrlf", "false"]);
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
        // The directory was exclusively created by this test; no user data is owned.
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn local_git_retains_staged_and_unstaged_inputs_even_when_net_head_diff_is_empty() {
    let repo = Repository::new();
    let source = "crates/stickymd-core/src/中文 space.rs";
    repo.write(source, "original");
    repo.write("crates/stickymd-render/src/staged-delete.rs", "delete");
    repo.write("apps/stickymd-win/src/unstaged-delete.rs", "delete");
    repo.write(".gitignore", "ignored/\n");
    let head = repo.commit();
    assert!(inspect(&repo.0).paths.unwrap().is_empty());
    repo.write(source, "staged");
    repo.git(&["add", "--", source]);
    repo.write(source, "original");
    assert!(
        repo.git(&["diff", "--name-only", "HEAD", "--", source])
            .is_empty()
    );
    repo.git(&[
        "rm",
        "--quiet",
        "--",
        "crates/stickymd-render/src/staged-delete.rs",
    ]);
    fs::remove_file(repo.0.join("apps/stickymd-win/src/unstaged-delete.rs")).unwrap();
    repo.write(
        "experiments/phase-01/persistence/src/untracked 中文.rs",
        "new",
    );
    repo.write("crates/stickymd-core/src/new directory/new.rs", "new");
    repo.write("ignored/private.rs", "ignored");
    let index = fs::read(repo.0.join(".git/index")).unwrap();
    let facts = inspect(&repo.0);
    assert_eq!(facts.head.as_deref(), Some(head.as_str()));
    assert_eq!(
        facts.paths.unwrap(),
        [
            "apps/stickymd-win/src/unstaged-delete.rs",
            "crates/stickymd-core/src/new directory/new.rs",
            source,
            "crates/stickymd-render/src/staged-delete.rs",
            "experiments/phase-01/persistence/src/untracked 中文.rs",
        ]
    );
    assert_eq!(
        fs::read(repo.0.join(".git/index")).unwrap(),
        index,
        "planning must not refresh/write the index"
    );
}

#[test]
fn local_git_keeps_old_and_new_owners_for_staged_and_unstaged_renames() {
    for staged in [false, true] {
        let repo = Repository::new();
        let old = "crates/stickymd-render/old space.rs";
        let new = "experiments/phase-01/persistence/new 中文.rs";
        repo.write(old, "rename content");
        repo.commit();
        repo.write(new, "rename content");
        fs::remove_file(repo.0.join(old)).unwrap();
        if staged {
            repo.git(&["add", "--all"]);
        }
        let paths = inspect(&repo.0).paths.unwrap();
        assert_eq!(paths, [old, new]);
        let selection = crate::ci::selection::select(&paths);
        assert_eq!(
            selection.modules,
            [
                crate::headless::Module::Render,
                crate::headless::Module::Windows,
                crate::headless::Module::Persistence
            ]
        );
    }
}

#[test]
fn local_git_unborn_missing_repository_and_real_merge_conflict_require_full() {
    let repo = Repository::new();
    repo.write("README.md", "unborn");
    let unborn = inspect(&repo.0);
    assert!(unborn.head.is_none() && unborn.paths.is_err());
    repo.write("crates/stickymd-core/src/conflict.rs", "base\n");
    repo.commit();
    repo.git(&["checkout", "--quiet", "-b", "other"]);
    repo.write("crates/stickymd-core/src/conflict.rs", "other\n");
    repo.commit();
    repo.git(&["checkout", "--quiet", "-b", "current", "HEAD~1"]);
    repo.write("crates/stickymd-core/src/conflict.rs", "current\n");
    repo.commit();
    let merge = Command::new("git")
        .args(["merge", "--no-edit", "other"])
        .current_dir(&repo.0)
        .output()
        .unwrap();
    assert!(!merge.status.success());
    assert!(inspect(&repo.0).paths.unwrap_err().contains("unmerged"));
    assert!(inspect(&repo.0.join("missing")).paths.is_err());
}

#[test]
fn local_git_submodule_changes_are_observed_and_unknown_ownership_is_full() {
    let child = Repository::new();
    child.write("source.rs", "one");
    child.commit();
    let repo = Repository::new();
    repo.write("README.md", "parent");
    repo.commit();
    repo.git(&[
        "-c",
        "protocol.file.allow=always",
        "submodule",
        "add",
        "--quiet",
        child.0.to_str().unwrap(),
        "vendor/dependency",
    ]);
    repo.commit();
    repo.write("vendor/dependency/source.rs", "changed");
    let paths = inspect(&repo.0).paths.unwrap();
    assert_eq!(paths, ["vendor/dependency"]);
    assert!(crate::ci::selection::select(&paths).full);
}

#[test]
fn local_git_status_parser_preserves_nul_paths_and_rejects_ambiguous_observations() {
    assert_eq!(decode_status(b" M crates/stickymd-core/a\nline.rs\0R  experiments/phase-01/persistence/new.rs\0crates/stickymd-render/old.rs\0").unwrap(), [
        "crates/stickymd-core/a\nline.rs",
        "crates/stickymd-render/old.rs",
        "experiments/phase-01/persistence/new.rs",
    ]);
    for bytes in [
        &b"?? nonutf\xff\0"[..],
        &b"?? README.md"[..],
        &b"?? ../outside\0"[..],
        &b"?? a\\b\0"[..],
        &b"?? /absolute\0"[..],
        &b"?? empty//part\0"[..],
        &b"ZZ README.md\0"[..],
        &b"!! ignored\0"[..],
        &b"R  new.rs\0"[..],
        &b"R  new.rs\0\0"[..],
        &b"?? \0"[..],
        &b" M path\0\0"[..],
        &b"UU conflict.rs\0"[..],
    ] {
        assert!(decode_status(bytes).is_err(), "{bytes:?}");
    }
}
