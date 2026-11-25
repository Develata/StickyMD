//! Options of `phase <id>` and `all`: the selection, CI shard, resource module and flags.

use super::Phase;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Selection {
    Phase(Phase),
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CiShard {
    Tests,
    Performance,
}

impl CiShard {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "tests" => Ok(Self::Tests),
            "performance" => Ok(Self::Performance),
            _ => Err(format!(
                "unknown CI shard `{value}`; expected tests or performance"
            )),
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Tests => "tests",
            Self::Performance => "performance",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ResourceModule {
    SourcePreview,
    Math,
    Images,
    Window,
    Zoom,
}

impl ResourceModule {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "source-preview" => Ok(Self::SourcePreview),
            "math" => Ok(Self::Math),
            "images" => Ok(Self::Images),
            "window" => Ok(Self::Window),
            "zoom" => Ok(Self::Zoom),
            _ => Err(format!(
                "unknown resource module `{value}`; expected source-preview, math, images, window, or zoom"
            )),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    pub(crate) selection: Selection,
    pub(crate) ci: bool,
    pub(crate) ci_shard: Option<CiShard>,
    pub(crate) performance: bool,
    pub(crate) runtime: bool,
    pub(crate) resources: bool,
    pub(crate) resource_module: Option<ResourceModule>,
    pub(crate) resource_resume: bool,
    pub(crate) resource_plan: bool,
    pub(crate) resource_failure_first: bool,
    pub(crate) release: bool,
    pub(crate) package: bool,
    pub(crate) json: bool,
    pub(crate) evidence_file: Option<PathBuf>,
}

impl Options {
    pub(crate) fn parse<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut args = args.into_iter();
        let selection = match args.next().as_deref() {
            Some("phase") => {
                let phase = args
                    .next()
                    .ok_or_else(|| "`phase` requires a number (00..14 or 11-b)".to_owned())?;
                Selection::Phase(Phase::parse(&phase)?)
            }
            Some("all") => Selection::All,
            Some("help" | "--help" | "-h") => return Err(Self::usage().to_owned()),
            Some(other) => return Err(format!("unknown command `{other}`\n{}", Self::usage())),
            None => return Err(Self::usage().to_owned()),
        };

        let mut options = Self {
            selection,
            ci: false,
            ci_shard: None,
            performance: false,
            runtime: false,
            resources: false,
            resource_module: None,
            resource_resume: false,
            resource_plan: false,
            resource_failure_first: false,
            release: false,
            package: false,
            json: false,
            evidence_file: None,
        };
        for argument in args {
            match argument.as_str() {
                "--ci" => options.ci = true,
                value if value.starts_with("--ci-shard=") => {
                    options.ci_shard = Some(CiShard::parse(
                        value
                            .strip_prefix("--ci-shard=")
                            .filter(|value| !value.is_empty())
                            .ok_or_else(|| "`--ci-shard` requires a value".to_owned())?,
                    )?);
                }
                "--performance" => options.performance = true,
                "--runtime" => options.runtime = true,
                "--resources" => options.resources = true,
                "--resource-resume" => options.resource_resume = true,
                "--resource-plan" => options.resource_plan = true,
                "--resource-failure-first" => options.resource_failure_first = true,
                value if value.starts_with("--resource-module=") => {
                    options.resource_module = Some(ResourceModule::parse(
                        value
                            .strip_prefix("--resource-module=")
                            .filter(|value| !value.is_empty())
                            .ok_or_else(|| "`--resource-module` requires a value".to_owned())?,
                    )?);
                }
                "--release" => options.release = true,
                "--package" => options.package = true,
                "--json" => options.json = true,
                value if value.starts_with("--evidence-file=") => {
                    let path = value
                        .strip_prefix("--evidence-file=")
                        .filter(|path| !path.is_empty())
                        .ok_or_else(|| "`--evidence-file` requires a path".to_owned())?;
                    options.evidence_file = Some(PathBuf::from(path));
                    options.json = true;
                }
                _ => return Err(format!("unknown option `{argument}`\n{}", Self::usage())),
            }
        }
        if options.ci
            && (options.performance
                || options.runtime
                || options.resources
                || options.release
                || options.package)
        {
            return Err("`--ci` cannot be combined with explicit local performance, runtime, resource, release, or package modes".to_owned());
        }
        if options.ci_shard.is_some()
            && (!options.ci || !matches!(options.selection, Selection::All))
        {
            return Err("`--ci-shard` requires `all --ci`".to_owned());
        }
        if options.resource_module.is_some()
            && (!options.resources
                || !matches!(
                    options.selection,
                    Selection::All
                        | Selection::Phase(
                            Phase::P10
                                | Phase::P11
                                | Phase::P11B
                                | Phase::P12
                                | Phase::P13
                                | Phase::P14
                        )
                ))
        {
            return Err(
                "`--resource-module` requires `--resources` with all or Phase 10 through 14"
                    .to_owned(),
            );
        }
        if options.resource_plan && !options.resource_resume {
            return Err("`--resource-plan` requires `--resource-resume`".into());
        }
        if options.resource_failure_first && !options.resource_resume {
            return Err("`--resource-failure-first` requires `--resource-resume`".into());
        }
        if options.resource_resume
            && (!options.resources
                || options.selection != Selection::Phase(Phase::P14)
                || options.evidence_file.is_none())
        {
            return Err("`--resource-resume` requires Phase 14 `--resources` and a diagnostic `--evidence-file`".into());
        }
        if options.resources && (options.performance || options.runtime) {
            return Err("`--resources` must run alone so the measured process is not contaminated by other smoke tasks".to_owned());
        }
        if (options.release || options.package)
            && (options.performance
                || options.runtime
                || options.resources
                || (options.release && options.package))
        {
            return Err("release and package modes must run separately from every other explicit mode so artifacts are attributable".to_owned());
        }
        if options.runtime
            && matches!(
                selection,
                Selection::Phase(
                    Phase::P00 | Phase::P01 | Phase::P02 | Phase::P15 | Phase::P16 | Phase::P18
                )
            )
        {
            return Err(
                "runtime smoke is defined only for Phase 03 through Phase 09, or `all`".to_owned(),
            );
        }
        if options.resources
            && !matches!(
                selection,
                Selection::Phase(
                    Phase::P05
                        | Phase::P06
                        | Phase::P07
                        | Phase::P08
                        | Phase::P09
                        | Phase::P10
                        | Phase::P11
                        | Phase::P11B
                        | Phase::P12
                        | Phase::P13
                        | Phase::P14,
                ) | Selection::All
            )
        {
            return Err(
                "resource measurement is defined only for Phase 05 through Phase 14, or `all`"
                    .to_owned(),
            );
        }
        if (options.release || options.package)
            && !matches!(
                selection,
                Selection::Phase(
                    Phase::P09
                        | Phase::P10
                        | Phase::P11
                        | Phase::P11B
                        | Phase::P12
                        | Phase::P13
                        | Phase::P14,
                ) | Selection::All
            )
        {
            return Err(
                "release and package modes are defined only for Phase 09 through Phase 14 or `all`"
                    .to_owned(),
            );
        }
        Ok(options)
    }

    pub(crate) const fn usage() -> &'static str {
        "usage: stickymd-smoke phase <00..18|11-b> [--performance|--runtime|--resources [--resource-module=<source-preview|math|images|window|zoom>] [--resource-resume [--resource-plan] [--resource-failure-first]]|--release|--package] [--json] [--evidence-file=<path>]\n       stickymd-smoke all [--ci [--ci-shard=<tests|performance>]|--performance|--runtime|--resources [--resource-module=<source-preview|math|images|window|zoom>]|--release|--package] [--json] [--evidence-file=<path>]\n       stickymd-smoke modules list | modules run <module[,module...]|all> [--mode=tests|performance|all] [--plan]\n       stickymd-smoke package-path --directory <directory>\n       stickymd-smoke dev-check [--plan] [--mode=tests|performance|all]\n       stickymd-smoke timings --input <json-or-log> [--input <path> ...] [--json]\n       stickymd-smoke startup-details --trace <v2-path> --details <sidecar-path> [--json]\n       stickymd-smoke phase-entry[-plan] <00..16|18|11-b|all> [--Parameter=value]\n       stickymd-smoke release <workspace-version|verify-promoted|verify-package|verify-workflow|verify-attachments|verify-release-set|verify-remote-state|platform-inputs|verify-reuse|macos-verify-package|macos-verify-artifact|macos-promote|macos-candidate|macos-source-tests|macos-ready> [options]; see tools/stickymd-smoke/README.md
       (building release bytes: stickymd-package; see tools/stickymd-package/README.md)"
    }
}
