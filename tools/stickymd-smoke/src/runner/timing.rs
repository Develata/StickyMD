//! Per-task elapsed time, including failure and reused-prerequisite lookups.
//! plan_ref: docs/plan/11_testing_and_release.md#shared-headless-prerequisite

use super::{EvidenceMeasurement, EvidenceResult};
use std::time::Duration;

pub(super) fn record(label: &str, elapsed: Duration, results: &mut [EvidenceResult], passed: bool) {
    let seconds = elapsed.as_secs_f64();
    if let Some(result) = results.iter_mut().rev().find(|result| result.id == label) {
        result.measurements.push(EvidenceMeasurement {
            name: "task.execution_seconds".into(),
            unit: "seconds".into(),
            value: seconds,
        });
    }
    eprintln!(
        "TASK_TIMING task={label:?} status={} elapsed_seconds={seconds:.6}",
        if passed { "PASSED" } else { "FAILED" }
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::EvidenceStatus;

    #[test]
    fn timing_preserves_failure_details_and_observations() {
        let original = super::super::failed_task("workspace tests", "expected failure");
        let mut results = [original.clone()];
        record(
            "workspace tests",
            Duration::from_millis(125),
            &mut results,
            false,
        );
        assert_eq!(results[0].status, EvidenceStatus::Failed);
        assert_eq!(results[0].detail, original.detail);
        assert_eq!(results[0].measurements[0].value, 0.125);
        assert_eq!(results[0].gates, original.gates);
        assert_eq!(results[0].samples, original.samples);
    }
}
