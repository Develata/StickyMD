//! Local release-tool checks: verification of the bytes `stickymd-package` builds, remote state,
//! the macOS promotion, the macOS readiness check and the reuse rule. These commands never build release bytes (plan 11
//! #platform-artifact-reuse: that is the packaging crate's), and never authorize or publish.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

mod attachments;
mod cli;
pub(crate) mod macos;
mod package;
mod package_rules;
mod package_runtime;
pub(crate) mod platform_inputs;
mod promoted;
mod remote_state;
pub(crate) mod workflow;

// The packaging crate's modules under the names the verification code has always used.
use stickymd_package::{identity, notices, package_content, sbom, temporary, windows};
pub(crate) use stickymd_package::{json, platform};

pub(crate) use cli::{Command, PackageOptions, parse};
pub(crate) use package::verify as verify_package;
use std::path::Path;

pub(crate) fn execute(root: &Path, command: &Command) -> Result<(), String> {
    match command {
        Command::VerifyPromoted(options) => promoted::verify(root, options),
        Command::VerifyPackage(options) => {
            verify_package(root, options, &mut std::io::stdout().lock())
        }
        Command::VerifyWorkflow {
            source,
            input,
            workflow,
            attempt,
            origin,
        } => workflow::verify_named_input(
            source,
            input,
            workflow,
            *attempt,
            origin
                .as_ref()
                .map(|(path, event)| (path.as_str(), event.as_str())),
        ),
        Command::VerifyAttachments {
            directory,
            attachments,
        } => {
            let version = crate::repository::workspace_version(root)?;
            let approved = attachments::parse_spec(attachments, &version)?;
            attachments::verify_directory(directory, &approved)?;
            println!("ATTACHMENTS=PASS");
            Ok(())
        }
        Command::VerifyReleaseSet {
            release,
            tag,
            attachments,
            windows,
            macos,
        } => {
            let set = attachments::verify_release_set(
                root,
                release,
                tag,
                attachments,
                &[
                    (platform::Platform::WindowsX64, windows.clone()),
                    (platform::Platform::MacOsArm64, macos.clone()),
                ],
            )?;
            println!("{}", set.json());
            Ok(())
        }
        Command::VerifyRemoteState(options) => remote_state::execute(root, options),
        Command::PlatformInputs { platform, commit } => {
            let fingerprint = platform_inputs::fingerprint(root, *platform, commit)?;
            println!(
                "{{\"platform\":\"{}\",\"commit\":\"{commit}\",\"algorithm\":\"{}\",\"fingerprint\":\"{fingerprint}\"}}",
                platform.label(),
                platform_inputs::ALGORITHM
            );
            Ok(())
        }
        Command::VerifyReuse {
            platform,
            build,
            release,
        } => {
            let reuse = match platform_inputs::verify_reuse(root, *platform, build, release)? {
                platform_inputs::Reuse::SameCommit => "same-commit".to_owned(),
                platform_inputs::Reuse::Ancestor { fingerprint } => {
                    format!("ancestor:{fingerprint}")
                }
            };
            println!(
                "{{\"platform\":\"{}\",\"build_source\":\"{build}\",\"release_source\":\"{release}\",\"reuse\":\"{reuse}\"}}",
                platform.label()
            );
            Ok(())
        }
        Command::MacosVerifyPackage { zip, source } => {
            macos::verify_command(root, zip, source.as_deref())
        }
        Command::MacosVerifyArtifact { directory, source } => {
            macos::verify_artifact_command(root, directory, source)
        }
        Command::MacosPromote {
            run_id,
            attempt,
            release,
            zip,
        } => macos::promote_command(root, *run_id, *attempt, release, zip),
        Command::MacosCandidate { release } => macos::candidate_command(root, release),
        Command::MacosSourceTests { release } => macos::source_tests_command(root, release),
        Command::MacosReady {
            release,
            batch,
            macos_major,
            driver,
        } => macos::ready_command(root, release, batch.as_deref(), *macos_major, driver),
        Command::WorkspaceVersion => {
            println!("{}", crate::repository::workspace_version(root)?);
            Ok(())
        }
    }
}
