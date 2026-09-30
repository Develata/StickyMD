//! Keep prerequisites fixed while moving an explicitly selected failed resource group first.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::{Options, ResourceModule, Task};
use crate::qualification::resource_diagnostics::{plan, priority};
use std::path::Path;

fn group(task: &Task) -> Option<ResourceModule> {
    match task {
        Task::Runtime { scenario, .. } => scenario.resource_group(),
        _ => None,
    }
}

pub(super) fn prepare(
    root: &Path,
    options: &Options,
    tasks: &[Task],
) -> Result<(Vec<Task>, priority::Selection), String> {
    let mut ordered = tasks.to_vec();
    if !options.resource_resume {
        return Ok((ordered, priority::Selection::default()));
    }
    let groups: Vec<_> = tasks.iter().filter_map(group).collect();
    let units = plan::units(
        &groups,
        std::env::var("STICKYMD_SMOKE_RESOURCE_CASE")
            .ok()
            .as_deref(),
    )?;
    let selection = priority::select(root, &units, options.resource_failure_first);
    if let Some(first) = selection.first.and_then(|unit| unit.group()) {
        reorder(&mut ordered, first)?;
    }
    Ok((ordered, selection))
}

fn reorder(tasks: &mut [Task], first: ResourceModule) -> Result<(), String> {
    let Some(start) = tasks.iter().position(|task| group(task).is_some()) else {
        return Ok(());
    };
    let end = tasks
        .iter()
        .rposition(|task| group(task).is_some())
        .unwrap();
    if tasks[start..=end].iter().any(|task| group(task).is_none()) {
        return Err(
            "failure-first requires contiguous resource tasks after their prerequisites".into(),
        );
    }
    tasks[start..=end].sort_by_key(|task| group(task) != Some(first));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn failure_priority_preserves_prerequisites_scope_and_every_resource_task() {
        let options = Options::parse(["phase", "14", "--resources"].map(str::to_owned)).unwrap();
        let original = super::super::build_plan(&options).unwrap();
        for first in crate::resource_plan::GROUPS {
            let mut ordered = original.clone();
            reorder(&mut ordered, first).unwrap();
            let index = original
                .iter()
                .position(|task| group(task).is_some())
                .unwrap();
            assert_eq!(ordered[..index], original[..index]);
            assert_eq!(group(&ordered[index]), Some(first));
            let mut remaining = ordered;
            remaining.retain(|task| group(task) != Some(first));
            assert_eq!(
                remaining,
                original
                    .iter()
                    .filter(|task| group(task) != Some(first))
                    .cloned()
                    .collect::<Vec<_>>()
            );
        }
        let mut interleaved = original;
        interleaved.insert(4, Task::Governance);
        let unchanged = interleaved.clone();
        assert!(reorder(&mut interleaved, ResourceModule::Zoom).is_err());
        assert_eq!(interleaved, unchanged);
    }
}
