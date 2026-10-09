//! End-to-end release reuse: successes recorded in one worktree are reused by a fresh
//! linked worktree after a release-shaped change, and product changes still rerun.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use super::status::success_status;
use super::{Lookup, ModuleId, compatible_success, lookup, record_success};
use crate::qualification::module_evidence::fixtures::valid_document;
use crate::qualification::module_evidence::g5_companions;
use crate::qualification::module_registry::modules;
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
    write_valid_evidence(&clone.main, ModuleId::G5, &first);
    record_success(&clone.main, ModuleId::G5, &first).unwrap();
    let document = fs::read_to_string(clone.main.join(ModuleId::G5.receipt())).unwrap();
    let screenshot = &g5_companions(&document).unwrap()[0];
    let linked = clone.add_linked_worktree();
    assert!(!linked.join(&screenshot.path).exists());
    assert!(compatible_success(linked, ModuleId::G5).unwrap().is_some());
    let archived = super::archived_artifact(linked, &screenshot.sha256);
    assert_eq!(receipt::sha256(&archived).unwrap(), screenshot.sha256);

    // A screenshot whose bytes no longer match its listed hash cannot be recorded.
    write(&clone.main, &screenshot.path, "tampered");
    assert!(record_success(&clone.main, ModuleId::G5, &first).is_err());
}

/// Normalization is decided for the manifest set as a whole: once the lockfile
/// disagrees with the workspace version, a version change in `Cargo.toml` alone must
/// change every functional fingerprint instead of hiding behind the raw lockfile.
#[test]
fn an_inconsistent_lockfile_keeps_the_version_in_every_fingerprint() {
    let clone = ReleaseClone::new();
    write(&clone.main, "Cargo.lock", &lockfile("0.0.9"));
    let before = modules()
        .map(|module| super::fingerprint::calculate(&clone.main, module).unwrap())
        .collect::<Vec<_>>();
    let manifest = fs::read_to_string(clone.main.join("Cargo.toml")).unwrap();
    write(
        &clone.main,
        "Cargo.toml",
        &manifest.replace("0.1.0", "0.1.1"),
    );
    for (module, before) in modules().zip(before) {
        assert_ne!(
            super::fingerprint::calculate(&clone.main, module).unwrap(),
            before,
            "{module:?}"
        );
    }
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

#[test]
fn real_manifests_release_bump_keeps_every_functional_fingerprint() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repository root");
    let clone = ReleaseClone::new();
    let members = [
        "crates/stickymd-core",
        "crates/stickymd-render",
        "apps/stickymd-win",
        "tools/stickymd-smoke",
    ];
    for relative in ["Cargo.toml", "Cargo.lock"]
        .into_iter()
        .map(str::to_owned)
        .chain(members.iter().map(|member| format!("{member}/Cargo.toml")))
    {
        let text = fs::read_to_string(repository.join(&relative)).expect("real manifest");
        write(&clone.main, &relative, &text);
    }
    commit(&clone.main);
    let modules = modules().collect::<Vec<_>>();
    let digests = |root: &Path| {
        super::fingerprint::PlanningInputs::read(root)
            .unwrap()
            .calculate_many(root, &modules)
            .unwrap()
            .digests
    };
    let before = digests(&clone.main);
    let workspace_before = super::fingerprint::workspace_inputs(&clone.main, b"").unwrap();

    let version = crate::repository::workspace_version(&clone.main).expect("workspace version");
    let bumped = "9.8.7";
    let manifest = fs::read_to_string(clone.main.join("Cargo.toml")).unwrap();
    let table = manifest
        .find("[workspace.package]")
        .expect("workspace.package");
    let (head, tail) = manifest.split_at(table);
    let tail = tail.replacen(
        &format!("version = \"{version}\""),
        &format!("version = \"{bumped}\""),
        1,
    );
    write(&clone.main, "Cargo.toml", &format!("{head}{tail}"));
    let mut lock = fs::read_to_string(clone.main.join("Cargo.lock")).unwrap();
    for member in members {
        let name = member.rsplit('/').next().unwrap();
        let old = format!("name = \"{name}\"\nversion = \"{version}\"\n");
        assert_eq!(lock.matches(&old).count(), 1, "{name}");
        lock = lock.replace(
            &old,
            &format!("name = \"{name}\"\nversion = \"{bumped}\"\n"),
        );
    }
    write(&clone.main, "Cargo.lock", &lock);
    commit(&clone.main);

    assert_eq!(
        before,
        digests(&clone.main),
        "release bump must not stale modules"
    );
    assert_ne!(
        workspace_before,
        super::fingerprint::workspace_inputs(&clone.main, b"").unwrap(),
        "the raw workspace identity must still change"
    );
}

#[test]
fn g5_reuse_verifies_archived_screenshots_and_the_record_origin() {
    let clone = ReleaseClone::new();
    let first = candidate("0.1.0", "c", "d");
    write_valid_evidence(&clone.main, ModuleId::G5, &first);
    record_success(&clone.main, ModuleId::G5, &first).unwrap();
    clone.release_bump("0.1.0", "0.1.1");
    let linked = clone.add_linked_worktree();
    assert!(matches!(
        lookup(linked, ModuleId::G5).unwrap(),
        Lookup::Valid(_)
    ));

    // The record's origin is authoritative: evidence from another version is stale.
    let record = super::record_path(linked, ModuleId::G5);
    let original = fs::read_to_string(&record).unwrap();
    fs::write(
        &record,
        original.replace(
            "\"origin_version\":\"0.1.0\"",
            "\"origin_version\":\"9.9.9\"",
        ),
    )
    .unwrap();
    let Lookup::Invalid(reason) = lookup(linked, ModuleId::G5).unwrap() else {
        panic!("a foreign origin version must invalidate the success");
    };
    assert!(reason.contains("version"), "{reason}");
    fs::write(&record, original).unwrap();

    // A missing archived screenshot makes the success invalid for reuse and readiness
    // alike, and the next formal registration of a passing run repairs the archive.
    let document = fs::read_to_string(clone.main.join(ModuleId::G5.receipt())).unwrap();
    let first_sha = g5_companions(&document).unwrap()[0].sha256.clone();
    fs::remove_file(super::archived_artifact(linked, &first_sha)).unwrap();
    let invalid = lookup(linked, ModuleId::G5).unwrap();
    assert_eq!(invalid.rerun_reason(), "INVALID_LAST_SUCCESS");
    let Lookup::Invalid(reason) = invalid else {
        unreachable!()
    };
    assert!(reason.contains("archived companion"), "{reason}");
    record_success(&clone.main, ModuleId::G5, &first).unwrap();
    assert!(matches!(
        lookup(linked, ModuleId::G5).unwrap(),
        Lookup::Valid(_)
    ));
}

#[test]
fn artifact_cleanup_keeps_screenshots_of_cases_unknown_to_this_tool() {
    let clone = ReleaseClone::new();
    let first = candidate("0.1.0", "c", "d");
    write_valid_evidence(&clone.main, ModuleId::G5, &first);
    record_success(&clone.main, ModuleId::G5, &first).unwrap();
    // A newer tool recorded a success (under other inputs) with a case this tool does
    // not know; this tool cannot validate it, but cleanup must keep its screenshot.
    let store = super::LedgerStore::for_repository(&clone.main).unwrap();
    let future_bytes = b"future case screenshot";
    let future_sha = crate::integrity::sha256_bytes(future_bytes).unwrap();
    let evidence = format!(
        "{{\"results\":[{{\"id\":\"G5-99\",\"status\":\"PASSED\",\"artifacts\":[{{\"path\":\"dist/evidence/g5-artifacts/G5-99.png\",\"sha256\":\"{future_sha}\"}}]}}]}}"
    );
    let record = super::record::LedgerRecord {
        module_id: "g5".to_owned(),
        input_fingerprint: "9".repeat(64),
        origin_source_commit: first.source_commit.clone(),
        origin_version: "0.2.0".to_owned(),
        origin_exe_sha256: first.exe_sha256.clone(),
        origin_zip_sha256: first.zip_sha256.clone(),
        evidence_sha256: crate::integrity::sha256_bytes(evidence.as_bytes()).unwrap(),
        recorded_at_unix: super::store::unix_seconds(),
    };
    for (path, bytes) in [
        (
            store.artifact(&future_sha).unwrap(),
            future_bytes.as_slice(),
        ),
        (
            store.evidence(&record.evidence_file()).unwrap(),
            evidence.as_bytes(),
        ),
        (
            store
                .module_record("g5", &record.input_fingerprint)
                .unwrap(),
            record.render().as_bytes(),
        ),
    ] {
        crate::atomic_evidence::write(&path, bytes).unwrap();
    }
    // Another success for this tool's inputs triggers cleanup; the screenshot stays.
    record_success(&clone.main, ModuleId::G5, &first).unwrap();
    assert!(store.artifact(&future_sha).unwrap().is_file());
    // Malformed artifact entries are not "no artifacts": recording fails closed.
    let document = fs::read_to_string(clone.main.join(ModuleId::G5.receipt())).unwrap();
    let sha256 = g5_companions(&document).unwrap()[0].sha256.clone();
    let malformed = document.replace(&sha256, "not-a-digest");
    receipt::write_receipt(&clone.main, ModuleId::G5.receipt(), &malformed).unwrap();
    assert!(record_success(&clone.main, ModuleId::G5, &first).is_err());
}

#[test]
fn diagnostics_cannot_target_the_shared_store() {
    let clone = ReleaseClone::new();
    let linked = clone.add_linked_worktree();
    let store = super::LedgerStore::for_repository(linked).unwrap();
    let digest = "0".repeat(64);
    for target in [
        store.module_record("g4", &digest).unwrap(),
        store.evidence(&format!("g4-{digest}.json")).unwrap(),
        store.artifact(&digest).unwrap(),
        store
            .root()
            .join("..")
            .join("qualification-ledger")
            .join(".lock"),
    ] {
        assert!(
            crate::qualification::validate_public_evidence_path(linked, &target).is_err(),
            "{}",
            target.display()
        );
    }
    // The rule needs no git: a differently cased alias is still recognized.
    let shouting = store
        .root()
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("STICKYMD")
        .join("Qualification-Ledger")
        .join("modules");
    assert!(crate::qualification::validate_public_evidence_path(linked, &shouting).is_err());
    // Ordinary diagnostic paths stay available.
    assert!(
        crate::qualification::validate_public_evidence_path(
            linked,
            Path::new("target/diagnostics/runtime.json")
        )
        .is_ok()
    );
}

#[cfg(windows)]
#[test]
fn a_store_root_redirected_by_a_junction_is_still_reserved() {
    let clone = ReleaseClone::new();
    let store = super::LedgerStore::for_repository(&clone.main).unwrap();
    let real = clone
        .main
        .parent()
        .expect("fixture base")
        .join("ledger-cache");
    fs::create_dir_all(&real).unwrap();
    fs::create_dir_all(store.root().parent().unwrap()).unwrap();
    // Junctions need no privilege; mklink expects backslash spellings.
    let link = store.root().to_string_lossy().replace('/', "\\");
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J", &link])
        .arg(&real)
        .stdout(std::process::Stdio::null())
        .status()
        .expect("start mklink");
    assert!(status.success(), "create junction");
    // The real location carries no store segment; it is still the store.
    let through_real = real
        .join("modules")
        .join("g4")
        .join(format!("{}.json", "0".repeat(64)));
    assert!(
        crate::qualification::validate_public_evidence_path(&clone.main, &through_real).is_err()
    );
    // Removing the clone removes the junction without following it.
    drop(clone);
    let _ = fs::remove_dir_all(&real);
}

#[test]
fn git_metadata_pointers_must_name_existing_directories() {
    let clone = ReleaseClone::new();
    let linked = clone.add_linked_worktree();
    let common = super::paths::git_common_dir_from_files(linked)
        .unwrap()
        .expect("linked worktree has git metadata");
    assert_eq!(
        common.canonicalize().unwrap(),
        clone.main.join(".git").canonicalize().unwrap()
    );
    assert_eq!(
        super::paths::git_common_dir_from_files(&clone.main).unwrap(),
        Some(clone.main.join(".git"))
    );

    let broken = clone.main.parent().unwrap().join("broken-gitfile");
    let private = broken.join("private-gitdir");
    fs::create_dir_all(&private).unwrap();
    for (gitfile, commondir) in [
        ("gitdir:\n".to_owned(), None),
        (
            format!("gitdir: {}\n", broken.join("missing").display()),
            None,
        ),
        (format!("gitdir: {}\n", private.display()), Some("")),
        (
            format!("gitdir: {}\n", private.display()),
            Some("../nowhere"),
        ),
        ("not a gitfile\n".to_owned(), None),
    ] {
        fs::write(broken.join(".git"), &gitfile).unwrap();
        let _ = fs::remove_file(private.join("commondir"));
        if let Some(text) = commondir {
            fs::write(private.join("commondir"), text).unwrap();
        }
        assert!(
            super::paths::git_common_dir_from_files(&broken).is_err(),
            "{gitfile:?} {commondir:?}"
        );
        // An uninterpretable `.git` leaves the store unknown: diagnostics are refused.
        assert!(
            crate::qualification::validate_public_evidence_path(
                &broken,
                Path::new("target/diagnostics/run.json")
            )
            .is_err()
        );
    }
    let _ = fs::remove_dir_all(&broken);
}

#[cfg(windows)]
#[test]
fn a_dangling_dot_git_link_is_not_mistaken_for_no_repository() {
    let clone = ReleaseClone::new();
    let root = clone.main.parent().unwrap().join("dangling-dot-git");
    fs::create_dir_all(&root).unwrap();
    let link = root.join(".git");
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(&link)
        .arg(root.join("missing-target"))
        .stdout(std::process::Stdio::null())
        .status()
        .expect("start mklink");
    assert!(status.success(), "create junction");
    assert!(super::paths::git_common_dir_from_files(&root).is_err());
    assert!(
        crate::qualification::validate_public_evidence_path(
            &root,
            Path::new("target/diagnostics/run.json")
        )
        .is_err()
    );
    let _ = fs::remove_dir_all(&root);
}

#[cfg(windows)]
#[test]
fn a_link_below_the_store_root_makes_the_store_refuse_to_operate() {
    let clone = ReleaseClone::new();
    let first = candidate("0.1.0", "c", "d");
    write_valid_evidence(&clone.main, ModuleId::G4, &first);
    record_success(&clone.main, ModuleId::G4, &first).unwrap();
    let store = super::LedgerStore::for_repository(&clone.main).unwrap();
    let evidence = store.root().join("evidence");
    let real = clone.main.parent().unwrap().join("evidence-elsewhere");
    fs::rename(&evidence, &real).unwrap();
    let link = evidence.to_string_lossy().replace('/', "\\");
    let status = Command::new("cmd")
        .args(["/C", "mklink", "/J", &link])
        .arg(&real)
        .stdout(std::process::Stdio::null())
        .status()
        .expect("start mklink");
    assert!(status.success(), "create junction");
    // Reads and writes both fail closed while the redirected subdirectory exists.
    assert!(compatible_success(&clone.main, ModuleId::G4).is_err());
    assert!(record_success(&clone.main, ModuleId::G4, &first).is_err());
    drop(clone);
    let _ = fs::remove_dir_all(&real);
}

/// Opt-in measurement: every module recorded, then looked up one by one versus one
/// snapshot (`--ignored --nocapture`). The fixture repository is small, so this isolates
/// the store, git and validation costs from fingerprinting a real checkout.
#[test]
#[ignore = "explicit timing profile for whole-ledger lookup"]
fn lookup_all_profile() {
    let clone = ReleaseClone::new();
    let first = candidate("0.1.0", "c", "d");
    for module in modules() {
        write_valid_evidence(&clone.main, module, &first);
        record_success(&clone.main, module, &first).unwrap();
    }
    for round in 1..=3 {
        let started = std::time::Instant::now();
        for module in modules() {
            assert!(matches!(
                lookup(&clone.main, module).unwrap(),
                Lookup::Valid(_)
            ));
        }
        let separate = started.elapsed().as_secs_f64();
        let started = std::time::Instant::now();
        let snapshot = super::lookup_all(&clone.main).unwrap();
        let together = started.elapsed().as_secs_f64();
        assert!(
            snapshot
                .modules
                .iter()
                .all(|(_, found)| matches!(found, Lookup::Valid(_)))
        );
        println!(
            "LOOKUP_PROFILE round={round} modules={} separate_seconds={separate:.6} snapshot_seconds={together:.6}",
            snapshot.modules.len()
        );
    }
}

fn write_valid_evidence(root: &Path, module: ModuleId, candidate: &Candidate) {
    let input = super::fingerprint::calculate(root, module).unwrap();
    let document = valid_document(root, module, candidate, &input, "run");
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
