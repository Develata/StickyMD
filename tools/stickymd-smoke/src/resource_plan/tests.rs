//! Resource plan regressions; fake drivers prove scheduling, never desktop acceptance.
use super::*;

#[test]
fn baseline_memory_over_limit_is_rejected_even_with_consistent_samples() {
    for (group, labels) in [
        (
            ResourceModule::SourcePreview,
            ["source", "preview", "split"],
        ),
        (
            ResourceModule::Math,
            ["source-20-math-lazy", "preview-20-math", "split-20-math"],
        ),
    ] {
        for (label, mib) in labels.into_iter().zip([40, 52, 64]) {
            let mut result = valid_resource_result(group);
            let over = (mib * 1024 * 1024 + 1) as f64;
            for sample in &mut result.samples {
                if sample.cohort == label {
                    sample.measurements[0].value = over;
                    sample.measurements[2].value = over;
                }
            }
            for metric in &mut result.measurements {
                if metric
                    .name
                    .starts_with(&format!("{label}.private_working_set_"))
                {
                    metric.value = over;
                }
            }
            assert!(
                validate_receipt(&document(group, &result), group).is_err(),
                "{label} accepted private working set above {mib} MiB"
            );
        }
    }
}

#[test]
fn baseline_case_cache_requires_the_same_memory_gate_as_group_recording() {
    use crate::release::json;
    for group in [ResourceModule::SourcePreview, ResourceModule::Math] {
        for &case in group.cases() {
            let Some(limit) = case.private_working_set_limit() else {
                continue;
            };
            let prefix = format!("{}.", case.label);
            let mut result = valid_resource_result(group);
            result.id = case.label.into();
            result.measurements.retain(|m| m.name.starts_with(&prefix));
            result.gates.retain(|g| g.metric.starts_with(&prefix));
            result.samples.retain(|s| s.cohort == case.label);
            for sample in &mut result.samples {
                sample.measurements[0].value = limit as f64;
                sample.measurements[2].value = limit as f64;
            }
            for metric in &mut result.measurements {
                if metric
                    .name
                    .starts_with(&format!("{prefix}private_working_set_"))
                {
                    metric.value = limit as f64;
                }
            }
            let validate = |result: &crate::evidence::EvidenceResult| {
                let root = json::parse(&document(group, result)).unwrap();
                diagnostic::Unit::Case(case)
                    .validate(&root.field("results").unwrap().array().unwrap()[0])
            };
            validate(&result).expect("memory at the hard boundary passes");
            result
                .gates
                .retain(|g| !g.metric.ends_with("private_working_set_max"));
            assert!(
                validate(&result).is_err(),
                "missing gate for {}",
                case.label
            );
        }
    }
}

#[test]
fn full_plan_preserves_nineteen_names_with_fifteen_executions_and_1500_seconds_saved() {
    let mut cache = ScenarioCache::default();
    let mut executions = 0;
    let mut aliases = Vec::new();
    let mut names = Vec::new();
    for group in GROUPS {
        for &case in group.cases() {
            names.push(case.label);
            let (value, origin) = cache
                .measure(case, || {
                    executions += 1;
                    Ok(case.formula_count)
                })
                .unwrap();
            assert_eq!(value, case.formula_count);
            if let Some(origin) = origin {
                aliases.push((case.label, origin));
            }
        }
    }
    assert_eq!((names.len(), executions, aliases.len()), (19, 15, 4));
    assert_eq!(
        minimum_wait_seconds(&GROUPS, false, None) - minimum_wait_seconds(&GROUPS, true, None),
        1500
    );
    assert!(aliases.contains(&("preview-no-images", "preview-no-math")));
}

#[test]
fn operation_history_and_cpu_protocol_are_part_of_measurement_identity() {
    let cases = ResourceModule::Images.cases();
    assert!(!cases[7].equivalent(cases[9]));
    let first = cases[7];
    assert_eq!(
        minimum_wait_seconds(&[ResourceModule::Math], true, Some("preview-1-math")),
        150
    );
    assert_eq!(cases[9].minimum_wait_seconds(), 475);
    assert!(!first.equivalent(ResourceCase {
        measure_cpu: false,
        ..first
    }));
    let mut cache = ScenarioCache::<usize>::default();
    assert!(
        cache
            .measure(first, || Err("interrupted cohort".into()))
            .is_err()
    );
    assert_eq!(cache.measure(first, || Ok(5)).unwrap(), (5, None));
    assert_eq!(
        cache
            .measure(first, || panic!("must reuse complete cohort"))
            .unwrap(),
        (5, Some(first.label))
    );
}

pub(crate) fn valid_resource_result(group: ResourceModule) -> crate::evidence::EvidenceResult {
    use crate::evidence::{EvidenceMeasurement, EvidenceResult, EvidenceSample, EvidenceStatus};
    let mut measurements = coverage_measurements(group);
    let mut samples = Vec::new();
    for (name, cpu, _) in group.cohorts() {
        for run in 1..=REPETITIONS {
            let mut values = [
                "private_working_set",
                "private_bytes",
                "peak_working_set",
                "peak_private_bytes",
            ]
            .map(|name| EvidenceMeasurement {
                name: name.into(),
                value: 1024.0,
                unit: "bytes".into(),
            })
            .to_vec();
            if cpu {
                values.push(EvidenceMeasurement {
                    name: "idle_cpu".into(),
                    value: 0.0,
                    unit: "percent".into(),
                });
            }
            samples.push(EvidenceSample {
                cohort: name.into(),
                run,
                shared_from: None,
                measurements: values,
            });
        }
        for metric in [
            "private_working_set_median",
            "private_working_set_max",
            "private_bytes_median",
            "private_bytes_max",
        ] {
            measurements.push(EvidenceMeasurement {
                name: format!("{name}.{metric}"),
                value: 1024.0,
                unit: "bytes".into(),
            });
        }
        if cpu {
            for metric in ["idle_cpu_median", "idle_cpu_p95", "idle_cpu_max"] {
                measurements.push(EvidenceMeasurement {
                    name: format!("{name}.{metric}"),
                    value: 0.0,
                    unit: "percent".into(),
                });
            }
        }
    }
    if group == ResourceModule::Window {
        for run in 1..=REPETITIONS {
            measurements.push(EvidenceMeasurement {
                name: format!("hidden-to-tray.run_{run}.fixture_bytes"),
                value: 20480.0,
                unit: "bytes".into(),
            });
        }
        measurements.push(EvidenceMeasurement {
            name: "window.stress_completed".into(),
            value: 1.0,
            unit: "count".into(),
        });
    }
    if group == ResourceModule::Zoom {
        measurements.push(EvidenceMeasurement {
            name: "zoom_cycles.private_growth".into(),
            value: 0.0,
            unit: "bytes".into(),
        });
    }
    EvidenceResult {
        id: group.task_label().into(),
        status: EvidenceStatus::Passed,
        detail: None,
        measurements,
        gates: required_gates(group),
        samples,
    }
}

pub(crate) fn document(group: ResourceModule, result: &crate::evidence::EvidenceResult) -> String {
    let base = crate::evidence::render_json(
        &"a".repeat(40),
        false,
        None,
        Some(&"c".repeat(64)),
        "phase-14",
        std::slice::from_ref(result),
        None,
    )
    .replace(
        "\"qualification_environment\":null",
        "\"qualification_environment\":{\"status\":\"VALID\"}",
    );
    format!(
        "{{\"resource_protocol\":1,\"resource_group\":\"{}\",{}",
        group.name(),
        &base[1..]
    )
}

#[test]
fn complete_coverage_rejects_missing_duplicate_wrong_count_and_foreign_group() {
    for group in GROUPS {
        let result = valid_resource_result(group);
        validate_receipt(&document(group, &result), group).unwrap();
        let mut missing = result.clone();
        missing.measurements.remove(0);
        assert!(validate_receipt(&document(group, &missing), group).is_err());
        let mut duplicate = result.clone();
        duplicate
            .measurements
            .push(duplicate.measurements[0].clone());
        assert!(validate_receipt(&document(group, &duplicate), group).is_err());
        let mut count = result.clone();
        count.measurements[0].value = 1.0;
        assert!(validate_receipt(&document(group, &count), group).is_err());
    }
    let window = valid_resource_result(ResourceModule::Window);
    assert!(
        validate_receipt(
            &document(ResourceModule::Window, &window),
            ResourceModule::Math
        )
        .is_err()
    );
}

#[test]
fn complete_coverage_rejects_missing_raw_samples_and_gates() {
    for group in GROUPS {
        let mut result = valid_resource_result(group);
        result.samples.clear();
        assert!(
            validate_receipt(&document(group, &result), group).is_err(),
            "{group:?} accepted no samples"
        );
        let mut result = valid_resource_result(group);
        result.gates.clear();
        assert!(
            validate_receipt(&document(group, &result), group).is_err(),
            "{group:?} accepted no gates"
        );
    }
}

#[test]
fn resource_observations_reject_duplicate_runs_wrong_statistics_units_and_false_aliases() {
    let group = ResourceModule::Math;
    let valid = valid_resource_result(group);
    for change in 0..5 {
        let mut result = valid.clone();
        match change {
            0 => result.samples[1].run = result.samples[0].run,
            1 => result.samples[0].measurements[0].value += 1.0,
            2 => result.samples[0].measurements[0].unit = "percent".into(),
            3 => result.samples[0].shared_from = Some("preview-1-math".into()),
            _ => result.samples[0].shared_from = Some("source".into()),
        }
        assert!(
            validate_receipt(&document(group, &result), group).is_err(),
            "mutation {change}"
        );
    }
    let mut aliased = valid.clone();
    for sample in &mut aliased.samples {
        if sample.cohort == "source-20-math-lazy" {
            sample.shared_from = Some("source".into());
        }
    }
    validate_receipt(&document(group, &aliased), group).unwrap();
}

#[test]
fn existing_resource_gates_reject_over_limit_observations_and_changed_limits() {
    for group in GROUPS {
        let valid = valid_resource_result(group);
        for gate in required_gates(group) {
            let mut over = valid.clone();
            let excess = gate.value + 1.0;
            if gate.metric == "zoom_cycles.private_growth" {
                over.measurements
                    .iter_mut()
                    .find(|m| m.name == gate.metric)
                    .unwrap()
                    .value = excess;
            } else {
                let (name, metric) = gate.metric.rsplit_once('.').unwrap();
                let raw = metric.strip_suffix("_max").unwrap();
                for sample in &mut over.samples {
                    if sample.cohort == name {
                        sample
                            .measurements
                            .iter_mut()
                            .find(|m| m.name == raw)
                            .unwrap()
                            .value = excess;
                    }
                }
                for value in &mut over.measurements {
                    if value.name.starts_with(&format!("{name}.{raw}_")) {
                        value.value = excess;
                    }
                }
            }
            assert!(
                validate_receipt(&document(group, &over), group).is_err(),
                "{}",
                gate.metric
            );
        }
        let mut wrong = valid.clone();
        wrong.gates[0].value += 1.0;
        assert!(validate_receipt(&document(group, &wrong), group).is_err());
    }
    let group = ResourceModule::Zoom;
    let mut negative_growth = valid_resource_result(group);
    negative_growth
        .measurements
        .iter_mut()
        .find(|m| m.name == "zoom_cycles.private_growth")
        .unwrap()
        .value = -1024.0;
    validate_receipt(&document(group, &negative_growth), group).unwrap();
}
