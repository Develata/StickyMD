//! Advisory diagnostic resume plans: no observations, evidence publication or qualification.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{Store, lookup::Lookup};
use crate::{
    cli::ResourceModule,
    evidence::escape_json,
    resource_plan::{ResourceCase, diagnostic::Unit},
};
use std::path::Path;

pub(crate) struct Plan {
    rows: Vec<Row>,
    fixed: u64,
    saved: u64,
}
struct Row {
    unit: Unit,
    action: &'static str,
    reason: String,
    wait: u64,
}

pub(crate) fn units(groups: &[ResourceModule], filter: Option<&str>) -> Result<Vec<Unit>, String> {
    let filter = filter.unwrap_or_default();
    let mut units = Vec::new();
    for &group in groups {
        if !filter.is_empty() && !group.cases().iter().any(|c| c.label == filter) {
            return Err(format!(
                "unknown resource case filter {filter} for {}",
                group.name()
            ));
        }
        if matches!(group, ResourceModule::Window | ResourceModule::Zoom) {
            units.push(Unit::Group(group));
        } else {
            units.extend(
                group
                    .cases()
                    .iter()
                    .copied()
                    .filter(|c| filter.is_empty() || c.label == filter)
                    .map(Unit::Case),
            );
        }
    }
    Ok(units)
}

impl Plan {
    fn build(
        units: &[Unit],
        mut inspect: impl FnMut(Unit) -> Result<Lookup, String>,
    ) -> Result<Self, String> {
        let mut seen: Vec<ResourceCase> = Vec::new();
        let mut rows = Vec::new();
        let mut fixed = 0;
        let mut saved = 0;
        for &unit in units {
            let shared = match unit {
                Unit::Case(case) => seen
                    .iter()
                    .find(|&&previous| previous.equivalent(case))
                    .copied(),
                Unit::Group(_) => None,
            };
            let (action, reason, wait) = if let Some(previous) = shared {
                (
                    "SHARE_IN_COMMAND",
                    format!("same complete cohort as {}", previous.label),
                    0,
                )
            } else {
                let lookup = inspect(unit)?;
                let wait = if lookup.result.is_some() {
                    0
                } else {
                    unit.minimum_wait_seconds()
                };
                (
                    if lookup.result.is_some() {
                        "REUSE_IF_VALID"
                    } else {
                        "RUN"
                    },
                    lookup.reason,
                    wait,
                )
            };
            if let Unit::Case(case) = unit {
                seen.push(case);
            }
            fixed += wait;
            saved += unit.minimum_wait_seconds() - wait;
            rows.push(Row {
                unit,
                action,
                reason,
                wait,
            });
        }
        Ok(Self { rows, fixed, saved })
    }

    pub(crate) fn disabled(units: &[Unit], reason: &str) -> Result<Self, String> {
        Self::build(units, |_| {
            Ok(Lookup {
                result: None,
                reason: format!("RESUME_DISABLED: {reason}"),
            })
        })
    }

    pub(crate) fn log(&self) {
        for row in &self.rows {
            eprintln!(
                "RESOURCE_RESUME_PLAN unit={} action={} fixed_wait_seconds={} reason=\"{}\"",
                row.unit.key(),
                row.action,
                row.wait,
                escape_json(&row.reason)
            );
        }
        eprintln!(
            "RESOURCE_RESUME_PLAN status=NOT_RUN planned_fixed_wait_seconds={} avoided_fixed_wait_seconds={} (advisory; execution revalidates; excludes probes, identity, startup and stress)",
            self.fixed, self.saved
        );
    }

    pub(crate) fn json(&self) -> String {
        let rows = self.rows.iter().map(|row| format!("{{\"unit\":\"{}\",\"action\":\"{}\",\"reason\":\"{}\",\"fixed_wait_seconds\":{}}}", row.unit.key(), row.action, escape_json(&row.reason), row.wait)).collect::<Vec<_>>().join(",");
        format!(
            "{{\"schema_version\":1,\"kind\":\"DIAGNOSTIC_PLAN\",\"status\":\"NOT_RUN\",\"planned_fixed_wait_seconds\":{},\"avoided_fixed_wait_seconds\":{},\"units\":[{rows}]}}",
            self.fixed, self.saved
        )
    }
}

impl Store {
    pub(crate) fn plan(&self, root: &Path, units: &[Unit]) -> Result<Plan, String> {
        self.verify(root)?;
        let plan = Plan::build(units, |unit| self.inspect(root, unit))?;
        self.verify(root)?;
        Ok(plan)
    }
}

pub(crate) fn preview(root: &Path, groups: &[ResourceModule]) -> Result<String, String> {
    let units = units(
        groups,
        std::env::var("STICKYMD_SMOKE_RESOURCE_CASE")
            .ok()
            .as_deref(),
    )?;
    let plan = match Store::open(root) {
        Ok(store) => store.plan(root, &units)?,
        Err(error) => Plan::disabled(&units, &error)?,
    };
    plan.log();
    Ok(plan.json())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn estimates_deduplicate_shared_cases_and_count_only_uncached_waits() {
        let units = units(&crate::resource_plan::GROUPS, None).unwrap();
        let mut checked = Vec::new();
        let plan = Plan::build(&units, |unit| {
            checked.push(unit);
            Ok(Lookup {
                result: (unit == Unit::Group(ResourceModule::Zoom)).then(|| {
                    crate::resource_plan::tests::valid_resource_result(ResourceModule::Zoom)
                }),
                reason: "test".into(),
            })
        })
        .unwrap();
        assert_eq!(
            plan.fixed,
            crate::resource_plan::minimum_wait_seconds(&crate::resource_plan::GROUPS, true, None)
                - 75
        );
        assert!(checked.len() < units.len());
        let json = crate::release::json::parse(&plan.json()).unwrap();
        assert_eq!(json.field("status").unwrap().string().unwrap(), "NOT_RUN");
        assert!(plan.json().contains("SHARE_IN_COMMAND"));
        assert!(plan.json().contains("REUSE_IF_VALID"));
    }
    #[test]
    fn invalid_filters_fail_before_identity_or_desktop_queries() {
        assert!(units(&[ResourceModule::Zoom], Some("source")).is_err());
        assert_eq!(
            units(&[ResourceModule::Math], Some("preview-no-math"))
                .unwrap()
                .len(),
            1
        );
    }
}
