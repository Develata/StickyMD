//! Whole-group diagnostic validation regression coverage.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{record, tests::identity};
use crate::{
    cli::ResourceModule,
    evidence::EvidenceStatus,
    resource_plan::{diagnostic::Unit, tests::valid_resource_result},
};

#[test]
fn whole_group_requires_all_samples_stress_fixtures_gates_and_fresh_origins() {
    for group in [ResourceModule::Window, ResourceModule::Zoom] {
        let unit = Unit::Group(group);
        let result = valid_resource_result(group);
        let document = record::encode(&identity(), unit, &result, 100.0, 100).unwrap();
        let reused = record::decode(&document, &identity(), unit, 101).unwrap();
        assert_eq!(reused.id, group.task_label());
        assert_eq!(reused.samples.len(), 15);
        assert!(reused.samples.iter().all(|s| {
            s.shared_from
                .as_deref()
                .unwrap()
                .starts_with("diagnostic-cache:")
        }));
        assert!(record::encode(&identity(), unit, &reused, 1.0, 101).is_err());
        assert!(
            crate::resource_plan::validate_receipt(
                &crate::resource_plan::tests::document(group, &reused),
                group
            )
            .is_err()
        );
        for problem in 0..6 {
            let mut invalid = result.clone();
            match problem {
                0 => {
                    invalid.samples.pop();
                }
                1 => invalid.samples[1].run = 1,
                2 => invalid.gates.clear(),
                3 => invalid.status = EvidenceStatus::Failed,
                4 => invalid.measurements.retain(|m| {
                    !m.name.starts_with("window.stress_") && !m.name.starts_with("zoom_cycles.")
                }),
                _ => invalid.measurements.retain(|m| {
                    !m.name.ends_with("fixture_bytes") && !m.name.ends_with("warmup_seconds")
                }),
            }
            assert!(
                record::encode(&identity(), unit, &invalid, 100.0, 100).is_err(),
                "{group:?} problem={problem}"
            );
        }
    }
    assert!(Unit::Group(ResourceModule::Math).registered().is_err());
    assert_eq!(
        Unit::Group(ResourceModule::Window).minimum_wait_seconds(),
        1350
    );
    assert_eq!(Unit::Group(ResourceModule::Zoom).minimum_wait_seconds(), 75);
}
