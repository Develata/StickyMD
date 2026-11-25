//! Command-line contract for selecting phase verification tasks.

mod args;
mod manual;
mod options;
mod phase;
mod qualification;
#[cfg(test)]
mod tests;

use args::{exact_named_values, named_bounded_usize, named_u64, named_value, optional_named_value};
pub(crate) use manual::{GuidedSession, ManualCommand, ManualSelection, ManualSession};
pub(crate) use options::{CiShard, Options, ResourceModule, Selection};
pub(crate) use phase::Phase;
use qualification::window_stress_usage;
pub(crate) use qualification::{
    G3Case, G4Case, G5Case, QualificationCommand, WindowStressOptions, WindowStressScenario,
};
use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CommandLine {
    MacGui(crate::mac_gui::Options),
    MacLogout(crate::mac_gui::logout::Options),
    MacLatency(crate::mac_gui::latency::Options),
    MacResources(crate::mac_resources::Options),
    // Boxed: the ledger's options outgrow the other commands' (clippy::large_enum_variant).
    MacLedger(Box<crate::mac_ledger::Options>),
    PhaseEntryPlan(Vec<String>),
    Release(crate::release::Command),
    Smoke(Options),
    PackagePath(PathBuf),
    Modules(crate::headless::Command),
    Ci(crate::ci::Command),
    Development(crate::development::Command),
    Timings(crate::timing_summary::Options),
    StartupDetails(crate::startup_details::Options),
    AcceptanceManual(ManualCommand),
    /// The standalone recorder on a tested machine without the repository.
    ManualRecord(StandaloneRecord),
    Qualification(QualificationCommand),
}

/// `acceptance manual record`: an exported observation task, the candidate ZIP and the
/// bundle file to write.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StandaloneRecord {
    pub(crate) task: PathBuf,
    pub(crate) zip: PathBuf,
    pub(crate) bundle: PathBuf,
}

impl CommandLine {
    pub(crate) fn parse<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let args: Vec<String> = args.into_iter().collect();
        match args.first().map(String::as_str) {
            Some("phase-entry") => crate::phase_entry::parse(&args[1..], false),
            Some("phase-entry-plan") => crate::phase_entry::parse(&args[1..], true),
            Some("release") => crate::release::parse(&args[1..]).map(Self::Release),
            Some("package-path") => package_path_directory(&args[1..]).map(Self::PackagePath),
            Some("modules") => crate::headless::Command::parse(&args[1..]).map(Self::Modules),
            Some("ci") => crate::ci::Command::parse(&args[1..]).map(Self::Ci),
            Some("dev-check") => crate::development::Command::parse(&args[1..]).map(Self::Development),
            Some("mac-gui") => crate::mac_gui::parse(&args[1..]).map(Self::MacGui),
            Some("mac-logout") => crate::mac_gui::logout::parse(&args[1..]).map(Self::MacLogout),
            Some("mac-latency") => crate::mac_gui::latency::parse(&args[1..]).map(Self::MacLatency),
            Some("mac-resources") => crate::mac_resources::parse(&args[1..]).map(Self::MacResources),
            Some("mac-ledger") => crate::mac_ledger::parse(&args[1..]).map(|options| Self::MacLedger(Box::new(options))),
            Some("timings") => crate::timing_summary::parse(&args[1..]).map(Self::Timings),
            Some("startup-details") => crate::startup_details::parse(&args[1..]).map(Self::StartupDetails),
            Some("acceptance") => match args.get(1).map(String::as_str) {
                Some("manual") if args.get(2).map(String::as_str) == Some("record") => {
                    Self::parse_manual_record(&args[3..]).map(Self::ManualRecord)
                }
                Some("manual") => Self::parse_manual(&args[2..]).map(Self::AcceptanceManual),
                _ => Err(
                    "usage: stickymd-smoke acceptance manual [run [--session=M1..M5]|guided [--session=G1..G2]|list|status|target|reset|export --task=<path> [--select=all|guided|M1..M5|G1..G2]|import --bundle=<path>|record --task=<path> --zip=<path> --bundle=<path>]"
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
            Some("target") if arguments.len() == 1 => Ok(ManualCommand::Target),
            Some("reset") if arguments.len() == 1 => Ok(ManualCommand::Reset),
            Some("export") => {
                let [task, selection] = exact_named_values(&arguments[1..], ["--task=", "--select="])?;
                Ok(ManualCommand::Export {
                    selection: selection.map_or(Ok(ManualSelection::Cases(None)), ManualSelection::parse)?,
                    task: PathBuf::from(task.ok_or("missing `--task=<path>`")?),
                })
            }
            Some("import") => {
                let [bundle] = exact_named_values(&arguments[1..], ["--bundle="])?;
                Ok(ManualCommand::Import {
                    bundle: PathBuf::from(bundle.ok_or("missing `--bundle=<path>`")?),
                })
            }
            _ => Err(
                "usage: stickymd-smoke acceptance manual [run [--session=M1..M5]|guided [--session=G1..G2]|list|status|target|reset|export --task=<path> [--select=all|guided|M1..M5|G1..G2]|import --bundle=<path>|record --task=<path> --zip=<path> --bundle=<path>]"
                    .to_owned(),
            ),
        }
    }

    fn parse_manual_record(arguments: &[String]) -> Result<StandaloneRecord, String> {
        let [task, zip, bundle] =
            exact_named_values(arguments, ["--task=", "--zip=", "--bundle="])?;
        let required = |value: Option<&str>, name: &str| {
            value
                .map(PathBuf::from)
                .ok_or_else(|| format!("missing `{name}<path>`"))
        };
        Ok(StandaloneRecord {
            task: required(task, "--task=")?,
            zip: required(zip, "--zip=")?,
            bundle: required(bundle, "--bundle=")?,
        })
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
            Some("source-freeze") if arguments.len() == 1 => {
                Ok(Self::Qualification(QualificationCommand::SourceFreeze))
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
                        "usage: stickymd-smoke qualification g4 [--zip=<path>] [--evidence-file=<path>] [--case=G4-01..G4-06]"
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
            Some("modules") if arguments.len() == 1 => {
                Ok(Self::Qualification(QualificationCommand::Modules))
            }
            Some("decision") => {
                let values = &arguments[1..];
                let manual_scope = values
                    .iter()
                    .find_map(|value| value.strip_prefix("--manual-scope="))
                    .map(str::to_owned);
                if values.len() != 3 + usize::from(manual_scope.is_some()) {
                    return Err("usage: stickymd-smoke qualification decision --key=<decision> --status=<state> --evidence=<reference> [--manual-scope=CASE:ROLE,...]".to_owned());
                }
                Ok(Self::Qualification(QualificationCommand::Decision {
                    key: named_value(values, "--key=")?.to_owned(),
                    status: named_value(values, "--status=")?.to_owned(),
                    evidence: named_value(values, "--evidence=")?.to_owned(),
                    manual_scope,
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
                "qualification requires environment, local, modules, source-freeze, attribution, window-stress, native-runtime, g3, g4, g5, decision, readiness, remote, or downloaded"
                    .to_owned(),
            ),
        }
    }
}

/// The directory of `package-path --directory <directory>`.
fn package_path_directory(arguments: &[String]) -> Result<PathBuf, String> {
    match arguments {
        [flag, directory] if flag == "--directory" && !directory.is_empty() => {
            Ok(PathBuf::from(directory))
        }
        _ => Err("usage: stickymd-smoke package-path --directory <directory>".to_owned()),
    }
}
