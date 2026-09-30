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
    cached: bool,
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
        first: Option<Unit>,
        mut inspect: impl FnMut(Unit) -> Result<Lookup, String>,
    ) -> Result<Self, String> {
        let mut seen: Vec<(ResourceCase, bool)> = Vec::new();
        let mut rows = Vec::new();
        let mut fixed = 0;
        let mut saved = 0;
        for &unit in units {
            let shared = match unit {
                Unit::Case(case) => seen
                    .iter()
                    .find(|&&(previous, _)| previous.equivalent(case))
                    .copied(),
                Unit::Group(_) => None,
            };
            let (action, reason, wait, cached) = if Some(unit) == first {
                (
                    "RUN",
                    "LAST_FAILURE_FORCE_RUN".into(),
                    unit.minimum_wait_seconds(),
                    false,
                )
            } else if let Some((previous, cached)) = shared {
                (
                    "SHARE_IN_COMMAND",
                    format!("same complete cohort as {}", previous.label),
                    0,
                    cached,
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
                    lookup.result.is_some(),
                )
            };
            if let Unit::Case(case) = unit {
                seen.push((case, cached));
            }
            fixed += wait;
            saved += unit.minimum_wait_seconds() - wait;
            rows.push(Row {
                unit,
                action,
                reason,
                wait,
                cached,
            });
        }
        Ok(Self { rows, fixed, saved })
    }

    pub(crate) fn disabled(units: &[Unit], reason: &str) -> Result<Self, String> {
        Self::build(units, None, |_| {
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

    pub(crate) fn cached_units(&self) -> Vec<Unit> {
        self.rows
            .iter()
            .filter(|row| row.cached)
            .map(|row| row.unit)
            .collect()
    }

    pub(crate) fn json(&self) -> String {
        let rows = self.rows.iter().map(|row| format!("{{\"unit\":\"{}\",\"action\":\"{}\",\"reason\":\"{}\",\"fixed_wait_seconds\":{}}}", row.unit.key(), row.action, escape_json(&row.reason), row.wait)).collect::<Vec<_>>().join(",");
        format!(
            "{{\"schema_version\":1,\"kind\":\"DIAGNOSTIC_PLAN\",\"status\":\"NOT_RUN\",\"planned_fixed_wait_seconds\":{},\"avoided_fixed_wait_seconds\":{},\"units\":[{rows}]}}",
            self.fixed, self.saved
        )
    }
}

/// The freshly created Store supplies the pre-read identity; never accepts an old Store.
pub(crate) fn prepare(
    root: &Path,
    units: &[Unit],
    first: Option<Unit>,
) -> Result<(Option<Store>, Plan), String> {
    let store = match Store::open(root) {
        Ok(store) => store,
        Err(error) => return Ok((None, Plan::disabled(units, &error)?)),
    };
    let plan = Plan::build(units, first, |unit| store.inspect(root, unit))?;
    store.verify(root)?;
    Ok((Some(store), plan))
}

pub(crate) fn prioritize(units: &mut [Unit], first: Option<Unit>) {
    if let Some(first) = first.filter(|first| units.contains(first)) {
        units.sort_by_key(|&unit| (unit.group() != first.group(), unit != first));
    }
}

pub(crate) fn preview(
    root: &Path,
    groups: &[ResourceModule],
    failure_first: bool,
) -> Result<String, String> {
    let mut units = units(
        groups,
        std::env::var("STICKYMD_SMOKE_RESOURCE_CASE")
            .ok()
            .as_deref(),
    )?;
    let priority = super::priority::select(root, &units, failure_first);
    prioritize(&mut units, priority.first);
    let (_, plan) = prepare(root, &units, priority.first)?;
    plan.log();
    Ok(plan.json())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_first_forces_five_fresh_samples_and_propagates_uncached_alias_budget() {
        let mut units = units(&crate::resource_plan::GROUPS, None).unwrap();
        let first = Unit::from_key("preview-20-math").unwrap();
        let expected = units.clone();
        prioritize(&mut units, Some(first));
        assert_eq!(units[0], first);
        assert!(
            units[..ResourceModule::Math.cases().len()]
                .iter()
                .all(|u| u.group() == Some(ResourceModule::Math))
        );
        let plan = Plan::build(&units, Some(first), |unit| {
            assert_ne!(unit, first, "forced unit must not inspect an old success");
            let result = match unit {
                Unit::Case(case) => super::super::equivalence_tests::result(case),
                Unit::Group(group) => crate::resource_plan::tests::valid_resource_result(group),
            };
            Ok(Lookup {
                result: Some(result),
                reason: "valid".into(),
            })
        })
        .unwrap();
        assert_eq!(plan.fixed, first.minimum_wait_seconds());
        assert_eq!(plan.rows[0].reason, "LAST_FAILURE_FORCE_RUN");
        assert!(!plan.cached_units().contains(&first));
        let alias = Unit::from_key("preview").unwrap();
        assert!(!plan.cached_units().contains(&alias));
        assert_eq!(
            plan.rows.iter().find(|r| r.unit == alias).unwrap().action,
            "SHARE_IN_COMMAND"
        );
        assert_eq!(units.len(), expected.len());
        for unit in expected {
            assert_eq!(units.iter().filter(|&&u| u == unit).count(), 1);
        }
    }
    #[test]
    fn estimates_deduplicate_shared_cases_and_count_only_uncached_waits() {
        let units = units(&crate::resource_plan::GROUPS, None).unwrap();
        let mut checked = Vec::new();
        let plan = Plan::build(&units, None, |unit| {
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
        assert_eq!(plan.cached_units(), [Unit::Group(ResourceModule::Zoom)]);
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
