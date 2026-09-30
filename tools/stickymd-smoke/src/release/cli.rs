//! Parsing only; release rules live in the corresponding responsibility modules.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    VerifyPromoted(PromotionOptions),
    VerifyPackage(PackageOptions),
    VerifyWorkflow {
        source: String,
        input: PathBuf,
    },
    VerifyRemoteState(RemoteStateOptions),
    Notices(PathBuf),
    PackageInputs(PackageInputOptions),
    BuildPackage(PackageBuildOptions),
    GenerateSbom(SbomBuildOptions),
    PreparePackage {
        inputs: PackageInputOptions,
        exe: PathBuf,
        directory: PathBuf,
    },
    WorkspaceVersion,
    Checksums(ChecksumOptions),
    PublishSbom(SbomOptions),
    PrepareSbom(SbomPreparationOptions),
    PublishPackage(PackagePublishOptions),
    SyftPlan(Option<PathBuf>),
    SyftPublish {
        kind: super::syft::Kind,
        input: PathBuf,
    },
    SyftVerify {
        archive: PathBuf,
        checksums: PathBuf,
        directory: PathBuf,
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
pub(crate) struct ChecksumOptions {
    pub zip: PathBuf,
    pub sbom: Option<PathBuf>,
    pub output: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SbomOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub zip: PathBuf,
    pub checksums: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct SbomPreparationOptions {
    pub package_directory: PathBuf,
    pub zip: Option<PathBuf>,
    pub syft: Option<PathBuf>,
    pub directory: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PackagePublishOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub checksums: PathBuf,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PackageInputOptions {
    pub version: Option<String>,
    pub commit: Option<String>,
    pub tag: Option<String>,
    pub exact: bool,
    pub allow_dirty: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PackageBuildOptions {
    pub inputs: PackageInputOptions,
    pub exe: Option<PathBuf>,
    pub directory: Option<PathBuf>,
    pub powershell: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct SbomBuildOptions {
    pub directory: Option<PathBuf>,
    pub zip: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub syft: Option<PathBuf>,
    pub powershell: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PromotionOptions {
    pub directory: PathBuf,
    pub source: String,
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
}

pub(crate) fn parse(args: &[String]) -> Result<Command, String> {
    let Some((command, args)) = args.split_first() else {
        return Err("expected release subcommand".to_owned());
    };
    let mut options = BTreeMap::new();
    let mut runtime = false;
    let mut exact = false;
    let mut allow_dirty = false;
    let mut allow_missing = false;
    let mut remaining = args.iter();
    while let Some(key) = remaining.next() {
        if matches!(
            key.as_str(),
            "--exact-candidate" | "--allow-dirty-validation"
        ) {
            let flag = if key == "--exact-candidate" {
                &mut exact
            } else {
                &mut allow_dirty
            };
            if *flag {
                return Err(format!("duplicate {key}"));
            }
            *flag = true;
            continue;
        }
        if key == "--runtime" {
            if runtime {
                return Err("duplicate --runtime".to_owned());
            }
            runtime = true;
            continue;
        }
        if key == "--allow-missing" {
            if allow_missing {
                return Err("duplicate --allow-missing".into());
            }
            allow_missing = true;
            continue;
        }
        let (key, value) = if let Some(pair) = key.split_once('=') {
            pair
        } else {
            (
                key.as_str(),
                remaining
                    .next()
                    .ok_or_else(|| format!("missing value for {key}"))?
                    .as_str(),
            )
        };
        if !key.starts_with("--") || value.is_empty() || options.insert(key, value).is_some() {
            return Err(format!("invalid or duplicate release option {key}"));
        }
    }
    let mut take = |key| {
        options
            .remove(key)
            .map(str::to_owned)
            .ok_or_else(|| format!("missing {key}"))
    };
    let command = match command.as_str() {
        "verify-promoted" => Command::VerifyPromoted(PromotionOptions {
            directory: PathBuf::from(take("--artifact-directory")?),
            source: take("--source-sha")?,
            zip_hash: take("--expected-zip-sha256")?,
            sbom_hash: take("--expected-sbom-sha256")?,
            tag: take("--release-tag")?,
        }),
        "verify-package" => Command::VerifyPackage(PackageOptions {
            directory: options.remove("--package-directory").map(PathBuf::from),
            zip: options.remove("--zip").map(PathBuf::from),
            checksums: options.remove("--checksums").map(PathBuf::from),
            runtime,
        }),
        "verify-workflow" => Command::VerifyWorkflow {
            source: take("--source-sha")?,
            input: PathBuf::from(take("--workflow-json")?),
        },
        "verify-remote-state" => Command::VerifyRemoteState(RemoteStateOptions {
            kind: super::remote_state::Kind::parse(&take("--kind")?)?,
            source: take("--source-sha")?,
            tag: take("--release-tag")?,
            input: PathBuf::from(take("--http-response")?),
            query_exit: take("--query-exit")?
                .parse()
                .map_err(|_| "invalid query exit code")?,
            allow_missing,
        }),
        "notices" => Command::Notices(PathBuf::from(take("--destination")?)),
        "workspace-version" => Command::WorkspaceVersion,
        "syft-plan" => Command::SyftPlan(options.remove("--syft-path").map(PathBuf::from)),
        "syft-publish" => Command::SyftPublish {
            kind: super::syft::Kind::parse(&take("--kind")?)?,
            input: PathBuf::from(take("--input")?),
        },
        "syft-verify" => Command::SyftVerify {
            archive: PathBuf::from(take("--archive")?),
            checksums: PathBuf::from(take("--checksums")?),
            directory: PathBuf::from(take("--staging-directory")?),
        },
        "checksums" => Command::Checksums(ChecksumOptions {
            zip: PathBuf::from(take("--zip")?),
            output: PathBuf::from(take("--output")?),
            sbom: options.remove("--sbom").map(PathBuf::from),
        }),
        "publish-sbom" => Command::PublishSbom(SbomOptions {
            input: PathBuf::from(take("--input")?),
            output: PathBuf::from(take("--output")?),
            zip: PathBuf::from(take("--zip")?),
            checksums: PathBuf::from(take("--checksums")?),
        }),
        "prepare-sbom" => Command::PrepareSbom(SbomPreparationOptions {
            package_directory: PathBuf::from(take("--package-directory")?),
            directory: PathBuf::from(take("--staging-directory")?),
            zip: options.remove("--zip").map(PathBuf::from),
            syft: options.remove("--syft-path").map(PathBuf::from),
        }),
        "generate-sbom" => Command::GenerateSbom(SbomBuildOptions {
            directory: options.remove("--package-directory").map(PathBuf::from),
            zip: options.remove("--zip").map(PathBuf::from),
            output: options.remove("--output").map(PathBuf::from),
            syft: options.remove("--syft-path").map(PathBuf::from),
            powershell: options.remove("--powershell").map(PathBuf::from),
        }),
        "publish-package" => Command::PublishPackage(PackagePublishOptions {
            input: PathBuf::from(take("--input")?),
            output: PathBuf::from(take("--output")?),
            checksums: PathBuf::from(take("--checksums")?),
        }),
        "package-inputs" | "prepare-package" | "build-package" => {
            let inputs = PackageInputOptions {
                version: options.remove("--version").map(str::to_owned),
                commit: options.remove("--commit-sha").map(str::to_owned),
                tag: options.remove("--release-tag").map(str::to_owned),
                exact,
                allow_dirty,
            };
            if command == "build-package" {
                Command::BuildPackage(PackageBuildOptions {
                    inputs,
                    exe: options.remove("--exe").map(PathBuf::from),
                    directory: options.remove("--output-directory").map(PathBuf::from),
                    powershell: options.remove("--powershell").map(PathBuf::from),
                })
            } else if command == "prepare-package" {
                Command::PreparePackage {
                    inputs,
                    exe: options
                        .remove("--exe")
                        .map(PathBuf::from)
                        .ok_or("missing --exe")?,
                    directory: options
                        .remove("--staging-directory")
                        .map(PathBuf::from)
                        .ok_or("missing --staging-directory")?,
                }
            } else {
                Command::PackageInputs(inputs)
            }
        }
        _ => return Err(format!("unknown release subcommand {command}")),
    };
    if !options.is_empty() {
        return Err(format!("unknown release options: {:?}", options.keys()));
    }
    if runtime && !matches!(command, Command::VerifyPackage(_)) {
        return Err("--runtime requires verify-package".to_owned());
    }
    if allow_missing && !matches!(command, Command::VerifyRemoteState(_)) {
        return Err("--allow-missing requires verify-remote-state".into());
    }
    if (exact || allow_dirty)
        && !matches!(
            command,
            Command::PackageInputs(_) | Command::PreparePackage { .. } | Command::BuildPackage(_)
        )
    {
        return Err("package input flags require package-inputs".to_owned());
    }
    Ok(command)
}
