//! Incomplete aggregate checkpoints; complete child groups own their success receipts.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::path::Path;

use crate::cli::Options;
use crate::evidence::{self, EvidenceResult, EvidenceStatus};
use crate::qualification_environment::QualificationEnvironment;

pub(super) fn emit(
    root: &Path,
    suite: &str,
    options: &Options,
    results: &mut Vec<EvidenceResult>,
    environment: Option<&QualificationEnvironment>,
) -> Result<(), String> {
    // Stdout-only JSON must remain a single document. Persistent checkpoints are
    // limited to resource runs with an explicitly selected evidence destination.
    let Some(path) = options
        .evidence_file
        .as_deref()
        .filter(|_| options.resources && options.json)
    else {
        return Ok(());
    };
    let marker = results.len();
    results.push(EvidenceResult {
        id: "resource qualification progress".to_owned(),
        status: EvidenceStatus::NotTested,
        detail: Some("INCOMPLETE: resource qualification has not finished".to_owned()),
        measurements: Vec::new(),
        gates: Vec::new(),
        samples: Vec::new(),
    });
    match evidence::emit(root, suite, results, environment, Some(path)) {
        Ok(()) => {
            results.pop();
            Ok(())
        }
        Err(error) => {
            let detail = format!("INCOMPLETE: resource progress evidence emission failed: {error}");
            // The runner still attempts final emission on error. Keep a failure
            // row so that attempt cannot promote the completed prefix to PASS.
            results[marker].status = EvidenceStatus::Failed;
            results[marker].detail = Some(detail.clone());
            Err(detail)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct Fixture(PathBuf);

    impl Fixture {
        fn new() -> Self {
            let nonce = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let root = std::env::temp_dir().join(format!(
                "stickymd-resource-progress-{}-{nonce}",
                std::process::id()
            ));
            fs::create_dir_all(root.join("dist/evidence/module-success")).unwrap();
            Self(root)
        }

        fn options(&self) -> Options {
            Options::parse([
                "phase".to_owned(),
                "14".to_owned(),
                "--resources".to_owned(),
                format!(
                    "--evidence-file={}",
                    self.0
                        .join("dist/evidence/resources-qualification.json")
                        .display()
                ),
            ])
            .unwrap()
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).unwrap();
        }
    }

    fn completed_stage() -> EvidenceResult {
        EvidenceResult {
            id: "completed source/preview stage".to_owned(),
            status: EvidenceStatus::Passed,
            detail: None,
            measurements: vec![crate::evidence::EvidenceMeasurement {
                name: "source.private_working_set_max".to_owned(),
                unit: "MiB".to_owned(),
                value: 14.5,
            }],
            gates: Vec::new(),
            samples: Vec::new(),
        }
    }

    #[test]
    fn resource_progress_replaces_stale_success_and_preserves_completed_measurements() {
        let fixture = Fixture::new();
        let options = fixture.options();
        let output = options.evidence_file.as_ref().unwrap();
        let ledger = fixture
            .0
            .join("dist/evidence/module-success/resources.json");
        fs::write(output, "stale successful receipt").unwrap();
        fs::write(&ledger, "last complete success").unwrap();
        let mut results = Vec::new();
        emit(&fixture.0, "phase-14", &options, &mut results, None).unwrap();
        assert!(results.is_empty());
        let initial = fs::read_to_string(output).unwrap();
        assert!(initial.contains("INCOMPLETE"));
        assert!(!initial.contains("PASSED"));

        results.push(completed_stage());
        emit(&fixture.0, "phase-14", &options, &mut results, None).unwrap();
        let partial = fs::read_to_string(output).unwrap();
        assert!(partial.contains("INCOMPLETE"));
        assert!(partial.contains("\"status\":\"NOT_TESTED\""));
        assert!(partial.contains("source.private_working_set_max"));
        assert!(partial.contains("14.500000"));
        assert_eq!(results, [completed_stage()]);
        assert_eq!(
            fs::read_to_string(&ledger).unwrap(),
            "last complete success"
        );

        // A final complete run replaces the temporary marker; use a diagnostic
        // destination so the synthetic fixture needs no real candidate ledger.
        let final_output = fixture.0.join("final.json");
        fs::write(&final_output, partial).unwrap();
        evidence::emit(&fixture.0, "phase-14", &results, None, Some(&final_output)).unwrap();
        let complete = fs::read_to_string(final_output).unwrap();
        assert!(!complete.contains("INCOMPLETE"));
        assert!(!complete.contains("NOT_TESTED"));
    }

    #[test]
    fn failed_resource_checkpoint_cannot_be_promoted_by_final_emission() {
        let fixture = Fixture::new();
        let mut options = fixture.options();
        options.evidence_file = Some(fixture.0.clone());
        let mut results = vec![completed_stage()];
        assert!(emit(&fixture.0, "phase-14", &options, &mut results, None).is_err());
        assert_eq!(results.last().unwrap().status, EvidenceStatus::Failed);
        let output = fixture.0.join("dist/evidence/resources-qualification.json");
        evidence::emit(&fixture.0, "phase-14", &results, None, Some(&output)).unwrap();
        let document = fs::read_to_string(output).unwrap();
        assert!(document.contains("INCOMPLETE"));
        assert!(document.contains("\"status\":\"FAILED\""));
        assert_eq!(
            fs::read_dir(fixture.0.join("dist/evidence/module-success"))
                .unwrap()
                .count(),
            0
        );
    }

    #[test]
    fn non_resource_and_stdout_runs_do_not_emit_checkpoints() {
        let fixture = Fixture::new();
        let mut options = fixture.options();
        let output = options.evidence_file.clone().unwrap();
        let mut results = vec![completed_stage()];
        options.resources = false;
        emit(&fixture.0, "phase-14", &options, &mut results, None).unwrap();
        assert!(!output.exists());
        options.resources = true;
        options.evidence_file = None;
        emit(&fixture.0, "phase-14", &options, &mut results, None).unwrap();
        assert_eq!(results, [completed_stage()]);
    }
}
