//! Explain the Rust-owned local check plan without creating success evidence.
//! plan_ref: docs/plan/11_testing_and_release.md#modular-headless-ci

use std::collections::BTreeSet;

use super::{Command, Plan};
use crate::{ci::selection::Selection, runner::headless::Description};

fn quote(value: &str) -> String {
    format!("\"{}\"", crate::evidence::escape_json(value))
}

fn array(values: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    format!(
        "[{}]",
        values
            .into_iter()
            .map(|value| quote(value.as_ref()))
            .collect::<Vec<_>>()
            .join(",")
    )
}

pub(super) fn task_reasons(selection: &Selection, check: &Description) -> Vec<String> {
    let mut reasons = BTreeSet::from([check.reason.to_owned()]);
    if selection.full {
        reasons.extend(selection.reasons.iter().cloned());
    } else {
        for module in &check.modules {
            reasons.extend(
                selection
                    .reasons_for(*module)
                    .into_iter()
                    .map(str::to_owned),
            );
        }
    }
    reasons.into_iter().collect()
}

pub(super) fn json(plan: &Plan, command: &Command) -> Result<String, String> {
    let modules = array(plan.selection.modules.iter().map(|module| module.name()));
    let module_reasons = plan
        .selection
        .modules
        .iter()
        .map(|module| {
            format!(
                "{{\"module\":{},\"reasons\":{}}}",
                quote(module.name()),
                array(plan.selection.reasons_for(*module))
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let tasks = plan
        .tasks
        .describe()?
        .iter()
        .map(|check| {
            format!(
                "{{\"label\":{},\"program\":{},\"args\":{},\"modules\":{},\"reasons\":{}}}",
                quote(check.label),
                check
                    .program
                    .map(quote)
                    .unwrap_or_else(|| "null".to_owned()),
                array(&check.args),
                array(check.modules.iter().map(|module| module.name())),
                array(task_reasons(&plan.selection, check)),
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    let paths = plan
        .facts
        .paths
        .as_ref()
        .map(array)
        .unwrap_or_else(|_| "null".to_owned());
    let dirty = plan
        .facts
        .paths
        .as_ref()
        .map(|paths| (!paths.is_empty()).to_string())
        .unwrap_or_else(|_| "null".to_owned());
    let checks = &plan.checks;
    Ok(format!(
        "{{\"schema_version\":1,\"kind\":\"local-development-plan\",\"status\":\"NOT_RUN\",\"head\":{},\"comparison\":\"HEAD plus index/worktree/untracked\",\"worktree_dirty\":{dirty},\"paths\":{paths},\"full\":{},\"modules\":{modules},\"mode\":{},\"reasons\":{},\"module_reasons\":[{module_reasons}],\"checks\":{{\"smoke\":{},\"dependency\":{},\"quality\":{},\"headless\":{},\"release\":{},\"portable\":{}}},\"tasks\":[{tasks}],\"execution\":\"serial tasks; Cargo manages compilation and independent tests\",\"qualification_receipt\":false}}",
        plan.facts
            .head
            .as_deref()
            .map(quote)
            .unwrap_or_else(|| "null".to_owned()),
        plan.selection.full,
        quote(command.mode.name()),
        array(&plan.selection.reasons),
        checks.smoke,
        checks.dependency,
        checks.quality,
        checks.headless,
        checks.release,
        checks.portable,
    ))
}
