//! Fine resource progress and incomplete case checkpoints; never successful evidence.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::*;
use crate::resource_plan::progress::Observer;

pub(super) fn command_state(root: &Path, options: &Options, stage: &str) -> Result<(), String> {
    let Some(output) = options
        .evidence_file
        .as_deref()
        .filter(|_| options.resources && options.json)
    else {
        return Ok(());
    };
    let path = root.join(output).with_extension("progress.json");
    crate::qualification::validate_public_evidence_path(root, &path)?;
    let json = format!(
        "{{\"schema_version\":1,\"status\":\"INCOMPLETE\",\"stage\":\"{}\",\"remaining_fixed_wait_seconds\":null}}",
        evidence::escape_json(stage)
    );
    crate::atomic_evidence::write(&path, json.as_bytes())
}

pub(super) struct Reporter<'a> {
    root: &'a Path,
    suite: &'a str,
    output: Option<&'a Path>,
    group: ResourceModule,
    completed: &'a [EvidenceResult],
    environment: Option<&'a QualificationEnvironment>,
    remaining: u64,
}

impl<'a> Reporter<'a> {
    pub(super) fn new(
        root: &'a Path,
        suite: &'a str,
        options: &'a Options,
        group: ResourceModule,
        completed: &'a [EvidenceResult],
        environment: Option<&'a QualificationEnvironment>,
        remaining: u64,
    ) -> Self {
        Self {
            root,
            suite,
            output: options.evidence_file.as_deref().filter(|_| options.json),
            group,
            completed,
            environment,
            remaining,
        }
    }

    pub(super) fn remaining(&self) -> u64 {
        self.remaining
    }

    pub(super) fn finish(&mut self, successful: bool) -> Result<(), String> {
        self.stage(
            "",
            0,
            if successful {
                "group-finished"
            } else {
                "failed"
            },
            0,
        )
    }
}

impl Observer for Reporter<'_> {
    fn stage(&mut self, cohort: &str, run: usize, stage: &str, seconds: u64) -> Result<(), String> {
        eprintln!(
            "RESOURCE_PROGRESS group={} cohort={cohort} run={run}/{} stage={stage} phase_fixed_seconds={seconds} remaining_fixed_wait_seconds={}",
            self.group.name(),
            crate::resource_plan::REPETITIONS,
            self.remaining
        );
        let Some(output) = self.output else {
            return Ok(());
        };
        let path = self.root.join(output).with_extension("progress.json");
        crate::qualification::validate_public_evidence_path(self.root, &path)?;
        let millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| format!("resource progress clock: {e}"))?
            .as_millis();
        let json = format!(
            "{{\"schema_version\":1,\"status\":\"INCOMPLETE\",\"group\":\"{}\",\"cohort\":\"{}\",\"run\":{run},\"repetitions\":{},\"stage\":\"{}\",\"phase_started_unix_ms\":{millis},\"phase_fixed_seconds\":{seconds},\"remaining_fixed_wait_seconds\":{}}}",
            self.group.name(),
            crate::evidence::escape_json(cohort),
            crate::resource_plan::REPETITIONS,
            crate::evidence::escape_json(stage),
            self.remaining
        );
        crate::atomic_evidence::write(&path, json.as_bytes())
    }

    fn waited(&mut self, seconds: u64) {
        self.remaining = self.remaining.saturating_sub(seconds);
    }

    fn checkpoint(&mut self, mut result: EvidenceResult) -> Result<(), String> {
        let Some(path) = self.output else {
            return Ok(());
        };
        // Neither callers nor a partially completed cohort can turn this snapshot into PASS.
        result.status = EvidenceStatus::NotTested;
        result.detail = Some("INCOMPLETE: resource group has not finished".into());
        let mut results = self.completed.to_vec();
        results.push(result);
        evidence::emit(
            self.root,
            self.suite,
            &results,
            self.environment,
            Some(path),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource_plan::progress::Observer;

    #[test]
    fn case_checkpoint_and_live_progress_never_promote_partial_success() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "stickymd-case-progress-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let output = root.join("diagnostic.json");
        let options = Options::parse(
            [
                "phase",
                "14",
                "--resources",
                "--json",
                &format!("--evidence-file={}", output.display()),
            ]
            .map(str::to_owned),
        )
        .unwrap();
        std::fs::write(output.with_extension("progress.json"), "old run").unwrap();
        command_state(&root, &options, "planning").unwrap();
        assert!(
            std::fs::read_to_string(output.with_extension("progress.json"))
                .unwrap()
                .contains("planning")
        );
        let mut reporter = Reporter::new(
            &root,
            "phase-14",
            &options,
            ResourceModule::Images,
            &[],
            None,
            450,
        );
        reporter
            .stage("preview-12-images", 1, "warmup", 30)
            .unwrap();
        reporter.waited(30);
        reporter.stage("preview-12-images", 1, "cpu", 60).unwrap();
        let stage = std::fs::read_to_string(output.with_extension("progress.json")).unwrap();
        assert!(stage.contains("\"remaining_fixed_wait_seconds\":420"));
        assert!(stage.contains("\"stage\":\"cpu\""));
        let mut result = EvidenceResult {
            id: "images".into(),
            status: EvidenceStatus::Passed,
            detail: None,
            measurements: Vec::new(),
            gates: Vec::new(),
            samples: Vec::new(),
        };
        result.measurements.push(EvidenceMeasurement {
            name: "preserved".into(),
            unit: "bytes".into(),
            value: 123.0,
        });
        reporter.checkpoint(result).unwrap();
        let checkpoint = std::fs::read_to_string(&output).unwrap();
        assert!(checkpoint.contains("INCOMPLETE"));
        assert!(checkpoint.contains("NOT_TESTED"));
        assert!(checkpoint.contains("preserved"));
        assert!(!root.join("dist/evidence/module-success").exists());
        reporter.finish(false).unwrap();
        assert!(
            std::fs::read_to_string(output.with_extension("progress.json"))
                .unwrap()
                .contains("\"stage\":\"failed\"")
        );
        // Atomic write must fail on a directory rather than silently lose progress.
        std::fs::remove_file(output.with_extension("progress.json")).unwrap();
        std::fs::create_dir(output.with_extension("progress.json")).unwrap();
        assert!(reporter.stage("next", 2, "warmup", 30).is_err());
        std::fs::remove_dir_all(root).unwrap();
    }
}
