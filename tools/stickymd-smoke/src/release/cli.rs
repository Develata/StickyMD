//! Parsing only; release rules live in the corresponding responsibility modules.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::{collections::BTreeMap, path::PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum Command {
    VerifyPromoted(PromotionOptions),
    VerifyPackage(PackageOptions),
    Notices(PathBuf),
    PackageInputs(PackageInputOptions),
    WorkspaceVersion,
    Checksums(ChecksumOptions),
    PublishSbom(SbomOptions),
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct PackageInputOptions {
    pub version: Option<String>,
    pub commit: Option<String>,
    pub tag: Option<String>,
    pub exact: bool,
    pub allow_dirty: bool,
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
        "notices" => Command::Notices(PathBuf::from(take("--destination")?)),
        "workspace-version" => Command::WorkspaceVersion,
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
        "package-inputs" => Command::PackageInputs(PackageInputOptions {
            version: options.remove("--version").map(str::to_owned),
            commit: options.remove("--commit-sha").map(str::to_owned),
            tag: options.remove("--release-tag").map(str::to_owned),
            exact,
            allow_dirty,
        }),
        _ => return Err(format!("unknown release subcommand {command}")),
    };
    if !options.is_empty() {
        return Err(format!("unknown release options: {:?}", options.keys()));
    }
    if runtime && !matches!(command, Command::VerifyPackage(_)) {
        return Err("--runtime requires verify-package".to_owned());
    }
    if (exact || allow_dirty) && !matches!(command, Command::PackageInputs(_)) {
        return Err("package input flags require package-inputs".to_owned());
    }
    Ok(command)
}
