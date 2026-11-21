//! Parsing only; packaging rules live in the corresponding responsibility modules. The option
//! syntax is shared with `stickymd-smoke release`, which parses its verification commands with
//! [`tokenize`].
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use crate::platform::Platform;
use std::{collections::BTreeMap, path::PathBuf};

/// A command that produces release bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Command {
    Notices {
        destination: PathBuf,
        platform: Platform,
    },
    PackageInputs(PackageInputOptions),
    BuildPackage(PackageBuildOptions),
    GenerateSbom(SbomBuildOptions),
    PreparePackage {
        inputs: PackageInputOptions,
        exe: PathBuf,
        directory: PathBuf,
    },
    MacosPackage {
        app: PathBuf,
        output: PathBuf,
        allow_dirty: bool,
    },
    MacosSbom {
        zip: PathBuf,
    },
    /// The macOS product input fingerprint of the checkout (plan 11 #phase-16-macos-host).
    AppInputs {
        home: Option<PathBuf>,
    },
    Checksums(ChecksumOptions),
    PublishSbom(SbomOptions),
    PrepareSbom(SbomPreparationOptions),
    PublishPackage(PackagePublishOptions),
    SyftPlan(Option<PathBuf>),
    SyftPublish {
        kind: crate::syft::Kind,
        input: PathBuf,
    },
    SyftVerify {
        archive: PathBuf,
        checksums: PathBuf,
        directory: PathBuf,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChecksumOptions {
    pub zip: PathBuf,
    pub sbom: Option<PathBuf>,
    pub output: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SbomOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub zip: PathBuf,
    pub checksums: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SbomPreparationOptions {
    pub package_directory: PathBuf,
    pub zip: Option<PathBuf>,
    pub syft: Option<PathBuf>,
    pub directory: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PackagePublishOptions {
    pub input: PathBuf,
    pub output: PathBuf,
    pub checksums: PathBuf,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageInputOptions {
    pub version: Option<String>,
    pub commit: Option<String>,
    pub tag: Option<String>,
    pub exact: bool,
    pub allow_dirty: bool,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct PackageBuildOptions {
    pub inputs: PackageInputOptions,
    pub exe: Option<PathBuf>,
    pub directory: Option<PathBuf>,
    pub powershell: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SbomBuildOptions {
    pub directory: Option<PathBuf>,
    pub zip: Option<PathBuf>,
    pub output: Option<PathBuf>,
    pub syft: Option<PathBuf>,
    pub powershell: Option<PathBuf>,
}

/// The option tokens of one command line: `--key value` or `--key=value` pairs (each key once,
/// values non-empty) and the bare flags.
#[derive(Debug, Default)]
pub struct Tokens<'a> {
    pub options: BTreeMap<&'a str, &'a str>,
    pub exact: bool,
    pub allow_dirty: bool,
    pub runtime: bool,
    pub allow_missing: bool,
}

impl Tokens<'_> {
    /// The value of a required option.
    pub fn take(&mut self, key: &str) -> Result<String, String> {
        self.options
            .remove(key)
            .map(str::to_owned)
            .ok_or_else(|| format!("missing {key}"))
    }

    /// The value of an optional option.
    pub fn optional(&mut self, key: &str) -> Option<String> {
        self.options.remove(key).map(str::to_owned)
    }
}

pub fn tokenize(args: &[String]) -> Result<Tokens<'_>, String> {
    let mut tokens = Tokens::default();
    let mut remaining = args.iter();
    while let Some(key) = remaining.next() {
        let flag = match key.as_str() {
            "--exact-candidate" => Some(&mut tokens.exact),
            "--allow-dirty-validation" => Some(&mut tokens.allow_dirty),
            "--runtime" => Some(&mut tokens.runtime),
            "--allow-missing" => Some(&mut tokens.allow_missing),
            _ => None,
        };
        if let Some(flag) = flag {
            if *flag {
                return Err(format!("duplicate {key}"));
            }
            *flag = true;
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
        if !key.starts_with("--") || value.is_empty() || tokens.options.insert(key, value).is_some()
        {
            return Err(format!("invalid or duplicate release option {key}"));
        }
    }
    Ok(tokens)
}

pub fn parse(args: &[String]) -> Result<Command, String> {
    let Some((command, args)) = args.split_first() else {
        return Err("expected a packaging command".to_owned());
    };
    let mut tokens = tokenize(args)?;
    let (exact, allow_dirty) = (tokens.exact, tokens.allow_dirty);
    if tokens.runtime {
        return Err("--runtime belongs to stickymd-smoke release verify-package".to_owned());
    }
    if tokens.allow_missing {
        return Err("--allow-missing belongs to stickymd-smoke release verify-remote-state".into());
    }
    let parsed = match command.as_str() {
        "notices" => Command::Notices {
            destination: PathBuf::from(tokens.take("--destination")?),
            // Windows remains the default, so the existing packaging scripts are unchanged.
            platform: tokens
                .optional("--platform")
                .map_or(Ok(Platform::WindowsX64), |value| Platform::parse(&value))?,
        },
        "macos-package" => Command::MacosPackage {
            app: PathBuf::from(tokens.take("--app")?),
            output: PathBuf::from(tokens.take("--output-directory")?),
            allow_dirty,
        },
        "macos-sbom" => Command::MacosSbom {
            zip: PathBuf::from(tokens.take("--zip")?),
        },
        "app-inputs" => Command::AppInputs {
            home: tokens.optional("--home").map(PathBuf::from),
        },
        "syft-plan" => Command::SyftPlan(tokens.optional("--syft-path").map(PathBuf::from)),
        "syft-publish" => Command::SyftPublish {
            kind: crate::syft::Kind::parse(&tokens.take("--kind")?)?,
            input: PathBuf::from(tokens.take("--input")?),
        },
        "syft-verify" => Command::SyftVerify {
            archive: PathBuf::from(tokens.take("--archive")?),
            checksums: PathBuf::from(tokens.take("--checksums")?),
            directory: PathBuf::from(tokens.take("--staging-directory")?),
        },
        "checksums" => Command::Checksums(ChecksumOptions {
            zip: PathBuf::from(tokens.take("--zip")?),
            output: PathBuf::from(tokens.take("--output")?),
            sbom: tokens.optional("--sbom").map(PathBuf::from),
        }),
        "publish-sbom" => Command::PublishSbom(SbomOptions {
            input: PathBuf::from(tokens.take("--input")?),
            output: PathBuf::from(tokens.take("--output")?),
            zip: PathBuf::from(tokens.take("--zip")?),
            checksums: PathBuf::from(tokens.take("--checksums")?),
        }),
        "prepare-sbom" => Command::PrepareSbom(SbomPreparationOptions {
            package_directory: PathBuf::from(tokens.take("--package-directory")?),
            directory: PathBuf::from(tokens.take("--staging-directory")?),
            zip: tokens.optional("--zip").map(PathBuf::from),
            syft: tokens.optional("--syft-path").map(PathBuf::from),
        }),
        "generate-sbom" => Command::GenerateSbom(SbomBuildOptions {
            directory: tokens.optional("--package-directory").map(PathBuf::from),
            zip: tokens.optional("--zip").map(PathBuf::from),
            output: tokens.optional("--output").map(PathBuf::from),
            syft: tokens.optional("--syft-path").map(PathBuf::from),
            powershell: tokens.optional("--powershell").map(PathBuf::from),
        }),
        "publish-package" => Command::PublishPackage(PackagePublishOptions {
            input: PathBuf::from(tokens.take("--input")?),
            output: PathBuf::from(tokens.take("--output")?),
            checksums: PathBuf::from(tokens.take("--checksums")?),
        }),
        "package-inputs" | "prepare-package" | "build-package" => {
            let inputs = PackageInputOptions {
                version: tokens.optional("--version"),
                commit: tokens.optional("--commit-sha"),
                tag: tokens.optional("--release-tag"),
                exact,
                allow_dirty,
            };
            if command == "build-package" {
                Command::BuildPackage(PackageBuildOptions {
                    inputs,
                    exe: tokens.optional("--exe").map(PathBuf::from),
                    directory: tokens.optional("--output-directory").map(PathBuf::from),
                    powershell: tokens.optional("--powershell").map(PathBuf::from),
                })
            } else if command == "prepare-package" {
                Command::PreparePackage {
                    inputs,
                    exe: tokens
                        .optional("--exe")
                        .map(PathBuf::from)
                        .ok_or("missing --exe")?,
                    directory: tokens
                        .optional("--staging-directory")
                        .map(PathBuf::from)
                        .ok_or("missing --staging-directory")?,
                }
            } else {
                Command::PackageInputs(inputs)
            }
        }
        _ => return Err(format!("unknown packaging command {command}")),
    };
    if !tokens.options.is_empty() {
        return Err(format!(
            "unknown release options: {:?}",
            tokens.options.keys()
        ));
    }
    if (exact || allow_dirty)
        && !matches!(
            parsed,
            Command::PackageInputs(_) | Command::PreparePackage { .. } | Command::BuildPackage(_)
        )
        && !(allow_dirty && !exact && matches!(parsed, Command::MacosPackage { .. }))
    {
        return Err("package input flags require package-inputs".to_owned());
    }
    Ok(parsed)
}

pub const USAGE: &str = "usage: stickymd-package <build-package|generate-sbom|package-inputs|prepare-package|publish-package|notices|checksums|prepare-sbom|publish-sbom|syft-plan|syft-publish|syft-verify|macos-package|macos-sbom|app-inputs> [options]; see tools/stickymd-package/README.md";
