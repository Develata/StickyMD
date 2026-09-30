//! Batch evidence and fresh-identity boundaries, including a deliberately synthetic profile.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{
    Store,
    equivalence_tests::result,
    fs,
    tests::{Root, identity},
};
use crate::cli::ResourceModule;

#[test]
fn complete_batches_check_identity_twice_and_partial_or_changed_batches_return_nothing() {
    let root = Root::new();
    let store = Store {
        identity: identity(),
        host: String::new(),
    };
    let cases = &ResourceModule::Math.cases()[0..2];
    // The first case is available only through its equivalent Source label.
    let origin = ResourceModule::SourcePreview.cases()[0];
    for case in [origin, cases[1]] {
        store
            .save_using(
                &root.0,
                case.into(),
                &result(case),
                450.0,
                || Ok(identity()),
            )
            .unwrap();
    }
    for drift_at in [0, 1, 2] {
        let mut checks = 0;
        let loaded = store.load_batch_using(&root.0, cases, || {
            checks += 1;
            let mut id = identity();
            if checks == drift_at {
                id.inputs = "0".repeat(64);
            }
            Ok(id)
        });
        assert_eq!(checks, if drift_at == 1 { 1 } else { 2 });
        if drift_at == 0 {
            let loaded = loaded.unwrap().unwrap();
            assert_eq!(loaded.len(), 2);
            assert_eq!(loaded[0].id, cases[0].label);
            assert!(loaded.iter().all(|r| r.samples.len() == 5));
            assert!(
                loaded[0]
                    .detail
                    .as_ref()
                    .unwrap()
                    .contains("origin_unit=source;")
            );
        } else {
            assert!(loaded.is_err());
        }
    }
    // Simulate invalidation after planning; partial results must not survive across sampling.
    let path = store.path(&root.0, cases[1].into()).unwrap();
    fs::write(&path, "broken").unwrap();
    let mut checks = 0;
    assert!(
        store
            .load_batch_using(&root.0, cases, || {
                checks += 1;
                Ok(identity())
            })
            .unwrap()
            .is_none()
    );
    assert_eq!(checks, 2);
    fs::remove_file(path).unwrap();
    assert!(
        store
            .load_batch_using(&root.0, cases, || Ok(identity()))
            .unwrap()
            .is_none()
    );
    assert!(
        store
            .load_batch_using(&root.0, &[cases[0], cases[0]], || Ok(identity()))
            .is_err()
    );
}

#[cfg(windows)]
#[test]
#[ignore = "real identity cost on synthetic temp records; requires STICKYMD_SMOKE_PROBE_REPOSITORY; not resource acceptance"]
fn native_diagnostic_batch_identity_profile() {
    let repository =
        std::path::PathBuf::from(std::env::var_os("STICKYMD_SMOKE_PROBE_REPOSITORY").unwrap());
    let actual = Store::open(&repository).unwrap();
    let root = Root::new();
    let fixture = Store {
        identity: identity(),
        host: String::new(),
    };
    let math = ResourceModule::Math.cases();
    let cases = [math[1], math[2], math[5]];
    for case in cases {
        fixture
            .save_using(
                &root.0,
                case.into(),
                &result(case),
                150.0,
                || Ok(identity()),
            )
            .unwrap();
    }
    for batch in [false, true] {
        let mut checks = 0;
        let started = std::time::Instant::now();
        let mut fresh = || {
            checks += 1;
            actual.matches(super::identity::capture(&repository, &actual.host)?)?;
            // Synthetic records stay exclusively inside Root's temp directory.
            Ok(identity())
        };
        if batch {
            assert!(
                fixture
                    .load_batch_using(&root.0, &cases, &mut fresh)
                    .unwrap()
                    .is_some()
            );
        } else {
            for case in cases {
                assert!(
                    fixture
                        .load_using(&root.0, case.into(), &mut fresh)
                        .unwrap()
                        .is_some()
                );
            }
        }
        assert_eq!(checks, if batch { 2 } else { 6 });
        eprintln!(
            "SYNTHETIC_BATCH_PROFILE batch={batch} real_identity_captures={checks} elapsed_seconds={:.6}; NOT_RESOURCE_ACCEPTANCE",
            started.elapsed().as_secs_f64()
        );
    }
}
