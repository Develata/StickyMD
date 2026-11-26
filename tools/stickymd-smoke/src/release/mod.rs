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
/// The source identity of a clean checkout of R (plan 11 #macos-readiness-check), required by
/// the macOS gate and promotion and, on every host, by `verify-reuse`.
mod source_identity;
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
            println!("{}", verify_reuse_command(root, *platform, build, release)?);
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

/// `verify-reuse`: whether `release` may carry `platform`'s artifact built on `build`, as the JSON
/// line the command prints (plan 11 #platform-artifact-reuse). The verdict is given only on a
/// clean checkout of R, the source identity of #macos-readiness-check, on Windows and macOS
/// alike: the exclusion table applied is this program's, so only on R is it R's (fix-round review
/// R3-2). `platform-inputs --commit` stays a query of any commit.
fn verify_reuse_command(
    root: &Path,
    platform: platform::Platform,
    build: &str,
    release: &str,
) -> Result<String, String> {
    source_identity::require(root, release, "verify-reuse")?;
    let reuse = match platform_inputs::verify_reuse(root, platform, build, release)? {
        platform_inputs::Reuse::SameCommit => "same-commit".to_owned(),
        platform_inputs::Reuse::Ancestor { fingerprint } => format!("ancestor:{fingerprint}"),
    };
    Ok(format!(
        "{{\"platform\":\"{}\",\"build_source\":\"{build}\",\"release_source\":\"{release}\",\"reuse\":\"{reuse}\"}}",
        platform.label()
    ))
}

#[cfg(test)]
mod tests {
    use super::platform::Platform;
    use super::temporary::TemporaryDirectory;
    use std::path::Path;

    fn git(root: &Path, arguments: &[&str]) -> String {
        let output = std::process::Command::new("git")
            .current_dir(root)
            .args(arguments)
            .env("GIT_AUTHOR_NAME", "t")
            .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
            .env("GIT_COMMITTER_NAME", "t")
            .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
            .output()
            .unwrap();
        assert!(output.status.success(), "git {arguments:?}: {output:?}");
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    }

    /// Fix-round review R3-2: the standalone `verify-reuse` gives its verdict only on a clean
    /// checkout of R, so the exclusion table it applies is R's; elsewhere it refuses, on both
    /// platforms, before judging.
    #[test]
    fn verify_reuse_judges_only_on_a_clean_checkout_of_r() {
        let scratch = TemporaryDirectory::new("verify-reuse-identity").unwrap();
        let root = scratch.path();
        git(root, &["init", "-q", "-b", "main"]);
        // Git on Windows may convert line endings, and the sample asks Git for a clean status.
        git(root, &["config", "core.autocrlf", "false"]);
        let commit = |path: &str, text: &str| {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
            git(root, &["add", "-A"]);
            git(root, &["commit", "-q", "-m", "c"]);
            git(root, &["rev-parse", "HEAD"])
        };
        commit("Cargo.toml", "[workspace]\n");
        let s = commit(
            "tools/stickymd-smoke/src/release/platform_inputs.rs",
            "the classifier",
        );
        let r = commit("docs/plan/a.md", "a document");
        git(root, &["checkout", "-q", &s]);
        for platform in [Platform::WindowsX64, Platform::MacOsArm64] {
            // HEAD is S, not R.
            let error = super::verify_reuse_command(root, platform, &s, &r).unwrap_err();
            assert!(error.contains(&format!("clean checkout of {r}")), "{error}");
        }
        git(root, &["checkout", "-q", &r]);
        std::fs::write(root.join("docs/plan/a.md"), "changed").unwrap();
        for platform in [Platform::WindowsX64, Platform::MacOsArm64] {
            let error = super::verify_reuse_command(root, platform, &s, &r).unwrap_err();
            assert!(error.contains("the work tree or index differs"), "{error}");
        }
        git(root, &["checkout", "-q", "--", "docs/plan/a.md"]);
        for platform in [Platform::WindowsX64, Platform::MacOsArm64] {
            let line = super::verify_reuse_command(root, platform, &s, &r).unwrap();
            assert!(line.contains("\"reuse\":\"ancestor:"), "{line}");
        }
        scratch.close().unwrap();
    }
}
