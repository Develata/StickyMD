//! Local release-tool checks; these commands never promote, authorize or publish.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

mod checksums;
mod cli;
mod identity;
pub(crate) mod json;
mod notices;
mod package;
mod package_content;
mod package_inputs;
mod package_publish;
mod package_rules;
mod package_runtime;
mod package_staging;
mod package_workflow;
mod promoted;
mod remote_state;
mod sbom;
mod sbom_preparation;
mod sbom_workflow;
mod syft;
mod temporary;
mod windows;
pub(crate) mod workflow;

pub(crate) use cli::{Command, PackageOptions, parse};
pub(crate) use cli::{PackageBuildOptions, SbomBuildOptions};
pub(crate) use package::verify as verify_package;
pub(crate) use package_workflow::build as build_package;
pub(crate) use sbom_workflow::generate as generate_sbom;
use std::path::Path;

pub(crate) fn execute(root: &Path, command: &Command) -> Result<(), String> {
    guard_outputs(root, command)?;
    match command {
        Command::VerifyPromoted(options) => promoted::verify(root, options),
        Command::VerifyPackage(options) => {
            verify_package(root, options, &mut std::io::stdout().lock())
        }
        Command::VerifyWorkflow { source, input } => workflow::verify_input(source, input),
        Command::VerifyRemoteState(options) => remote_state::execute(root, options),
        Command::Notices(destination) => notices::generate(root, destination),
        Command::PackageInputs(options) => package_inputs::execute(root, options),
        Command::BuildPackage(options) => {
            build_package(root, options, &mut std::io::stdout().lock())
        }
        Command::GenerateSbom(options) => {
            generate_sbom(root, options, &mut std::io::stdout().lock())
        }
        Command::PreparePackage {
            inputs,
            exe,
            directory,
        } => package_staging::prepare(root, inputs, exe, directory),
        Command::WorkspaceVersion => {
            println!("{}", crate::repository::workspace_version(root)?);
            Ok(())
        }
        Command::Checksums(options) => checksums::generate(options),
        Command::PublishSbom(options) => sbom::publish(options),
        Command::PrepareSbom(options) => sbom_preparation::execute(root, options),
        Command::PublishPackage(options) => package_publish::execute(options),
        Command::SyftPlan(provided) => syft::plan(root, provided.as_deref()),
        Command::SyftPublish { kind, input } => syft::publish(root, *kind, input),
        Command::SyftVerify {
            archive,
            checksums,
            directory,
        } => syft::verify(archive, checksums, directory),
    }
}

/// Release commands write where the caller says; none may write qualification storage
/// (the last-success ledger, the manual observation store, release decisions), which only
/// their coordinators replace under their locks. Paths are resolved the way the writers
/// resolve them (`std::path::absolute`, relative to the working directory), so the check
/// and the write name the same file.
fn guard_outputs(root: &Path, command: &Command) -> Result<(), String> {
    let mut files = Vec::new();
    let mut directories = Vec::new();
    match command {
        Command::Notices(destination) => files.push(destination),
        Command::PreparePackage { directory, .. } | Command::SyftVerify { directory, .. } => {
            directories.push(directory);
        }
        Command::Checksums(options) => files.push(&options.output),
        Command::PublishSbom(options) => files.extend([&options.output, &options.checksums]),
        Command::PublishPackage(options) => files.extend([&options.output, &options.checksums]),
        Command::PrepareSbom(options) => directories.push(&options.directory),
        Command::BuildPackage(options) => directories.extend(options.directory.as_ref()),
        Command::GenerateSbom(options) => {
            directories.extend(options.directory.as_ref());
            files.extend(options.output.as_ref());
        }
        // No caller-chosen output: temporary snapshots, fixed tool caches or no files.
        Command::VerifyPromoted(_)
        | Command::VerifyPackage(_)
        | Command::VerifyWorkflow { .. }
        | Command::VerifyRemoteState(_)
        | Command::PackageInputs(_)
        | Command::WorkspaceVersion
        | Command::SyftPlan(_)
        | Command::SyftPublish { .. } => {}
    }
    let resolve = |path: &std::path::PathBuf| {
        std::path::absolute(path)
            .map_err(|error| format!("cannot resolve output {}: {error}", path.display()))
    };
    for file in files {
        crate::qualification::validate_public_evidence_path(root, &resolve(file)?)?;
    }
    for directory in directories {
        crate::qualification::validate_public_output_directory(root, &resolve(directory)?)?;
    }
    Ok(())
}

#[cfg(test)]
mod output_guard_tests {
    use std::fs;
    use std::path::PathBuf;

    use super::Command;
    use super::cli::{ChecksumOptions, PackagePublishOptions, SbomPreparationOptions};

    #[test]
    fn release_writers_cannot_target_qualification_storage() {
        let root =
            std::env::temp_dir().join(format!("stickymd-release-guard-{}", std::process::id()));
        let evidence = root.join("dist/evidence");
        fs::create_dir_all(&evidence).unwrap();
        let log = evidence.join("manual-observations.json");
        fs::write(&log, b"keep").unwrap();
        let other = PathBuf::from("dist/release/x");
        for command in [
            Command::Checksums(ChecksumOptions {
                zip: other.clone(),
                sbom: None,
                output: log.clone(),
            }),
            Command::Notices(evidence.join("release-decisions.lock")),
            Command::PublishPackage(PackagePublishOptions {
                input: other.clone(),
                output: other.clone(),
                checksums: evidence.join("manual-targets.json"),
            }),
            Command::PrepareSbom(SbomPreparationOptions {
                package_directory: other.clone(),
                zip: None,
                syft: None,
                directory: evidence.join("manual-archive/x"),
            }),
        ] {
            let error = super::execute(&root, &command).unwrap_err();
            assert!(error.contains("coordinator-owned"), "{command:?}: {error}");
        }
        assert_eq!(fs::read(&log).unwrap(), b"keep");
        // Created exclusively by this test under the temp directory.
        let _ = fs::remove_dir_all(root);
    }

    /// A relative output is resolved against the working directory, as the writers do,
    /// not against the repository root.
    #[test]
    fn relative_outputs_are_checked_where_they_are_written() {
        let cwd = std::env::current_dir().unwrap();
        let root = cwd.join(format!(
            "target/stickymd-release-guard-{}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("dist/evidence")).unwrap();
        let relative = root
            .strip_prefix(&cwd)
            .unwrap()
            .join("dist/evidence/manual-observations.json");
        let command = Command::Checksums(ChecksumOptions {
            zip: PathBuf::from("dist/release/x"),
            sbom: None,
            output: relative,
        });
        let error = super::execute(&root, &command).unwrap_err();
        assert!(error.contains("coordinator-owned"), "{error}");
        // Created exclusively by this test under the crate's target directory.
        let _ = fs::remove_dir_all(root);
    }
}
