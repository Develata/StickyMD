//! Exercise the runner's hint lifecycle without taking over the desktop.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::*;
use crate::{
    cli::ResourceModule,
    resource_plan::{self, progress::Console},
};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "stickymd-failure-first-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        assert!(self.0.is_absolute() && self.0.starts_with(std::env::temp_dir()));
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn failure_priority_observer_tracks_exact_case_forces_sampling_and_clears_only_fresh_success() {
    let root = Root::new();
    let group = ResourceModule::Math;
    let case = group.cases()[5];
    let unit = Unit::Case(case);
    let mut console = Console { remaining: 150 };
    let mut resume = Resume::new(true, priority::Selection::default());
    {
        let mut observer = resume.observe(&root.0, group, &mut console);
        observer.stage("desktop-probe", 0, "probe", 0).unwrap();
        assert_eq!(observer.failed("probe failed".into()), "probe failed");
        assert!(!root.0.join("target").exists());
        observer.stage(case.label, 0, "case-start", 0).unwrap();
        assert_eq!(observer.failed("sample failed".into()), "sample failed");
    }
    let priority = priority::select(&root.0, &[unit], true);
    assert_eq!(priority.first, Some(unit));
    let mut resume = Resume::new(true, priority);
    // The same state as a host where cache identity is unavailable: diagnostics still run.
    resume.enabled = false;
    {
        let mut observer = resume.observe(&root.0, group, &mut console);
        let mut cases = group.cases().to_vec();
        observer.order_cases(&mut cases);
        assert_eq!(cases[0], case);
        assert_eq!(&cases[1..], &group.cases()[..5]);
        assert!(observer.load(unit).unwrap().is_none());
        assert!(observer.load_all(&cases).unwrap().is_none());
        assert_eq!(priority::select(&root.0, &[unit], true).first, Some(unit));
        let mut result = resource_plan::tests::valid_resource_result(group);
        result.id = case.label.into();
        result
            .measurements
            .retain(|m| m.name.starts_with(&format!("{}.", case.label)));
        result
            .gates
            .retain(|g| g.metric.starts_with(&format!("{}.", case.label)));
        result.samples.retain(|s| s.cohort == case.label);
        observer.stage(case.label, 0, "case-start", 0).unwrap();
        observer.save(unit, &result, 150.0).unwrap();
        observer.stage(case.label, 5, "case-finished", 0).unwrap();
        assert_eq!(
            observer.failed("later cleanup failed".into()),
            "later cleanup failed"
        );
    }
    assert_eq!(
        console.remaining, 150,
        "forced run must not credit cached waits"
    );
    assert!(priority::select(&root.0, &[unit], true).first.is_none());
}

#[test]
fn failure_hint_errors_preserve_original_and_window_zoom_always_record_whole_groups() {
    let root = Root::new();
    let mut console = Console { remaining: 75 };
    let mut resume = Resume::new(true, priority::Selection::default());
    for group in [ResourceModule::Window, ResourceModule::Zoom] {
        let unit = Unit::Group(group);
        let mut observer = resume.observe(&root.0, group, &mut console);
        observer
            .stage(group.cohorts()[0].0, 5, "case-finished", 0)
            .unwrap();
        assert_eq!(observer.failed("stress failure".into()), "stress failure");
        assert_eq!(priority::select(&root.0, &[unit], true).first, Some(unit));
    }
    let blocked = Root::new();
    fs::write(blocked.0.join("target"), b"preserved").unwrap();
    let mut observer = resume.observe(&blocked.0, ResourceModule::Zoom, &mut console);
    let error = observer.failed("original failure".into());
    assert!(
        error.starts_with("original failure; failure hint write also failed:"),
        "{error}"
    );
    assert_eq!(fs::read(blocked.0.join("target")).unwrap(), b"preserved");
    let mut disabled = Resume::new(false, priority::Selection::default());
    assert_eq!(
        disabled
            .observe(&blocked.0, ResourceModule::Zoom, &mut console)
            .failed("ordinary resource failure".into()),
        "ordinary resource failure"
    );
}
