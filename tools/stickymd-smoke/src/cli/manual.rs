//! Manual acceptance commands: selections, guided sessions and standalone runs.

use std::path::PathBuf;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ManualCommand {
    Run {
        session: Option<ManualSession>,
    },
    Guided {
        session: Option<GuidedSession>,
    },
    List,
    Status,
    /// USER declaration of the target environments for the current candidate.
    Target,
    /// Start a new observation context after an approved `RESET-MANUAL-LOG-<digest>`.
    Reset,
    /// Write an observation task for a recorder on another machine.
    Export {
        selection: ManualSelection,
        task: PathBuf,
    },
    /// Import an observation bundle recorded from an exported task.
    Import {
        bundle: PathBuf,
    },
}

/// The scenarios a recorder session walks: the cases of one manual session (all when
/// `None`), or the steps of one guided session (all when `None`).
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ManualSelection {
    Cases(Option<ManualSession>),
    Guided(Option<GuidedSession>),
}

impl ManualSelection {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value {
            "all" => Ok(Self::Cases(None)),
            "guided" => Ok(Self::Guided(None)),
            value if value.starts_with('M') => {
                ManualSession::parse(value).map(|session| Self::Cases(Some(session)))
            }
            value if value.starts_with('G') => {
                GuidedSession::parse(value).map(|session| Self::Guided(Some(session)))
            }
            _ => Err(format!(
                "unknown selection `{value}`; expected all, guided, M1..M5 or G1..G2"
            )),
        }
    }

    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::Cases(None) => "all",
            Self::Guided(None) => "guided",
            Self::Cases(Some(session)) => session.as_str(),
            Self::Guided(Some(session)) => session.as_str(),
        }
    }
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
