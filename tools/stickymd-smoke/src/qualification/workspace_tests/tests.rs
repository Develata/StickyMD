//! Controlled executor/identity regressions; these are not desktop qualification evidence.
use super::*;
use std::{cell::Cell, fs, path::PathBuf};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "stickymd-shared-tests-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn receipt(&self) -> Vec<u8> {
        fs::read(self.0.join(RECEIPT)).unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn identity() -> Identity {
    Identity {
        source: "a".repeat(40),
        fingerprint: "b".repeat(64),
        reusable: true,
    }
}

fn seed(fixture: &Fixture) {
    assert_eq!(
        execute_with(&fixture.0, || Ok(identity()), || Ok(())).status,
        EvidenceStatus::Passed
    );
}

#[test]
fn formal_channels_run_the_complete_prerequisite_once_and_report_reuse_origin() {
    let fixture = Fixture::new();
    let runs = Cell::new(0);
    for (mode, expected) in [("runtime", "RAN_PASS"), ("performance", "REUSED_PASS")] {
        let options = Options::parse([
            "phase".into(),
            "14".into(),
            format!("--{mode}"),
            format!("--evidence-file=dist/evidence/{mode}-qualification.json"),
        ])
        .unwrap();
        assert!(eligible(&fixture.0, &options));
        let result = execute_with(
            &fixture.0,
            || Ok(identity()),
            || {
                runs.set(runs.get() + 1);
                Ok(())
            },
        );
        assert_eq!(result.status, EvidenceStatus::Passed);
        assert!(result.detail.unwrap().starts_with(expected));
        assert!(result.measurements.iter().all(|value| value.value >= 0.0));
        if expected == "REUSED_PASS" {
            assert!(
                result
                    .measurements
                    .iter()
                    .any(|m| m.name == "workspace.origin_run_seconds")
            );
            assert!(
                !result
                    .measurements
                    .iter()
                    .any(|m| m.name == "workspace.run_seconds")
            );
        }
    }
    assert_eq!(runs.get(), 1);
    assert!(
        crate::qualification::validate_public_evidence_path(&fixture.0, Path::new(RECEIPT))
            .is_err()
    );
}

#[test]
fn changed_source_or_execution_identity_runs_again_without_reusing_old_success() {
    let fixture = Fixture::new();
    seed(&fixture);
    let mut input = identity();
    for change_source in [false, true] {
        if change_source {
            input.source = "c".repeat(40);
        } else {
            input.fingerprint = "d".repeat(64);
        }
        let runs = Cell::new(0);
        let result = execute_with(
            &fixture.0,
            || Ok(input.clone()),
            || {
                runs.set(runs.get() + 1);
                Ok(())
            },
        );
        assert_eq!(result.status, EvidenceStatus::Passed);
        assert_eq!(runs.get(), 1);
    }
}

#[test]
fn failed_tests_and_input_drift_preserve_the_previous_success() {
    let fixture = Fixture::new();
    seed(&fixture);
    let previous = fixture.receipt();
    let changed = Identity {
        fingerprint: "e".repeat(64),
        ..identity()
    };
    let failed = execute_with(
        &fixture.0,
        || Ok(changed.clone()),
        || Err("test assertion failed".into()),
    );
    assert_eq!(failed.status, EvidenceStatus::Failed);
    assert!(failed.detail.unwrap().contains("test assertion failed"));
    assert_eq!(fixture.receipt(), previous);
    let calls = Cell::new(0);
    let drift = execute_with(
        &fixture.0,
        || {
            calls.set(calls.get() + 1);
            if calls.get() == 1 {
                Ok(changed.clone())
            } else {
                Ok(identity())
            }
        },
        || Ok(()),
    );
    assert_eq!(drift.status, EvidenceStatus::Failed);
    assert!(drift.detail.unwrap().contains("changed during tests"));
    assert_eq!(fixture.receipt(), previous);
}

#[test]
fn changes_during_reuse_are_rejected_and_uncacheable_settings_never_update_success() {
    let fixture = Fixture::new();
    seed(&fixture);
    let previous = fixture.receipt();
    let calls = Cell::new(0);
    let result = execute_with(
        &fixture.0,
        || {
            calls.set(calls.get() + 1);
            Ok(if calls.get() == 1 {
                identity()
            } else {
                Identity {
                    fingerprint: "f".repeat(64),
                    ..identity()
                }
            })
        },
        || panic!("reuse must not start cargo"),
    );
    assert_eq!(result.status, EvidenceStatus::Failed);
    let runs = Cell::new(0);
    for _ in 0..2 {
        let result = execute_with(
            &fixture.0,
            || {
                Ok(Identity {
                    reusable: false,
                    ..identity()
                })
            },
            || {
                runs.set(runs.get() + 1);
                Ok(())
            },
        );
        assert_eq!(result.status, EvidenceStatus::Passed);
        assert!(result.detail.unwrap().contains("BYPASS"));
    }
    assert_eq!(runs.get(), 2);
    assert_eq!(fixture.receipt(), previous);
}

#[test]
fn malformed_partial_or_failed_receipts_cannot_suppress_a_full_run() {
    let fixture = Fixture::new();
    seed(&fixture);
    let complete = String::from_utf8(fixture.receipt()).unwrap();
    for invalid in [
        "{}".into(),
        complete.replace("FULL_WORKSPACE", "SELECTED_MODULE"),
        complete.replace("PASSED", "FAILED"),
        complete.replace(COMMAND, "cargo test -p stickymd-smoke"),
        complete.replace("SOURCE_BOUND", "ARTIFACT_BOUND"),
    ] {
        crate::atomic_evidence::write(&fixture.0.join(RECEIPT), invalid.as_bytes()).unwrap();
        let result = execute_with(&fixture.0, || Ok(identity()), || Ok(()));
        assert_eq!(result.status, EvidenceStatus::Passed);
        assert!(result.detail.unwrap().starts_with("RAN_PASS"));
    }
}

#[test]
fn receipt_write_failure_cannot_be_reported_as_success() {
    let fixture = Fixture::new();
    fs::create_dir_all(fixture.0.join(RECEIPT)).unwrap();
    let result = execute_with(&fixture.0, || Ok(identity()), || Ok(()));
    assert_eq!(result.status, EvidenceStatus::Failed);
    assert!(fixture.0.join(RECEIPT).is_dir());
}

#[test]
fn diagnostic_partial_and_ci_requests_are_ineligible_for_shared_success() {
    let root = std::env::temp_dir();
    for args in [
        vec!["phase", "14", "--runtime"],
        vec!["phase", "14", "--ci"],
        vec![
            "phase",
            "08",
            "--runtime",
            "--evidence-file=dist/evidence/runtime-qualification.json",
        ],
        vec![
            "phase",
            "14",
            "--runtime",
            "--evidence-file=diagnostic.json",
        ],
        vec![
            "phase",
            "14",
            "--performance",
            "--evidence-file=dist/evidence/runtime-qualification.json",
        ],
    ] {
        assert!(!eligible(
            &root,
            &Options::parse(args.into_iter().map(str::to_owned)).unwrap()
        ));
    }
}

#[test]
fn real_identity_checks_a_clean_freeze_and_rejects_a_later_source_edit() {
    let fixture = Fixture::new();
    initialize_source(&fixture);
    let first = super::identity::capture(&fixture.0).unwrap();
    let second = super::identity::capture(&fixture.0).unwrap();
    assert_eq!(first, second);
    // The executor is controlled; all Git, freeze, toolchain, config and byte checks are real.
    let runs = Cell::new(0);
    for _ in 0..2 {
        assert_eq!(
            execute(&fixture.0, || {
                runs.set(runs.get() + 1);
                Ok(())
            })
            .status,
            EvidenceStatus::Passed
        );
    }
    assert_eq!(runs.get(), if first.reusable { 1 } else { 2 });
    fs::write(fixture.0.join("Cargo.toml"), "changed while testing").unwrap();
    let result = execute(&fixture.0, || {
        panic!("dirty source cannot start shared prerequisite")
    });
    assert_eq!(result.status, EvidenceStatus::Failed);
    assert!(result.detail.unwrap().contains("clean worktree"));
}

fn initialize_source(fixture: &Fixture) {
    fs::write(
        fixture.0.join("Cargo.toml"),
        "[workspace]\n[workspace.package]\nversion = \"0.1.1\"\n",
    )
    .unwrap();
    fs::write(fixture.0.join("Cargo.lock"), "version = 4\n").unwrap();
    fs::write(fixture.0.join(".gitignore"), "/dist/\n/cache/\n/subdir/\n").unwrap();
    let git = |args: &[&str]| {
        crate::repository::command_text(&fixture.0, "git", args).unwrap();
    };
    git(&["init", "--quiet"]);
    git(&["add", "."]);
    git(&[
        "-c",
        "user.name=Smoke Test",
        "-c",
        "user.email=smoke@example.invalid",
        "-c",
        "commit.gpgsign=false",
        "-c",
        "core.hooksPath=disabled-hooks",
        "commit",
        "--quiet",
        "-m",
        "fixture",
    ]);
    crate::qualification::source_freeze::create(&fixture.0).unwrap();
}

#[test]
fn relative_cargo_home_uses_the_cargo_child_working_directory() {
    const CHILD_ROOT: &str = "SMOKE_RELATIVE_CARGO_HOME_FIXTURE";
    if let Some(root) = std::env::var_os(CHILD_ROOT) {
        let root = PathBuf::from(root);
        let cargo_home = std::env::var_os("CARGO_HOME").unwrap();
        let config = root.join(if cargo_home.is_empty() {
            "cache/.cargo/config.toml"
        } else {
            "cache/config.toml"
        });
        let before = super::identity::capture(&root).unwrap();
        fs::write(config, "invalid TOML = [").unwrap();
        let after = super::identity::capture(&root).unwrap();
        if cargo_home.is_empty() || cargo_home == "cache" {
            assert_ne!(
                before.fingerprint, after.fingerprint,
                "Cargo reads the changed config"
            );
        } else {
            assert!(
                !before.reusable,
                "drive-relative configuration cannot share success"
            );
        }
        assert!(!after.reusable);
        let output = std::process::Command::new("cargo")
            .args(ARGS)
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(String::from_utf8_lossy(&output.stderr).contains("config.toml"));
        return;
    }
    let fixture = Fixture::new();
    initialize_source(&fixture);
    fs::create_dir_all(fixture.0.join("cache/.cargo")).unwrap();
    fs::create_dir(fixture.0.join("subdir")).unwrap();
    let rustup_home =
        crate::repository::command_text(&fixture.0, "rustup", &["show", "home"]).unwrap();
    let mut homes = vec![std::ffi::OsString::from("cache"), std::ffi::OsString::new()];
    #[cfg(windows)]
    if let Some(std::path::Component::Prefix(prefix)) = fixture.0.components().next()
        && let std::path::Prefix::Disk(drive) | std::path::Prefix::VerbatimDisk(drive) =
            prefix.kind()
    {
        homes.push(format!("{}:cache", char::from(drive)).into());
    }
    for home in homes.drain(..) {
        fs::write(
            fixture.0.join("cache/config.toml"),
            "# supported empty config\n",
        )
        .unwrap();
        fs::write(
            fixture.0.join("cache/.cargo/config.toml"),
            "# supported empty config\n",
        )
        .unwrap();
        let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "qualification::workspace_tests::tests::relative_cargo_home_uses_the_cargo_child_working_directory", "--nocapture"])
        .env("CARGO_HOME", home)
        .env("USERPROFILE", fixture.0.join("cache"))
        .env("HOME", fixture.0.join("cache"))
        .env("RUSTUP_HOME", &rustup_home)
        .env(CHILD_ROOT, &fixture.0)
        .current_dir(fixture.0.join("subdir"))
        .output()
        .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}
