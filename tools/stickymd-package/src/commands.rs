//! Dispatch of the packaging commands, behind the output guard. These commands build or publish
//! release bytes; they never promote, authorize or publish a release.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use crate::cli::Command;
use crate::{
    app_inputs, checksums, macos, notices, output_guard, package_inputs, package_publish,
    package_staging, package_workflow, sbom, sbom_preparation, sbom_workflow, syft,
};
use std::path::Path;

pub fn execute(root: &Path, command: &Command) -> Result<(), String> {
    guard_outputs(root, command)?;
    match command {
        Command::Notices {
            destination,
            platform,
        } => notices::generate(root, destination, *platform),
        Command::PackageInputs(options) => package_inputs::execute(root, options),
        Command::BuildPackage(options) => {
            package_workflow::build(root, options, &mut std::io::stdout().lock())
        }
        Command::GenerateSbom(options) => {
            sbom_workflow::generate(root, options, &mut std::io::stdout().lock())
        }
        Command::PreparePackage {
            inputs,
            exe,
            directory,
        } => package_staging::prepare(root, inputs, exe, directory),
        Command::MacosPackage {
            app,
            output,
            allow_dirty,
        } => macos::package_command(root, app, output, *allow_dirty),
        Command::MacosSbom { zip } => macos::sbom_command(root, zip),
        Command::AppInputs { home } => {
            println!("{}", app_inputs::compute(root, home.as_deref())?);
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

/// Packaging commands write where the caller says, never into qualification storage (the
/// last-success ledger, the manual observation store, release decisions), which only the
/// coordinators of `stickymd-smoke` replace under their locks.
fn guard_outputs(root: &Path, command: &Command) -> Result<(), String> {
    let mut files = Vec::new();
    let mut directories = Vec::new();
    match command {
        Command::Notices { destination, .. } => files.push(destination),
        Command::MacosPackage { output, .. } => directories.push(output),
        Command::MacosSbom { zip } => files.push(zip),
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
        // No caller-chosen output: fixed tool caches or no files.
        Command::PackageInputs(_)
        | Command::AppInputs { .. }
        | Command::SyftPlan(_)
        | Command::SyftPublish { .. } => {}
    }
    for file in files {
        output_guard::check_file(root, file)?;
    }
    for directory in directories {
        output_guard::check_directory(root, directory)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;

    use crate::cli::{ChecksumOptions, Command, PackagePublishOptions, SbomPreparationOptions};

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
            Command::Notices {
                destination: evidence.join("release-decisions.lock"),
                platform: crate::platform::Platform::MacOsArm64,
            },
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
            // Any evidence path, a diagnostic one included: release outputs are not evidence.
            Command::Checksums(ChecksumOptions {
                zip: other.clone(),
                sbom: None,
                output: evidence.join("diagnostic.txt"),
            }),
            // Passing through the evidence directory on the way elsewhere would create it.
            Command::Checksums(ChecksumOptions {
                zip: other.clone(),
                sbom: None,
                output: evidence.join("../SHA256SUMS.txt"),
            }),
            Command::Checksums(ChecksumOptions {
                zip: other.clone(),
                sbom: None,
                output: root.join("target/resource-diagnostics/x.txt"),
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

    /// The clone-wide module ledger lives in the git common directory, outside `dist/`.
    #[test]
    fn the_clone_wide_ledger_store_is_refused() {
        let root =
            std::env::temp_dir().join(format!("stickymd-release-store-{}", std::process::id()));
        fs::create_dir_all(root.join(".git/stickymd/qualification-ledger")).unwrap();
        let command = Command::Checksums(ChecksumOptions {
            zip: PathBuf::from("dist/release/x"),
            sbom: None,
            output: root.join(".git/stickymd/qualification-ledger/records.json"),
        });
        let error = super::execute(&root, &command).unwrap_err();
        assert!(error.contains("coordinator-owned"), "{error}");
        let _ = fs::remove_dir_all(root);
    }
}
