//! Diagnostic cache invalidation, raw evidence and storage boundary regression tests.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::*;
use crate::{
    cli::ResourceModule,
    evidence::{EvidenceStatus, escape_json},
    release::json,
};

pub(super) fn identity() -> Identity {
    Identity {
        fingerprint: "a".repeat(64),
        source: "b".repeat(40),
        executable: "c".repeat(64),
        harness: "d".repeat(64),
        inputs: "e".repeat(64),
        environment: "f".repeat(64),
        dirty: false,
    }
}

fn case_result(cpu: bool) -> (Unit, EvidenceResult) {
    let case = ResourceModule::Math.cases()[if cpu { 0 } else { 1 }];
    let mut result = crate::resource_plan::tests::valid_resource_result(ResourceModule::Math);
    result.id = case.label.into();
    result
        .measurements
        .retain(|m| m.name.starts_with(&format!("{}.", case.label)));
    result
        .gates
        .retain(|g| g.metric.starts_with(&format!("{}.", case.label)));
    result.samples.retain(|s| s.cohort == case.label);
    (case.into(), result)
}

pub(super) struct Root(pub(super) PathBuf);
impl Root {
    pub(super) fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "stickymd-resource-resume-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        // This test exclusively created the absolute temp root. Tests detach any owned junction first.
        assert!(self.0.starts_with(std::env::temp_dir()));
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn complete_diagnostic_case_roundtrips_with_historical_origin_and_separate_duration() {
    for cpu in [false, true] {
        let (case, result) = case_result(cpu);
        let document = record::encode(&identity(), case, &result, 151.25, 1_000).unwrap();
        let reused = record::decode(&document, &identity(), case, 1_001).unwrap();
        assert!(
            reused
                .detail
                .as_deref()
                .unwrap()
                .contains("DIAGNOSTIC_REUSED")
        );
        assert_eq!(reused.samples.len(), 5);
        for (new, old) in reused.samples.iter().zip(&result.samples) {
            assert_eq!(new.measurements, old.measurements);
            assert!(
                new.shared_from
                    .as_deref()
                    .unwrap()
                    .starts_with("diagnostic-cache:")
            );
        }
        assert_eq!(
            reused.measurements.last().unwrap().name,
            format!("{}.origin_execution_seconds", case.key())
        );
        assert_eq!(reused.measurements.last().unwrap().value, 151.25);
        assert!(record::encode(&identity(), case, &reused, 0.2, 1_002).is_err());
        assert!(crate::resource_plan::validate_receipt(&document, ResourceModule::Math).is_err());
    }
}

#[test]
fn diagnostic_cache_rejects_partial_failed_inconsistent_or_shared_observations() {
    let (case, good) = case_result(true);
    let reject = |mutate: fn(&mut EvidenceResult)| {
        let mut invalid = good.clone();
        mutate(&mut invalid);
        assert!(record::encode(&identity(), case, &invalid, 450.0, 100).is_err());
    };
    reject(|r| r.status = EvidenceStatus::Failed);
    reject(|r| {
        r.samples.pop();
    });
    reject(|r| r.samples[1].run = 1);
    reject(|r| r.samples[0].measurements[0].unit = "MB".into());
    reject(|r| r.samples[0].measurements[0].value = f64::NAN);
    reject(|r| r.samples[0].shared_from = Some("source".into()));
    reject(|r| {
        r.gates.clear();
    });
    reject(|r| r.gates[0].value = 1.0);
    reject(|r| {
        r.measurements
            .iter_mut()
            .find(|m| m.name.ends_with("warmup_seconds"))
            .unwrap()
            .value = 1.0
    });
    reject(|r| {
        r.measurements
            .iter_mut()
            .find(|m| m.name.ends_with("private_bytes_max"))
            .unwrap()
            .value += 1.0
    });
    reject(|r| {
        for sample in &mut r.samples {
            sample
                .measurements
                .iter_mut()
                .find(|m| m.name == "idle_cpu")
                .unwrap()
                .value = 0.11;
        }
        for m in &mut r.measurements {
            if m.name.contains("idle_cpu_") {
                m.value = 0.11;
            }
        }
    });
}

#[test]
fn diagnostic_cache_rejects_corruption_expiry_future_and_any_changed_identity_component() {
    let (case, result) = case_result(false);
    let document = record::encode(&identity(), case, &result, 150.0, 100).unwrap();
    assert!(record::decode(&document, &identity(), case, 100 + MAX_AGE).is_ok());
    for time in [99, 101 + MAX_AGE] {
        assert!(record::decode(&document, &identity(), case, time).is_err());
    }
    assert!(
        record::decode(
            &document.replace("DIAGNOSTIC_ONLY", "INVALID"),
            &identity(),
            case,
            100
        )
        .is_err()
    );
    for part in 0..4 {
        let mut changed = identity();
        match part {
            0 => changed.fingerprint = "0".repeat(64),
            1 => changed.source = "0".repeat(40),
            2 => changed.executable = "0".repeat(64),
            _ => changed.harness = "0".repeat(64),
        }
        assert!(record::decode(&document, &changed, case, 100).is_err());
    }
    let outer = json::parse(&document).unwrap();
    let invalid = outer.field("body").unwrap().string().unwrap().replace(
        "memory_samples\\\",\\\"unit\\\":\\\"count\\\",\\\"value\\\":5.000000",
        "memory_samples\\\",\\\"unit\\\":\\\"count\\\",\\\"value\\\":4.000000",
    );
    assert_ne!(invalid, outer.field("body").unwrap().string().unwrap());
    let forged = format!(
        "{{\"sha256\":\"{}\",\"body\":\"{}\"}}",
        digest::bytes(invalid.as_bytes()).unwrap(),
        escape_json(&invalid)
    );
    assert!(record::decode(&forged, &identity(), case, 100).is_err());
}

#[test]
fn diagnostic_store_misses_invalid_records_and_aborts_on_drift_without_overwriting_success() {
    let root = Root::new();
    let store = Store {
        identity: identity(),
        host: String::new(),
    };
    let (case, result) = case_result(false);
    let load = || store.load_using(&root.0, case, || Ok(identity()));
    assert!(load().unwrap().is_none());
    store
        .save_using(&root.0, case, &result, 150.0, || Ok(identity()))
        .unwrap();
    assert!(load().unwrap().is_some());
    let path = store.path(&root.0, case).unwrap();
    let original = fs::read(&path).unwrap();
    let mut checks = 0;
    let drift = store.load_using(&root.0, case, || {
        checks += 1;
        let mut value = identity();
        if checks == 2 {
            value.harness = "0".repeat(64);
        }
        Ok(value)
    });
    assert!(drift.unwrap_err().contains("changed"));
    let mut failed = result.clone();
    failed.samples.pop();
    assert!(
        store
            .save_using(&root.0, case, &failed, 10.0, || Ok(identity()))
            .is_err()
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(crate::qualification::validate_public_evidence_path(&root.0, &path).is_err());
    fs::write(&path, "broken JSON").unwrap();
    assert!(load().unwrap().is_none());
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(
        store
            .save_using(&root.0, case, &result, 150.0, || Ok(identity()))
            .is_err()
    );
    assert!(!root.0.join("dist").exists());
}

#[test]
fn diagnostic_digest_uses_sha256() {
    assert_eq!(
        digest::bytes(b"abc").unwrap(),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}

#[test]
fn every_load_and_save_keeps_two_fresh_checks_and_rejects_either_side_of_drift() {
    let root = Root::new();
    let store = Store {
        identity: identity(),
        host: String::new(),
    };
    let (unit, result) = case_result(false);
    store
        .save_using(&root.0, unit, &result, 150.0, || Ok(identity()))
        .unwrap();
    let path = store.path(&root.0, unit).unwrap();
    for saving in [false, true] {
        for drift_at in [0, 1, 2] {
            let original = fs::read(&path).unwrap();
            let mut checks = 0;
            let mut fresh = || {
                checks += 1;
                let mut id = identity();
                if checks == drift_at {
                    id.inputs = "0".repeat(64);
                }
                Ok(id)
            };
            let outcome = if saving {
                store.save_using(&root.0, unit, &result, 150.0, &mut fresh)
            } else {
                store.load_using(&root.0, unit, &mut fresh).map(|_| ())
            };
            assert_eq!(outcome.is_ok(), drift_at == 0);
            assert_eq!(checks, if drift_at == 1 { 1 } else { 2 });
            if drift_at != 0 {
                assert_eq!(fs::read(&path).unwrap(), original);
            }
        }
    }
}

#[test]
fn latest_pointer_explains_changed_components_but_never_reuses_old_identity() {
    let root = Root::new();
    let store = Store {
        identity: identity(),
        host: String::new(),
    };
    let (unit, result) = case_result(false);
    store
        .save_using(&root.0, unit, &result, 150.0, || Ok(identity()))
        .unwrap();
    for field in ["source", "executable", "harness", "inputs", "environment"] {
        let mut changed = identity();
        changed.fingerprint = "0".repeat(64);
        match field {
            "source" => changed.source = "0".repeat(40),
            "executable" => changed.executable = "0".repeat(64),
            "harness" => changed.harness = "0".repeat(64),
            "inputs" => changed.inputs = "0".repeat(64),
            _ => changed.environment = "0".repeat(64),
        }
        let changed = Store {
            identity: changed,
            host: String::new(),
        };
        let inspected = changed.inspect(&root.0, unit).unwrap();
        assert!(inspected.result.is_none());
        assert_eq!(inspected.reason, format!("IDENTITY_CHANGED: {field}"));
    }
    let hint = root
        .0
        .join(DIRECTORY)
        .join("latest")
        .join(format!("{}.key", unit.key()));
    fs::write(&hint, "../../dist").unwrap();
    // A corrupt advisory pointer does not override a valid current record.
    assert!(store.inspect(&root.0, unit).unwrap().result.is_some());
    fs::remove_file(store.path(&root.0, unit).unwrap()).unwrap();
    let inspected = store.inspect(&root.0, unit).unwrap();
    assert!(inspected.result.is_none());
    assert!(inspected.reason.contains("LATEST_UNAVAILABLE"));
}

#[test]
fn expired_and_corrupt_records_have_distinct_bounded_reasons() {
    let root = Root::new();
    let store = Store {
        identity: identity(),
        host: String::new(),
    };
    let (unit, result) = case_result(false);
    let path = store.path(&root.0, unit).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    for (created, reason) in [(1, "EXPIRED"), (now().unwrap() + 3600, "FUTURE_TIMESTAMP")] {
        fs::write(
            &path,
            record::encode(&identity(), unit, &result, 150.0, created).unwrap(),
        )
        .unwrap();
        let inspected = store.inspect(&root.0, unit).unwrap();
        assert!(inspected.result.is_none());
        assert_eq!(inspected.reason, format!("INVALID_RECORD: {reason}"));
    }
    let valid = record::encode(&identity(), unit, &result, 150.0, now().unwrap()).unwrap();
    fs::write(&path, valid.replace("DIAGNOSTIC_ONLY", "OTHER")).unwrap();
    assert_eq!(
        store.inspect(&root.0, unit).unwrap().reason,
        "INVALID_RECORD: CHECKSUM_MISMATCH"
    );
}

#[cfg(windows)]
#[test]
#[ignore = "explicit real identity timing; requires STICKYMD_SMOKE_PROBE_REPOSITORY"]
fn native_diagnostic_identity_lookup_profile() {
    let root = PathBuf::from(std::env::var_os("STICKYMD_SMOKE_PROBE_REPOSITORY").unwrap());
    let started = std::time::Instant::now();
    eprintln!("IDENTITY_PROFILE_BEGIN first_lookup");
    let store = Store::open(&root).unwrap();
    let _ = store
        .load(&root, Unit::Group(ResourceModule::Zoom))
        .unwrap();
    eprintln!(
        "IDENTITY_PROFILE_END first_lookup elapsed_seconds={:.6}",
        started.elapsed().as_secs_f64()
    );
    let started = std::time::Instant::now();
    eprintln!("IDENTITY_PROFILE_BEGIN subsequent_lookup");
    let _ = store
        .load(&root, Unit::Group(ResourceModule::Zoom))
        .unwrap();
    eprintln!(
        "IDENTITY_PROFILE_END subsequent_lookup elapsed_seconds={:.6}",
        started.elapsed().as_secs_f64()
    );
}

#[test]
fn diagnostic_inputs_include_tracked_and_untracked_fixture_bytes_and_execution_identity() {
    use crate::qualification::module_ledger::fingerprint;
    let root = Root::new();
    let git = |args: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .args(args)
                .current_dir(&root.0)
                .output()
                .unwrap()
                .status
                .success()
        )
    };
    git(&["init", "--quiet"]);
    fs::write(root.0.join("fixture.md"), "first").unwrap();
    git(&["add", "fixture.md"]);
    let initial = fingerprint::workspace_inputs(&root.0, b"environment-one").unwrap();
    assert_ne!(
        initial,
        fingerprint::workspace_inputs(&root.0, b"environment-two").unwrap()
    );
    fs::write(root.0.join("fixture.md"), "second").unwrap();
    assert_ne!(
        initial,
        fingerprint::workspace_inputs(&root.0, b"environment-one").unwrap()
    );
    fs::write(root.0.join("fixture.md"), "first").unwrap();
    fs::write(root.0.join("untracked-config"), "change").unwrap();
    assert_ne!(
        initial,
        fingerprint::workspace_inputs(&root.0, b"environment-one").unwrap()
    );
}

#[cfg(windows)]
#[test]
fn diagnostic_cache_aliases_are_protected_and_cannot_escape_target() {
    use std::os::windows::process::CommandExt;
    let root = Root::new();
    let cache = root.0.join("target/resource-diagnostics");
    let alias = root.0.join("alias");
    fs::create_dir_all(&cache).unwrap();
    let junction = |path: &Path, target: &Path| {
        assert!(std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", "New-Item -ItemType Junction -Path $env:SMOKE_DIAGNOSTIC_LINK -Target $env:SMOKE_DIAGNOSTIC_TARGET -ErrorAction Stop | Out-Null"])
            .env("SMOKE_DIAGNOSTIC_LINK", path).env("SMOKE_DIAGNOSTIC_TARGET", target)
            .creation_flags(0x0800_0000).output().unwrap().status.success());
    };
    junction(&alias, &cache);
    let protected = crate::qualification::validate_public_evidence_path(
        &root.0,
        &alias.join("v1/missing.json"),
    );
    fs::remove_dir(&alias).unwrap();
    assert!(protected.is_err());
    let outside = root.0.join("dist");
    fs::create_dir(&outside).unwrap();
    let version = cache.join("v2");
    junction(&version, &outside);
    let store = Store {
        identity: identity(),
        host: String::new(),
    };
    let escaped = store.path(&root.0, case_result(false).0);
    fs::remove_dir(&version).unwrap();
    assert!(escaped.unwrap_err().contains("escaped"));
    assert_eq!(fs::read_dir(&outside).unwrap().count(), 0);
}

#[cfg(windows)]
#[test]
#[ignore = "explicit live desktop identity check; requires STICKYMD_SMOKE_PROBE_REPOSITORY"]
fn native_diagnostic_resource_identity_is_stable() {
    let root = std::env::var_os("STICKYMD_SMOKE_PROBE_REPOSITORY")
        .map(PathBuf::from)
        .unwrap();
    let store = Store::open(&root).expect("complete diagnostic identity");
    store
        .verify(&root)
        .expect("same identity after a fresh byte/environment check");
}
