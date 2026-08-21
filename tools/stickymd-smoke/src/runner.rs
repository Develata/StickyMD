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
    Phase7AssetTests,
    Phase7Performance,
    Phase8WindowTests,
    Phase8Performance,
    Phase9ConvergenceTests,
    FormatCheck,
    ClippyCheck,
    DependencyPolicy,
    ReleaseBuild,
    PackageArtifact,
    GenerateSbom,
    VerifyPackage,
    RuntimeLaunch,
    RuntimePortable,
    RuntimePreview,
    RuntimeMath,
    RuntimeAssets,
    RuntimeResources,
    RuntimeMathResources,
    RuntimeImageResources,
    RuntimeWindowShell,
    RuntimeWindowResources,
    RuntimeStartup,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Task {
    Governance,
    Cargo {
        id: TaskId,
        label: &'static str,
        args: Vec<&'static str>,
    },
    PowerShell {
        id: TaskId,
        label: &'static str,
        script: &'static str,
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
    Assets,
    Resources,
    MathResources,
    ImageResources,
    WindowShell,
    WindowResources,
    Startup,
}

impl Task {
    const fn id(&self) -> TaskId {
        match self {
            Self::Governance => TaskId::Governance,
            Self::Cargo { id, .. } | Self::PowerShell { id, .. } | Self::Runtime { id, .. } => *id,
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
        Task::PowerShell { label, .. } => label,
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
            scenario: RuntimeScenario::Assets,
            ..
        } => "copied Release local-image Preview lifecycle",
        Task::Runtime {
            scenario: RuntimeScenario::Resources,
            ..
        } => "copied Release Source/Preview/Split resource measurement",
        Task::Runtime {
            scenario: RuntimeScenario::MathResources,
            ..
        } => "copied Release Phase 6 math resource matrix",
        Task::Runtime {
            scenario: RuntimeScenario::ImageResources,
            ..
        } => "copied Release Phase 7 image resource matrix",
        Task::Runtime {
            scenario: RuntimeScenario::WindowShell,
            ..
        } => "copied Release Phase 8 close-to-tray/show lifecycle",
        Task::Runtime {
            scenario: RuntimeScenario::WindowResources,
            ..
        } => "copied Release Phase 8 hidden-window resource matrix",
        Task::Runtime {
            scenario: RuntimeScenario::Startup,
            ..
        } => "copied Release Phase 9 editor-ready cold/warm startup matrix",
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
        Task::PowerShell {
            label,
            script,
            args,
            ..
        } => {
            let status = Command::new("pwsh")
                .args(["-NoProfile", "-File"])
                .arg(root.join(script))
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
        match options.selection {
            Selection::Phase(Phase::P06) => push_unique(&mut tasks, runtime_math_resources()),
            Selection::Phase(Phase::P07) => push_unique(&mut tasks, runtime_image_resources()),
            Selection::Phase(Phase::P08) => push_unique(&mut tasks, runtime_window_resources()),
            Selection::All => {
                push_unique(&mut tasks, runtime_math_resources());
                push_unique(&mut tasks, runtime_image_resources());
                push_unique(&mut tasks, runtime_window_resources());
            }
            _ => push_unique(&mut tasks, runtime_resources()),
        }
        return Ok(tasks);
    }
    if options.package {
        push_unique(&mut tasks, release_build());
        push_unique(&mut tasks, package_artifact());
        push_unique(&mut tasks, generate_sbom());
        push_unique(&mut tasks, verify_package());
        return Ok(tasks);
    }
    if options.release {
        push_unique(&mut tasks, format_check());
        push_unique(&mut tasks, clippy_check());
        push_unique(&mut tasks, workspace_tests());
        push_unique(&mut tasks, dependency_policy());
        push_unique(&mut tasks, release_build());
        push_unique(&mut tasks, package_artifact());
        push_unique(&mut tasks, generate_sbom());
        push_unique(&mut tasks, verify_package());
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
        Selection::Phase(Phase::P07) => push_unique(&mut tasks, phase7_asset_tests()),
        Selection::Phase(Phase::P08) => push_unique(&mut tasks, phase8_window_tests()),
        Selection::Phase(Phase::P09) => push_unique(&mut tasks, phase9_convergence_tests()),
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
                Phase::P07 => push_unique(&mut tasks, phase7_performance()),
                Phase::P08 => push_unique(&mut tasks, phase8_performance()),
                Phase::P09 => {
                    if options.performance {
                        push_unique(&mut tasks, phase1_markdown_performance());
                        push_unique(&mut tasks, phase1_persistence_performance());
                        push_unique(&mut tasks, phase2_performance());
                        push_unique(&mut tasks, phase3_performance());
                        push_unique(&mut tasks, phase4_performance());
                        push_unique(&mut tasks, phase5_performance());
                        push_unique(&mut tasks, phase6_performance());
                        push_unique(&mut tasks, phase7_performance());
                        push_unique(&mut tasks, phase8_performance());
                    }
                }
            }
        }
    }

    if options.performance && selected_phases(options.selection).contains(&Phase::P09) {
        push_unique(&mut tasks, release_build());
        push_unique(&mut tasks, runtime_startup());
    }

    if options.runtime {
        push_unique(&mut tasks, release_build());
        match options.selection {
            Selection::Phase(Phase::P03) => push_unique(&mut tasks, runtime_launch()),
            Selection::Phase(Phase::P04) => push_unique(&mut tasks, runtime_portable()),
            Selection::Phase(Phase::P05) => push_unique(&mut tasks, runtime_preview()),
            Selection::Phase(Phase::P06) => push_unique(&mut tasks, runtime_math()),
            Selection::Phase(Phase::P07) => push_unique(&mut tasks, runtime_assets()),
            Selection::Phase(Phase::P08) => push_unique(&mut tasks, runtime_window_shell()),
            Selection::Phase(Phase::P09) => {
                push_unique(&mut tasks, runtime_startup());
            }
            Selection::All => {
                push_unique(&mut tasks, runtime_portable());
                push_unique(&mut tasks, runtime_preview());
                push_unique(&mut tasks, runtime_math());
                push_unique(&mut tasks, runtime_assets());
                push_unique(&mut tasks, runtime_window_shell());
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

fn powershell(
    id: TaskId,
    label: &'static str,
    script: &'static str,
    args: &[&'static str],
) -> Task {
    Task::PowerShell {
        id,
        label,
        script,
        args: args.to_vec(),
    }
}

fn format_check() -> Task {
    cargo(
        TaskId::FormatCheck,
        "workspace formatting",
        &["fmt", "--check"],
    )
}

fn clippy_check() -> Task {
    cargo(
        TaskId::ClippyCheck,
        "workspace strict clippy",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )
}

fn dependency_policy() -> Task {
    cargo(
        TaskId::DependencyPolicy,
        "dependency policy",
        &["deny", "check"],
    )
}

fn package_artifact() -> Task {
    powershell(
        TaskId::PackageArtifact,
        "portable package creation",
        "tools/release/package.ps1",
        &[],
    )
}

fn generate_sbom() -> Task {
    powershell(
        TaskId::GenerateSbom,
        "SPDX SBOM generation",
        "tools/release/generate-sbom.ps1",
        &[],
    )
}

fn verify_package() -> Task {
    powershell(
        TaskId::VerifyPackage,
        "portable package verification",
        "tools/release/verify-package.ps1",
        &["-Runtime"],
    )
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

fn phase7_asset_tests() -> Task {
    cargo(
        TaskId::Phase7AssetTests,
        "Phase 7 managed-image/preview/export tests",
        &["test", "--workspace", "--locked"],
    )
}

fn phase8_window_tests() -> Task {
    cargo(
        TaskId::Phase8WindowTests,
        "Phase 8 native-window state/geometry/lifecycle tests",
        &["test", "-p", "stickymd-win", "--locked", "phase8_"],
    )
}

fn phase9_convergence_tests() -> Task {
    cargo(
        TaskId::Phase9ConvergenceTests,
        "Phase 9 full workspace convergence tests",
        &["test", "--workspace", "--locked"],
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

fn phase7_performance() -> Task {
    cargo(
        TaskId::Phase7Performance,
        "Phase 7 image/export Release baseline",
        &[
            "test",
            "--workspace",
            "--release",
            "--locked",
            "phase7_",
            "--",
            "--ignored",
            "--nocapture",
        ],
    )
}

fn phase8_performance() -> Task {
    cargo(
        TaskId::Phase8Performance,
        "Phase 8 native-window Release baseline",
        &[
            "test",
            "-p",
            "stickymd-win",
            "--release",
            "--locked",
            "phase8_",
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

const fn runtime_assets() -> Task {
    Task::Runtime {
        id: TaskId::RuntimeAssets,
        scenario: RuntimeScenario::Assets,
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

const fn runtime_image_resources() -> Task {
    Task::Runtime {
        id: TaskId::RuntimeImageResources,
        scenario: RuntimeScenario::ImageResources,
    }
}

const fn runtime_window_shell() -> Task {
    Task::Runtime {
        id: TaskId::RuntimeWindowShell,
        scenario: RuntimeScenario::WindowShell,
    }
}

const fn runtime_window_resources() -> Task {
    Task::Runtime {
        id: TaskId::RuntimeWindowResources,
        scenario: RuntimeScenario::WindowResources,
    }
}

const fn runtime_startup() -> Task {
    Task::Runtime {
        id: TaskId::RuntimeStartup,
        scenario: RuntimeScenario::Startup,
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
            release: false,
            package: false,
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
            release: false,
            package: false,
        })
        .expect("valid performance plan");
        let ids: BTreeSet<_> = tasks.iter().map(super::Task::id).collect();
        assert_eq!(ids.len(), tasks.len());
        assert!(ids.contains(&TaskId::Phase2Performance));
        assert!(ids.contains(&TaskId::Phase3Performance));
        assert!(ids.contains(&TaskId::Phase4Performance));
        assert!(ids.contains(&TaskId::Phase5Performance));
        assert!(ids.contains(&TaskId::Phase6Performance));
        assert!(ids.contains(&TaskId::Phase7Performance));
        assert!(ids.contains(&TaskId::Phase8Performance));
    }

    #[test]
    fn ci_plan_includes_every_headless_performance_task_but_no_runtime_task() {
        let tasks = build_plan(&Options {
            selection: Selection::All,
            ci: true,
            performance: false,
            runtime: false,
            resources: false,
            release: false,
            package: false,
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
            TaskId::Phase7Performance,
            TaskId::Phase8Performance,
        ] {
            assert!(ids.contains(&expected));
        }
        assert!(!ids.contains(&TaskId::RuntimeLaunch));
        assert!(!ids.contains(&TaskId::RuntimePortable));
        assert!(!ids.contains(&TaskId::RuntimePreview));
        assert!(!ids.contains(&TaskId::RuntimeMath));
        assert!(!ids.contains(&TaskId::RuntimeAssets));
        assert!(!ids.contains(&TaskId::RuntimeResources));
        assert!(!ids.contains(&TaskId::RuntimeMathResources));
        assert!(!ids.contains(&TaskId::RuntimeImageResources));
        assert!(!ids.contains(&TaskId::RuntimeWindowShell));
        assert!(!ids.contains(&TaskId::RuntimeWindowResources));
        assert!(!ids.contains(&TaskId::RuntimeStartup));
    }

    #[test]
    fn phase6_resources_select_the_math_matrix() {
        let tasks = build_plan(&Options {
            selection: Selection::Phase(crate::cli::Phase::P06),
            ci: false,
            performance: false,
            runtime: false,
            resources: true,
            release: false,
            package: false,
        })
        .expect("valid Phase 6 resource plan");
        let ids: BTreeSet<_> = tasks.iter().map(super::Task::id).collect();
        assert!(ids.contains(&TaskId::RuntimeMathResources));
        assert!(!ids.contains(&TaskId::RuntimeResources));
    }

    #[test]
    fn phase7_resources_select_the_image_matrix() {
        let tasks = build_plan(&Options {
            selection: Selection::Phase(crate::cli::Phase::P07),
            ci: false,
            performance: false,
            runtime: false,
            resources: true,
            release: false,
            package: false,
        })
        .expect("valid Phase 7 resource plan");
        let ids: BTreeSet<_> = tasks.iter().map(super::Task::id).collect();
        assert!(ids.contains(&TaskId::RuntimeImageResources));
        assert!(!ids.contains(&TaskId::RuntimeResources));
    }

    #[test]
    fn phase8_plan_routes_headless_runtime_and_resources_explicitly() {
        let headless = build_plan(&Options {
            selection: Selection::Phase(crate::cli::Phase::P08),
            ci: false,
            performance: false,
            runtime: false,
            resources: false,
            release: false,
            package: false,
        })
        .expect("valid Phase 8 headless plan");
        assert!(
            headless
                .iter()
                .any(|task| task.id() == TaskId::Phase8WindowTests)
        );

        let runtime = build_plan(&Options {
            selection: Selection::Phase(crate::cli::Phase::P08),
            ci: false,
            performance: false,
            runtime: true,
            resources: false,
            release: false,
            package: false,
        })
        .expect("valid Phase 8 runtime plan");
        assert!(
            runtime
                .iter()
                .any(|task| task.id() == TaskId::RuntimeWindowShell)
        );

        let resources = build_plan(&Options {
            selection: Selection::Phase(crate::cli::Phase::P08),
            ci: false,
            performance: false,
            runtime: false,
            resources: true,
            release: false,
            package: false,
        })
        .expect("valid Phase 8 resource plan");
        assert!(
            resources
                .iter()
                .any(|task| task.id() == TaskId::RuntimeWindowResources)
        );
        assert!(!resources.iter().any(|task| {
            matches!(
                task.id(),
                TaskId::RuntimeResources
                    | TaskId::RuntimeMathResources
                    | TaskId::RuntimeImageResources
            )
        }));
    }

    #[test]
    fn phase9_performance_runs_existing_baselines_once_then_startup_runtime() {
        let tasks = build_plan(&Options {
            selection: Selection::Phase(crate::cli::Phase::P09),
            ci: false,
            performance: true,
            runtime: false,
            resources: false,
            release: false,
            package: false,
        })
        .expect("valid Phase 9 performance plan");
        let ids: BTreeSet<_> = tasks.iter().map(super::Task::id).collect();
        assert_eq!(ids.len(), tasks.len());
        for expected in [
            TaskId::Phase2Performance,
            TaskId::Phase3Performance,
            TaskId::Phase4Performance,
            TaskId::Phase5Performance,
            TaskId::Phase6Performance,
            TaskId::Phase7Performance,
            TaskId::Phase8Performance,
            TaskId::ReleaseBuild,
            TaskId::RuntimeStartup,
        ] {
            assert!(ids.contains(&expected));
        }
    }

    #[test]
    fn phase9_package_mode_uses_the_checked_in_release_pipeline_once() {
        let tasks = build_plan(&Options {
            selection: Selection::Phase(crate::cli::Phase::P09),
            ci: false,
            performance: false,
            runtime: false,
            resources: false,
            release: false,
            package: true,
        })
        .expect("valid Phase 9 package plan");
        let ids: Vec<_> = tasks.iter().map(super::Task::id).collect();
        assert_eq!(
            ids,
            vec![
                TaskId::Governance,
                TaskId::ReleaseBuild,
                TaskId::PackageArtifact,
                TaskId::GenerateSbom,
                TaskId::VerifyPackage,
            ]
        );
    }

    #[test]
    fn phase9_release_mode_adds_quality_gates_without_runtime_or_manual_work() {
        let tasks = build_plan(&Options {
            selection: Selection::Phase(crate::cli::Phase::P09),
            ci: false,
            performance: false,
            runtime: false,
            resources: false,
            release: true,
            package: false,
        })
        .expect("valid Phase 9 release plan");
        let ids: BTreeSet<_> = tasks.iter().map(super::Task::id).collect();
        for expected in [
            TaskId::FormatCheck,
            TaskId::ClippyCheck,
            TaskId::WorkspaceTests,
            TaskId::DependencyPolicy,
            TaskId::ReleaseBuild,
            TaskId::PackageArtifact,
            TaskId::GenerateSbom,
            TaskId::VerifyPackage,
        ] {
            assert!(ids.contains(&expected));
        }
        assert!(!ids.iter().any(|id| {
            matches!(
                id,
                TaskId::RuntimeLaunch
                    | TaskId::RuntimePortable
                    | TaskId::RuntimePreview
                    | TaskId::RuntimeMath
                    | TaskId::RuntimeAssets
                    | TaskId::RuntimeStartup
            )
        }));
    }

    use std::collections::BTreeSet;
}
