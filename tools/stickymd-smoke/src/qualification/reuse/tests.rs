//! The reuse judgement and its audit record over a throwaway repository.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::{REUSE_RECORD, candidate_source_matches, check_record, remove_record, write_record};
use crate::release::platform_inputs::{ALGORITHM, Reuse};

/// A throwaway repository: a first commit, then one changing only a document, one only the
/// macOS host (neither a Windows input), and one the Windows host.
pub(in crate::qualification) struct Repository {
    pub(in crate::qualification) root: PathBuf,
    pub(in crate::qualification) first: String,
    pub(in crate::qualification) document: String,
    pub(in crate::qualification) mac: String,
    pub(in crate::qualification) windows: String,
}

impl Drop for Repository {
    fn drop(&mut self) {
        // Created exclusively by this test under the temp directory.
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn git(root: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(root)
        .args(arguments)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .output()
        .expect("run git");
    assert!(output.status.success(), "git {arguments:?}: {output:?}");
    String::from_utf8(output.stdout)
        .expect("git output is UTF-8")
        .trim()
        .to_owned()
}

fn unique_directory(label: &str) -> PathBuf {
    static SEQUENCE: AtomicU64 = AtomicU64::new(0);
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!(
        "stickymd-{label}-{}-{nonce}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

pub(in crate::qualification) fn repository() -> Repository {
    let root = unique_directory("reuse-repository");
    fs::create_dir(&root).expect("repository directory");
    git(&root, &["init", "-q", "-b", "main"]);
    let commit = |files: &[(&str, &str)]| {
        for (path, text) in files {
            let path = root.join(path);
            fs::create_dir_all(path.parent().expect("parent")).expect("directories");
            fs::write(path, text).expect("file");
        }
        git(&root, &["add", "-A"]);
        git(&root, &["commit", "-q", "-m", "c"]);
        git(&root, &["rev-parse", "HEAD"])
    };
    let first = commit(&[
        ("Cargo.toml", "[workspace]\n"),
        // A tree is judged only with its classifier in place (fix-round review R3-3).
        (
            "tools/stickymd-smoke/src/release/platform_inputs.rs",
            "the classifier",
        ),
        ("apps/stickymd-win/src/main.rs", "win 1"),
        ("apps/stickymd-mac/src/main.rs", "mac 1"),
        ("docs/plan/a.md", "doc 1"),
    ]);
    let document = commit(&[("docs/plan/a.md", "doc 2")]);
    let mac = commit(&[("apps/stickymd-mac/src/main.rs", "mac 2")]);
    let windows = commit(&[("apps/stickymd-win/src/main.rs", "win 2")]);
    Repository {
        root,
        first,
        document,
        mac,
        windows,
    }
}

#[test]
fn the_same_commit_is_accepted_without_git() {
    // No repository at all: any Git command here would fail.
    let nowhere = unique_directory("reuse-no-repository");
    let commit = "0123456789abcdef0123456789abcdef01234567";
    assert_eq!(
        candidate_source_matches(&nowhere, commit, commit),
        Ok(Reuse::SameCommit)
    );
    // The form is checked before the comparison: one spelling only, on both sides.
    let upper = commit.to_ascii_uppercase();
    for (build, release) in [(commit, upper.as_str()), (upper.as_str(), upper.as_str())] {
        let error = candidate_source_matches(&nowhere, build, release)
            .expect_err("an uppercase spelling is refused");
        assert!(error.contains("lowercase"), "{error}");
    }
    assert!(candidate_source_matches(&nowhere, "not a commit", "not a commit").is_err());
}

#[test]
fn an_ancestor_with_unchanged_windows_inputs_is_accepted() {
    let repository = repository();
    // A document and a macOS-only change leave the Windows inputs as they were.
    for release in [&repository.document, &repository.mac] {
        match candidate_source_matches(&repository.root, &repository.first, release) {
            Ok(Reuse::Ancestor { fingerprint }) => {
                assert_eq!(fingerprint.len(), 64, "{fingerprint}");
            }
            other => panic!("{release}: {other:?}"),
        }
    }
}

#[test]
fn a_changed_windows_input_is_refused_by_name() {
    let repository = repository();
    let error = candidate_source_matches(&repository.root, &repository.first, &repository.windows)
        .expect_err("the Windows host changed");
    assert!(error.contains("apps/stickymd-win/src/main.rs"), "{error}");
    assert!(!error.contains("apps/stickymd-mac/src/main.rs"), "{error}");
}

#[test]
fn a_commit_that_is_not_an_ancestor_is_refused() {
    let repository = repository();
    // Backwards: the release commit is older than the build source.
    let error = candidate_source_matches(&repository.root, &repository.mac, &repository.first)
        .expect_err("a descendant is not an ancestor");
    assert!(error.contains("not an ancestor"), "{error}");
}

#[test]
fn the_reuse_record_follows_the_judgement_and_the_candidate_receipt() {
    let repository = repository();
    let root = &repository.root;
    let (build, release) = (&repository.first, &repository.mac);
    let reuse = candidate_source_matches(root, build, release).expect("reusable");
    let Reuse::Ancestor { fingerprint } = &reuse else {
        panic!("{reuse:?}");
    };
    let digest = "d".repeat(64);
    // Promote of a reused artifact writes the record readiness then accepts.
    write_record(root, &reuse, build, release, &digest).expect("write record");
    let text = fs::read_to_string(root.join(REUSE_RECORD)).expect("record");
    assert_eq!(
        text,
        format!(
            "{{\"schema_version\":1,\"platform\":\"windows-x64\",\"build_source\":\"{build}\",\"release_source\":\"{release}\",\"build_inputs\":\"{fingerprint}\",\"release_inputs\":\"{fingerprint}\",\"algorithm\":\"{ALGORITHM}\",\"candidate_receipt_sha256\":\"{digest}\"}}\n"
        )
    );
    crate::qualification::json::parse_object(&text).expect("strict JSON");
    check_record(root, &reuse, build, release, &digest).expect("current record");
    // Another candidate receipt, release commit or a damaged record is stale.
    assert!(check_record(root, &reuse, build, release, &"e".repeat(64)).is_err());
    assert!(check_record(root, &reuse, build, &repository.document, &digest).is_err());
    fs::write(root.join(REUSE_RECORD), text.replace("\n", " \n")).expect("damage");
    assert!(check_record(root, &reuse, build, release, &digest).is_err());
    // With S equal to R a record is stale; Promote on the same commit removes it.
    assert!(check_record(root, &Reuse::SameCommit, release, release, &digest).is_err());
    write_record(root, &Reuse::SameCommit, release, release, &digest).expect("same commit");
    assert!(!root.join(REUSE_RECORD).exists());
    check_record(root, &Reuse::SameCommit, release, release, &digest).expect("no record");
    // A reuse without its record is not ready; removing twice is not an error.
    let error = check_record(root, &reuse, build, release, &digest).expect_err("missing");
    assert!(error.contains("missing"), "{error}");
    remove_record(root).expect("idempotent removal");
}
