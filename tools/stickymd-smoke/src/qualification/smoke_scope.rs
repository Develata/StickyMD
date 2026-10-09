//! Formal output paths require the complete corresponding task plan.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::module_registry::{self, ModuleId};
use super::path_identity;
use crate::cli::{Options, Phase, Selection};
use std::path::Path;

pub(crate) const RESOURCE_SUMMARY: &str = "dist/evidence/resources-qualification.json";

pub(crate) fn is_formal_measurement_path(root: &Path, path: &Path) -> bool {
    path_identity::matches_receipt(root, path, RESOURCE_SUMMARY)
        || matches!(
            module_registry::module_for_receipt(root, path),
            Some(ModuleId::Runtime | ModuleId::Performance)
        )
}

pub(crate) fn validate(root: &Path, options: &Options) -> Result<bool, String> {
    validate_with_filter(
        root,
        options,
        std::env::var_os("STICKYMD_SMOKE_RESOURCE_CASE").is_some_and(|v| !v.is_empty()),
    )
}

fn validate_with_filter(root: &Path, options: &Options, filtered: bool) -> Result<bool, String> {
    let Some(path) = options.evidence_file.as_deref() else {
        return Ok(false);
    };
    super::validate_public_evidence_path(root, path)
        .map_err(|error| format!("formal qualification output: {error}"))?;
    let resources = path_identity::matches_receipt(root, path, RESOURCE_SUMMARY);
    let module = module_registry::module_for_receipt(root, path);
    if !resources && module.is_none() {
        if options.resource_resume {
            let ignored = std::process::Command::new("git")
                .args(["check-ignore", "-q", "--"])
                .arg(root.join(path))
                .current_dir(root)
                .status()
                .map_err(|e| e.to_string())?;
            if !path_identity::is_within(root, path, "target") || !ignored.success() {
                return Err("diagnostic resume requires an ignored evidence path under target/ so checkpoints cannot change source identity".into());
            }
        }
        return Ok(false);
    }
    let mode_matches = if resources {
        options.resources && !options.runtime && !options.performance
    } else {
        match module {
            Some(ModuleId::Runtime) => {
                options.runtime && !options.performance && !options.resources
            }
            Some(ModuleId::Performance) => {
                options.performance && !options.runtime && !options.resources
            }
            _ => false,
        }
    };
    if !mode_matches
        || options.selection != Selection::Phase(Phase::P14)
        || options.ci
        || options.ci_shard.is_some()
        || options.release
        || options.package
        || options.resource_module.is_some()
        || options.resource_resume
        || options.resource_failure_first
        || filtered
        || !options.json
    {
        return Err("formal qualification output requires its complete Phase 14 mode; use a diagnostic evidence path for partial/filtered requests (resource child receipts are coordinator-owned)".into());
    }
    Ok(resources)
}

pub(super) fn validate_task_coverage(document: &str, runtime: bool) -> Result<(), String> {
    let parsed = crate::release::json::parse(document)?;
    if parsed.field("suite")?.string()? != "phase-14" {
        return Err("formal module receipt must have suite phase-14".into());
    }
    let results = parsed.field("results")?.array()?;
    let required = crate::runner::formal_task_labels(runtime)?;
    if results.len() != required.len() {
        return Err("incomplete formal module task coverage".into());
    }
    for (result, expected) in results.iter().zip(required) {
        if result.field("id")?.string()? != expected
            || result.field("status")?.string()? != "PASSED"
        {
            return Err(format!("missing successful formal module task {expected}"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formal_coverage_requires_candidate_verification_not_an_unused_local_build() {
        for runtime in [true, false] {
            let results = crate::runner::formal_task_labels(runtime)
                .unwrap()
                .into_iter()
                .map(|label| format!(r#"{{"id":"{label}","status":"PASSED"}}"#))
                .collect::<Vec<_>>()
                .join(",");
            let document = format!(r#"{{"suite":"phase-14","results":[{results}]}}"#);
            validate_task_coverage(&document, runtime).unwrap();
            let old = document.replace(
                "promoted candidate identity and artifact verification",
                "Release Windows app build",
            );
            assert!(validate_task_coverage(&old, runtime).is_err());
        }
    }
    #[test]
    fn one_passing_sentinel_does_not_cover_the_formal_task_plan() {
        let document = r#"{"suite":"phase-14","results":[{"id":"copied Release Phase 8 close-to-tray/show lifecycle","status":"PASSED"}]}"#;
        assert!(validate_task_coverage(document, true).is_err());
        assert!(validate_task_coverage(document, false).is_err());
    }
    #[test]
    fn partial_and_filtered_requests_cannot_reuse_or_overwrite_formal_receipts() {
        let root = std::env::temp_dir();
        let mut options = Options::parse(
            [
                "phase",
                "14",
                "--resources",
                "--evidence-file=dist/evidence/resources-qualification.json",
            ]
            .map(str::to_owned),
        )
        .unwrap();
        assert!(validate_with_filter(&root, &options, false).unwrap());
        options.resource_resume = true;
        assert!(validate_with_filter(&root, &options, false).is_err());
        options.evidence_file =
            Some("dist/evidence/./resources/../resources-qualification.json".into());
        assert!(validate_with_filter(&root, &options, false).is_err());
        options.evidence_file = Some(RESOURCE_SUMMARY.into());
        options.resource_resume = false;
        options.resource_failure_first = true;
        assert!(validate_with_filter(&root, &options, false).is_err());
        options.resource_failure_first = false;
        assert!(validate_with_filter(&root, &options, true).is_err());
        options.resource_module = Some(crate::cli::ResourceModule::Window);
        assert!(validate_with_filter(&root, &options, false).is_err());
        options.evidence_file =
            Some("dist/evidence/./resources/../resources-qualification.json".into());
        assert!(validate_with_filter(&root, &options, false).is_err());
        options.evidence_file = Some("dist/evidence/window-diagnostic.json".into());
        assert!(!validate_with_filter(&root, &options, false).unwrap());
        options.resource_module = None;
        options.evidence_file = Some(crate::cli::ResourceModule::Window.receipt().into());
        assert!(validate_with_filter(&root, &options, false).is_err());
        options.evidence_file = Some("dist/evidence/runtime-qualification.json".into());
        assert!(validate_with_filter(&root, &options, false).is_err());
        options.resources = false;
        options.runtime = true;
        assert!(!validate_with_filter(&root, &options, false).unwrap());
        options.selection = Selection::Phase(Phase::P08);
        assert!(validate_with_filter(&root, &options, false).is_err());
    }
}
