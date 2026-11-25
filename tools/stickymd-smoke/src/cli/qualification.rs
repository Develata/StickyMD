//! `qualification` subcommands, their exact cases and the window-stress options.

use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum QualificationCommand {
    Environment {
        evidence_file: Option<PathBuf>,
    },
    LocalCampaign,
    Modules,
    SourceFreeze,
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
        /// `CASE:ROLE,...` for a manual waiver.
        manual_scope: Option<String>,
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

    #[cfg(windows)]
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
    G406,
}

impl G4Case {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_uppercase().as_str() {
            "G4-01" => Ok(Self::G401),
            "G4-02" => Ok(Self::G402),
            "G4-03" => Ok(Self::G403),
            "G4-04" => Ok(Self::G404),
            "G4-05" => Ok(Self::G405),
            "G4-06" => Ok(Self::G406),
            _ => Err(format!("unknown G4 case `{value}`; expected G4-01..G4-06")),
        }
    }

    #[cfg(windows)]
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::G401 => "G4-01",
            Self::G402 => "G4-02",
            Self::G403 => "G4-03",
            Self::G404 => "G4-04",
            Self::G405 => "G4-05",
            Self::G406 => "G4-06",
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

    #[cfg(windows)]
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
    pub(super) fn parse(value: &str) -> Result<Self, String> {
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

    #[cfg(windows)]
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

pub(super) fn window_stress_usage() -> String {
    "usage: stickymd-smoke qualification window-stress --scenario=<collapse|tray|controls|view-mode|collapse-tray|combined> --runs=<1..1000> --collapse-cycles=<0..10000> --tray-cycles=<0..10000> --control-cycles=<0..10000> --view-mode-cycles=<0..10000> --persistence-cycles=<0..10000>".to_owned()
}
