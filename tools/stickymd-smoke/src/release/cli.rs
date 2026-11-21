//! Parsing only; release rules live in the corresponding responsibility modules.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    VerifyPromoted(PromotionOptions),
    VerifyPackage(PackageOptions),
    VerifyWorkflow {
        source: String,
        input: PathBuf,
        workflow: String,
        attempt: Option<u64>,
        /// The workflow file and trigger the run must report, when given.
        origin: Option<(String, String)>,
    },
    VerifyAttachments {
        directory: PathBuf,
        attachments: String,
    },
    VerifyReleaseSet {
        release: String,
        tag: String,
        attachments: String,
        windows: PathBuf,
        macos: PathBuf,
    },
    VerifyRemoteState(RemoteStateOptions),
    WorkspaceVersion,
    MacosPromote {
        run_id: u64,
        attempt: u64,
        release: String,
        zip: PathBuf,
    },
    MacosCandidate {
        release: String,
    },
    MacosVerifyPackage {
        zip: PathBuf,
        source: Option<String>,
    },
    MacosVerifyArtifact {
        directory: PathBuf,
        source: String,
    },
    PlatformInputs {
        platform: super::platform::Platform,
        commit: String,
    },
    VerifyReuse {
        platform: super::platform::Platform,
        build: String,
        release: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RemoteStateOptions {
    pub kind: super::remote_state::Kind,
    pub source: String,
    pub tag: String,
    pub input: PathBuf,
    pub query_exit: i32,
    pub allow_missing: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PromotionOptions {
    pub directory: PathBuf,
    pub source: String,
    /// The commit the package was built from, when it is an ancestor of `source` (plan 11
    /// reuse rule); the package README names it.
    pub build: Option<String>,
    pub zip_hash: String,
    pub sbom_hash: String,
    pub tag: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PackageOptions {
    pub directory: Option<PathBuf>,
    pub zip: Option<PathBuf>,
    pub checksums: Option<PathBuf>,
    pub runtime: bool,
    /// The commit the package was built from when it is not HEAD; it must be reusable on HEAD
    /// (plan 11 #platform-artifact-reuse).
    pub build: Option<String>,
}

/// The packaging commands now run as `stickymd-package <command>` (plan 11
/// #platform-artifact-reuse); naming one here says where it went.
const MOVED: &[&str] = &[
    "build-package",
    "generate-sbom",
    "package-inputs",
    "prepare-package",
    "publish-package",
    "notices",
    "checksums",
    "prepare-sbom",
    "publish-sbom",
    "syft-plan",
    "syft-publish",
    "syft-verify",
    "macos-package",
    "macos-sbom",
];

pub(crate) fn parse(args: &[String]) -> Result<Command, String> {
    let Some((command, args)) = args.split_first() else {
        return Err("expected release subcommand".to_owned());
    };
    if MOVED.contains(&command.as_str()) {
        return Err(format!(
            "release {command} builds release bytes and moved to the packaging crate: run `cargo run -p stickymd-package -- {command}`"
        ));
    }
    let mut tokens = stickymd_package::cli::tokenize(args)?;
    let (runtime, allow_missing) = (tokens.runtime, tokens.allow_missing);
    if tokens.exact || tokens.allow_dirty {
        return Err("package input flags require stickymd-package package-inputs".to_owned());
    }
    let command = match command.as_str() {
        "verify-promoted" => Command::VerifyPromoted(PromotionOptions {
            directory: PathBuf::from(tokens.take("--artifact-directory")?),
            source: tokens.take("--source-sha")?,
            zip_hash: tokens.take("--expected-zip-sha256")?,
            sbom_hash: tokens.take("--expected-sbom-sha256")?,
            tag: tokens.take("--release-tag")?,
            build: tokens.optional("--build-sha"),
        }),
        "verify-package" => Command::VerifyPackage(PackageOptions {
            directory: tokens.optional("--package-directory").map(PathBuf::from),
            zip: tokens.optional("--zip").map(PathBuf::from),
            checksums: tokens.optional("--checksums").map(PathBuf::from),
            runtime,
            build: tokens.optional("--build-sha"),
        }),
        "verify-workflow" => Command::VerifyWorkflow {
            source: tokens.take("--source-sha")?,
            input: PathBuf::from(tokens.take("--workflow-json")?),
            workflow: tokens
                .optional("--workflow")
                .unwrap_or_else(|| "release".to_owned()),
            attempt: tokens
                .optional("--attempt")
                .map(|value| value.parse().map_err(|_| "invalid --attempt"))
                .transpose()?,
            origin: match (
                tokens.optional("--workflow-path"),
                tokens.optional("--event"),
            ) {
                (Some(path), Some(event)) => Some((path, event)),
                (None, None) => None,
                _ => return Err("--workflow-path and --event go together".to_owned()),
            },
        },
        "verify-attachments" => Command::VerifyAttachments {
            directory: PathBuf::from(tokens.take("--directory")?),
            attachments: tokens.take("--attachments")?,
        },
        "verify-release-set" => Command::VerifyReleaseSet {
            release: tokens.take("--release-source")?,
            tag: tokens.take("--release-tag")?,
            attachments: tokens.take("--attachments")?,
            windows: PathBuf::from(tokens.take("--windows-directory")?),
            macos: PathBuf::from(tokens.take("--macos-directory")?),
        },
        "verify-remote-state" => Command::VerifyRemoteState(RemoteStateOptions {
            kind: super::remote_state::Kind::parse(&tokens.take("--kind")?)?,
            source: tokens.take("--source-sha")?,
            tag: tokens.take("--release-tag")?,
            input: PathBuf::from(tokens.take("--http-response")?),
            query_exit: tokens
                .take("--query-exit")?
                .parse()
                .map_err(|_| "invalid query exit code")?,
            allow_missing,
        }),
        "workspace-version" => Command::WorkspaceVersion,
        "macos-promote" => Command::MacosPromote {
            run_id: tokens
                .take("--run-id")?
                .parse()
                .map_err(|_| "invalid --run-id")?,
            attempt: tokens
                .take("--attempt")?
                .parse()
                .map_err(|_| "invalid --attempt")?,
            release: tokens.take("--release-source")?,
            zip: PathBuf::from(tokens.take("--zip")?),
        },
        "macos-candidate" => Command::MacosCandidate {
            release: tokens.take("--release-source")?,
        },
        "macos-verify-package" => Command::MacosVerifyPackage {
            zip: PathBuf::from(tokens.take("--zip")?),
            source: tokens.optional("--source-sha"),
        },
        "macos-verify-artifact" => Command::MacosVerifyArtifact {
            directory: PathBuf::from(tokens.take("--directory")?),
            source: tokens.take("--source-sha")?,
        },
        "platform-inputs" => Command::PlatformInputs {
            platform: super::platform::Platform::parse(&tokens.take("--platform")?)?,
            commit: tokens.take("--commit")?,
        },
        "verify-reuse" => Command::VerifyReuse {
            platform: super::platform::Platform::parse(&tokens.take("--platform")?)?,
            build: tokens.take("--build-source")?,
            release: tokens.take("--release-source")?,
        },
        _ => return Err(format!("unknown release subcommand {command}")),
    };
    if !tokens.options.is_empty() {
        return Err(format!(
            "unknown release options: {:?}",
            tokens.options.keys()
        ));
    }
    if runtime && !matches!(command, Command::VerifyPackage(_)) {
        return Err("--runtime requires verify-package".to_owned());
    }
    if allow_missing && !matches!(command, Command::VerifyRemoteState(_)) {
        return Err("--allow-missing requires verify-remote-state".into());
    }
    Ok(command)
}
