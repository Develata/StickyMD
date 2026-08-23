//! Deduplicated task planning and subprocess execution.

use std::path::Path;
use std::process::Command;

use crate::cli::{Options, Phase, Selection};
use crate::evidence::{
    self, EvidenceGate, EvidenceMeasurement, EvidenceResult, EvidenceSample, EvidenceStatus,
};
use crate::governance;
use crate::qualification_environment::{
    self, QualificationEnvironment, QualificationEnvironmentStatus,
};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum TaskId {
    Governance,
    QualificationEnvironment,
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
    Phase10UxTests,
    Phase10Performance,
    Phase11BTests,
    Phase11BPerformance,
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
    RuntimePhase10,
    RuntimePhase11B,
    RuntimeZoomResources,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Task {
    Governance,
    QualificationEnvironment,
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
    Phase10,
    Phase11B,
    ZoomResources,
}

enum TaskExecution {
    Passed(TaskEvidence),
    Failed {
        detail: String,
        evidence: TaskEvidence,
    },
    #[cfg(not(windows))]
    NotTested(String),
}

#[derive(Default)]
struct TaskEvidence {
    measurements: Vec<EvidenceMeasurement>,
    gates: Vec<EvidenceGate>,
    samples: Vec<EvidenceSample>,
}

impl Task {
    const fn id(&self) -> TaskId {
        match self {
            Self::Governance => TaskId::Governance,
            Self::QualificationEnvironment => TaskId::QualificationEnvironment,
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
    if !options.json {
        println!(
            "StickyMD smoke: selection={label} mode={} tasks={}",
            if options.ci { "ci" } else { "local" },
            tasks.len()
        );
    }

    let mut results = Vec::with_capacity(tasks.len() + 8);
    let mut environment = None;
    for (index, task) in tasks.iter().enumerate() {
        let task_name = task_label(task);
        if !options.json {
            println!("[{}/{}] {task_name}", index + 1, tasks.len());
        }
        if matches!(task, Task::QualificationEnvironment) {
            let observed = qualification_environment::inspect();
            environment = Some(observed.clone());
            let status = environment_evidence_status(&observed);
            let detail = (status != EvidenceStatus::Passed).then(|| observed.summary());
            results.push(EvidenceResult {
                id: task_name.to_owned(),
                status,
                detail,
                measurements: Vec::new(),
                gates: Vec::new(),
                samples: Vec::new(),
            });
            if observed.status != QualificationEnvironmentStatus::Valid {
                if options.json {
                    evidence::emit(
                        root,
                        &label,
                        &results,
                        environment.as_ref(),
                        options.evidence_file.as_deref(),
                    )?;
                }
                return Err(environment_failure(&observed));
            }
            continue;
        }

        if options.resources && is_resource_stage(task) {
            let observed = qualification_environment::inspect();
            environment = Some(observed.clone());
            let status = environment_evidence_status(&observed);
            results.push(EvidenceResult {
                id: format!("qualification environment before {task_name}"),
                status,
                detail: (status != EvidenceStatus::Passed).then(|| observed.summary()),
                measurements: Vec::new(),
                gates: Vec::new(),
                samples: Vec::new(),
            });
            if observed.status != QualificationEnvironmentStatus::Valid {
                if options.json {
                    emit_partial(
                        root,
                        &label,
                        &results,
                        environment.as_ref(),
                        options.evidence_file.as_deref(),
                    )?;
                }
                return Err(environment_failure(&observed));
            }
        }

        match run_task(root, task, options.json) {
            Ok(TaskExecution::Passed(evidence)) => results.push(EvidenceResult {
                id: task_name.to_owned(),
                status: EvidenceStatus::Passed,
                detail: None,
                measurements: evidence.measurements,
                gates: evidence.gates,
                samples: evidence.samples,
            }),
            Ok(TaskExecution::Failed { detail, evidence }) => {
                results.push(EvidenceResult {
                    id: task_name.to_owned(),
                    status: EvidenceStatus::Failed,
                    detail: Some(detail.clone()),
                    measurements: evidence.measurements,
                    gates: evidence.gates,
                    samples: evidence.samples,
                });
                if options.json {
                    evidence::emit(
                        root,
                        &label,
                        &results,
                        environment.as_ref(),
                        options.evidence_file.as_deref(),
                    )?;
                }
                return Err(detail);
            }
            #[cfg(not(windows))]
            Ok(TaskExecution::NotTested(detail)) => {
                results.push(EvidenceResult {
                    id: task_name.to_owned(),
                    status: EvidenceStatus::NotTested,
                    detail: Some(detail.clone()),
                    measurements: Vec::new(),
                    gates: Vec::new(),
                    samples: Vec::new(),
                });
                if options.json {
                    evidence::emit(
                        root,
                        &label,
                        &results,
                        environment.as_ref(),
                        options.evidence_file.as_deref(),
                    )?;
                }
                return Err(format!("`{task_name}` is NOT_TESTED: {detail}"));
            }
            Err(error) => {
                results.push(EvidenceResult {
                    id: task_name.to_owned(),
                    status: EvidenceStatus::Failed,
                    detail: Some(error.clone()),
                    measurements: Vec::new(),
                    gates: Vec::new(),
                    samples: Vec::new(),
                });
                if options.json {
                    evidence::emit(
                        root,
                        &label,
                        &results,
                        environment.as_ref(),
                        options.evidence_file.as_deref(),
                    )?;
                }
                return Err(error);
            }
        }
        if options.resources && is_resource_stage(task) && options.json {
            emit_partial(
                root,
                &label,
                &results,
                environment.as_ref(),
                options.evidence_file.as_deref(),
            )?;
        }
    }
    if requires_full_readiness(options)
        && let Err(error) = governance::verify_ready_status(root, options.selection)
    {
        results.push(EvidenceResult {
            id: "acceptance readiness".to_owned(),
            status: EvidenceStatus::Failed,
            detail: Some(error.clone()),
            measurements: Vec::new(),
            gates: Vec::new(),
            samples: Vec::new(),
        });
        if options.json {
            evidence::emit(
                root,
                &label,
                &results,
                environment.as_ref(),
                options.evidence_file.as_deref(),
            )?;
        }
        return Err(error);
    }
    results.push(EvidenceResult {
        id: if options.ci {
            "requested headless CI task set".to_owned()
        } else {
            "acceptance readiness".to_owned()
        },
        status: EvidenceStatus::Passed,
        detail: None,
        measurements: Vec::new(),
        gates: Vec::new(),
        samples: Vec::new(),
    });
    if options.json {
        evidence::emit(
            root,
            &label,
            &results,
            environment.as_ref(),
            options.evidence_file.as_deref(),
        )?;
    } else {
        println!("StickyMD smoke PASS: {label}");
    }
    Ok(())
}

fn emit_partial(
    root: &Path,
    label: &str,
    results: &[EvidenceResult],
    environment: Option<&QualificationEnvironment>,
    output_file: Option<&Path>,
) -> Result<(), String> {
    let mut partial = results.to_vec();
    partial.push(EvidenceResult {
        id: "resource qualification campaign".to_owned(),
        status: EvidenceStatus::Incomplete,
        detail: Some("additional required resource scenarios have not completed".to_owned()),
        measurements: Vec::new(),
        gates: Vec::new(),
        samples: Vec::new(),
    });
    evidence::emit(root, label, &partial, environment, output_file)
}

const fn environment_evidence_status(environment: &QualificationEnvironment) -> EvidenceStatus {
    match environment.status {
        QualificationEnvironmentStatus::Valid => EvidenceStatus::Passed,
        QualificationEnvironmentStatus::EnvironmentBlocked
        | QualificationEnvironmentStatus::Unsupported => EvidenceStatus::NotTested,
        QualificationEnvironmentStatus::Error => EvidenceStatus::Failed,
    }
}

fn environment_failure(environment: &QualificationEnvironment) -> String {
    match environment.status {
        QualificationEnvironmentStatus::EnvironmentBlocked => "Qualification environment is blocked by locked/non-interactive desktop. Unlock the active Windows session and rerun the Phase 14 evidence campaign.".to_owned(),
        QualificationEnvironmentStatus::Unsupported => {
            "qualification environment is unsupported on this host".to_owned()
        }
        QualificationEnvironmentStatus::Error => format!(
            "qualification environment inspection failed: {}",
            environment.summary()
        ),
        QualificationEnvironmentStatus::Valid => {
            "qualification environment unexpectedly reported a failure".to_owned()
        }
    }
}

const fn is_resource_stage(task: &Task) -> bool {
    matches!(
        task,
        Task::Runtime {
            scenario: RuntimeScenario::Resources
                | RuntimeScenario::MathResources
                | RuntimeScenario::ImageResources
                | RuntimeScenario::WindowResources
                | RuntimeScenario::ZoomResources,
            ..
        }
    )
}

const fn requires_full_readiness(options: &Options) -> bool {
    !options.ci
}

fn task_label(task: &Task) -> &'static str {
    match task {
        Task::Governance => "governance contracts",
        Task::QualificationEnvironment => "qualification environment preflight",
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
        Task::Runtime {
            scenario: RuntimeScenario::Phase10,
            ..
        } => "copied Release Phase 10 compact/tool-window/opacity lifecycle",
        Task::Runtime {
            scenario: RuntimeScenario::Phase11B,
            ..
        } => "copied Release Phase 11-B semantic-conversion lifecycle",
        Task::Runtime {
            scenario: RuntimeScenario::ZoomResources,
            ..
        } => "copied Release Phase 10 zoom resource matrix",
    }
}

fn run_task(root: &Path, task: &Task, capture_output: bool) -> Result<TaskExecution, String> {
    match task {
        Task::Governance => {
            governance::verify(root).map(|()| TaskExecution::Passed(TaskEvidence::default()))
        }
        Task::QualificationEnvironment => Err(
            "qualification environment tasks are handled by the evidence coordinator".to_owned(),
        ),
        Task::Cargo { label, args, .. } => {
            let mut command = Command::new("cargo");
            command.args(args).current_dir(root);
            if capture_output {
                run_captured(command, label)
                    .map(|()| TaskExecution::Passed(TaskEvidence::default()))
            } else {
                run_inherited(command, label)
                    .map(|()| TaskExecution::Passed(TaskEvidence::default()))
            }
        }
        Task::PowerShell {
            label,
            script,
            args,
            ..
        } => {
            let mut command = Command::new("pwsh");
            command
                .args(["-NoProfile", "-File"])
                .arg(root.join(script))
                .args(args)
                .current_dir(root);
            if capture_output {
                run_captured(command, label)
                    .map(|()| TaskExecution::Passed(TaskEvidence::default()))
            } else {
                run_inherited(command, label)
                    .map(|()| TaskExecution::Passed(TaskEvidence::default()))
            }
        }
        Task::Runtime { scenario, .. } => run_runtime(root, *scenario, capture_output),
    }
}

fn run_inherited(mut command: Command, label: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("cannot start `{label}`: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("`{label}` failed with {status}"))
    }
}

fn run_captured(mut command: Command, label: &str) -> Result<(), String> {
    let output = command
        .output()
        .map_err(|error| format!("cannot start `{label}`: {error}"))?;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    let detail = detail.trim();
    Err(if detail.is_empty() {
        format!("`{label}` failed with {}", output.status)
    } else {
        format!("`{label}` failed with {}: {detail}", output.status)
    })
}

#[cfg(windows)]
fn run_runtime(
    root: &Path,
    scenario: RuntimeScenario,
    quiet: bool,
) -> Result<TaskExecution, String> {
    crate::runtime::run(root, scenario, quiet).map(|outcome| {
        if let Some(detail) = outcome.gate_failure {
            TaskExecution::Failed {
                detail,
                evidence: TaskEvidence {
                    measurements: outcome.measurements,
                    gates: outcome.gates,
                    samples: outcome.samples,
                },
            }
        } else {
            TaskExecution::Passed(TaskEvidence {
                measurements: outcome.measurements,
                gates: outcome.gates,
                samples: outcome.samples,
            })
        }
    })
}

#[cfg(not(windows))]
fn run_runtime(
    _root: &Path,
    _scenario: RuntimeScenario,
    _quiet: bool,
) -> Result<TaskExecution, String> {
    Ok(TaskExecution::NotTested(
        "runtime smoke requires Windows".to_owned(),
    ))
}

fn build_plan(options: &Options) -> Result<Vec<Task>, String> {
    let mut tasks = Vec::new();
    push_unique(&mut tasks, governance());
    if options.performance || options.runtime || options.resources {
        push_unique(&mut tasks, qualification_environment());
    }
    if options.resources {
        push_unique(&mut tasks, release_build());
        match options.selection {
            Selection::Phase(Phase::P06) => push_unique(&mut tasks, runtime_math_resources()),
            Selection::Phase(Phase::P07) => push_unique(&mut tasks, runtime_image_resources()),
            Selection::Phase(Phase::P08) => push_unique(&mut tasks, runtime_window_resources()),
            Selection::Phase(Phase::P10) => {
                push_unique(&mut tasks, runtime_resources());
                push_unique(&mut tasks, runtime_math_resources());
                push_unique(&mut tasks, runtime_image_resources());
                push_unique(&mut tasks, runtime_window_resources());
                push_unique(&mut tasks, runtime_zoom_resources());
            }
            Selection::Phase(Phase::P11 | Phase::P11B | Phase::P12 | Phase::P13 | Phase::P14) => {
                push_unique(&mut tasks, runtime_resources());
                push_unique(&mut tasks, runtime_math_resources());
                push_unique(&mut tasks, runtime_image_resources());
                push_unique(&mut tasks, runtime_window_resources());
                push_unique(&mut tasks, runtime_zoom_resources());
            }
            Selection::All => {
                push_unique(&mut tasks, runtime_resources());
                push_unique(&mut tasks, runtime_math_resources());
                push_unique(&mut tasks, runtime_image_resources());
                push_unique(&mut tasks, runtime_window_resources());
                push_unique(&mut tasks, runtime_zoom_resources());
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
        Selection::Phase(Phase::P10) => push_unique(&mut tasks, phase10_ux_tests()),
        Selection::Phase(Phase::P11) => push_unique(&mut tasks, workspace_tests()),
        Selection::Phase(Phase::P11B) => push_unique(&mut tasks, phase11b_tests()),
        Selection::Phase(Phase::P12 | Phase::P13 | Phase::P14) => {
            push_unique(&mut tasks, workspace_tests());
        }
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
                Phase::P10 => {
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
                    push_unique(&mut tasks, phase10_performance());
                }
                Phase::P11 => {
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
                    push_unique(&mut tasks, phase10_performance());
                }
                Phase::P11B => {
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
                    push_unique(&mut tasks, phase10_performance());
                    push_unique(&mut tasks, phase11b_performance());
                }
                Phase::P12 | Phase::P13 | Phase::P14 => {
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
                    push_unique(&mut tasks, phase10_performance());
                    push_unique(&mut tasks, phase11b_performance());
                }
            }
        }
    }

    if options.performance
        && selected_phases(options.selection).iter().any(|phase| {
            matches!(
                phase,
                Phase::P09
                    | Phase::P10
                    | Phase::P11
                    | Phase::P11B
                    | Phase::P12
                    | Phase::P13
                    | Phase::P14
            )
        })
    {
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
            Selection::Phase(Phase::P10) => {
                push_unique(&mut tasks, runtime_phase10());
            }
            Selection::Phase(Phase::P11 | Phase::P11B | Phase::P12 | Phase::P13 | Phase::P14) => {
                push_unique(&mut tasks, runtime_portable());
                push_unique(&mut tasks, runtime_preview());
                push_unique(&mut tasks, runtime_math());
                push_unique(&mut tasks, runtime_assets());
                push_unique(&mut tasks, runtime_window_shell());
                push_unique(&mut tasks, runtime_phase10());
                if matches!(
                    options.selection,
                    Selection::Phase(Phase::P11B | Phase::P12 | Phase::P13 | Phase::P14)
                ) {
                    push_unique(&mut tasks, runtime_phase11b());
                }
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

const fn qualification_environment() -> Task {
    Task::QualificationEnvironment
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

fn phase10_ux_tests() -> Task {
    cargo(
        TaskId::Phase10UxTests,
        "Phase 10 UX correction tests",
        &[
            "test",
            "-p",
            "stickymd-render",
            "-p",
            "stickymd-win",
            "--locked",
            "phase10_",
        ],
    )
}

fn phase11b_tests() -> Task {
    cargo(
        TaskId::Phase11BTests,
        "Phase 11-B semantic-conversion and Pin orthogonality tests",
        &["test", "--workspace", "--locked", "phase11b_"],
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

fn phase10_performance() -> Task {
    cargo(
        TaskId::Phase10Performance,
        "Phase 10 zoom/window Release baseline",
        &[
            "test",
            "-p",
            "stickymd-render",
            "-p",
            "stickymd-win",
            "--release",
            "--locked",
            "phase10_",
            "--",
            "--ignored",
            "--nocapture",
        ],
    )
}

fn phase11b_performance() -> Task {
    cargo(
        TaskId::Phase11BPerformance,
        "Phase 11-B semantic-conversion Release baseline",
        &[
            "test",
            "-p",
            "stickymd-render",
            "--release",
            "--locked",
            "phase11b_performance_",
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

const fn runtime_phase10() -> Task {
    Task::Runtime {
        id: TaskId::RuntimePhase10,
        scenario: RuntimeScenario::Phase10,
    }
}

const fn runtime_phase11b() -> Task {
    Task::Runtime {
        id: TaskId::RuntimePhase11B,
        scenario: RuntimeScenario::Phase11B,
    }
}

const fn runtime_zoom_resources() -> Task {
    Task::Runtime {
        id: TaskId::RuntimeZoomResources,
        scenario: RuntimeScenario::ZoomResources,
    }
}

#[cfg(test)]
mod tests {
    use super::{TaskId, build_plan, requires_full_readiness};
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
            json: false,
            evidence_file: None,
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
            json: false,
            evidence_file: None,
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
        assert!(ids.contains(&TaskId::Phase11BPerformance));
        assert!(ids.contains(&TaskId::QualificationEnvironment));
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
            json: false,
            evidence_file: None,
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
            TaskId::Phase10Performance,
            TaskId::Phase11BPerformance,
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
        assert!(!ids.contains(&TaskId::RuntimePhase10));
        assert!(!ids.contains(&TaskId::RuntimePhase11B));
        assert!(!ids.contains(&TaskId::RuntimeZoomResources));
        assert!(!ids.contains(&TaskId::QualificationEnvironment));
        assert!(!requires_full_readiness(&Options {
            selection: Selection::All,
            ci: true,
            performance: false,
            runtime: false,
            resources: false,
            release: false,
            package: false,
            json: true,
            evidence_file: None,
        }));
    }

    #[test]
    fn local_modes_retain_full_acceptance_readiness_gate() {
        assert!(requires_full_readiness(&Options {
            selection: Selection::Phase(crate::cli::Phase::P10),
            ci: false,
            performance: false,
            runtime: true,
            resources: false,
            release: false,
            package: false,
            json: true,
            evidence_file: None,
        }));
    }

    #[test]
    fn phase10_routes_headless_runtime_resources_and_performance_once() {
        let options = |performance, runtime, resources| Options {
            selection: Selection::Phase(crate::cli::Phase::P10),
            ci: false,
            performance,
            runtime,
            resources,
            release: false,
            package: false,
            json: true,
            evidence_file: None,
        };
        let headless = build_plan(&options(false, false, false)).expect("Phase 10 headless plan");
        assert_eq!(
            headless
                .iter()
                .filter(|task| task.id() == TaskId::Phase10UxTests)
                .count(),
            1
        );
        let runtime = build_plan(&options(false, true, false)).expect("Phase 10 runtime plan");
        assert_eq!(
            runtime
                .iter()
                .filter(|task| task.id() == TaskId::RuntimePhase10)
                .count(),
            1
        );
        let resources = build_plan(&options(false, false, true)).expect("Phase 10 resources plan");
        for expected in [
            TaskId::RuntimeResources,
            TaskId::RuntimeMathResources,
            TaskId::RuntimeImageResources,
            TaskId::RuntimeWindowResources,
            TaskId::RuntimeZoomResources,
        ] {
            assert_eq!(
                resources
                    .iter()
                    .filter(|task| task.id() == expected)
                    .count(),
                1
            );
        }
        let performance =
            build_plan(&options(true, false, false)).expect("Phase 10 performance plan");
        assert_eq!(
            performance
                .iter()
                .filter(|task| task.id() == TaskId::Phase10Performance)
                .count(),
            1
        );
        assert_eq!(
            performance
                .iter()
                .filter(|task| task.id() == TaskId::RuntimeStartup)
                .count(),
            1
        );
    }

    #[test]
    fn phase11_routes_convergence_runtime_resources_and_performance_once() {
        let options = |performance, runtime, resources| Options {
            selection: Selection::Phase(crate::cli::Phase::P11),
            ci: false,
            performance,
            runtime,
            resources,
            release: false,
            package: false,
            json: true,
            evidence_file: None,
        };
        let headless = build_plan(&options(false, false, false)).expect("Phase 11 headless plan");
        assert_eq!(
            headless
                .iter()
                .filter(|task| task.id() == TaskId::WorkspaceTests)
                .count(),
            1
        );
        let runtime = build_plan(&options(false, true, false)).expect("Phase 11 runtime plan");
        for expected in [
            TaskId::RuntimePortable,
            TaskId::RuntimePreview,
            TaskId::RuntimeMath,
            TaskId::RuntimeAssets,
            TaskId::RuntimeWindowShell,
            TaskId::RuntimePhase10,
        ] {
            assert_eq!(
                runtime.iter().filter(|task| task.id() == expected).count(),
                1
            );
        }
        let resources = build_plan(&options(false, false, true)).expect("Phase 11 resources plan");
        for expected in [
            TaskId::RuntimeResources,
            TaskId::RuntimeMathResources,
            TaskId::RuntimeImageResources,
            TaskId::RuntimeWindowResources,
            TaskId::RuntimeZoomResources,
        ] {
            assert_eq!(
                resources
                    .iter()
                    .filter(|task| task.id() == expected)
                    .count(),
                1
            );
        }
        let performance =
            build_plan(&options(true, false, false)).expect("Phase 11 performance plan");
        assert_eq!(
            performance
                .iter()
                .filter(|task| task.id() == TaskId::RuntimeStartup)
                .count(),
            1
        );
        assert_eq!(
            performance
                .iter()
                .filter(|task| task.id() == TaskId::Phase10Performance)
                .count(),
            1
        );
    }

    #[test]
    fn phase11b_routes_amendment_tests_runtime_and_release_performance_once() {
        let options = |performance, runtime| Options {
            selection: Selection::Phase(crate::cli::Phase::P11B),
            ci: false,
            performance,
            runtime,
            resources: false,
            release: false,
            package: false,
            json: true,
            evidence_file: None,
        };
        let headless = build_plan(&options(false, false)).expect("Phase 11-B headless plan");
        assert_eq!(
            headless
                .iter()
                .filter(|task| task.id() == TaskId::Phase11BTests)
                .count(),
            1
        );
        let runtime = build_plan(&options(false, true)).expect("Phase 11-B runtime plan");
        assert_eq!(
            runtime
                .iter()
                .filter(|task| task.id() == TaskId::RuntimePhase11B)
                .count(),
            1
        );
        let performance = build_plan(&options(true, false)).expect("Phase 11-B performance plan");
        assert_eq!(
            performance
                .iter()
                .filter(|task| task.id() == TaskId::Phase11BPerformance)
                .count(),
            1
        );
        assert_eq!(
            performance
                .iter()
                .filter(|task| task.id() == TaskId::RuntimeStartup)
                .count(),
            1
        );
    }

    #[test]
    fn phase12_routes_final_headless_runtime_resources_and_performance_once() {
        let options = |performance, runtime, resources| Options {
            selection: Selection::Phase(crate::cli::Phase::P12),
            ci: false,
            performance,
            runtime,
            resources,
            release: false,
            package: false,
            json: true,
            evidence_file: None,
        };
        let headless = build_plan(&options(false, false, false)).expect("Phase 12 headless plan");
        assert_eq!(
            headless
                .iter()
                .filter(|task| task.id() == TaskId::WorkspaceTests)
                .count(),
            1
        );
        let runtime = build_plan(&options(false, true, false)).expect("Phase 12 runtime plan");
        assert_eq!(
            runtime
                .iter()
                .filter(|task| task.id() == TaskId::RuntimePhase11B)
                .count(),
            1
        );
        let resources = build_plan(&options(false, false, true)).expect("Phase 12 resources plan");
        for expected in [
            TaskId::RuntimeResources,
            TaskId::RuntimeMathResources,
            TaskId::RuntimeImageResources,
            TaskId::RuntimeWindowResources,
            TaskId::RuntimeZoomResources,
        ] {
            assert_eq!(
                resources
                    .iter()
                    .filter(|task| task.id() == expected)
                    .count(),
                1
            );
        }
        let performance =
            build_plan(&options(true, false, false)).expect("Phase 12 performance plan");
        for expected in [
            TaskId::Phase10Performance,
            TaskId::Phase11BPerformance,
            TaskId::RuntimeStartup,
        ] {
            assert_eq!(
                performance
                    .iter()
                    .filter(|task| task.id() == expected)
                    .count(),
                1
            );
        }
    }

    #[test]
    fn phase13_routes_environment_before_every_local_gui_campaign() {
        let options = |performance, runtime, resources| Options {
            selection: Selection::Phase(crate::cli::Phase::P13),
            ci: false,
            performance,
            runtime,
            resources,
            release: false,
            package: false,
            json: true,
            evidence_file: None,
        };
        for plan in [
            build_plan(&options(true, false, false)).expect("performance plan"),
            build_plan(&options(false, true, false)).expect("runtime plan"),
            build_plan(&options(false, false, true)).expect("resource plan"),
        ] {
            assert_eq!(plan[0].id(), TaskId::Governance);
            assert_eq!(plan[1].id(), TaskId::QualificationEnvironment);
            assert_eq!(
                plan.iter()
                    .filter(|task| task.id() == TaskId::QualificationEnvironment)
                    .count(),
                1
            );
        }

        let runtime = build_plan(&options(false, true, false)).expect("runtime plan");
        assert!(
            runtime
                .iter()
                .any(|task| task.id() == TaskId::RuntimePhase11B)
        );
        let resources = build_plan(&options(false, false, true)).expect("resource plan");
        for expected in [
            TaskId::RuntimeResources,
            TaskId::RuntimeMathResources,
            TaskId::RuntimeImageResources,
            TaskId::RuntimeWindowResources,
            TaskId::RuntimeZoomResources,
        ] {
            assert!(resources.iter().any(|task| task.id() == expected));
        }
    }

    #[test]
    fn phase14_routes_environment_before_every_local_gui_campaign() {
        let options = |performance, runtime, resources| Options {
            selection: Selection::Phase(crate::cli::Phase::P14),
            ci: false,
            performance,
            runtime,
            resources,
            release: false,
            package: false,
            json: true,
            evidence_file: None,
        };
        for plan in [
            build_plan(&options(true, false, false)).expect("performance plan"),
            build_plan(&options(false, true, false)).expect("runtime plan"),
            build_plan(&options(false, false, true)).expect("resource plan"),
        ] {
            assert_eq!(plan[0].id(), TaskId::Governance);
            assert_eq!(plan[1].id(), TaskId::QualificationEnvironment);
            assert_eq!(
                plan.iter()
                    .filter(|task| task.id() == TaskId::QualificationEnvironment)
                    .count(),
                1
            );
        }
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
            json: false,
            evidence_file: None,
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
            json: false,
            evidence_file: None,
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
            json: false,
            evidence_file: None,
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
            json: false,
            evidence_file: None,
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
            json: false,
            evidence_file: None,
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
            json: false,
            evidence_file: None,
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
            json: false,
            evidence_file: None,
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
            json: false,
            evidence_file: None,
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
