//! Narrow the existing full headless plan at Cargo package boundaries.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

use std::collections::BTreeSet;
use std::path::Path;

use super::{Task, TaskExecution, build_plan, run_task, task_label};
use crate::cli::Options;
use crate::headless::{Command, Mode, Module, Request};

pub(crate) fn execute(root: &Path, command: &Command) -> Result<(), String> {
    let Command::Run(request) = command else {
        for module in Module::ALL {
            println!(
                "{}: {}",
                module.name(),
                module.package().or(module.manifest()).unwrap()
            );
        }
        return Ok(());
    };
    verify_workspace(root)?;
    let tasks = plan(request)?;
    if request.plan_only {
        println!("{}", plan_json(request, &tasks));
        return Ok(());
    }
    #[cfg(not(windows))]
    if request.modules.contains(&Module::Windows) {
        return Err("NOT_TESTED: the windows module requires a Windows host".to_owned());
    }
    let scope = request
        .modules
        .iter()
        .map(|module| module.name())
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "StickyMD selected headless: modules={scope} mode={} tasks={}",
        request.mode.name(),
        tasks.len()
    );
    for (index, task) in tasks.iter().enumerate() {
        println!("[{}/{}] {}", index + 1, tasks.len(), task_label(task));
        match run_task(root, task, false)? {
            TaskExecution::Passed(_) => {}
            #[cfg(windows)]
            TaskExecution::Failed { detail, .. } => return Err(detail),
            #[cfg(not(windows))]
            TaskExecution::NotTested(detail) => return Err(format!("NOT_TESTED: {detail}")),
        }
    }
    println!(
        "StickyMD selected headless PASS: modules={scope} mode={}; no qualification receipt written",
        request.mode.name()
    );
    Ok(())
}

fn verify_workspace(root: &Path) -> Result<(), String> {
    // Cargo owns workspace membership; do not approximate TOML or wildcard membership.
    let listing = crate::repository::command_text(
        root,
        "cargo",
        &[
            "tree",
            "--workspace",
            "--depth",
            "0",
            "--prefix",
            "none",
            "--format",
            "{p}",
            "--locked",
            "--color",
            "never",
        ],
    )?;
    verify_workspace_listing(&listing)
}

fn verify_workspace_listing(listing: &str) -> Result<(), String> {
    let observed: BTreeSet<_> = listing
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .collect();
    let registered: BTreeSet<_> = Module::ALL
        .into_iter()
        .filter_map(Module::package)
        .collect();
    if observed != registered {
        return Err(format!(
            "headless module registry does not cover the Cargo workspace: observed={observed:?}, registered={registered:?}; update the registry or use `all --ci`"
        ));
    }
    Ok(())
}

fn plan(request: &Request) -> Result<Vec<Task>, String> {
    let mut args = vec!["all".to_owned(), "--ci".to_owned()];
    match request.mode {
        Mode::Tests => args.push("--ci-shard=tests".to_owned()),
        Mode::Performance => args.push("--ci-shard=performance".to_owned()),
        Mode::All => {}
    }
    build_plan(&Options::parse(args)?)?
        .into_iter()
        .filter_map(|task| project(task, &request.modules).transpose())
        .collect()
}

fn project(task: Task, selected: &[Module]) -> Result<Option<Task>, String> {
    let Task::Cargo { id, label, args } = task else {
        return match task {
            Task::Governance => Ok(Some(task)),
            _ => Err("full headless plan contains an unsupported non-Cargo task".to_owned()),
        };
    };
    let owners = owners(&args)?;
    let retained: Vec<_> = owners
        .iter()
        .copied()
        .filter(|module| selected.contains(module))
        .collect();
    if retained.is_empty() {
        return Ok(None);
    }
    if retained == owners {
        return Ok(Some(Task::Cargo { id, label, args }));
    }
    // Preserve every filter/profile/harness argument. Only replace the package selector.
    let mut narrowed = Vec::with_capacity(args.len() + retained.len() * 2);
    let mut index = 0;
    while index < args.len() {
        match args[index] {
            "--workspace" => index += 1,
            "-p" => index += 2,
            "--" => {
                break;
            }
            value => {
                narrowed.push(value);
                index += 1;
            }
        }
    }
    for module in retained {
        narrowed.extend([
            "-p",
            module
                .package()
                .ok_or("cannot narrow an experiment manifest")?,
        ]);
    }
    narrowed.extend_from_slice(&args[index..]);
    Ok(Some(Task::Cargo {
        id,
        label: if id == super::TaskId::WorkspaceTests {
            "selected package tests"
        } else {
            label
        },
        args: narrowed,
    }))
}

fn owners(args: &[&str]) -> Result<Vec<Module>, String> {
    let mut owners = BTreeSet::new();
    let mut selectors = BTreeSet::new();
    let mut index = 0;
    while index < args.len() && args[index] != "--" {
        let selector = args[index];
        match selector {
            "--workspace" => {
                owners.extend(
                    Module::ALL
                        .into_iter()
                        .filter(|module| module.package().is_some()),
                );
                selectors.insert(selector);
            }
            "-p" | "--manifest-path" => {
                index += 1;
                let value = args
                    .get(index)
                    .ok_or("missing Cargo package selector value")?;
                let module = Module::ALL
                    .into_iter()
                    .find(|module| {
                        (if selector == "-p" {
                            module.package()
                        } else {
                            module.manifest()
                        }) == Some(*value)
                    })
                    .ok_or_else(|| format!("unregistered Cargo target `{value}`"))?;
                owners.insert(module);
                selectors.insert(selector);
            }
            value
                if value.starts_with("--package")
                    || value.starts_with("--exclude")
                    || value.starts_with("--manifest-path=")
                    || value.starts_with("-p")
                    || value.starts_with("--workspace=")
                    || value == "--all" =>
            {
                return Err(format!(
                    "unsupported Cargo selector `{value}` in headless plan"
                ));
            }
            _ => {}
        }
        index += 1;
    }
    if owners.is_empty() || selectors.len() != 1 {
        return Err("headless Cargo task must have one known kind of package selector".to_owned());
    }
    Ok(owners.into_iter().collect())
}

fn plan_json(request: &Request, tasks: &[Task]) -> String {
    let quoted = |value: &str| format!("\"{}\"", crate::evidence::escape_json(value));
    let modules = request
        .modules
        .iter()
        .map(|module| quoted(module.name()))
        .collect::<Vec<_>>()
        .join(",");
    let commands = tasks
        .iter()
        .map(|task| {
            let args = match task {
                Task::Cargo { args, .. } => args
                    .iter()
                    .map(|arg| quoted(arg))
                    .collect::<Vec<_>>()
                    .join(","),
                _ => String::new(),
            };
            format!(
                "{{\"label\":{},\"program\":{},\"args\":[{args}]}}",
                quoted(task_label(task)),
                if matches!(task, Task::Cargo { .. }) {
                    "\"cargo\""
                } else {
                    "null"
                }
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema_version\":1,\"kind\":\"selected-headless-plan\",\"status\":\"NOT_RUN\",\"modules\":[{modules}],\"mode\":\"{}\",\"tasks\":[{commands}]}}",
        request.mode.name()
    )
}

#[cfg(test)]
mod tests;
