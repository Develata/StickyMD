//! Indivisible diagnostic cache units; group reuse never resumes partial stress work.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{GROUPS, ResourceCase, coverage, observations};
use crate::{cli::ResourceModule, release::json::Value};

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
}

impl From<ResourceCase> for Unit {
    fn from(case: ResourceCase) -> Self {
        Self::Case(case)
    }
}
