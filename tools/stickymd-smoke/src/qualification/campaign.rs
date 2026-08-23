//! Ordered Phase 13 local qualification campaign over the exact frozen candidate.

use std::path::{Path, PathBuf};

use crate::cli::{Options, Phase, Selection};
use crate::runner;

use super::{decisions, readiness, receipt};

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

    let candidate = receipt::generate_candidate(root)?;
    decisions::project(root, &candidate)?;
    println!("RELEASE_SOURCE_COMMIT={}", candidate.source_commit);
    println!("RELEASE_EXE_SHA256={}", candidate.exe_sha256);
    println!("RELEASE_ZIP_SHA256={}", candidate.zip_sha256);

    runner::execute(
        root,
        &Options {
            selection: Selection::All,
            ci: true,
            performance: false,
            runtime: false,
            resources: false,
            release: false,
            package: false,
            json: true,
            evidence_file: Some(PathBuf::from(
                "dist/evidence/headless-ci-qualification.json",
            )),
        },
    )?;
    run_mode(
        root,
        false,
        true,
        false,
        false,
        "dist/evidence/runtime-qualification.json",
    )?;
    run_mode(
        root,
        true,
        false,
        false,
        false,
        "dist/evidence/performance-qualification.json",
    )?;
    run_mode(
        root,
        false,
        false,
        true,
        false,
        "dist/evidence/resources-qualification.json",
    )?;
    readiness::evaluate(root, true)
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
            selection: Selection::Phase(Phase::P13),
            ci: false,
            performance,
            runtime,
            resources,
            release,
            package: false,
            json: true,
            evidence_file: Some(PathBuf::from(evidence_file)),
        },
    )
}
