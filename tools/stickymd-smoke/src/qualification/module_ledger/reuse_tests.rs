//! End-to-end release reuse: successes recorded in one worktree are reused by a fresh
//! linked worktree after a release-shaped change, and product changes still rerun.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::json;
use super::{ModuleId, compatible_success, modules, record_success, success_status};
use crate::qualification::receipt::{self, Candidate, RELEASE_ARTIFACT_NAME};

/// A main worktree plus an optional linked worktree, both owned by this test.
struct ReleaseClone {
    main: PathBuf,
    linked: PathBuf,
}

impl ReleaseClone {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let base = std::env::temp_dir().join(format!(
            "stickymd-ledger-reuse-{}-{nonce}",
            std::process::id()
        ));
        let clone = Self {
            main: base.join("main"),
            linked: base.join("release-worktree"),
        };
        fs::create_dir_all(&clone.main).expect("main worktree");
        git(&clone.main, &["init", "--quiet"]);
        for (key, value) in [
            ("user.name", "StickyMD ledger test"),
            ("user.email", "ledger@example.invalid"),
            ("commit.gpgsign", "false"),
            ("core.autocrlf", "false"),
        ] {
            git(&clone.main, &["config", key, value]);
        }
        // As in the real repository, evidence and build output are never committed.
        write(&clone.main, ".gitignore", "/dist/\n/target/\n");
        write(&clone.main, "Cargo.toml", &root_manifest("0.1.0"));
        write(
            &clone.main,
            "crates/stickymd-core/Cargo.toml",
            "[package]\nname = \"stickymd-core\"\nversion.workspace = true\n",
        );
        write(&clone.main, "Cargo.lock", &lockfile("0.1.0"));
        write(
            &clone.main,
            "crates/stickymd-core/src/lib.rs",
            "pub fn core() {}\n",
        );
        write(
            &clone.main,
            "tools/stickymd-smoke/src/qualification/g4/cases/dock.rs",
            "// harness\n",
        );
        write(&clone.main, "docs/release-checklist.md", "- [ ] tag\n");
        write(&clone.main, "docs/release-notes/0.1.0.md", "# 0.1.0\n");
        write(&clone.main, "docs/report/2026-10-01-prep.md", "prep\n");
        commit(&clone.main);
        clone
    }

    /// Exactly the files a patch release touches besides product code.
    fn release_bump(&self, from: &str, to: &str) {
        let manifest = fs::read_to_string(self.main.join("Cargo.toml")).unwrap();
        write(&self.main, "Cargo.toml", &manifest.replace(from, to));
        let lock = fs::read_to_string(self.main.join("Cargo.lock")).unwrap();
        write(
            &self.main,
            "Cargo.lock",
            &lock.replace(&format!("\"{from}\""), &format!("\"{to}\"")),
        );
        write(
            &self.main,
            &format!("docs/release-notes/{to}.md"),
            "# notes\n",
        );
        write(&self.main, "docs/release-checklist.md", "- [x] tag\n");
        write(&self.main, "docs/report/2026-10-09-prep.md", "prep\n");
        commit(&self.main);
    }

    fn add_linked_worktree(&self) -> &Path {
        let target = self.linked.to_str().expect("UTF-8 temp path");
        git(
            &self.main,
            &["worktree", "add", "--quiet", "--detach", target, "HEAD"],
        );
        &self.linked
    }
}

impl Drop for ReleaseClone {
    fn drop(&mut self) {
        // Both directories were created exclusively by this test.
        let _ = fs::remove_dir_all(&self.linked);
        let _ = fs::remove_dir_all(&self.main);
        if let Some(base) = self.main.parent() {
            let _ = fs::remove_dir(base);
        }
    }
}

#[test]
fn release_shaped_change_reuses_every_functional_module_from_a_linked_worktree() {
    let clone = ReleaseClone::new();
    let first = candidate("0.1.0", "c", "d");
    for module in modules() {
        write_valid_evidence(&clone.main, module, &first);
        record_success(&clone.main, module, &first)
            .unwrap_or_else(|error| panic!("record {module:?}: {error}"));
    }
    clone.release_bump("0.1.0", "0.1.1");
    let linked = clone.add_linked_worktree();
    assert!(
        !linked.join("dist").exists(),
        "the release worktree starts without local evidence"
    );
    let second = candidate("0.1.1", "e", "f");
    for module in modules() {
        let success = compatible_success(linked, module)
            .unwrap_or_else(|error| panic!("{module:?}: {error}"))
            .unwrap_or_else(|| panic!("{module:?} must be reused after a release-only change"));
        assert_eq!(success_status(&success, Some(&second)), "REUSED_PASS");
        assert_eq!(success.origin_version, "0.1.0");
        assert_eq!(success.origin_exe_sha256, first.exe_sha256);
    }

    // Product code is a functional input of every module: it must rerun.
    write(
        linked,
        "crates/stickymd-core/src/lib.rs",
        "pub fn core() { /* changed */ }\n",
    );
    for module in modules() {
        assert!(
            compatible_success(linked, module).unwrap().is_none(),
            "{module:?} must rerun after a product change"
        );
    }
}

#[test]
fn g5_screenshots_are_verified_from_the_store_in_a_linked_worktree() {
    let clone = ReleaseClone::new();
    let first = candidate("0.1.0", "c", "d");
    let screenshot = "dist/evidence/g5-artifacts/G5-01-light.png";
    write(&clone.main, screenshot, "png bytes");
    let sha256 = receipt::sha256(&clone.main.join(screenshot)).unwrap();
    let document = format!(
        concat!(
            "{{\"worktree_dirty\":false,\"results\":[",
            "{{\"id\":\"G5-01\",\"status\":\"PASSED\",\"artifacts\":[{{\"path\":\"{}\",\"sha256\":\"{}\"}}]}}",
            "]}}"
        ),
        screenshot, sha256
    );
    receipt::write_receipt(&clone.main, ModuleId::G5.receipt(), &document).unwrap();
    record_success(&clone.main, ModuleId::G5, &first).unwrap();
    let linked = clone.add_linked_worktree();
    assert!(!linked.join(screenshot).exists());
    let success = compatible_success(linked, ModuleId::G5).unwrap().unwrap();
    let archived = success.artifact(&sha256).unwrap();
    assert_eq!(receipt::sha256(&archived).unwrap(), sha256);

    // A screenshot whose bytes no longer match its listed hash cannot be recorded.
    write(&clone.main, screenshot, "tampered");
    assert!(record_success(&clone.main, ModuleId::G5, &first).is_err());
}

#[test]
fn records_with_different_inputs_coexist_instead_of_replacing_each_other() {
    let clone = ReleaseClone::new();
    let first = candidate("0.1.0", "c", "d");
    write_valid_evidence(&clone.main, ModuleId::G4, &first);
    record_success(&clone.main, ModuleId::G4, &first).unwrap();
    // Another worktree qualifies different harness inputs and records its own success.
    let linked = clone.add_linked_worktree();
    write(
        linked,
        "tools/stickymd-smoke/src/qualification/g4/cases/dock.rs",
        "// newer harness\n",
    );
    write_valid_evidence(linked, ModuleId::G4, &first);
    record_success(linked, ModuleId::G4, &first).unwrap();
    assert!(compatible_success(linked, ModuleId::G4).unwrap().is_some());
    // The original inputs still find their own record.
    assert!(
        compatible_success(&clone.main, ModuleId::G4)
            .unwrap()
            .is_some()
    );
}

fn write_valid_evidence(root: &Path, module: ModuleId, candidate: &Candidate) {
    let document = match module {
        ModuleId::Resource(group) => {
            let result = crate::resource_plan::tests::valid_resource_result(group);
            let document = crate::resource_plan::tests::document(group, &result);
            let input = super::fingerprint::calculate(root, module).unwrap();
            format!(
                "{{\"resource_input_fingerprint\":\"{input}\",{}",
                &document[1..]
            )
        }
        ModuleId::Runtime | ModuleId::Performance => {
            let labels = crate::runner::formal_task_labels(module == ModuleId::Runtime).unwrap();
            let results = labels
                .iter()
                .map(|label| {
                    format!(
                        "{{\"id\":\"{}\",\"status\":\"PASSED\"}}",
                        json::escape(label)
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                concat!(
                    "{{\"schema_version\":2,\"commit\":\"{}\",\"worktree_dirty\":false,",
                    "\"executable_sha256\":\"{}\",\"suite\":\"phase-14\",\"results\":[{}]}}"
                ),
                candidate.source_commit, candidate.exe_sha256, results
            )
        }
        ModuleId::G3 | ModuleId::G4 | ModuleId::G5 => {
            "{\"worktree_dirty\":false,\"results\":[{\"id\":\"exact\",\"status\":\"PASSED\"}]}"
                .to_owned()
        }
    };
    receipt::write_receipt(root, module.receipt(), &document).unwrap();
}

fn candidate(version: &str, exe: &str, zip: &str) -> Candidate {
    Candidate {
        source_commit: "a".repeat(40),
        version: version.to_owned(),
        cargo_lock_sha256: "b".repeat(64),
        exe_sha256: exe.repeat(64),
        zip_sha256: zip.repeat(64),
        sbom_sha256: "e".repeat(64),
        target: "x86_64-pc-windows-msvc".to_owned(),
        workflow_run_id: 1,
        workflow_attempt: 1,
        artifact_id: 2,
        artifact_name: RELEASE_ARTIFACT_NAME.to_owned(),
        zip_name: format!("StickyMD-{version}-windows-x64-portable.zip"),
    }
}

fn root_manifest(version: &str) -> String {
    format!(
        "[workspace]\nmembers = [\"crates/stickymd-core\"]\n\n[workspace.package]\nversion = \"{version}\"\nedition = \"2024\"\n"
    )
}

fn lockfile(version: &str) -> String {
    format!(
        concat!(
            "# This file is automatically @generated by Cargo.\n",
            "version = 4\n\n",
            "[[package]]\nname = \"stickymd-core\"\nversion = \"{}\"\n"
        ),
        version
    )
}

fn write(root: &Path, relative: &str, contents: &str) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
    fs::write(path, contents).expect("write fixture");
}

fn git(directory: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .args(arguments)
        .current_dir(directory)
        .status()
        .expect("start git");
    assert!(status.success(), "git {arguments:?} failed");
}

fn commit(root: &Path) {
    git(root, &["add", "--all"]);
    git(root, &["commit", "--quiet", "-m", "fixture"]);
}
