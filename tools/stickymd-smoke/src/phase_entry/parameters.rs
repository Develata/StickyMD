//! Decode shell parameter facts; semantic values reuse the canonical CLI parsers.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use crate::cli::{
    CommandLine, G3Case, G4Case, G5Case, GuidedSession, ManualSession, Options, Phase,
};
use std::collections::{BTreeMap, BTreeSet};

pub(super) struct Parameters {
    /// None denotes the retained `all.ps1` entry.
    pub phase: Option<Phase>,
    values: BTreeMap<String, String>,
    inactive: BTreeSet<String>,
}

impl Parameters {
    pub fn parse(arguments: &[String]) -> Result<Self, String> {
        let phase = match arguments.first().map(String::as_str) {
            Some("all") => None,
            Some(value) => Some(Phase::parse(value)?),
            None => return Err("phase-entry requires 00..14, 11-b or all".into()),
        };
        let mut values = BTreeMap::new();
        let mut inactive = BTreeSet::new();
        for argument in &arguments[1..] {
            let (key, value) = argument
                .strip_prefix("--")
                .and_then(|v| v.split_once('='))
                .ok_or("phase-entry expects --Parameter=value")?;
            let key = key.to_ascii_lowercase();
            let kind = parameter_kind(phase, &key)?;
            let value = match kind {
                Kind::Switch => match value.to_ascii_lowercase().as_str() {
                    "true" => "true".into(),
                    "false" => "false".into(),
                    _ => return Err(format!("{key} requires true or false")),
                },
                Kind::Number => value
                    .parse::<u64>()
                    .map_err(|_| format!("{key} requires an unsigned integer"))?
                    .to_string(),
                Kind::Text => value.into(),
            };
            if (matches!(kind, Kind::Switch) && value == "false")
                || (matches!(kind, Kind::Number) && value == "0")
            {
                inactive.insert(key.clone());
            }
            if values.insert(key.clone(), value).is_some() {
                return Err(format!("duplicate phase-entry parameter {key}"));
            }
        }
        let p = Self {
            phase,
            values,
            inactive,
        };
        p.validate_values()?;
        Ok(p)
    }

    fn validate_values(&self) -> Result<(), String> {
        for (key, value) in &self.values {
            match key.as_str() {
                "manualsession" => {
                    ManualSession::parse(value)?;
                }
                "guidedsession" => {
                    GuidedSession::parse(value)?;
                }
                "g3case" => {
                    G3Case::parse(value)?;
                }
                "g4case" => {
                    G4Case::parse(value)?;
                }
                "g5case" => {
                    G5Case::parse(value)?;
                }
                "resourcemodule" => {
                    Options::parse([
                        "phase".into(),
                        "14".into(),
                        "--resources".into(),
                        format!("--resource-module={}", value.to_ascii_lowercase()),
                    ])?;
                }
                "cishard" => {
                    Options::parse([
                        "all".into(),
                        "--ci".into(),
                        format!("--ci-shard={}", value.to_ascii_lowercase()),
                    ])?;
                }
                _ => {}
            }
        }
        if self.phase == Some(Phase::P14) {
            // PowerShell ValidateSet was case-insensitive even for unused values.
            // Execution still passes the original spelling to the canonical parser.
            let mut validation = self.window_stress();
            validation[2] = validation[2].to_ascii_lowercase();
            CommandLine::parse(validation)?;
        }
        Ok(())
    }

    pub fn active(&self, name: &str) -> bool {
        self.values.get(name).is_some_and(|value| !value.is_empty())
            && !self.inactive.contains(name)
    }

    pub fn append(&self, target: &mut Vec<String>, name: &str, flag: &str) {
        if self.active(name) {
            target.push(format!("--{flag}={}", self.values[name]));
        }
    }

    pub fn flag(&self, target: &mut Vec<String>, name: &str, flag: &str) {
        if self.active(name) {
            target.push(format!("--{flag}"));
        }
    }

    pub fn window_stress(&self) -> Vec<String> {
        let mut result = vec!["qualification".into(), "window-stress".into()];
        for (name, flag, default) in [
            ("windowstressscenario", "scenario", "combined"),
            ("windowstressruns", "runs", "10"),
            ("collapsecycles", "collapse-cycles", "1000"),
            ("traycycles", "tray-cycles", "100"),
            ("controlcycles", "control-cycles", "100"),
            ("viewmodecycles", "view-mode-cycles", "100"),
            ("persistencecycles", "persistence-cycles", "100"),
        ] {
            result.push(format!(
                "--{flag}={}",
                self.values.get(name).map_or(default, String::as_str)
            ));
        }
        result
    }
}

#[derive(Clone, Copy)]
enum Kind {
    Switch,
    Number,
    Text,
}

// Availability is part of shell compatibility; semantic parsing remains in cli.
fn parameter_kind(phase: Option<Phase>, key: &str) -> Result<Kind, String> {
    use Phase::*;
    let range = |first, last, all| phase.map_or(all, |p| (first..=last).contains(&p));
    let (supported, kind) = match key {
        "ci" => (range(P12, P14, true), Kind::Switch),
        "cishard" => (phase.is_none(), Kind::Text),
        "performance" => (range(P01, P14, true), Kind::Switch),
        "runtime" => (range(P03, P14, true), Kind::Switch),
        "resources" => (range(P05, P14, true), Kind::Switch),
        "release" | "package" => (range(P09, P14, true), Kind::Switch),
        "json" => (range(P10, P14, true), Kind::Switch),
        "evidencefile" => (range(P11, P14, false), Kind::Text),
        "resourcemodule" => (matches!(phase, None | Some(P10 | P14)), Kind::Text),
        "manual" | "readiness" | "explain" => (range(P12, P14, false), Kind::Switch),
        "decisionkey" | "decisionstatus" | "decisionevidence" | "downloadedzip" => {
            (range(P12, P14, false), Kind::Text)
        }
        "remoterunid" | "remoteattempt" => (range(P12, P14, false), Kind::Number),
        "candidate" => (range(P12, P13, false), Kind::Switch),
        "environment" | "campaign" | "manuallist" | "manualstatus" => {
            (range(P13, P14, false), Kind::Switch)
        }
        "manualsession" => (range(P13, P14, false), Kind::Text),
        "sourcefreeze"
        | "attribution"
        | "windowstress"
        | "guided"
        | "g3"
        | "g4"
        | "g5"
        | "resourceresume"
        | "resourceplan"
        | "resourcefailurefirst" => (phase == Some(P14), Kind::Switch),
        "guidedsession"
        | "g3zip"
        | "g3case"
        | "g4zip"
        | "g4case"
        | "g5zip"
        | "g5case"
        | "windowstressscenario" => (phase == Some(P14), Kind::Text),
        "windowstressruns" | "collapsecycles" | "traycycles" | "controlcycles"
        | "viewmodecycles" | "persistencecycles" => (phase == Some(P14), Kind::Number),
        _ => return Err(format!("unknown phase-entry parameter {key}")),
    };
    if supported {
        Ok(kind)
    } else {
        Err(format!(
            "{key} is not supported by entry {}",
            phase.map_or("all", Phase::number)
        ))
    }
}
