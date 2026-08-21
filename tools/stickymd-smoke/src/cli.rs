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
}

impl Phase {
    pub(crate) const ALL: [Self; 9] = [
        Self::P00,
        Self::P01,
        Self::P02,
        Self::P03,
        Self::P04,
        Self::P05,
        Self::P06,
        Self::P07,
        Self::P08,
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
            _ => Err(format!("unknown phase `{value}`; expected 00..08")),
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
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Selection {
    Phase(Phase),
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    pub(crate) selection: Selection,
    pub(crate) ci: bool,
    pub(crate) performance: bool,
    pub(crate) runtime: bool,
    pub(crate) resources: bool,
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
                    .ok_or_else(|| "`phase` requires a number (00..08)".to_owned())?;
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
        };
        for argument in args {
            match argument.as_str() {
                "--ci" => options.ci = true,
                "--performance" => options.performance = true,
                "--runtime" => options.runtime = true,
                "--resources" => options.resources = true,
                _ => return Err(format!("unknown option `{argument}`\n{}", Self::usage())),
            }
        }
        if options.ci && (options.performance || options.runtime || options.resources) {
            return Err("`--ci` cannot be combined with environment-sensitive `--performance`, `--runtime`, or `--resources`".to_owned());
        }
        if options.resources && (options.performance || options.runtime) {
            return Err("`--resources` must run alone so the measured process is not contaminated by other smoke tasks".to_owned());
        }
        if options.runtime
            && matches!(
                selection,
                Selection::Phase(Phase::P00 | Phase::P01 | Phase::P02)
            )
        {
            return Err(
                "runtime smoke is defined only for Phase 03 through Phase 08, or `all`".to_owned(),
            );
        }
        if options.resources
            && !matches!(
                selection,
                Selection::Phase(Phase::P05 | Phase::P06 | Phase::P07 | Phase::P08)
                    | Selection::All
            )
        {
            return Err(
                "resource measurement is defined only for Phase 05 through Phase 08, or `all`"
                    .to_owned(),
            );
        }
        Ok(options)
    }

    pub(crate) const fn usage() -> &'static str {
        "usage: stickymd-smoke phase <00..08> [--performance] [--runtime] [--resources]\n       stickymd-smoke all [--ci] [--performance] [--runtime] [--resources]"
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
}
