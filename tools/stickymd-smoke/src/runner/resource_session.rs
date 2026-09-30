//! One resource command: shared cohorts, timing and durable group checkpoints.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::*;
use crate::qualification::resource_modules::Campaign;
use std::time::Instant;

pub(super) struct Session {
    campaign: Option<Campaign>,
    remaining_wait: u64,
    #[cfg(windows)]
    cache: crate::runtime::resources::Cache,
    #[cfg(windows)]
    resume: super::resource_resume::Resume,
}

impl Session {
    pub(super) fn prepare(root: &Path, options: &Options, tasks: &[Task]) -> Result<Self, String> {
        let campaign = crate::qualification::smoke_scope::validate(root, options)?
            .then(|| Campaign::prepare(root))
            .transpose()?;
        let groups: Vec<_> = tasks
            .iter()
            .filter_map(|task| match task {
                Task::Runtime { scenario, .. } => scenario.resource_group(),
                _ => None,
            })
            .filter(|group| {
                campaign
                    .as_ref()
                    .is_none_or(|campaign| campaign.needs_run(*group))
            })
            .collect();
        let filter = std::env::var("STICKYMD_SMOKE_RESOURCE_CASE").ok();
        let minimum = crate::resource_plan::minimum_wait_seconds(&groups, true, filter.as_deref());
        let repeated =
            crate::resource_plan::minimum_wait_seconds(&groups, false, filter.as_deref());
        if !groups.is_empty() {
            eprintln!(
                "RESOURCE_PLAN groups={} minimum_fixed_wait_seconds={minimum} shared_cohort_savings_seconds={} (excludes startup, fixture preparation, transitions and stress)",
                groups
                    .iter()
                    .map(|group| group.name())
                    .collect::<Vec<_>>()
                    .join(","),
                repeated - minimum
            );
        }
        Ok(Self {
            campaign,
            remaining_wait: minimum,
            #[cfg(windows)]
            cache: Default::default(),
            #[cfg(windows)]
            resume: super::resource_resume::Resume::new(options.resource_resume),
        })
    }

    pub(super) fn remaining_wait(&self) -> u64 {
        self.remaining_wait
    }
    pub(super) fn update_remaining_wait(&mut self, remaining: u64) {
        self.remaining_wait = remaining;
    }

    pub(super) fn reuse(
        &self,
        root: &Path,
        group: ResourceModule,
    ) -> Result<Option<EvidenceResult>, String> {
        self.campaign
            .as_ref()
            .map_or(Ok(None), |campaign| campaign.reuse(root, group))
    }

    pub(super) fn run(
        &mut self,
        root: &Path,
        group: ResourceModule,
        json: bool,
        environment: Option<&QualificationEnvironment>,
        observer: &mut dyn crate::resource_plan::progress::Observer,
    ) -> Result<EvidenceResult, String> {
        let started = Instant::now();
        let mut result = self.measure(root, group, json, observer).map_err(|error| {
            format!(
                "{} resource group failed after {:.3} seconds: {error}",
                group.name(),
                started.elapsed().as_secs_f64()
            )
        })?;
        result.measurements.push(EvidenceMeasurement {
            name: "group.execution_seconds".into(),
            unit: "seconds".into(),
            value: started.elapsed().as_secs_f64(),
        });
        if result.status == EvidenceStatus::Passed
            && let Some(campaign) = &self.campaign
            && let Err(error) = campaign.record(root, group, &result, environment)
        {
            result.status = EvidenceStatus::Failed;
            result.detail = Some(error);
        }
        eprintln!(
            "RESOURCE_GROUP={} STATUS={} elapsed_seconds={:.3}",
            group.name(),
            if result
                .detail
                .as_deref()
                .is_some_and(|d| d.starts_with("DIAGNOSTIC_REUSED"))
            {
                "DIAGNOSTIC_REUSED"
            } else if result.status == EvidenceStatus::Passed {
                "RAN_PASS"
            } else {
                "FAILED"
            },
            started.elapsed().as_secs_f64()
        );
        Ok(result)
    }

    #[cfg(windows)]
    fn measure(
        &mut self,
        root: &Path,
        group: ResourceModule,
        json: bool,
        observer: &mut dyn crate::resource_plan::progress::Observer,
    ) -> Result<EvidenceResult, String> {
        let mut observer = self.resume.observe(root, observer);
        let output =
            crate::runtime::resources::run(root, group, json, &mut self.cache, &mut observer)?;
        Ok(measured_result(group, output))
    }

    #[cfg(not(windows))]
    fn measure(
        &mut self,
        _root: &Path,
        _group: ResourceModule,
        _json: bool,
        _observer: &mut dyn crate::resource_plan::progress::Observer,
    ) -> Result<EvidenceResult, String> {
        Err("native resource qualification requires Windows".into())
    }
}

#[cfg(windows)]
fn measured_result(
    group: ResourceModule,
    output: crate::runtime::resources::Output,
) -> EvidenceResult {
    let historical = output.samples.iter().any(|sample| {
        sample
            .shared_from
            .as_deref()
            .is_some_and(|source| source.starts_with("diagnostic-cache:"))
    });
    EvidenceResult {
        id: group.task_label().into(),
        status: if output.failure.is_some() {
            EvidenceStatus::Failed
        } else {
            EvidenceStatus::Passed
        },
        detail: Some(match output.failure {
            Some(error) => error,
            None => format!(
                "{}; shared cohorts: {}",
                if historical {
                    "DIAGNOSTIC_REUSED (includes historical observations)"
                } else {
                    "RAN_PASS"
                },
                if output.shared.is_empty() {
                    "none".into()
                } else {
                    output.shared.join("; ")
                }
            ),
        }),
        measurements: output.measurements,
        gates: output.gates,
        samples: output.samples,
    }
}

pub(super) fn append_result(
    results: &mut Vec<EvidenceResult>,
    result: EvidenceResult,
) -> Result<(), String> {
    let failure = (result.status != EvidenceStatus::Passed).then(|| {
        result
            .detail
            .clone()
            .unwrap_or_else(|| format!("{} did not pass", result.id))
    });
    results.push(result);
    match failure {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn early_resource_failure_keeps_samples_in_final_json_and_leaves_all_success_ledgers_untouched()
    {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "stickymd-resource-failure-{}-{nonce}",
            std::process::id()
        ));
        let ledger_root = root.join("dist/evidence/module-success");
        fs::create_dir_all(&ledger_root).unwrap();
        for group in crate::resource_plan::GROUPS {
            fs::write(
                ledger_root.join(format!("{}.json", group.ledger_id())),
                b"last complete success",
            )
            .unwrap();
        }
        let output = crate::runtime::resources::Output {
            measurements: crate::resource_plan::cohort_coverage("source", 1, 1, 30),
            samples: vec![EvidenceSample {
                cohort: "source".into(),
                run: 1,
                shared_from: None,
                measurements: vec![EvidenceMeasurement {
                    name: "idle_cpu".into(),
                    unit: "percent".into(),
                    value: 0.11,
                }],
            }],
            gates: vec![EvidenceGate {
                metric: "source.idle_cpu_max".into(),
                comparator: "<=".into(),
                value: 0.1,
                unit: "percent".into(),
                source: "contract".into(),
            }],
            failure: Some("source idle CPU 0.11 exceeds 0.1".into()),
            ..Default::default()
        };
        let result = measured_result(ResourceModule::SourcePreview, output);
        let expected = result.clone();
        let mut results = Vec::new();
        assert!(append_result(&mut results, result).is_err());
        assert_eq!(results, [expected]);
        let path = root.join(crate::qualification::smoke_scope::RESOURCE_SUMMARY);
        evidence::emit(&root, "phase-14", &results, None, Some(&path)).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        let json = crate::release::json::parse(&text).unwrap();
        let result = &json.field("results").unwrap().array().unwrap()[0];
        assert_eq!(result.field("status").unwrap().string().unwrap(), "FAILED");
        assert_eq!(result.field("samples").unwrap().array().unwrap().len(), 1);
        assert_eq!(result.field("gates").unwrap().array().unwrap().len(), 1);
        assert!(text.contains("0.110000"));
        for group in crate::resource_plan::GROUPS {
            assert_eq!(
                fs::read(ledger_root.join(format!("{}.json", group.ledger_id()))).unwrap(),
                b"last complete success"
            );
            assert!(!root.join(group.receipt()).exists());
        }
        fs::remove_dir_all(root).unwrap();
    }
}
