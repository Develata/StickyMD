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
    P12,
    P13,
    P14,
}

impl Phase {
    pub(crate) const ALL: [Self; 16] = [
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
        Self::P12,
        Self::P13,
        Self::P14,
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
            "12" | "phase-12" => Ok(Self::P12),
            "13" | "phase-13" => Ok(Self::P13),
            "14" | "phase-14" => Ok(Self::P14),
            _ => Err(format!("unknown phase `{value}`; expected 00..14 or 11-b")),
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
            Self::P12 => "12",
            Self::P13 => "13",
            Self::P14 => "14",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CommandLine {
    Smoke(Options),
    AcceptanceManual(ManualCommand),
    Qualification(QualificationCommand),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ManualCommand {
    Run { session: Option<ManualSession> },
    Guided { session: Option<GuidedSession> },
    List,
    Status,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum GuidedSession {
    G1,
    G2,
}

impl GuidedSession {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_uppercase().as_str() {
            "G1" => Ok(Self::G1),
            "G2" => Ok(Self::G2),
            _ => Err(format!("unknown guided session `{value}`; expected G1..G2")),
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::G1 => "G1",
            Self::G2 => "G2",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum ManualSession {
    M1,
    M2,
    M3,
    M4,
    M5,
}

impl ManualSession {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_uppercase().as_str() {
            "M1" => Ok(Self::M1),
            "M2" => Ok(Self::M2),
            "M3" => Ok(Self::M3),
            "M4" => Ok(Self::M4),
            "M5" => Ok(Self::M5),
            _ => Err(format!("unknown manual session `{value}`; expected M1..M5")),
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::M1 => "M1",
            Self::M2 => "M2",
            Self::M3 => "M3",
            Self::M4 => "M4",
            Self::M5 => "M5",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum QualificationCommand {
    Environment {
        evidence_file: Option<PathBuf>,
    },
    LocalCampaign,
    Candidate,
    StartupAttribution,
    WindowStress(WindowStressOptions),
    NativeRuntime {
        executable: PathBuf,
    },
    G3Exact {
        zip: Option<PathBuf>,
        evidence_file: Option<PathBuf>,
        case: Option<G3Case>,
    },
    G4Exact {
        zip: Option<PathBuf>,
        evidence_file: Option<PathBuf>,
        case: Option<G4Case>,
    },
    G5Exact {
        zip: Option<PathBuf>,
        evidence_file: Option<PathBuf>,
        case: Option<G5Case>,
    },
    Decision {
        key: String,
        status: String,
        evidence: String,
    },
    Readiness {
        explain: bool,
    },
    Remote {
        run_id: u64,
        attempt: u64,
    },
    Downloaded {
        zip: PathBuf,
    },
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum G3Case {
    G301,
    G302,
    G303,
    G304,
    G305,
}

impl G3Case {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_uppercase().as_str() {
            "G3-01" => Ok(Self::G301),
            "G3-02" => Ok(Self::G302),
            "G3-03" => Ok(Self::G303),
            "G3-04" => Ok(Self::G304),
            "G3-05" => Ok(Self::G305),
            _ => Err(format!("unknown G3 case `{value}`; expected G3-01..G3-05")),
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::G301 => "G3-01",
            Self::G302 => "G3-02",
            Self::G303 => "G3-03",
            Self::G304 => "G3-04",
            Self::G305 => "G3-05",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum G4Case {
    G401,
    G402,
    G403,
    G404,
    G405,
}

impl G4Case {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_uppercase().as_str() {
            "G4-01" => Ok(Self::G401),
            "G4-02" => Ok(Self::G402),
            "G4-03" => Ok(Self::G403),
            "G4-04" => Ok(Self::G404),
            "G4-05" => Ok(Self::G405),
            _ => Err(format!("unknown G4 case `{value}`; expected G4-01..G4-05")),
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::G401 => "G4-01",
            Self::G402 => "G4-02",
            Self::G403 => "G4-03",
            Self::G404 => "G4-04",
            Self::G405 => "G4-05",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(crate) enum G5Case {
    G501,
    G502,
    G503,
    G504,
}

impl G5Case {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_uppercase().as_str() {
            "G5-01" => Ok(Self::G501),
            "G5-02" => Ok(Self::G502),
            "G5-03" => Ok(Self::G503),
            "G5-04" => Ok(Self::G504),
            _ => Err(format!("unknown G5 case `{value}`; expected G5-01..G5-04")),
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::G501 => "G5-01",
            Self::G502 => "G5-02",
            Self::G503 => "G5-03",
            Self::G504 => "G5-04",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum WindowStressScenario {
    Collapse,
    Tray,
    Controls,
    ViewMode,
    CollapseTray,
    Combined,
}

impl WindowStressScenario {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "collapse" => Ok(Self::Collapse),
            "tray" => Ok(Self::Tray),
            "controls" => Ok(Self::Controls),
            "view-mode" => Ok(Self::ViewMode),
            "collapse-tray" => Ok(Self::CollapseTray),
            "combined" => Ok(Self::Combined),
            _ => Err(format!(
                "unknown window-stress scenario `{value}`; expected collapse, tray, controls, view-mode, collapse-tray, or combined"
            )),
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Collapse => "collapse",
            Self::Tray => "tray",
            Self::Controls => "controls",
            Self::ViewMode => "view-mode",
            Self::CollapseTray => "collapse-tray",
            Self::Combined => "combined",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WindowStressOptions {
    pub(crate) scenario: WindowStressScenario,
    pub(crate) runs: usize,
    pub(crate) collapse_cycles: usize,
    pub(crate) tray_cycles: usize,
    pub(crate) control_cycles: usize,
    pub(crate) view_mode_cycles: usize,
    pub(crate) persistence_cycles: usize,
}

impl CommandLine {
    pub(crate) fn parse<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let args: Vec<String> = args.into_iter().collect();
        match args.first().map(String::as_str) {
            Some("acceptance") => match args.get(1).map(String::as_str) {
                Some("manual") => Self::parse_manual(&args[2..]).map(Self::AcceptanceManual),
                _ => Err(
                    "usage: stickymd-smoke acceptance manual [run [--session=M1..M5]|guided [--session=G1..G2]|list|status]"
                        .to_owned(),
                ),
            },
            Some("qualification") => Self::parse_qualification(&args[1..]),
            _ => Options::parse(args).map(Self::Smoke),
        }
    }

    fn parse_manual(arguments: &[String]) -> Result<ManualCommand, String> {
        match arguments.first().map(String::as_str) {
            None => Ok(ManualCommand::Run { session: None }),
            Some("run") if arguments.len() == 1 => Ok(ManualCommand::Run { session: None }),
            Some("run") if arguments.len() == 2 => Ok(ManualCommand::Run {
                session: Some(ManualSession::parse(named_value(
                    &arguments[1..],
                    "--session=",
                )?)?),
            }),
            Some("guided") if arguments.len() == 1 => {
                Ok(ManualCommand::Guided { session: None })
            }
            Some("guided") if arguments.len() == 2 => Ok(ManualCommand::Guided {
                session: Some(GuidedSession::parse(named_value(
                    &arguments[1..],
                    "--session=",
                )?)?),
            }),
            Some("list") if arguments.len() == 1 => Ok(ManualCommand::List),
            Some("status") if arguments.len() == 1 => Ok(ManualCommand::Status),
            _ => Err(
                "usage: stickymd-smoke acceptance manual [run [--session=M1..M5]|guided [--session=G1..G2]|list|status]"
                    .to_owned(),
            ),
        }
    }

    fn parse_qualification(arguments: &[String]) -> Result<Self, String> {
        match arguments.first().map(String::as_str) {
            Some("environment") => {
                let evidence_file = match arguments.get(1).map(String::as_str) {
                    None => None,
                    Some(value) if arguments.len() == 2 => Some(PathBuf::from(
                        value
                            .strip_prefix("--evidence-file=")
                            .filter(|value| !value.is_empty())
                            .ok_or_else(|| {
                                "usage: stickymd-smoke qualification environment [--evidence-file=<path>]"
                                    .to_owned()
                            })?,
                    )),
                    _ => {
                        return Err(
                            "usage: stickymd-smoke qualification environment [--evidence-file=<path>]"
                                .to_owned(),
                        );
                    }
                };
                Ok(Self::Qualification(QualificationCommand::Environment {
                    evidence_file,
                }))
            }
            Some("candidate") if arguments.len() == 1 => {
                Ok(Self::Qualification(QualificationCommand::Candidate))
            }
            Some("attribution") if arguments.len() == 1 => Ok(Self::Qualification(
                QualificationCommand::StartupAttribution,
            )),
            Some("window-stress") => {
                let values = &arguments[1..];
                if values.len() != 7 {
                    return Err(window_stress_usage());
                }
                Ok(Self::Qualification(QualificationCommand::WindowStress(
                    WindowStressOptions {
                        scenario: WindowStressScenario::parse(named_value(
                            values,
                            "--scenario=",
                        )?)?,
                        runs: named_bounded_usize(values, "--runs=", 1, 1_000)?,
                        collapse_cycles: named_bounded_usize(
                            values,
                            "--collapse-cycles=",
                            0,
                            10_000,
                        )?,
                        tray_cycles: named_bounded_usize(
                            values,
                            "--tray-cycles=",
                            0,
                            10_000,
                        )?,
                        control_cycles: named_bounded_usize(
                            values,
                            "--control-cycles=",
                            0,
                            10_000,
                        )?,
                        view_mode_cycles: named_bounded_usize(
                            values,
                            "--view-mode-cycles=",
                            0,
                            10_000,
                        )?,
                        persistence_cycles: named_bounded_usize(
                            values,
                            "--persistence-cycles=",
                            0,
                            10_000,
                        )?,
                    },
                )))
            }
            Some("native-runtime") => {
                let values = &arguments[1..];
                if values.len() != 1 {
                    return Err(
                        "usage: stickymd-smoke qualification native-runtime --exe=<path>"
                            .to_owned(),
                    );
                }
                Ok(Self::Qualification(QualificationCommand::NativeRuntime {
                    executable: PathBuf::from(named_value(values, "--exe=")?),
                }))
            }
            Some("g3") => {
                let values = &arguments[1..];
                if values.len() > 3
                    || values.iter().any(|value| {
                        !value.starts_with("--zip=")
                            && !value.starts_with("--evidence-file=")
                            && !value.starts_with("--case=")
                    })
                {
                    return Err(
                        "usage: stickymd-smoke qualification g3 [--zip=<path>] [--evidence-file=<path>] [--case=G3-01..G3-05]"
                            .to_owned(),
                    );
                }
                let zip = optional_named_value(values, "--zip=")?.map(PathBuf::from);
                let evidence_file =
                    optional_named_value(values, "--evidence-file=")?.map(PathBuf::from);
                let case = optional_named_value(values, "--case=")?
                    .map(G3Case::parse)
                    .transpose()?;
                Ok(Self::Qualification(QualificationCommand::G3Exact {
                    zip,
                    evidence_file,
                    case,
                }))
            }
            Some("g4") => {
                let values = &arguments[1..];
                if values.len() > 3
                    || values.iter().any(|value| {
                        !value.starts_with("--zip=")
                            && !value.starts_with("--evidence-file=")
                            && !value.starts_with("--case=")
                    })
                {
                    return Err(
                        "usage: stickymd-smoke qualification g4 [--zip=<path>] [--evidence-file=<path>] [--case=G4-01..G4-05]"
                            .to_owned(),
                    );
                }
                let zip = optional_named_value(values, "--zip=")?.map(PathBuf::from);
                let evidence_file =
                    optional_named_value(values, "--evidence-file=")?.map(PathBuf::from);
                let case = optional_named_value(values, "--case=")?
                    .map(G4Case::parse)
                    .transpose()?;
                Ok(Self::Qualification(QualificationCommand::G4Exact {
                    zip,
                    evidence_file,
                    case,
                }))
            }
            Some("g5") => {
                let values = &arguments[1..];
                if values.len() > 3
                    || values.iter().any(|value| {
                        !value.starts_with("--zip=")
                            && !value.starts_with("--evidence-file=")
                            && !value.starts_with("--case=")
                    })
                {
                    return Err(
                        "usage: stickymd-smoke qualification g5 [--zip=<path>] [--evidence-file=<path>] [--case=G5-01..G5-04]"
                            .to_owned(),
                    );
                }
                let zip = optional_named_value(values, "--zip=")?.map(PathBuf::from);
                let evidence_file =
                    optional_named_value(values, "--evidence-file=")?.map(PathBuf::from);
                let case = optional_named_value(values, "--case=")?
                    .map(G5Case::parse)
                    .transpose()?;
                Ok(Self::Qualification(QualificationCommand::G5Exact {
                    zip,
                    evidence_file,
                    case,
                }))
            }
            Some("local") if arguments.len() == 1 => {
                Ok(Self::Qualification(QualificationCommand::LocalCampaign))
            }
            Some("decision") => {
                let values = &arguments[1..];
                if values.len() != 3 {
                    return Err("usage: stickymd-smoke qualification decision --key=<decision> --status=<state> --evidence=<reference>".to_owned());
                }
                Ok(Self::Qualification(QualificationCommand::Decision {
                    key: named_value(values, "--key=")?.to_owned(),
                    status: named_value(values, "--status=")?.to_owned(),
                    evidence: named_value(values, "--evidence=")?.to_owned(),
                }))
            }
            Some("readiness") => {
                let explain = match arguments.get(1).map(String::as_str) {
                    None => false,
                    Some("--explain") if arguments.len() == 2 => true,
                    _ => {
                        return Err(
                            "usage: stickymd-smoke qualification readiness [--explain]".to_owned()
                        );
                    }
                };
                Ok(Self::Qualification(QualificationCommand::Readiness {
                    explain,
                }))
            }
            Some("remote") => {
                let values = &arguments[1..];
                if values.len() != 2 {
                    return Err(
                        "usage: stickymd-smoke qualification remote --run-id=<id> --attempt=<n>"
                            .to_owned(),
                    );
                }
                Ok(Self::Qualification(QualificationCommand::Remote {
                    run_id: named_u64(values, "--run-id=")?,
                    attempt: named_u64(values, "--attempt=")?,
                }))
            }
            Some("downloaded") => {
                let values = &arguments[1..];
                if values.len() != 1 {
                    return Err(
                        "usage: stickymd-smoke qualification downloaded --zip=<path>".to_owned(),
                    );
                }
                Ok(Self::Qualification(QualificationCommand::Downloaded {
                    zip: PathBuf::from(named_value(values, "--zip=")?),
                }))
            }
            _ => Err(
                "qualification requires environment, local, candidate, attribution, window-stress, native-runtime, g3, g4, g5, decision, readiness, remote, or downloaded"
                    .to_owned(),
            ),
        }
    }
}

fn named_value<'a>(arguments: &'a [String], prefix: &str) -> Result<&'a str, String> {
    arguments
        .iter()
        .find_map(|argument| argument.strip_prefix(prefix))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("missing `{prefix}<value>`"))
}

fn optional_named_value<'a>(
    arguments: &'a [String],
    prefix: &str,
) -> Result<Option<&'a str>, String> {
    let mut values = arguments
        .iter()
        .filter_map(|argument| argument.strip_prefix(prefix));
    let value = values.next();
    if values.next().is_some() {
        return Err(format!("duplicate `{prefix}<value>`"));
    }
    match value {
        Some("") => Err(format!("empty `{prefix}<value>`")),
        value => Ok(value),
    }
}

fn named_u64(arguments: &[String], prefix: &str) -> Result<u64, String> {
    let value = named_value(arguments, prefix)?;
    value
        .parse::<u64>()
        .map_err(|error| format!("invalid `{prefix}{value}`: {error}"))
}

fn named_bounded_usize(
    arguments: &[String],
    prefix: &str,
    minimum: usize,
    maximum: usize,
) -> Result<usize, String> {
    let value = named_value(arguments, prefix)?;
    let parsed = value
        .parse::<usize>()
        .map_err(|error| format!("invalid `{prefix}{value}`: {error}"))?;
    if !(minimum..=maximum).contains(&parsed) {
        return Err(format!(
            "`{prefix}{value}` is outside {minimum}..={maximum}"
        ));
    }
    Ok(parsed)
}

fn window_stress_usage() -> String {
    "usage: stickymd-smoke qualification window-stress --scenario=<collapse|tray|controls|view-mode|collapse-tray|combined> --runs=<1..1000> --collapse-cycles=<0..10000> --tray-cycles=<0..10000> --control-cycles=<0..10000> --view-mode-cycles=<0..10000> --persistence-cycles=<0..10000>".to_owned()
}

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

use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    pub(crate) selection: Selection,
    pub(crate) ci: bool,
    pub(crate) ci_shard: Option<CiShard>,
    pub(crate) performance: bool,
    pub(crate) runtime: bool,
    pub(crate) resources: bool,
    pub(crate) resource_module: Option<ResourceModule>,
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
        "usage: stickymd-smoke phase <00..14|11-b> [--performance|--runtime|--resources [--resource-module=<source-preview|math|images|window|zoom>]|--release|--package] [--json] [--evidence-file=<path>]\n       stickymd-smoke all [--ci [--ci-shard=<tests|performance>]|--performance|--runtime|--resources [--resource-module=<source-preview|math|images|window|zoom>]|--release|--package] [--json] [--evidence-file=<path>]"
    }
}

#[cfg(test)]
mod tests {
    use super::{
        CiShard, CommandLine, GuidedSession, ManualCommand, ManualSession, Options, Phase,
        QualificationCommand, ResourceModule, Selection, WindowStressOptions, WindowStressScenario,
    };

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
    fn ci_shards_are_explicit_and_require_the_all_ci_contract() {
        let tests =
            Options::parse(args(&["all", "--ci", "--ci-shard=tests"])).expect("tests CI shard");
        assert_eq!(tests.ci_shard, Some(CiShard::Tests));
        let performance = Options::parse(args(&["all", "--ci", "--ci-shard=performance"]))
            .expect("performance CI shard");
        assert_eq!(performance.ci_shard, Some(CiShard::Performance));
        assert!(
            Options::parse(args(&["all", "--ci-shard=tests"]))
                .expect_err("shard without CI")
                .contains("requires `all --ci`")
        );
    }

    #[test]
    fn phase14_resources_can_select_one_measurement_module() {
        let options = Options::parse(args(&[
            "phase",
            "14",
            "--resources",
            "--resource-module=window",
        ]))
        .expect("targeted Phase 14 window resource module");
        assert_eq!(options.resource_module, Some(ResourceModule::Window));
        assert!(
            Options::parse(args(&[
                "phase",
                "08",
                "--resources",
                "--resource-module=window",
            ]))
            .expect_err("historical phase matrix must retain its own module")
            .contains("Phase 10 through 14")
        );
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
    fn phase12_and_qualification_commands_are_explicit() {
        let phase = Options::parse(args(&["phase", "12", "--release", "--json"]))
            .expect("valid Phase 12 release mode");
        assert_eq!(phase.selection, Selection::Phase(Phase::P12));

        let readiness = CommandLine::parse(args(&["qualification", "readiness", "--explain"]))
            .expect("valid readiness command");
        assert_eq!(
            readiness,
            CommandLine::Qualification(QualificationCommand::Readiness { explain: true })
        );

        let decision = CommandLine::parse(args(&[
            "qualification",
            "decision",
            "--key=RELEASE-VERSION",
            "--status=USER-APPROVED",
            "--evidence=USER-message",
        ]))
        .expect("valid decision command");
        assert_eq!(
            decision,
            CommandLine::Qualification(QualificationCommand::Decision {
                key: "RELEASE-VERSION".to_owned(),
                status: "USER-APPROVED".to_owned(),
                evidence: "USER-message".to_owned(),
            })
        );

        let manual = CommandLine::parse(args(&["acceptance", "manual"]))
            .expect("valid manual recorder command");
        assert_eq!(
            manual,
            CommandLine::AcceptanceManual(ManualCommand::Run { session: None })
        );

        let native_runtime = CommandLine::parse(args(&[
            "qualification",
            "native-runtime",
            "--exe=target/release/stickymd-win.exe",
        ]))
        .expect("valid native-runtime dependency gate");
        assert_eq!(
            native_runtime,
            CommandLine::Qualification(QualificationCommand::NativeRuntime {
                executable: std::path::PathBuf::from("target/release/stickymd-win.exe"),
            })
        );
    }

    #[test]
    fn phase13_environment_and_manual_sessions_are_explicit() {
        for mode in [
            "--performance",
            "--runtime",
            "--resources",
            "--release",
            "--package",
        ] {
            let options = Options::parse(args(&["phase", "13", mode, "--json"]))
                .expect("valid Phase 13 mode");
            assert_eq!(options.selection, Selection::Phase(Phase::P13));
        }

        let environment = CommandLine::parse(args(&[
            "qualification",
            "environment",
            "--evidence-file=dist/evidence/qualification-environment.json",
        ]))
        .expect("valid environment preflight");
        assert_eq!(
            environment,
            CommandLine::Qualification(QualificationCommand::Environment {
                evidence_file: Some(std::path::PathBuf::from(
                    "dist/evidence/qualification-environment.json"
                )),
            })
        );

        let manual = CommandLine::parse(args(&["acceptance", "manual", "run", "--session=M3"]))
            .expect("valid manual session");
        assert_eq!(
            manual,
            CommandLine::AcceptanceManual(ManualCommand::Run {
                session: Some(ManualSession::M3),
            })
        );
    }

    #[test]
    fn phase14_and_guided_manual_sessions_are_explicit() {
        for mode in [
            "--performance",
            "--runtime",
            "--resources",
            "--release",
            "--package",
        ] {
            let options = Options::parse(args(&["phase", "14", mode, "--json"]))
                .expect("valid Phase 14 mode");
            assert_eq!(options.selection, Selection::Phase(Phase::P14));
        }

        let guided = CommandLine::parse(args(&["acceptance", "manual", "guided", "--session=G2"]))
            .expect("valid guided manual session");
        assert_eq!(
            guided,
            CommandLine::AcceptanceManual(ManualCommand::Guided {
                session: Some(GuidedSession::G2),
            })
        );

        let g3 = CommandLine::parse(args(&[
            "qualification",
            "g3",
            "--zip=dist/candidate.zip",
            "--evidence-file=dist/evidence/g3.json",
        ]))
        .expect("valid exact-candidate G3 command");
        assert_eq!(
            g3,
            CommandLine::Qualification(QualificationCommand::G3Exact {
                zip: Some(std::path::PathBuf::from("dist/candidate.zip")),
                evidence_file: Some(std::path::PathBuf::from("dist/evidence/g3.json")),
                case: None,
            })
        );

        let targeted_g3 = CommandLine::parse(args(&["qualification", "g3", "--case=G3-05"]))
            .expect("valid targeted G3 command");
        assert_eq!(
            targeted_g3,
            CommandLine::Qualification(QualificationCommand::G3Exact {
                zip: None,
                evidence_file: None,
                case: Some(super::G3Case::G305),
            })
        );

        assert!(
            CommandLine::parse(args(&[
                "qualification",
                "g3",
                "--case=G3-04",
                "--case=G3-05",
            ]))
            .is_err()
        );

        let g4 = CommandLine::parse(args(&[
            "qualification",
            "g4",
            "--zip=dist/candidate.zip",
            "--evidence-file=dist/evidence/g4.json",
        ]))
        .expect("valid exact-candidate G4 command");
        assert_eq!(
            g4,
            CommandLine::Qualification(QualificationCommand::G4Exact {
                zip: Some(std::path::PathBuf::from("dist/candidate.zip")),
                evidence_file: Some(std::path::PathBuf::from("dist/evidence/g4.json")),
                case: None,
            })
        );

        let targeted_g4 = CommandLine::parse(args(&["qualification", "g4", "--case=G4-05"]))
            .expect("valid targeted G4 command");
        assert_eq!(
            targeted_g4,
            CommandLine::Qualification(QualificationCommand::G4Exact {
                zip: None,
                evidence_file: None,
                case: Some(super::G4Case::G405),
            })
        );

        let g5 = CommandLine::parse(args(&[
            "qualification",
            "g5",
            "--zip=dist/candidate.zip",
            "--evidence-file=dist/evidence/g5.json",
        ]))
        .expect("valid exact-candidate G5 command");
        assert_eq!(
            g5,
            CommandLine::Qualification(QualificationCommand::G5Exact {
                zip: Some(std::path::PathBuf::from("dist/candidate.zip")),
                evidence_file: Some(std::path::PathBuf::from("dist/evidence/g5.json")),
                case: None,
            })
        );

        let targeted_g5 = CommandLine::parse(args(&["qualification", "g5", "--case=G5-04"]))
            .expect("valid targeted G5 command");
        assert_eq!(
            targeted_g5,
            CommandLine::Qualification(QualificationCommand::G5Exact {
                zip: None,
                evidence_file: None,
                case: Some(super::G5Case::G504),
            })
        );
    }

    #[test]
    fn phase14_window_stress_diagnostic_is_typed_and_bounded() {
        let command = CommandLine::parse(args(&[
            "qualification",
            "window-stress",
            "--scenario=combined",
            "--runs=10",
            "--collapse-cycles=1000",
            "--tray-cycles=100",
            "--control-cycles=100",
            "--view-mode-cycles=100",
            "--persistence-cycles=1",
        ]))
        .expect("valid window-stress diagnostic");
        assert_eq!(
            command,
            CommandLine::Qualification(QualificationCommand::WindowStress(WindowStressOptions {
                scenario: WindowStressScenario::Combined,
                runs: 10,
                collapse_cycles: 1000,
                tray_cycles: 100,
                control_cycles: 100,
                view_mode_cycles: 100,
                persistence_cycles: 1,
            }))
        );

        let error = CommandLine::parse(args(&[
            "qualification",
            "window-stress",
            "--scenario=tray",
            "--runs=0",
            "--collapse-cycles=0",
            "--tray-cycles=1",
            "--control-cycles=0",
            "--view-mode-cycles=0",
            "--persistence-cycles=0",
        ]))
        .expect_err("zero independent runs must be rejected");
        assert!(error.contains("outside 1..=1000"));

        let error = CommandLine::parse(args(&[
            "qualification",
            "window-stress",
            "--scenario=tray",
            "--runs=1001",
            "--collapse-cycles=0",
            "--tray-cycles=1",
            "--control-cycles=0",
            "--view-mode-cycles=0",
            "--persistence-cycles=0",
        ]))
        .expect_err("more than one thousand independent runs must be rejected");
        assert!(error.contains("outside 1..=1000"));
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
