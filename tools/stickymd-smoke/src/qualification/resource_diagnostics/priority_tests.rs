//! Hint lifetime, scope and lack of success authority.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::super::{equivalence_tests, plan, tests::Root};
use super::*;
use crate::{cli::ResourceModule, resource_plan::GROUPS};

#[test]
fn failure_priority_only_selects_requested_complete_units_and_never_writes_a_plan() {
    let root = Root::new();
    let units = plan::units(&GROUPS, None).unwrap();
    assert!(select(&root.0, &units, true).first.is_none());
    assert!(!root.0.join(DIRECTORY).exists());
    for unit in &units {
        failed(&root.0, *unit).unwrap();
        let before = fs::read(path(&root.0).unwrap()).unwrap();
        assert!(select(&root.0, &units, false).first.is_none());
        assert_eq!(select(&root.0, &units, true).first, Some(*unit));
        let excluded: Vec<_> = units.iter().copied().filter(|v| v != unit).collect();
        assert!(select(&root.0, &excluded, true).first.is_none());
        assert_eq!(fs::read(path(&root.0).unwrap()).unwrap(), before);
    }
    let first = Unit::from_key("preview-200-unique").unwrap();
    failed(&root.0, first).unwrap();
    let before = fs::read(path(&root.0).unwrap()).unwrap();
    // No source, EXE or harness exists here: the advisory hint still orders, never grants success.
    let json = plan::preview(&root.0, &GROUPS, true).unwrap();
    let parsed = crate::release::json::parse(&json).unwrap();
    let rows = parsed.field("units").unwrap().array().unwrap();
    assert_eq!(
        rows[0].field("unit").unwrap().string().unwrap(),
        first.key()
    );
    assert!(json.contains("NOT_RUN") && json.contains("RESUME_DISABLED"));
    assert!(!json.contains("REUSE_IF_VALID"));
    assert_eq!(fs::read(path(&root.0).unwrap()).unwrap(), before);
    assert_eq!(fs::read_dir(root.0.join(DIRECTORY)).unwrap().count(), 1);
    assert!(!root.0.join("dist").exists());
    assert!(
        crate::qualification::validate_public_evidence_path(&root.0, &path(&root.0).unwrap())
            .is_err()
    );
    for group in [ResourceModule::Window, ResourceModule::Zoom] {
        assert_eq!(
            Unit::from_key(Unit::Group(group).key()),
            Some(Unit::Group(group))
        );
        for (label, _, _) in group.cohorts() {
            assert!(Unit::from_key(label).is_none());
            assert!(plan::units(&[group], Some(label)).is_err());
        }
    }
    assert!(Unit::from_key("../source").is_none());
}

#[test]
fn invalid_expired_future_and_corrupt_failure_hints_fall_back_without_rewriting() {
    let root = Root::new();
    let unit = Unit::from_key("preview-200-unique").unwrap();
    let current = now().unwrap();
    let good = encode(unit, current, "FAILED").unwrap();
    let forged = good.replace("preview-200-unique", "preview-200-broken");
    for document in [
        "broken json".into(),
        forged,
        encode(unit, current - MAX_AGE - 2, "FAILED").unwrap(),
        encode(unit, current + 100, "FAILED").unwrap(),
        encode(unit, current, "CLEARED").unwrap(),
        "x".repeat(4097),
    ] {
        write(&root.0, &document).unwrap();
        assert!(select(&root.0, &[unit], true).first.is_none());
        assert_eq!(
            fs::read_to_string(path(&root.0).unwrap()).unwrap(),
            document
        );
    }
    assert!(decode(&good, current).unwrap().is_some());
    assert!(decode(&good, current + MAX_AGE).unwrap().is_some());
}

#[test]
fn only_fresh_complete_success_clears_the_same_failure_revision() {
    let root = Root::new();
    let case = ResourceModule::Math.cases()[5];
    let unit = Unit::Case(case);
    let original = equivalence_tests::result(case);
    failed(&root.0, unit).unwrap();
    let selected = select(&root.0, &[unit], true);
    let before = fs::read(path(&root.0).unwrap()).unwrap();
    let mut partial = original.clone();
    partial.samples.pop();
    assert!(selected.complete(&root.0, unit, &partial).is_err());
    let mut historical = original.clone();
    for sample in &mut historical.samples {
        sample.shared_from = Some("diagnostic-cache:old".into());
    }
    assert!(selected.complete(&root.0, unit, &historical).is_err());
    assert_eq!(fs::read(path(&root.0).unwrap()).unwrap(), before);
    // A newer failure, even of the same unit, wins over this command's success.
    for next in [unit, Unit::Group(ResourceModule::Zoom)] {
        failed(&root.0, next).unwrap();
        let newer = fs::read(path(&root.0).unwrap()).unwrap();
        selected.complete(&root.0, unit, &original).unwrap();
        assert_eq!(fs::read(path(&root.0).unwrap()).unwrap(), newer);
    }
    for group in [ResourceModule::Window, ResourceModule::Zoom] {
        let group_unit = Unit::Group(group);
        failed(&root.0, group_unit).unwrap();
        let selected = select(&root.0, &[group_unit], true);
        let result = crate::resource_plan::tests::valid_resource_result(group);
        selected.complete(&root.0, group_unit, &result).unwrap();
        assert!(read(&root.0).unwrap().is_none());
    }
    failed(&root.0, unit).unwrap();
    select(&root.0, &[unit], false)
        .complete(&root.0, unit, &original)
        .unwrap();
    assert!(read(&root.0).unwrap().is_none());
}

#[test]
fn stale_or_damaged_failure_hint_during_measurement_does_not_fail_fresh_success() {
    let root = Root::new();
    let unit = Unit::Group(ResourceModule::Zoom);
    let result = crate::resource_plan::tests::valid_resource_result(ResourceModule::Zoom);
    let created = now().unwrap() - MAX_AGE - 1;
    let expired = encode(unit, created, "FAILED").unwrap();
    // Valid when this simulated long measurement started, expired when it finished.
    let selected = Selection {
        first: Some(unit),
        hint: decode(&expired, created).unwrap(),
    };
    write(&root.0, &expired).unwrap();
    selected.complete(&root.0, unit, &result).unwrap();
    assert_eq!(fs::read_to_string(path(&root.0).unwrap()).unwrap(), expired);
    for replacement in ["broken json", ""] {
        failed(&root.0, unit).unwrap();
        let selected = select(&root.0, &[unit], true);
        write(&root.0, replacement).unwrap();
        selected.complete(&root.0, unit, &result).unwrap();
        assert_eq!(
            fs::read_to_string(path(&root.0).unwrap()).unwrap(),
            replacement
        );
    }
}

#[cfg(windows)]
#[test]
fn clearing_valid_failure_hint_still_reports_atomic_write_errors() {
    use std::os::windows::fs::OpenOptionsExt;
    let root = Root::new();
    let unit = Unit::Group(ResourceModule::Zoom);
    failed(&root.0, unit).unwrap();
    let selected = select(&root.0, &[unit], true);
    let before = fs::read(path(&root.0).unwrap()).unwrap();
    // Permit other readers while preventing the replace operation, without changing ACLs.
    let locked = fs::OpenOptions::new()
        .read(true)
        .share_mode(1)
        .open(path(&root.0).unwrap())
        .unwrap();
    let result = crate::resource_plan::tests::valid_resource_result(ResourceModule::Zoom);
    assert!(selected.complete(&root.0, unit, &result).is_err());
    assert_eq!(fs::read(path(&root.0).unwrap()).unwrap(), before);
    drop(locked);
    selected.complete(&root.0, unit, &result).unwrap();
    assert!(read(&root.0).unwrap().is_none());
}
