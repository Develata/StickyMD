//! Zoom observations share one case registry with coverage and wait budgeting.
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

use super::{ResourceCase, case};

#[derive(Clone, Copy)]
pub(crate) struct ZoomCase {
    pub(crate) fixture: ResourceCase,
    // Only the Windows executor consumes the configured percentage at runtime.
    #[cfg_attr(not(any(windows, test)), allow(dead_code))]
    pub(crate) percent: u16,
}

const fn zoom(label: &'static str, view: &'static str, percent: u16) -> ZoomCase {
    ZoomCase {
        fixture: case(label, view, 20, 12, true),
        percent,
    }
}

pub(crate) const CASES: &[ZoomCase] = &[
    zoom("source-zoom-50", "source", 50),
    zoom("source-zoom-100", "source", 100),
    zoom("source-zoom-300", "source", 300),
    zoom("preview-zoom-50", "preview", 50),
    zoom("preview-zoom-100", "preview", 100),
    zoom("preview-zoom-300", "preview", 300),
    zoom("split-zoom-50", "split", 50),
    zoom("split-zoom-100", "split", 100),
    zoom("split-zoom-300", "split", 300),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ResourceModule;
    use crate::resource_plan::{REPETITIONS, minimum_wait_seconds};

    #[test]
    fn all_views_have_independent_five_sample_cohorts_at_each_zoom() {
        for view in ["source", "preview", "split"] {
            let percentages: Vec<_> = CASES
                .iter()
                .filter(|case| case.fixture.view_mode == view)
                .map(|case| case.percent)
                .collect();
            assert_eq!(percentages, [50, 100, 300]);
        }
        assert_eq!(REPETITIONS, 5);
        let cohorts = ResourceModule::Zoom.cohorts();
        let names: std::collections::BTreeSet<_> = cohorts.iter().map(|c| c.0).collect();
        assert_eq!(names.len(), 9);
        assert!(cohorts.iter().all(|(_, cpu, warmup)| *cpu && *warmup == 30));
        assert_eq!(
            minimum_wait_seconds(&[ResourceModule::Zoom], true, None),
            4050
        );
    }

    #[test]
    fn split_only_or_short_warmup_receipts_cannot_claim_complete_zoom_coverage() {
        use crate::resource_plan::{tests, validate_receipt};
        let group = ResourceModule::Zoom;
        let complete = tests::valid_resource_result(group);
        for short_warmup in [false, true] {
            let mut result = complete.clone();
            if short_warmup {
                for metric in &mut result.measurements {
                    if metric.name.ends_with(".warmup_seconds") {
                        metric.value = 5.0;
                    }
                }
            } else {
                result
                    .samples
                    .retain(|sample| sample.cohort.starts_with("split-"));
            }
            assert!(validate_receipt(&tests::document(group, &result), group).is_err());
        }
    }
    #[test]
    fn memory_only_zoom_receipt_cannot_close_the_cpu_matrix() {
        use crate::resource_plan::{tests, validate_receipt};
        let group = ResourceModule::Zoom;
        let mut result = tests::valid_resource_result(group);
        validate_receipt(&tests::document(group, &result), group).unwrap();
        for sample in &mut result.samples {
            sample.measurements.retain(|m| m.name != "idle_cpu");
        }
        assert!(validate_receipt(&tests::document(group, &result), group).is_err());
    }
}
