//! Ordered local/source preflight before remote artifact promotion.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use std::path::{Path, PathBuf};

use crate::cli::{Options, Phase, Selection};
use crate::runner;

use super::{decisions, source_freeze};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FailureClass {
    Ordinary,
    Global,
}

#[derive(Default)]
struct CampaignLedger {
    completed: Vec<&'static str>,
    failures: Vec<String>,
}

impl CampaignLedger {
    fn record(&mut self, channel: &'static str, result: Result<(), String>) -> Result<(), String> {
        self.completed.push(channel);
        if let Err(error) = result {
            if classify_error(&error) == FailureClass::Global {
                return Err(format!("global qualification stop in {channel}: {error}"));
            }
            eprintln!("QUALIFICATION_CHANNEL_FAILED channel={channel} error={error}");
            self.failures.push(format!("{channel}: {error}"));
        }
        Ok(())
    }

    fn finish(self) -> Result<(), String> {
        if self.failures.is_empty() {
            Ok(())
        } else {
            Err(format!(
                "Phase 14 qualification completed independent channels with {} failure(s): {}",
                self.failures.len(),
                self.failures.join(" | ")
            ))
        }
    }
}

pub(super) fn run(root: &Path) -> Result<(), String> {
    super::record_environment(
        root,
        Some(Path::new("dist/evidence/qualification-environment.json")),
    )?;
    run_mode(
        root,
        false,
        false,
        false,
        true,
        "dist/evidence/automated-qualification.json",
    )?;

    let source = source_freeze::create(root)?;
    decisions::project(root, &source)?;
    println!("RELEASE_SOURCE_COMMIT={}", source.source_commit);
    println!("RELEASE_CARGO_LOCK_SHA256={}", source.cargo_lock_sha256);

    let mut ledger = CampaignLedger::default();
    ledger.record(
        "headless-ci",
        runner::execute(
            root,
            &Options {
                selection: Selection::All,
                ci: true,
                ci_shard: None,
                performance: false,
                runtime: false,
                resources: false,
                resource_module: None,
                release: false,
                package: false,
                json: true,
                evidence_file: Some(PathBuf::from(
                    "dist/evidence/headless-ci-qualification.json",
                )),
            },
        ),
    )?;
    println!("LOCAL_PREFLIGHT=PASS");
    println!(
        "NEXT=push approved Source Freeze, run remote release workflow, then Promote downloaded artifact"
    );
    ledger.finish()
}

fn classify_error(error: &str) -> FailureClass {
    let normalized = error.to_ascii_lowercase();
    if normalized.contains("environment is blocked")
        || normalized.contains("unsupported on this host")
        || normalized.contains("stale receipt")
        || normalized.contains("identity mismatch")
        || normalized.contains("schema corruption")
        || normalized.contains("p0")
        || normalized.contains("data-safety")
        || normalized.contains("security invariant")
    {
        FailureClass::Global
    } else {
        FailureClass::Ordinary
    }
}

fn run_mode(
    root: &Path,
    performance: bool,
    runtime: bool,
    resources: bool,
    release: bool,
    evidence_file: &str,
) -> Result<(), String> {
    runner::execute(
        root,
        &Options {
            selection: Selection::Phase(Phase::P14),
            ci: false,
            ci_shard: None,
            performance,
            runtime,
            resources,
            resource_module: None,
            release,
            package: false,
            json: true,
            evidence_file: Some(PathBuf::from(evidence_file)),
        },
    )
}

#[cfg(test)]
mod tests {
    use super::{CampaignLedger, FailureClass, classify_error};

    #[test]
    fn ordinary_runtime_performance_and_resource_failures_preserve_independent_channels() {
        let mut ledger = CampaignLedger::default();
        ledger
            .record("runtime", Err("runtime ordinary failure".to_owned()))
            .expect("ordinary runtime failure continues");
        ledger
            .record("performance", Err("startup exceeded boundary".to_owned()))
            .expect("performance failure continues");
        ledger
            .record("resources", Err("resource limit exceeded".to_owned()))
            .expect("resource failure continues");
        assert_eq!(ledger.completed, ["runtime", "performance", "resources"]);
        assert_eq!(ledger.failures.len(), 3);
        assert!(ledger.finish().is_err());
    }

    #[test]
    fn environment_identity_and_data_safety_failures_are_global_stops() {
        for error in [
            "qualification environment is blocked",
            "candidate identity mismatch",
            "P0 data loss",
            "data-safety invariant failed",
            "STALE RECEIPT: executable changed",
            "receipt schema corruption",
        ] {
            assert_eq!(classify_error(error), FailureClass::Global);
        }
        assert_eq!(
            classify_error("ordinary runtime rendering failure"),
            FailureClass::Ordinary
        );
    }
}
