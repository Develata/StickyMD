//! Deduplicated task planning and subprocess execution.

use std::path::Path;
use std::process::Command;

use crate::cli::{Options, Phase, Selection};
use crate::governance;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum TaskId {
    Governance,
    WorkspaceCheck,
    Phase1MarkdownMathTests,
    Phase1PersistenceTests,
    WorkspaceTests,
    CoreTests,
    RenderWinTests,
    CoreWinTests,
    Phase1MarkdownMathPerformance,
    Phase1PersistencePerformance,
    Phase2Performance,
    Phase3Performance,
    Phase4Performance,
    Phase5PreviewTests,
    Phase5Performance,
    Phase6MathTests,
    Phase6Performance,
    ReleaseBuild,
    RuntimeLaunch,
    RuntimePortable,
    RuntimePreview,
    RuntimeMath,
    RuntimeResources,
    RuntimeMathResources,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Task {
    Governance,
    Cargo {
        id: TaskId,
        label: &'static str,
        args: Vec<&'static str>,
    },
    Runtime {
        id: TaskId,
        scenario: RuntimeScenario,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeScenario {
    Launch,
    Portable,
    Preview,
    Math,
    Resources,
    MathResources,
}

impl Task {
    const fn id(&self) -> TaskId {
        match self {
            Self::Governance => TaskId::Governance,
            Self::Cargo { id, .. } | Self::Runtime { id, .. } => *id,
        }
    }
}

pub(crate) fn execute(root: &Path, options: &Options) -> Result<(), String> {
    let tasks = build_plan(options)?;
    let label = match options.selection {
        Selection::Phase(phase) => format!("phase-{}", phase.number()),
        Selection::All => "all".to_owned(),
    };
    println!(
        "StickyMD smoke: selection={label} mode={} tasks={}",
        if options.ci { "ci" } else { "local" },
        tasks.len()
    );

    for (index, task) in tasks.iter().enumerate() {
        println!("[{}/{}] {}", index + 1, tasks.len(), task_label(task));
        run_task(root, task)?;
    }
    governance::verify_ready_status(root, options.selection)?;
    println!("StickyMD smoke PASS: {label}");
    Ok(())
}

fn task_label(task: &Task) -> &'static str {
    match task {
        Task::Governance => "governance contracts",
        Task::Cargo { label, .. } => label,
        Task::Runtime {
            scenario: RuntimeScenario::Launch,
            ..
        } => "copied Release native-shell launch",
        Task::Runtime {
            scenario: RuntimeScenario::Portable,
            ..
        } => "copied Release portable instance lifecycle",
        Task::Runtime {
            scenario: RuntimeScenario::Preview,
            ..
        } => "copied Release Preview/Split lifecycle",
        Task::Runtime {
            scenario: RuntimeScenario::Math,
            ..
        } => "copied Release RaTeX Preview/Split lifecycle",
        Task::Runtime {
            scenario: RuntimeScenario::Resources,
            ..
        } => "copied Release Source/Preview/Split resource measurement",
        Task::Runtime {
            scenario: RuntimeScenario::MathResources,
            ..
        } => "copied Release Phase 6 math resource matrix",
    }
}

fn run_task(root: &Path, task: &Task) -> Result<(), String> {
    match task {
        Task::Governance => governance::verify(root),
        Task::Cargo { label, args, .. } => {
            let status = Command::new("cargo")
                .args(args)
                .current_dir(root)
                .status()
                .map_err(|error| format!("cannot start `{label}`: {error}"))?;
            if status.success() {
                Ok(())
            } else {
                Err(format!("`{label}` failed with {status}"))
            }
        }
        Task::Runtime { scenario, .. } => run_runtime(root, *scenario),
    }
}

#[cfg(windows)]
fn run_runtime(root: &Path, scenario: RuntimeScenario) -> Result<(), String> {
    crate::runtime::run(root, scenario)
}

#[cfg(not(windows))]
fn run_runtime(_root: &Path, _scenario: RuntimeScenario) -> Result<(), String> {
    Err("runtime smoke requires Windows".to_owned())
}

fn build_plan(options: &Options) -> Result<Vec<Task>, String> {
    let mut tasks = Vec::new();
    push_unique(&mut tasks, governance());
    if options.resources {
        push_unique(&mut tasks, release_build());
        if matches!(
            options.selection,
            Selection::Phase(Phase::P06) | Selection::All
        ) {
            push_unique(&mut tasks, runtime_math_resources());
        } else {
            push_unique(&mut tasks, runtime_resources());
        }
        return Ok(tasks);
    }
    match options.selection {
        Selection::All => {
            push_unique(&mut tasks, phase1_markdown_tests());
            push_unique(&mut tasks, phase1_persistence_tests());
            push_unique(&mut tasks, workspace_tests());
        }
        Selection::Phase(Phase::P00) => {}
        Selection::Phase(Phase::P01) => {
            push_unique(&mut tasks, workspace_check());
            push_unique(&mut tasks, phase1_markdown_tests());
            push_unique(&mut tasks, phase1_persistence_tests());
        }
        Selection::Phase(Phase::P02) => push_unique(&mut tasks, core_tests()),
        Selection::Phase(Phase::P03) => push_unique(&mut tasks, render_win_tests()),
        Selection::Phase(Phase::P04) => push_unique(&mut tasks, core_win_tests()),
        Selection::Phase(Phase::P05) => push_unique(&mut tasks, phase5_preview_tests()),
        Selection::Phase(Phase::P06) => push_unique(&mut tasks, phase6_math_tests()),
    }

    // CI owns every headless task. `--performance` remains the explicit local
    // spelling, while CI runs the same Release baselines without treating
    // machine-specific measurements as portable receipts.
    if options.performance || options.ci {
        for phase in selected_phases(options.selection) {
            match phase {
                Phase::P00 => {}
                Phase::P01 => {
                    push_unique(&mut tasks, phase1_markdown_performance());
                    push_unique(&mut tasks, phase1_persistence_performance());
                }
                Phase::P02 => push_unique(&mut tasks, phase2_performance()),
                Phase::P03 => push_unique(&mut tasks, phase3_performance()),
                Phase::P04 => push_unique(&mut tasks, phase4_performance()),
                Phase::P05 => push_unique(&mut tasks, phase5_performance()),
                Phase::P06 => push_unique(&mut tasks, phase6_performance()),
            }
        }
    }

    if options.runtime {
        push_unique(&mut tasks, release_build());
        match options.selection {
            Selection::Phase(Phase::P03) => push_unique(&mut tasks, runtime_launch()),
            Selection::Phase(Phase::P04) => push_unique(&mut tasks, runtime_portable()),
            Selection::Phase(Phase::P05) => push_unique(&mut tasks, runtime_preview()),
            Selection::Phase(Phase::P06) => push_unique(&mut tasks, runtime_math()),
            Selection::All => {
                push_unique(&mut tasks, runtime_portable());
                push_unique(&mut tasks, runtime_preview());
                push_unique(&mut tasks, runtime_math());
            }
            Selection::Phase(Phase::P00 | Phase::P01 | Phase::P02) => {
                return Err("selected phase has no runtime smoke".to_owned());
            }
        }
    }
    Ok(tasks)
}

fn selected_phases(selection: Selection) -> Vec<Phase> {
    match selection {
        Selection::Phase(phase) => vec![phase],
        Selection::All => Phase::ALL.to_vec(),
    }
}

fn push_unique(tasks: &mut Vec<Task>, task: Task) {
    if !tasks.iter().any(|existing| existing.id() == task.id()) {
        tasks.push(task);
    }
}

const fn governance() -> Task {
    Task::Governance
}

fn cargo(id: TaskId, label: &'static str, args: &[&'static str]) -> Task {
    Task::Cargo {
        id,
        label,
        args: args.to_vec(),
    }
}

fn workspace_check() -> Task {
    cargo(
        TaskId::WorkspaceCheck,
        "workspace check",
        &["check", "--workspace", "--locked"],
    )
}

fn phase1_markdown_tests() -> Task {
    cargo(
        TaskId::Phase1MarkdownMathTests,
        "Phase 1 Markdown/Math tests",
        &[
            "test",
            "--manifest-path",
            "experiments/phase-01/markdown-math/Cargo.toml",
            "--locked",
        ],
    )
}

fn phase1_persistence_tests() -> Task {
    cargo(
        TaskId::Phase1PersistenceTests,
        "Phase 1 persistence tests",
        &[
            "test",
            "--manifest-path",
            "experiments/phase-01/persistence/Cargo.toml",
            "--locked",
        ],
    )
}

fn workspace_tests() -> Task {
    cargo(
        TaskId::WorkspaceTests,
        "workspace tests",
        &["test", "--workspace", "--locked"],
    )
}

fn core_tests() -> Task {
    cargo(
        TaskId::CoreTests,
        "Phase 2 core tests",
        &["test", "-p", "stickymd-core", "--locked"],
    )
}

fn render_win_tests() -> Task {
    cargo(
        TaskId::RenderWinTests,
        "Phase 3 render/app tests",
        &[
            "test",
            "-p",
            "stickymd-render",
            "-p",
            "stickymd-win",
            "--locked",
        ],
    )
}

fn core_win_tests() -> Task {
    cargo(
        TaskId::CoreWinTests,
        "Phase 4 core/app tests",
        &[
            "test",
            "-p",
            "stickymd-core",
            "-p",
            "stickymd-win",
            "--locked",
        ],
    )
}

fn phase5_preview_tests() -> Task {
    cargo(
        TaskId::Phase5PreviewTests,
        "Phase 5 semantic/native-preview tests",
        &[
            "test",
            "-p",
            "stickymd-render",
            "-p",
            "stickymd-win",
            "--locked",
        ],
    )
}

fn phase6_math_tests() -> Task {
    cargo(
        TaskId::Phase6MathTests,
        "Phase 6 RaTeX/native-math tests",
        &[
            "test",
            "-p",
            "stickymd-render",
            "-p",
            "stickymd-win",
            "--locked",
        ],
    )
}

fn phase1_markdown_performance() -> Task {
    cargo(
        TaskId::Phase1MarkdownMathPerformance,
        "Phase 1 Markdown/Math Release measurement",
        &[
            "run",
            "--release",
            "--manifest-path",
            "experiments/phase-01/markdown-math/Cargo.toml",
            "--locked",
        ],
    )
}

fn phase1_persistence_performance() -> Task {
    cargo(
        TaskId::Phase1PersistencePerformance,
        "Phase 1 persistence Release smoke",
        &[
            "run",
            "--release",
            "--manifest-path",
            "experiments/phase-01/persistence/Cargo.toml",
            "--locked",
        ],
    )
}

fn phase2_performance() -> Task {
    cargo(
        TaskId::Phase2Performance,
        "Phase 2 core Release baseline",
        &[
            "bench",
            "-p",
            "stickymd-core",
            "--bench",
            "release_baseline",
            "--locked",
        ],
    )
}

fn phase3_performance() -> Task {
    cargo(
        TaskId::Phase3Performance,
        "Phase 3 source-pipeline Release baseline",
        &[
            "test",
            "-p",
            "stickymd-win",
            "--release",
            "--locked",
            "phase3_source_pipeline_release_baseline",
            "--",
            "--ignored",
            "--nocapture",
        ],
    )
}

fn phase4_performance() -> Task {
    cargo(
        TaskId::Phase4Performance,
        "Phase 4 persistence Release baseline",
        &[
            "test",
            "-p",
            "stickymd-win",
            "--release",
            "--locked",
            "phase4_persistence_release_baseline",
            "--",
            "--ignored",
            "--nocapture",
        ],
    )
}

fn phase5_performance() -> Task {
    cargo(
        TaskId::Phase5Performance,
        "Phase 5 native-preview Release baseline",
        &[
            "test",
            "-p",
            "stickymd-render",
            "--release",
            "--locked",
            "phase5_preview_release_baseline",
            "--",
            "--ignored",
            "--nocapture",
        ],
    )
}

fn phase6_performance() -> Task {
    cargo(
        TaskId::Phase6Performance,
        "Phase 6 native-math Release baseline",
        &[
            "test",
            "--workspace",
            "--release",
            "--locked",
            "phase6_",
            "--",
            "--ignored",
            "--nocapture",
        ],
    )
}

fn release_build() -> Task {
    cargo(
        TaskId::ReleaseBuild,
        "Release Windows app build",
        &["build", "-p", "stickymd-win", "--release", "--locked"],
    )
}

const fn runtime_launch() -> Task {
    Task::Runtime {
        id: TaskId::RuntimeLaunch,
        scenario: RuntimeScenario::Launch,
    }
}

const fn runtime_portable() -> Task {
    Task::Runtime {
        id: TaskId::RuntimePortable,
        scenario: RuntimeScenario::Portable,
    }
}

const fn runtime_preview() -> Task {
    Task::Runtime {
        id: TaskId::RuntimePreview,
        scenario: RuntimeScenario::Preview,
    }
}

const fn runtime_math() -> Task {
    Task::Runtime {
        id: TaskId::RuntimeMath,
        scenario: RuntimeScenario::Math,
    }
}

const fn runtime_resources() -> Task {
    Task::Runtime {
        id: TaskId::RuntimeResources,
        scenario: RuntimeScenario::Resources,
    }
}

const fn runtime_math_resources() -> Task {
    Task::Runtime {
        id: TaskId::RuntimeMathResources,
        scenario: RuntimeScenario::MathResources,
    }
}

#[cfg(test)]
mod tests {
    use super::{TaskId, build_plan};
    use crate::cli::{Options, Selection};

    #[test]
    fn all_plan_uses_one_consolidated_workspace_test() {
        let tasks = build_plan(&Options {
            selection: Selection::All,
            ci: true,
            performance: false,
            runtime: false,
            resources: false,
        })
        .expect("valid all plan");
        let ids: Vec<_> = tasks.iter().map(super::Task::id).collect();
        assert_eq!(
            ids.iter()
                .filter(|id| **id == TaskId::WorkspaceTests)
                .count(),
            1
        );
        assert!(!ids.contains(&TaskId::CoreTests));
        assert!(!ids.contains(&TaskId::RenderWinTests));
        assert!(!ids.contains(&TaskId::CoreWinTests));
    }

    #[test]
    fn all_performance_plan_contains_each_measurement_once() {
        let tasks = build_plan(&Options {
            selection: Selection::All,
            ci: false,
            performance: true,
            runtime: false,
            resources: false,
        })
        .expect("valid performance plan");
        let ids: BTreeSet<_> = tasks.iter().map(super::Task::id).collect();
        assert_eq!(ids.len(), tasks.len());
        assert!(ids.contains(&TaskId::Phase2Performance));
        assert!(ids.contains(&TaskId::Phase3Performance));
        assert!(ids.contains(&TaskId::Phase4Performance));
        assert!(ids.contains(&TaskId::Phase5Performance));
        assert!(ids.contains(&TaskId::Phase6Performance));
    }

    #[test]
    fn ci_plan_includes_every_headless_performance_task_but_no_runtime_task() {
        let tasks = build_plan(&Options {
            selection: Selection::All,
            ci: true,
            performance: false,
            runtime: false,
            resources: false,
        })
        .expect("valid CI plan");
        let ids: BTreeSet<_> = tasks.iter().map(super::Task::id).collect();
        for expected in [
            TaskId::Phase1MarkdownMathPerformance,
            TaskId::Phase1PersistencePerformance,
            TaskId::Phase2Performance,
            TaskId::Phase3Performance,
            TaskId::Phase4Performance,
            TaskId::Phase5Performance,
            TaskId::Phase6Performance,
        ] {
            assert!(ids.contains(&expected));
        }
        assert!(!ids.contains(&TaskId::RuntimeLaunch));
        assert!(!ids.contains(&TaskId::RuntimePortable));
        assert!(!ids.contains(&TaskId::RuntimePreview));
        assert!(!ids.contains(&TaskId::RuntimeMath));
        assert!(!ids.contains(&TaskId::RuntimeResources));
        assert!(!ids.contains(&TaskId::RuntimeMathResources));
    }

    #[test]
    fn phase6_resources_select_the_math_matrix() {
        let tasks = build_plan(&Options {
            selection: Selection::Phase(crate::cli::Phase::P06),
            ci: false,
            performance: false,
            runtime: false,
            resources: true,
        })
        .expect("valid Phase 6 resource plan");
        let ids: BTreeSet<_> = tasks.iter().map(super::Task::id).collect();
        assert!(ids.contains(&TaskId::RuntimeMathResources));
        assert!(!ids.contains(&TaskId::RuntimeResources));
    }

    use std::collections::BTreeSet;
}
