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
mod package_rules;
mod package_runtime;
mod package_staging;
mod promoted;
mod sbom;
mod syft;
mod temporary;
mod windows;
pub(crate) mod workflow;

pub(crate) use cli::{Command, PackageOptions, parse};
pub(crate) use package::verify as verify_package;
use std::path::Path;

pub(crate) fn execute(root: &Path, command: &Command) -> Result<(), String> {
    match command {
        Command::VerifyPromoted(options) => promoted::verify(root, options),
        Command::VerifyPackage(options) => {
            verify_package(root, options, &mut std::io::stdout().lock())
        }
        Command::VerifyWorkflow { source, input } => workflow::verify_input(source, input),
        Command::Notices(destination) => notices::generate(root, destination),
        Command::PackageInputs(options) => package_inputs::execute(root, options),
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
        Command::SyftPlan(provided) => syft::plan(root, provided.as_deref()),
        Command::SyftPublish { kind, input } => syft::publish(root, *kind, input),
        Command::SyftVerify {
            archive,
            checksums,
            directory,
        } => syft::verify(archive, checksums, directory),
    }
}
