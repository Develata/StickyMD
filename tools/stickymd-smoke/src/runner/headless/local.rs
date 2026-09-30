//! Local development checks projected from the shared headless task graph.
//! plan_ref: docs/plan/11_testing_and_release.md#modular-headless-ci

use std::{path::Path, time::Instant};

use super::{Module, Request, Task, TaskExecution, owners, plan, run_task, task_label};
use crate::{ci::Checks, headless::Mode};

pub(crate) struct Description {
    pub(crate) label: &'static str,
    pub(crate) program: Option<&'static str>,
    pub(crate) args: Vec<&'static str>,
    pub(crate) modules: Vec<Module>,
    pub(crate) reason: &'static str,
}

/// An in-memory execution plan, never a qualification or last-success receipt.
pub(crate) struct LocalPlan {
    tasks: Vec<Task>,
    windows_required: bool,
}

impl LocalPlan {
    pub(crate) fn new(request: &Request, full: bool, checks: &Checks) -> Result<Self, String> {
        let mut tasks = vec![Task::Governance, super::super::format_check()];
        if checks.quality {
            let mut task = super::super::clippy_check();
            if let Task::Cargo { args, .. } = &mut task {
                // Preserve strict flags while ensuring Cargo cannot update the lockfile.
                let boundary = args.iter().position(|arg| *arg == "--").unwrap();
                args.insert(boundary, "--locked");
            }
            if full {
                tasks.push(task);
            } else if let Some(task) = super::project(task, &request.modules)? {
                tasks.push(task);
            }
        }
        if checks.dependency {
            let mut task = super::super::dependency_policy();
            if let Task::Cargo { args, .. } = &mut task {
                args.insert(1, "--locked");
            }
            tasks.push(task);
        }
        let selected = if full {
            // Registry drift must retain Cargo's actual workspace, including new crates.
            // Explicit `modules run` still rejects drift; the conservative local fallback
            // instead uses the original full graph, with its workspace Cargo selectors.
            let mut args = vec!["all".to_owned(), "--ci".to_owned()];
            match request.mode {
                Mode::Tests => args.push("--ci-shard=tests".to_owned()),
                Mode::Performance => args.push("--ci-shard=performance".to_owned()),
                Mode::All => {}
            }
            super::build_plan(&crate::cli::Options::parse(args)?)?
        } else {
            plan(request)?
        };
        for task in selected {
            super::super::push_unique(&mut tasks, task);
        }
        if checks.release {
            let mut task = super::super::release_build();
            if let Task::Cargo { args, .. } = &mut task {
                if full {
                    *args = vec!["build", "--workspace", "--release", "--locked"];
                }
                args.push("--message-format=json-render-diagnostics");
            }
            tasks.push(task);
            tasks.push(super::super::verify_native_runtime_dependencies());
        }
        Ok(Self {
            tasks,
            windows_required: request.modules.contains(&Module::Windows),
        })
    }

    pub(crate) fn describe(&self) -> Result<Vec<Description>, String> {
        self.tasks.iter().map(describe).collect()
    }

    pub(crate) fn execute(&self, root: &Path) -> Result<(), String> {
        #[cfg(not(windows))]
        if self.windows_required {
            return Err("NOT_TESTED: selected Windows checks require a Windows host".to_owned());
        }
        #[cfg(windows)]
        let _ = self.windows_required;
        let mut executable = None;
        for (index, task) in self.tasks.iter().enumerate() {
            println!("[{}/{}] {}", index + 1, self.tasks.len(), task_label(task));
            let started = Instant::now();
            let outcome = match task {
                Task::Cargo {
                    id: super::super::TaskId::ReleaseBuild,
                    args,
                    ..
                } => super::local_build::run(root, args).map(|path| {
                    executable = Some(path);
                    TaskExecution::Passed(super::super::TaskEvidence::default())
                }),
                Task::NativeRuntimeDependencies => executable
                    .as_deref()
                    .ok_or_else(|| {
                        "local native-runtime gate requires this run's successful build artifact"
                            .to_owned()
                    })
                    .and_then(crate::pe_dependencies::verify_portable_executable)
                    .map(|_| TaskExecution::Passed(super::super::TaskEvidence::default())),
                _ => run_task(root, task, false),
            };
            super::super::timing::record(
                task_label(task),
                started.elapsed(),
                &mut [],
                matches!(&outcome, Ok(TaskExecution::Passed(_))),
            );
            match outcome? {
                TaskExecution::Passed(_) => {}
                #[cfg(windows)]
                TaskExecution::Failed { detail, .. } => return Err(detail),
                #[cfg(not(windows))]
                TaskExecution::NotTested(detail) => return Err(format!("NOT_TESTED: {detail}")),
            }
        }
        Ok(())
    }
}

fn describe(task: &Task) -> Result<Description, String> {
    use super::super::TaskId;
    let (program, args, modules, reason) = match task {
        Task::Governance => (None, vec![], vec![], "shared governance is always required"),
        Task::NativeRuntimeDependencies => (
            None,
            vec![],
            vec![Module::Windows],
            "selected Windows build requires its native-runtime gate",
        ),
        Task::Cargo { id, args, .. } => match id {
            TaskId::FormatCheck => (
                Some("cargo"),
                args.clone(),
                vec![],
                "shared formatting is always required",
            ),
            TaskId::DependencyPolicy => (
                Some("cargo"),
                args.clone(),
                vec![],
                "selected code or conservative fallback requires dependency policy",
            ),
            _ => (
                Some("cargo"),
                args.clone(),
                owners(args)?,
                "selected modules and their reverse dependencies",
            ),
        },
        _ => return Err("local development plan contains a non-headless task".to_owned()),
    };
    Ok(Description {
        label: task_label(task),
        program,
        args,
        modules,
        reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn local(modules: &[Module], full: bool, mode: Mode) -> LocalPlan {
        LocalPlan::new(
            &Request {
                modules: modules.to_vec(),
                mode,
                plan_only: true,
            },
            full,
            &Checks::for_modules(full, modules),
        )
        .unwrap()
    }

    #[test]
    fn local_documentation_retains_only_shared_formatting_and_governance() {
        let descriptions = local(&[], false, Mode::Tests).describe().unwrap();
        assert_eq!(descriptions.len(), 2);
        assert!(descriptions[0].program.is_none());
        assert_eq!(descriptions[1].args, ["fmt", "--check"]);
    }

    #[test]
    fn local_selection_deduplicates_tests_and_keeps_locked_strict_commands() {
        let descriptions = local(
            &[Module::Core, Module::Render, Module::Windows],
            false,
            Mode::Tests,
        )
        .describe()
        .unwrap();
        let tests: Vec<_> = descriptions
            .iter()
            .filter(|check| check.args.first() == Some(&"test"))
            .collect();
        assert_eq!(tests.len(), 1);
        assert!(!tests[0].args.contains(&"--workspace"));
        assert!(!tests[0].modules.contains(&Module::Smoke));
        for check in descriptions
            .iter()
            .filter(|check| matches!(check.args.first(), Some(&"test" | &"build" | &"clippy")))
        {
            assert!(check.args.contains(&"--locked"), "{}", check.label);
        }
        assert_eq!(descriptions.last().unwrap().modules, [Module::Windows]);
    }

    #[test]
    fn local_full_fallback_preserves_workspace_commands_and_serial_performance() {
        let descriptions = local(&Module::ALL, true, Mode::All).describe().unwrap();
        for program in ["clippy", "test", "build"] {
            assert!(
                descriptions
                    .iter()
                    .any(|check| check.args.first() == Some(&program)
                        && check.args.contains(&"--workspace"))
            );
        }
        let tasks = &local(&[Module::Render], false, Mode::Performance).tasks;
        for task in tasks {
            if let Task::Cargo { args, .. } = task
                && args.first() == Some(&"test")
            {
                assert!(args.contains(&"--ignored"));
                assert!(args.contains(&"--test-threads=1"));
            }
        }
        assert!(
            !tasks
                .iter()
                .any(|task| matches!(task, Task::Runtime { .. }))
        );
    }
}
