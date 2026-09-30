//! Complete Window/Zoom diagnostics, including stress and successful temporary-root cleanup.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{Observer, Output, Path, ResourceModule, window, zoom};
use crate::{
    evidence::{EvidenceResult, EvidenceStatus},
    resource_plan::diagnostic::Unit,
};

pub(super) fn run(
    repository: &Path,
    root: &Path,
    group: ResourceModule,
    output: &mut Output,
    observer: &mut dyn Observer,
) -> Result<bool, String> {
    observer.verify()?;
    if let Some(saved) = observer.load(Unit::Group(group))? {
        output.measurements.extend(saved.measurements);
        output.gates.extend(saved.gates);
        output.samples.extend(saved.samples);
        output.shared.extend(saved.detail);
        output.checkpoint(group, observer)?;
        return Ok(false);
    }
    match group {
        ResourceModule::Window => {
            window::run_window_resource_measurement(repository, root, output, observer)?
        }
        ResourceModule::Zoom => {
            zoom::run_zoom_resource_measurement(repository, root, output, observer)?
        }
        _ => return Err("unregistered whole diagnostic group".into()),
    }
    Ok(true)
}

/// Called only after group execution and temporary-root cleanup both succeed.
pub(super) fn finish(
    group: ResourceModule,
    output: &mut Output,
    fresh: bool,
    elapsed: f64,
    observer: &mut dyn Observer,
) {
    if fresh && output.failure.is_none() {
        output.failure = save(group, output, elapsed, observer).err();
    }
}

fn save(
    group: ResourceModule,
    output: &Output,
    elapsed: f64,
    observer: &mut dyn Observer,
) -> Result<(), String> {
    observer.save(
        Unit::Group(group),
        &EvidenceResult {
            id: group.task_label().into(),
            status: EvidenceStatus::Passed,
            detail: None,
            // The next invocation performs its own desktop probe.
            measurements: output
                .measurements
                .iter()
                .filter(|m| m.name != "desktop_probe.execution_seconds")
                .cloned()
                .collect(),
            gates: output.gates.clone(),
            samples: output.samples.clone(),
        },
        elapsed,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    struct Writer(usize);
    impl Observer for Writer {
        fn save(&mut self, _: Unit, result: &EvidenceResult, _: f64) -> Result<(), String> {
            self.0 += 1;
            assert!(
                !result
                    .measurements
                    .iter()
                    .any(|m| m.name.starts_with("desktop_probe."))
            );
            Err("injected write failure".into())
        }
        fn stage(&mut self, _: &str, _: usize, _: &str, _: u64) -> Result<(), String> {
            Ok(())
        }
        fn waited(&mut self, _: u64) {}
        fn checkpoint(&mut self, _: EvidenceResult) -> Result<(), String> {
            Ok(())
        }
    }
    #[test]
    fn cleanup_and_sampling_failures_never_publish_and_write_errors_keep_samples() {
        let mut writer = Writer(0);
        let mut output = Output {
            failure: Some("cleanup failed".into()),
            ..Default::default()
        };
        finish(ResourceModule::Zoom, &mut output, true, 1.0, &mut writer);
        assert_eq!(writer.0, 0);
        assert_eq!(output.failure.as_deref(), Some("cleanup failed"));
        output.failure = None;
        finish(ResourceModule::Zoom, &mut output, false, 1.0, &mut writer);
        assert_eq!(writer.0, 0);
        output.samples =
            crate::resource_plan::tests::valid_resource_result(ResourceModule::Zoom).samples;
        finish(ResourceModule::Zoom, &mut output, true, 1.0, &mut writer);
        assert_eq!(writer.0, 1);
        assert_eq!(output.samples.len(), 15);
        assert_eq!(output.failure.as_deref(), Some("injected write failure"));
    }
}
