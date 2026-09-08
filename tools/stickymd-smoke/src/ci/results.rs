//! Aggregate only the requested CI jobs, never qualification receipts.
//! plan_ref: docs/plan/11_testing_and_release.md#modular-headless-ci

use super::Checks;
use crate::headless::{Module, parse_modules};
use std::collections::BTreeMap;

const JOBS: [&str; 6] = [
    "plan",
    "dependency",
    "quality",
    "headless",
    "release",
    "portable",
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Status {
    Success,
    Failure,
    Cancelled,
    Skipped,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Results {
    cancelled: bool,
    full: bool,
    modules: Vec<Module>,
    statuses: [Status; 6],
}

impl Results {
    pub(super) fn parse(args: &[String]) -> Result<Self, String> {
        let mut values = BTreeMap::new();
        for arg in args {
            let (key, value) = arg
                .strip_prefix("--")
                .and_then(|arg| arg.split_once('='))
                .ok_or("invalid CI result argument")?;
            if !["full", "modules", "cancelled"].contains(&key) && !JOBS.contains(&key) {
                return Err(format!("unknown CI result field {key}"));
            }
            if values.insert(key, value).is_some() {
                return Err(format!("duplicate CI result field {key}"));
            }
        }
        let field = |key| {
            values
                .get(key)
                .copied()
                .ok_or_else(|| format!("missing CI result field {key}"))
        };
        let cancelled = match field("cancelled")? {
            "true" => true,
            "false" => false,
            _ => return Err("invalid workflow cancellation state".to_owned()),
        };
        let full = match field("full")? {
            "true" => true,
            "false" => false,
            _ => return Err("invalid full CI scope".to_owned()),
        };
        let modules = match field("modules")? {
            "" => Vec::new(),
            list => parse_modules(list)?,
        };
        if full && modules != Module::ALL {
            return Err("full CI scope requires all registered module names".to_owned());
        }
        let mut statuses = [Status::Skipped; 6];
        for (index, job) in JOBS.iter().enumerate() {
            statuses[index] = match field(job)? {
                "success" => Status::Success,
                "failure" => Status::Failure,
                "cancelled" => Status::Cancelled,
                "skipped" => Status::Skipped,
                value => return Err(format!("invalid job status {job}={value}")),
            };
        }
        Ok(Self {
            cancelled,
            full,
            modules,
            statuses,
        })
    }

    pub(super) fn verify(&self) -> Result<(), String> {
        if self.cancelled {
            return Err("CI workflow was cancelled".to_owned());
        }
        let checks = Checks::for_modules(self.full, &self.modules);
        let required = [
            true,
            checks.dependency,
            checks.quality,
            checks.headless,
            checks.release,
            checks.portable,
        ];
        for ((job, required), actual) in JOBS.iter().zip(required).zip(self.statuses) {
            let expected = if required {
                Status::Success
            } else {
                Status::Skipped
            };
            if actual != expected {
                return Err(format!(
                    "CI job {job}: expected {expected:?}, observed {actual:?}"
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ci_results_reject_failure_cancellation_and_unexpected_skips() {
        let success = Results {
            cancelled: false,
            full: true,
            modules: Module::ALL.to_vec(),
            statuses: [Status::Success; 6],
        };
        assert!(success.verify().is_ok());
        let mut cancelled_workflow = success.clone();
        cancelled_workflow.cancelled = true;
        assert!(cancelled_workflow.verify().is_err());
        for index in 0..6 {
            for status in [Status::Failure, Status::Cancelled, Status::Skipped] {
                let mut bad = success.clone();
                bad.statuses[index] = status;
                assert!(bad.verify().is_err(), "{index} {status:?}");
            }
        }
        let docs = Results {
            cancelled: false,
            full: false,
            modules: vec![],
            statuses: [
                Status::Success,
                Status::Skipped,
                Status::Skipped,
                Status::Skipped,
                Status::Skipped,
                Status::Skipped,
            ],
        };
        assert!(docs.verify().is_ok());
        let mut unexpected = docs;
        unexpected.statuses[3] = Status::Failure;
        assert!(unexpected.verify().is_err());
    }

    #[test]
    fn ci_result_input_requires_known_complete_unique_status_fields() {
        let args = [
            "--cancelled=false",
            "--full=false",
            "--modules=smoke",
            "--plan=success",
            "--dependency=success",
            "--quality=success",
            "--headless=success",
            "--release=skipped",
            "--portable=skipped",
        ]
        .map(str::to_owned);
        assert!(Results::parse(&args).unwrap().verify().is_ok());
        assert!(Results::parse(&args[..7]).is_err());
        let mut duplicate = args.to_vec();
        duplicate.push(args[0].clone());
        assert!(Results::parse(&duplicate).is_err());
        let mut invalid = args;
        invalid[3] = "--plan=unknown".to_owned();
        assert!(Results::parse(&invalid).is_err());
    }
}
