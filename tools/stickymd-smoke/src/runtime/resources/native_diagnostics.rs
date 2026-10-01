//! Explicit resource investigations share production executors but never publish qualification receipts.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use super::*;

#[test]
#[ignore = "exclusive desktop and STICKYMD_SMOKE_PROBE_REPOSITORY; 45 fresh resource observations, diagnostic only"]
fn native_window_comparison_diagnostic() {
    super::super::native_diagnostics::diagnose("window-comparison", |repository, root| {
        probe::run(repository, root)?;
        let mut output = Output::default();
        let mut observer = crate::resource_plan::progress::Console {
            remaining: crate::resource_plan::window_comparison::CASES
                .iter()
                .map(|case| case.fixture.minimum_wait_seconds())
                .sum(),
        };
        let result = comparison::run(repository, root, &mut output, &mut observer);
        Ok(RuntimeEvidence {
            measurements: output.measurements,
            gates: output.gates,
            samples: output.samples,
            gate_failure: result.err(),
        })
    });
}

#[test]
#[ignore = "exclusive desktop and STICKYMD_SMOKE_PROBE_REPOSITORY; full Zoom group, diagnostic only"]
fn native_zoom_cpu_diagnostic() {
    super::super::native_diagnostics::diagnose("zoom-cpu", |repository, root| {
        probe::run(repository, root)?;
        let mut output = Output::default();
        let mut observer = crate::resource_plan::progress::Console {
            remaining: crate::resource_plan::minimum_wait_seconds(
                &[ResourceModule::Zoom],
                false,
                None,
            ),
        };
        let result =
            zoom::run_zoom_resource_measurement(repository, root, &mut output, &mut observer);
        Ok(RuntimeEvidence {
            measurements: output.measurements,
            gates: output.gates,
            samples: output.samples,
            gate_failure: result.err(),
        })
    });
}
