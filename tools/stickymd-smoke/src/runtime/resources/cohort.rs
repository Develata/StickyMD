//! Capture each complete sample before checking an irreversible any/max gate.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{super::*, Output};

pub(super) struct Cohort {
    label: String,
    warmup_seconds: u64,
    memory: Vec<process_metrics::MemorySample>,
    cpu: Vec<f64>,
}

impl Cohort {
    pub(super) fn new(label: &str, warmup_seconds: u64) -> Self {
        Self {
            label: label.into(),
            warmup_seconds,
            memory: Vec::new(),
            cpu: Vec::new(),
        }
    }

    pub(super) fn memory(
        &mut self,
        output: &mut Output,
        sample: process_metrics::MemorySample,
        limit: Option<u64>,
    ) -> Result<(), String> {
        self.memory.push(sample);
        output.samples.push(EvidenceSample {
            cohort: self.label.clone(),
            run: self.memory.len(),
            shared_from: None,
            measurements: [
                ("private_working_set", sample.private_working_set_bytes),
                ("private_bytes", sample.private_bytes),
                ("peak_working_set", sample.peak_working_set_bytes),
                ("peak_private_bytes", sample.peak_private_bytes),
            ]
            .into_iter()
            .map(|(name, value)| EvidenceMeasurement {
                name: name.into(),
                unit: "bytes".into(),
                value: value as f64,
            })
            .collect(),
        });
        if let Some(limit) = limit {
            check_max(
                output,
                &format!("{}.private_working_set_max", self.label),
                sample.private_working_set_bytes as f64,
                limit as f64,
                "bytes",
                "docs/plan/10_performance_reliability.md#initial-engineering-targets",
            )?;
        }
        Ok(())
    }

    // Call only after measure_idle_cpu has completed its entire 60-second window.
    pub(super) fn cpu(&mut self, output: &mut Output, value: f64) -> Result<(), String> {
        self.cpu.push(value);
        let sample = output
            .samples
            .iter_mut()
            .rev()
            .find(|sample| sample.cohort == self.label && sample.run == self.cpu.len())
            .ok_or_else(|| {
                format!(
                    "{} CPU sample is missing its memory observation",
                    self.label
                )
            })?;
        sample.measurements.push(EvidenceMeasurement {
            name: "idle_cpu".into(),
            unit: "percent".into(),
            value,
        });
        check_max(
            output,
            &format!("{}.idle_cpu_max", self.label),
            value,
            IDLE_CPU_PERCENT_LIMIT,
            "percent",
            "docs/plan/10_performance_reliability.md#initial-engineering-targets",
        )
    }

    pub(super) fn finish(&self, output: &mut Output) -> Result<(), String> {
        output
            .measurements
            .extend(crate::resource_plan::cohort_coverage(
                &self.label,
                self.memory.len(),
                self.cpu.len(),
                self.warmup_seconds,
            ));
        // Partial failures carry raw observations/counts, not full-cohort statistics.
        if self.memory.len() == RESOURCE_REPETITIONS {
            output
                .measurements
                .extend(memory_measurements(&self.label, &self.memory));
        }
        if self.cpu.len() == RESOURCE_REPETITIONS {
            output
                .measurements
                .extend(cpu_measurements(&self.label, &self.cpu));
        }
        if self.memory.len() == RESOURCE_REPETITIONS
            && (self.cpu.is_empty() || self.cpu.len() == RESOURCE_REPETITIONS)
        {
            print_resource_summary(&self.label, &self.memory, &self.cpu)?;
        }
        Ok(())
    }
}

pub(super) fn check_max(
    output: &mut Output,
    metric: &str,
    observed: f64,
    limit: f64,
    unit: &str,
    source: &str,
) -> Result<(), String> {
    if !output.gates.iter().any(|gate| gate.metric == metric) {
        output.gates.push(EvidenceGate {
            metric: metric.into(),
            comparator: "<=".into(),
            value: limit,
            unit: unit.into(),
            source: source.into(),
        });
    }
    if !observed.is_finite() || observed > limit {
        Err(format!(
            "{metric} observed {observed} {unit} exceeds {limit} {unit}; stopping resource sampling"
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory(bytes: u64) -> process_metrics::MemorySample {
        process_metrics::MemorySample {
            private_working_set_bytes: bytes,
            private_bytes: bytes + 1,
            peak_working_set_bytes: bytes + 2,
            peak_private_bytes: bytes + 3,
        }
    }

    fn collect_cpu(values: &[f64]) -> Output {
        let mut output = Output::default();
        let mut cohort = Cohort::new("source", RESOURCE_WARMUP.as_secs());
        let result = (|| {
            for &value in values {
                cohort.memory(&mut output, memory(1024), None)?;
                cohort.cpu(&mut output, value)?;
            }
            Ok::<_, String>(())
        })();
        cohort.finish(&mut output).unwrap();
        output.failure = result.err();
        output
    }

    fn value(output: &Output, name: &str) -> f64 {
        output
            .measurements
            .iter()
            .find(|m| m.name == name)
            .unwrap()
            .value
    }

    #[test]
    fn cpu_failure_stops_at_the_first_complete_over_limit_sample_and_keeps_it() {
        for (values, expected) in [
            ([0.11, 0.0, 0.0, 0.0, 0.0], 1),
            ([0.05, 0.11, 0.0, 0.0, 0.0], 2),
            ([0.05, 0.05, 0.05, 0.05, 0.11], 5),
        ] {
            let output = collect_cpu(&values);
            assert!(output.failure.as_ref().unwrap().contains("0.11"));
            assert_eq!(output.samples.len(), expected);
            assert_eq!(value(&output, "source.memory_samples"), expected as f64);
            assert_eq!(value(&output, "source.cpu_samples"), expected as f64);
            assert_eq!(value(&output, "source.cpu_interval_seconds"), 60.0);
            assert_eq!(value(&output, "source.warmup_seconds"), 30.0);
            let last = output.samples.last().unwrap();
            assert_eq!(last.run, expected);
            assert_eq!(last.measurements.last().unwrap().value, 0.11);
            assert_eq!(output.gates.len(), 1);
            assert_eq!(output.gates[0].value, 0.1);
            if expected < 5 {
                assert!(!output.measurements.iter().any(|m| m.name.ends_with("p95")));
            }
        }
    }

    #[test]
    fn passing_at_the_cpu_boundary_keeps_all_five_samples_and_original_statistics() {
        let output = collect_cpu(&[0.1; RESOURCE_REPETITIONS]);
        assert!(output.failure.is_none());
        assert_eq!(output.samples.len(), 5);
        assert_eq!(value(&output, "source.cpu_samples"), 5.0);
        assert_eq!(value(&output, "source.idle_cpu_max"), 0.1);
        assert_eq!(value(&output, "source.idle_cpu_p95"), 0.1);
        assert_eq!(value(&output, "source.private_working_set_max"), 1024.0);
    }

    #[test]
    fn memory_max_gates_keep_the_failing_observation_and_accept_equality() {
        for (label, limit) in [
            ("source", 40 * 1024 * 1024),
            ("preview", 52 * 1024 * 1024),
            ("split", 64 * 1024 * 1024),
            ("hidden-to-tray", HIDDEN_PRIVATE_WORKING_SET_LIMIT),
            ("split-zoom-50", 64 * 1024 * 1024),
        ] {
            let mut output = Output::default();
            let mut cohort = Cohort::new(label, 30);
            cohort
                .memory(&mut output, memory(limit), Some(limit))
                .unwrap();
            assert!(
                cohort
                    .memory(&mut output, memory(limit + 1), Some(limit))
                    .is_err()
            );
            cohort.finish(&mut output).unwrap();
            assert_eq!(output.samples.len(), 2);
            assert_eq!(output.samples[1].measurements[0].value, (limit + 1) as f64);
            assert_eq!(value(&output, &format!("{label}.memory_samples")), 2.0);
            assert_eq!(value(&output, &format!("{label}.cpu_samples")), 0.0);
        }
    }

    #[test]
    fn growth_gate_allows_a_decrease_but_rejects_more_than_eight_mib() {
        let mut output = Output::default();
        let limit = ZOOM_RESOURCE_PRIVATE_GROWTH_LIMIT as f64;
        for growth in [-1024.0, 0.0, limit] {
            check_max(
                &mut output,
                "zoom_cycles.private_growth",
                growth,
                limit,
                "bytes",
                "existing zoom harness limit",
            )
            .unwrap();
        }
        assert!(
            check_max(
                &mut output,
                "zoom_cycles.private_growth",
                limit + 1.0,
                limit,
                "bytes",
                "existing zoom harness limit",
            )
            .is_err()
        );
    }
}
