//! Command-line contract for selecting phase verification tasks.

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum Phase {
    P00,
    P01,
    P02,
    P03,
    P04,
    P05,
    P06,
    P07,
    P08,
    P09,
    P10,
    P11,
    P11B,
}

impl Phase {
    pub(crate) const ALL: [Self; 13] = [
        Self::P00,
        Self::P01,
        Self::P02,
        Self::P03,
        Self::P04,
        Self::P05,
        Self::P06,
        Self::P07,
        Self::P08,
        Self::P09,
        Self::P10,
        Self::P11,
        Self::P11B,
    ];

    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "0" | "00" | "phase-00" => Ok(Self::P00),
            "1" | "01" | "phase-01" => Ok(Self::P01),
            "2" | "02" | "phase-02" => Ok(Self::P02),
            "3" | "03" | "phase-03" => Ok(Self::P03),
            "4" | "04" | "phase-04" => Ok(Self::P04),
            "5" | "05" | "phase-05" => Ok(Self::P05),
            "6" | "06" | "phase-06" => Ok(Self::P06),
            "7" | "07" | "phase-07" => Ok(Self::P07),
            "8" | "08" | "phase-08" => Ok(Self::P08),
            "9" | "09" | "phase-09" => Ok(Self::P09),
            "10" | "phase-10" => Ok(Self::P10),
            "11" | "phase-11" => Ok(Self::P11),
            "11-b" | "phase-11-b" => Ok(Self::P11B),
            _ => Err(format!("unknown phase `{value}`; expected 00..11-b")),
        }
    }

    pub(crate) const fn number(self) -> &'static str {
        match self {
            Self::P00 => "00",
            Self::P01 => "01",
            Self::P02 => "02",
            Self::P03 => "03",
            Self::P04 => "04",
            Self::P05 => "05",
            Self::P06 => "06",
            Self::P07 => "07",
            Self::P08 => "08",
            Self::P09 => "09",
            Self::P10 => "10",
            Self::P11 => "11",
            Self::P11B => "11-b",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Selection {
    Phase(Phase),
    All,
}

use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    pub(crate) selection: Selection,
    pub(crate) ci: bool,
    pub(crate) performance: bool,
    pub(crate) runtime: bool,
    pub(crate) resources: bool,
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
                    .ok_or_else(|| "`phase` requires a number (00..11-b)".to_owned())?;
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
            performance: false,
            runtime: false,
            resources: false,
            release: false,
            package: false,
            json: false,
            evidence_file: None,
        };
        for argument in args {
            match argument.as_str() {
                "--ci" => options.ci = true,
                "--performance" => options.performance = true,
                "--runtime" => options.runtime = true,
                "--resources" => options.resources = true,
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
                Selection::Phase(Phase::P00 | Phase::P01 | Phase::P02)
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
                        | Phase::P11B,
                ) | Selection::All
            )
        {
            return Err(
                "resource measurement is defined only for Phase 05 through Phase 11-B, or `all`"
                    .to_owned(),
            );
        }
        if (options.release || options.package)
            && !matches!(
                selection,
                Selection::Phase(Phase::P09 | Phase::P10 | Phase::P11 | Phase::P11B)
                    | Selection::All
            )
        {
            return Err(
                "release and package modes are defined only for Phase 09/10/11/11-B or `all`"
                    .to_owned(),
            );
        }
        Ok(options)
    }

    pub(crate) const fn usage() -> &'static str {
        "usage: stickymd-smoke phase <00..11-b> [--performance|--runtime|--resources|--release|--package] [--json] [--evidence-file=<path>]\n       stickymd-smoke all [--ci|--performance|--runtime|--resources|--release|--package] [--json] [--evidence-file=<path>]"
    }
}

#[cfg(test)]
mod tests {
    use super::{Options, Phase, Selection};

    fn args<'a>(values: &'a [&'a str]) -> impl Iterator<Item = String> + 'a {
        values.iter().map(|value| (*value).to_owned())
    }

    #[test]
    fn parses_phase_and_explicit_modes() {
        let options = Options::parse(args(&["phase", "03", "--performance", "--runtime"]))
            .expect("valid CLI");
        assert_eq!(options.selection, Selection::Phase(Phase::P03));
        assert!(options.performance);
        assert!(options.runtime);
        assert!(!options.resources);
        assert!(!options.release);
        assert!(!options.package);
        assert!(!options.json);
    }

    #[test]
    fn phase8_accepts_explicit_runtime_and_resource_modes() {
        let runtime =
            Options::parse(args(&["phase", "08", "--runtime"])).expect("Phase 8 runtime mode");
        assert_eq!(runtime.selection, Selection::Phase(Phase::P08));
        assert!(runtime.runtime);

        let resources = Options::parse(args(&["phase", "phase-08", "--resources"]))
            .expect("Phase 8 resource mode");
        assert_eq!(resources.selection, Selection::Phase(Phase::P08));
        assert!(resources.resources);
    }

    #[test]
    fn phase9_accepts_release_and_package_as_separate_modes() {
        let release =
            Options::parse(args(&["phase", "09", "--release"])).expect("Phase 9 release mode");
        assert_eq!(release.selection, Selection::Phase(Phase::P09));
        assert!(release.release);

        let package = Options::parse(args(&["phase", "phase-09", "--package"]))
            .expect("Phase 9 package mode");
        assert!(package.package);

        let error = Options::parse(args(&["phase", "09", "--release", "--package"]))
            .expect_err("artifact-producing modes must not overlap");
        assert!(error.contains("must run separately"));
    }

    #[test]
    fn rejects_environment_sensitive_ci_modes() {
        let error = Options::parse(args(&["all", "--ci", "--runtime"]))
            .expect_err("CI runtime smoke must be rejected");
        assert!(error.contains("cannot be combined"));
    }

    #[test]
    fn rejects_runtime_for_non_runtime_phase() {
        let error = Options::parse(args(&["phase", "02", "--runtime"]))
            .expect_err("Phase 02 has no runtime smoke");
        assert!(error.contains("Phase 03"));
    }

    #[test]
    fn phase10_and_json_are_supported() {
        let options =
            Options::parse(args(&["phase", "10", "--json"])).expect("Phase 10 JSON evidence mode");
        assert_eq!(options.selection, Selection::Phase(Phase::P10));
        assert!(options.json);
    }

    #[test]
    fn phase11_supports_every_local_convergence_mode() {
        for mode in [
            "--performance",
            "--runtime",
            "--resources",
            "--release",
            "--package",
        ] {
            let options = Options::parse(args(&["phase", "11", mode, "--json"]))
                .expect("valid Phase 11 mode");
            assert_eq!(options.selection, Selection::Phase(Phase::P11));
            assert!(options.json);
        }
    }

    #[test]
    fn phase11b_supports_every_local_amendment_mode() {
        for mode in [
            "--performance",
            "--runtime",
            "--resources",
            "--release",
            "--package",
        ] {
            let options = Options::parse(args(&["phase", "11-b", mode, "--json"]))
                .expect("valid Phase 11-B mode");
            assert_eq!(options.selection, Selection::Phase(Phase::P11B));
            assert!(options.json);
        }
    }

    #[test]
    fn evidence_file_enables_json_without_terminal_redirection() {
        let options = Options::parse(args(&[
            "phase",
            "11",
            "--performance",
            "--evidence-file=docs/report/evidence/phase-11.json",
        ]))
        .expect("valid evidence file option");
        assert!(options.json);
        assert_eq!(
            options.evidence_file.as_deref(),
            Some(std::path::Path::new("docs/report/evidence/phase-11.json"))
        );
    }
}
