//! Dispatch of the packaging commands, behind the output guard. These commands build or publish
//! release bytes; they never promote, authorize or publish a release.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use crate::cli::Command;
use crate::{
    app_inputs, checksums, macos, notices, output_guard, package_inputs, package_publish,
    package_staging, package_workflow, sbom, sbom_preparation, sbom_workflow, syft,
};
use std::path::{Path, PathBuf};

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
        Command::NativeRuntime { exe } => native_runtime(root, exe),
        Command::MacosVerifyPackage { zip, source } => {
            macos::verify_command(root, zip, source.as_deref())
        }
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

/// The PE import gate: the release executable imports no developer runtime. A relative path is
/// the repository's.
pub fn native_runtime(root: &Path, exe: &Path) -> Result<(), String> {
    let exe = if exe.is_absolute() {
        exe.to_path_buf()
    } else {
        root.join(exe)
    };
    let report = crate::pe_dependencies::verify_portable_executable(&exe)?;
    println!("PORTABLE_NATIVE_DEPENDENCIES={}", report.imports.join(","));
    println!("DEVELOPER_RUNTIME_IMPORTS=none");
    Ok(())
}

/// Packaging commands write where the caller says, never into qualification storage (the
/// last-success ledger, the manual observation store, release decisions), which only the
/// coordinators of `stickymd-smoke` replace under their locks.
fn guard_outputs(root: &Path, command: &Command) -> Result<(), String> {
    use crate::platform::Platform;
    let mut files: Vec<PathBuf> = Vec::new();
    let mut directories: Vec<PathBuf> = Vec::new();
    // Where the commands write when no directory is given.
    let default_directory = root.join("dist");
    let version = || crate::repository::workspace_version(root);
    match command {
        Command::Notices { destination, .. } => files.push(destination.clone()),
        Command::MacosPackage { output, .. } => {
            directories.push(output.clone());
            files.push(output.join(Platform::MacOsArm64.archive_name(&version()?)));
        }
        Command::MacosSbom { zip } => {
            let directory = zip.parent().map(Path::to_path_buf).unwrap_or_default();
            files.push(zip.clone());
            files.push(directory.join(Platform::MacOsArm64.checksum_file()));
            files.push(directory.join(Platform::MacOsArm64.sbom_file()));
        }
        Command::PreparePackage { directory, .. } | Command::SyftVerify { directory, .. } => {
            directories.push(directory.clone());
        }
        Command::Checksums(options) => files.push(options.output.clone()),
        Command::PublishSbom(options) => {
            files.extend([options.output.clone(), options.checksums.clone()]);
        }
        Command::PublishPackage(options) => {
            files.extend([options.output.clone(), options.checksums.clone()]);
        }
        Command::PrepareSbom(options) => directories.push(options.directory.clone()),
        Command::BuildPackage(options) => {
            let directory = options.directory.clone().unwrap_or(default_directory);
            let inputs = crate::package_inputs::resolve(root, &options.inputs)?;
            files.push(directory.join(&inputs.archive));
            files.push(directory.join(Platform::WindowsX64.checksum_file()));
            directories.push(directory);
        }
        Command::GenerateSbom(options) => {
            let directory = options.directory.clone().unwrap_or(default_directory);
            files.push(
                options
                    .output
                    .clone()
                    .unwrap_or_else(|| directory.join(Platform::WindowsX64.sbom_file())),
            );
            files.push(directory.join(Platform::WindowsX64.checksum_file()));
            directories.push(directory);
        }
        // No caller-chosen output: fixed tool caches or no files.
        Command::PackageInputs(_)
        | Command::NativeRuntime { .. }
        | Command::MacosVerifyPackage { .. }
        | Command::AppInputs { .. }
        | Command::SyftPlan(_)
        | Command::SyftPublish { .. } => {}
    }
    let guard = output_guard::Guard::new(root)?;
    for file in &files {
        guard.check_file(file)?;
    }
    for directory in &directories {
        guard.check_directory(directory)?;
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
