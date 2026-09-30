//! Cross-invocation aliases retain complete raw cohorts and their original lifetime.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{
    Store, fs, record,
    tests::{Root, identity},
};
use crate::{
    cli::ResourceModule,
    evidence::EvidenceResult,
    resource_plan::{self, ResourceCase, diagnostic::Unit},
};

pub(super) fn result(case: ResourceCase) -> EvidenceResult {
    let group = resource_plan::GROUPS
        .into_iter()
        .find(|g| g.cases().contains(&case))
        .unwrap();
    let mut result = resource_plan::tests::valid_resource_result(group);
    result.id = case.label.into();
    result
        .measurements
        .retain(|m| m.name.starts_with(&format!("{}.", case.label)));
    result
        .gates
        .retain(|g| g.metric.starts_with(&format!("{}.", case.label)));
    result.samples.retain(|s| s.cohort == case.label);
    result
}

#[test]
fn all_four_equivalent_pairs_reuse_both_directions_without_new_records_or_renewal() {
    let source = ResourceModule::SourcePreview.cases();
    let math = ResourceModule::Math.cases();
    let images = ResourceModule::Images.cases();
    for (a, b) in [
        (source[0], math[0]),
        (source[1], math[3]),
        (source[2], math[4]),
        (math[1], images[2]),
    ] {
        for (origin, target) in [(a, b), (b, a)] {
            let root = Root::new();
            let store = Store {
                identity: identity(),
                host: String::new(),
            };
            let original = result(origin);
            store
                .save_using(&root.0, origin.into(), &original, 150.0, || Ok(identity()))
                .unwrap();
            let path = store.path(&root.0, origin.into()).unwrap();
            let bytes = fs::read(&path).unwrap();
            // A distinct Store represents a new command; no ScenarioCache is shared.
            let next = Store {
                identity: identity(),
                host: String::new(),
            };
            let lookup = next.inspect(&root.0, target.into()).unwrap();
            assert!(lookup.reason.starts_with("EQUIVALENT_COMPLETE_RECORD:"));
            let reused = next
                .load_using(&root.0, target.into(), || Ok(identity()))
                .unwrap()
                .unwrap();
            assert_eq!(reused.id, target.label);
            assert_eq!(reused.samples.len(), 5);
            assert_eq!(reused.gates, result(target).gates);
            for (actual, initial) in reused.samples.iter().zip(&original.samples) {
                assert_eq!(actual.cohort, target.label);
                assert_eq!(actual.run, initial.run);
                assert_eq!(actual.measurements, initial.measurements);
                assert!(
                    actual
                        .shared_from
                        .as_deref()
                        .unwrap()
                        .ends_with(&format!(":{}", origin.label))
                );
            }
            let detail = reused.detail.as_deref().unwrap();
            assert!(detail.contains(&format!("origin_unit={}", origin.label)));
            assert!(detail.contains(&format!("requested_unit={}", target.label)));
            assert_eq!(
                reused.measurements.last().unwrap().name,
                format!("{}.origin_execution_seconds", target.label)
            );
            assert!(
                record::encode(
                    &identity(),
                    target.into(),
                    &reused,
                    1.0,
                    super::now().unwrap()
                )
                .is_err()
            );
            assert_eq!(fs::read(&path).unwrap(), bytes);
            assert!(!next.path(&root.0, target.into()).unwrap().exists());
        }
    }
}

#[test]
fn alternatives_still_reject_corrupt_expired_or_different_protocols() {
    let root = Root::new();
    let store = Store {
        identity: identity(),
        host: String::new(),
    };
    let origin = ResourceModule::Math.cases()[1];
    let target = ResourceModule::Images.cases()[2];
    let path = store.path(&root.0, origin.into()).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    for document in [
        "corrupt".into(),
        record::encode(&identity(), origin.into(), &result(origin), 150.0, 1).unwrap(),
    ] {
        fs::write(&path, document).unwrap();
        assert!(
            store
                .inspect(&root.0, target.into())
                .unwrap()
                .result
                .is_none()
        );
    }
    store
        .save_using(&root.0, origin.into(), &result(origin), 150.0, || {
            Ok(identity())
        })
        .unwrap();
    for case in ResourceModule::Images
        .cases()
        .iter()
        .filter(|c| **c != target)
    {
        assert!(
            store
                .inspect(&root.0, (*case).into())
                .unwrap()
                .result
                .is_none()
        );
    }
    assert!(Unit::Group(ResourceModule::Window).equivalents().is_empty());
    assert!(Unit::Group(ResourceModule::Zoom).equivalents().is_empty());
    let mut reused = store
        .inspect(&root.0, target.into())
        .unwrap()
        .result
        .unwrap();
    assert!(
        Unit::Case(ResourceModule::Math.cases()[2])
            .project(target.into(), &mut reused)
            .is_err()
    );
}
