//! Per-group resource success promotion and compatible reuse.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::module_ledger::{self, fingerprint};
use super::module_registry::ModuleId;
use super::{json, receipt};
use crate::cli::ResourceModule;
use crate::evidence::{self, EvidenceResult, EvidenceStatus};
use crate::qualification_environment::QualificationEnvironment;
use std::path::Path;

#[cfg(test)]
mod tests;

struct Entry {
    group: ResourceModule,
    fingerprint: String,
    reusable: bool,
}

pub(crate) struct Campaign {
    candidate: receipt::Candidate,
    entries: Vec<Entry>,
}

impl Campaign {
    pub(crate) fn prepare(root: &Path) -> Result<Self, String> {
        let planning_started = std::time::Instant::now();
        let candidate = receipt::read_candidate(root)?;
        receipt::validate_candidate_against_repository(root, &candidate)?;
        let mut entries = Vec::new();
        let fingerprint_started = std::time::Instant::now();
        let inputs = fingerprint::PlanningInputs::read(root)?;
        let fingerprints =
            inputs.calculate_many(root, &crate::resource_plan::GROUPS.map(ModuleId::Resource))?;
        eprintln!(
            "RESOURCE_FINGERPRINT_BATCH groups={} input_files={} input_bytes={} elapsed_seconds={:.6}",
            fingerprints.digests.len(),
            fingerprints.input_files,
            fingerprints.input_bytes,
            fingerprint_started.elapsed().as_secs_f64()
        );
        for (group, input) in crate::resource_plan::GROUPS
            .into_iter()
            .zip(fingerprints.digests)
        {
            let started = std::time::Instant::now();
            let reusable = match module_ledger::compatible_success_for_input(
                root,
                ModuleId::Resource(group),
                &input,
            ) {
                Ok(success) => success,
                Err(error) => {
                    eprintln!(
                        "RESOURCE_GROUP={} STATUS=RUN_REQUIRED REASON={}",
                        group.name(),
                        error.replace(['\r', '\n'], " ")
                    );
                    None
                }
            };
            eprintln!(
                "RESOURCE_COMPATIBILITY group={} elapsed_seconds={:.6}",
                group.name(),
                started.elapsed().as_secs_f64()
            );
            eprintln!(
                "RESOURCE_GROUP={} STATUS={} REASON={}",
                group.name(),
                if reusable.is_some() {
                    "COMPATIBLE_LAST_SUCCESS"
                } else {
                    "RUN_REQUIRED"
                },
                if reusable.is_some() {
                    "INPUT_FINGERPRINT_MATCH"
                } else {
                    module_ledger::rerun_reason(root, ModuleId::Resource(group))
                }
            );
            entries.push(Entry {
                group,
                fingerprint: input,
                reusable: reusable.is_some(),
            });
        }
        eprintln!(
            "RESOURCE_PLANNING elapsed_seconds={:.6}",
            planning_started.elapsed().as_secs_f64()
        );
        Ok(Self { candidate, entries })
    }

    pub(crate) fn needs_run(&self, group: ResourceModule) -> bool {
        !self.entry(group).reusable
    }

    pub(crate) fn reuse(
        &self,
        root: &Path,
        group: ResourceModule,
    ) -> Result<Option<EvidenceResult>, String> {
        if !self.entry(group).reusable {
            return Ok(None);
        }
        let current = self.verify_current(root, group)?;
        // Planning determines scheduling only. Re-read the ledger and its archive using
        // the input digest just verified above, without hashing the same inputs twice.
        let success =
            module_ledger::compatible_success_for_input(root, ModuleId::Resource(group), &current)?
                .ok_or_else(|| {
                    format!(
                        "{} resource last-success is no longer compatible",
                        group.name()
                    )
                })?;
        let detail = format!(
            "REUSED_PASS origin_source={} origin_exe={} origin_zip={} evidence={}",
            success.origin_source_commit,
            success.origin_exe_sha256,
            success.origin_zip_sha256,
            success.evidence_path.display()
        );
        eprintln!("RESOURCE_GROUP={} {detail}", group.name());
        Ok(Some(EvidenceResult {
            id: group.task_label().into(),
            status: EvidenceStatus::Passed,
            detail: Some(detail),
            // Historical metrics stay in their origin receipt and are never stamped as a fresh run.
            measurements: Vec::new(),
            gates: Vec::new(),
            samples: Vec::new(),
        }))
    }

    pub(crate) fn record(
        &self,
        root: &Path,
        group: ResourceModule,
        result: &EvidenceResult,
        environment: Option<&QualificationEnvironment>,
    ) -> Result<(), String> {
        self.verify_current(root, group)?;
        let base = evidence::render_json(
            &self.candidate.source_commit,
            false,
            None,
            Some(&self.candidate.exe_sha256),
            "phase-14",
            std::slice::from_ref(result),
            environment,
        );
        let document = format!(
            "{{\"resource_protocol\":1,\"resource_group\":\"{}\",\"resource_input_fingerprint\":\"{}\",{}",
            group.name(),
            json::escape(&self.entry(group).fingerprint),
            &base[1..]
        );
        crate::resource_plan::validate_receipt(&document, group)?;
        receipt::write_receipt(root, group.receipt(), &document)?;
        self.verify_current(root, group)?;
        module_ledger::record_success(root, ModuleId::Resource(group), &self.candidate)
    }

    fn entry(&self, group: ResourceModule) -> &Entry {
        self.entries
            .iter()
            .find(|entry| entry.group == group)
            .expect("complete group registry")
    }

    fn verify_current(&self, root: &Path, group: ResourceModule) -> Result<String, String> {
        if receipt::read_candidate(root)? != self.candidate {
            return Err("promoted candidate changed during the resource campaign".into());
        }
        receipt::validate_candidate_against_repository(root, &self.candidate)?;
        let current = fingerprint::calculate(root, ModuleId::Resource(group))?;
        if current != self.entry(group).fingerprint {
            return Err(format!(
                "{} resource inputs changed during measurement",
                group.name()
            ));
        }
        Ok(current)
    }
}
