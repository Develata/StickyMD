//! Indivisible diagnostic cache units; group reuse never resumes partial stress work.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{GROUPS, ResourceCase, coverage, observations};
use crate::{cli::ResourceModule, evidence::EvidenceResult, release::json::Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unit {
    Case(ResourceCase),
    Group(ResourceModule),
}

impl Unit {
    pub(crate) fn key(self) -> &'static str {
        match self {
            Self::Case(case) => case.label,
            Self::Group(ResourceModule::Window) => "group-window",
            Self::Group(ResourceModule::Zoom) => "group-zoom",
            Self::Group(_) => "unregistered-group",
        }
    }
    pub(crate) fn result_id(self) -> &'static str {
        match self {
            Self::Case(case) => case.label,
            Self::Group(group) => group.task_label(),
        }
    }
    pub(crate) fn registered(self) -> Result<(), String> {
        match self {
            Self::Case(case) if GROUPS.iter().any(|g| g.cases().contains(&case)) => Ok(()),
            Self::Group(ResourceModule::Window | ResourceModule::Zoom) => Ok(()),
            _ => Err("unregistered diagnostic unit".into()),
        }
    }
    pub(crate) fn minimum_wait_seconds(self) -> u64 {
        match self {
            Self::Case(case) => case.minimum_wait_seconds(),
            Self::Group(group) => super::minimum_wait_seconds(&[group], false, None),
        }
    }
    pub(crate) fn validate(self, result: &Value) -> Result<(), String> {
        self.registered()?;
        match self {
            Self::Case(case) => observations::validate_case(result, case),
            Self::Group(group) => coverage::validate_group_result(result, group),
        }
    }

    /// Stable registered alternatives only; Window/Zoom remain indivisible groups.
    pub(crate) fn equivalents(self) -> Vec<Self> {
        let Self::Case(requested) = self else {
            return Vec::new();
        };
        GROUPS
            .iter()
            .flat_map(|g| g.cases())
            .copied()
            .filter(|case| *case != requested && requested.equivalent(*case))
            .map(Self::Case)
            .collect()
    }

    /// A bijective label projection of an already validated complete historical cohort.
    pub(crate) fn project(self, origin: Self, result: &mut EvidenceResult) -> Result<(), String> {
        self.registered()?;
        origin.registered()?;
        let (Self::Case(target), Self::Case(source)) = (self, origin) else {
            return Err("whole groups cannot be diagnostic aliases".into());
        };
        if !target.equivalent(source)
            || result.id != source.label
            || result.samples.iter().any(|s| s.cohort != source.label)
        {
            return Err("incompatible diagnostic alias".into());
        }
        let prefix = format!("{}.", source.label);
        let rename = |name: &mut String| {
            if let Some(suffix) = name.strip_prefix(&prefix) {
                *name = format!("{}.{suffix}", target.label);
            }
        };
        result.id = target.label.into();
        for m in &mut result.measurements {
            rename(&mut m.name);
        }
        for g in &mut result.gates {
            rename(&mut g.metric);
        }
        for sample in &mut result.samples {
            sample.cohort = target.label.into();
        }
        result
            .detail
            .get_or_insert_default()
            .push_str(&format!("; requested_unit={}", target.label));
        Ok(())
    }
}

impl From<ResourceCase> for Unit {
    fn from(case: ResourceCase) -> Self {
        Self::Case(case)
    }
}
