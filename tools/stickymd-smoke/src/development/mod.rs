//! Conservative checks for staged, unstaged and untracked local development inputs.
//! plan_ref: docs/plan/11_testing_and_release.md#modular-headless-ci

mod git;
mod projection;
#[cfg(test)]
mod tests;

use std::path::Path;

use crate::{
    ci::{
        Checks, registry,
        selection::{self, Selection},
    },
    headless::{Mode, Request},
    runner::headless::LocalPlan,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Command {
    pub(crate) mode: Mode,
    pub(crate) plan_only: bool,
}

impl Command {
    pub(crate) fn parse(args: &[String]) -> Result<Self, String> {
        let mut mode = None;
        let mut plan_only = false;
        for arg in args {
            match arg.as_str() {
                "--plan" if !plan_only => plan_only = true,
                value if value.starts_with("--mode=") && mode.is_none() => {
                    mode = Some(match &value[7..] {
                        "tests" => Mode::Tests,
                        "performance" => Mode::Performance,
                        "all" => Mode::All,
                        _ => return Err(Self::usage()),
                    });
                }
                _ => return Err(Self::usage()),
            }
        }
        Ok(Self {
            mode: mode.unwrap_or(Mode::Tests),
            plan_only,
        })
    }

    fn usage() -> String {
        "usage: stickymd-smoke dev-check [--plan] [--mode=tests|performance|all]".to_owned()
    }
}

struct Plan {
    facts: git::Facts,
    selection: Selection,
    checks: Checks,
    tasks: LocalPlan,
}

fn prepare(
    facts: git::Facts,
    command: &Command,
    verify_registry: impl FnOnce() -> Result<(), String>,
) -> Result<Plan, String> {
    let mut selection = match &facts.paths {
        Ok(paths) => selection::select(paths),
        Err(reason) => Selection::full(format!("local Git observation failed: {reason}")),
    };
    if !selection.full
        && !selection.modules.is_empty()
        && let Err(error) = verify_registry()
    {
        selection = Selection::full(format!("Cargo module registry drift: {error}"));
    }
    let checks = Checks::for_modules(selection.full, &selection.modules);
    let tasks = LocalPlan::new(
        &Request {
            modules: selection.modules.clone(),
            mode: command.mode,
            plan_only: command.plan_only,
        },
        selection.full,
        &checks,
    )?;
    Ok(Plan {
        facts,
        selection,
        checks,
        tasks,
    })
}

pub(crate) fn execute(root: &Path, command: &Command) -> Result<(), String> {
    let plan = prepare(git::inspect(root), command, || registry::verify(root))?;
    if command.plan_only {
        println!("{}", projection::json(&plan, command)?);
        return Ok(());
    }
    println!(
        "StickyMD local development: modules={} mode={} full={}",
        plan.selection
            .modules
            .iter()
            .map(|module| module.name())
            .collect::<Vec<_>>()
            .join(","),
        command.mode.name(),
        plan.selection.full,
    );
    for check in plan.tasks.describe()? {
        let reasons = projection::task_reasons(&plan.selection, &check);
        println!("{}: {}", check.label, reasons.join("; "));
    }
    plan.tasks.execute(root)?;
    println!(
        "StickyMD local development PASS: requested checks only; no qualification receipt written"
    );
    Ok(())
}
