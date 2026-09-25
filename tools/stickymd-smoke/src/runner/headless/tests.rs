use super::*;
use crate::cli::{CiShard, CommandLine};
use crate::runner::TaskId;

fn request(modules: &[Module], mode: Mode) -> Request {
    Request {
        modules: modules.to_vec(),
        mode,
        plan_only: true,
    }
}

#[test]
fn all_modules_preserve_the_existing_full_plan_and_shards_exactly() {
    for (mode, shard) in [
        (Mode::Tests, Some(CiShard::Tests)),
        (Mode::Performance, Some(CiShard::Performance)),
        (Mode::All, None),
    ] {
        let mut options = Options::parse(["all", "--ci"].map(str::to_owned)).unwrap();
        options.ci_shard = shard;
        assert_eq!(
            plan(&request(&Module::ALL, mode)).unwrap(),
            build_plan(&options).unwrap()
        );
    }
}

// Expand the original Cargo selectors into (module, task, command) coverage units.
// This is independent of the production narrowing loop, including argument order.
fn coverage(tasks: &[Task]) -> BTreeSet<(Module, TaskId, Vec<&'static str>)> {
    let mut units = BTreeSet::new();
    for task in tasks {
        let Task::Cargo { id, args, .. } = task else {
            continue;
        };
        let boundary = args
            .iter()
            .position(|arg| *arg == "--")
            .unwrap_or(args.len());
        let cargo_args = &args[..boundary];
        let selected: Vec<_> = Module::ALL
            .into_iter()
            .filter(|module| {
                (cargo_args.contains(&"--workspace") && module.package().is_some())
                    || cargo_args.windows(2).any(|pair| {
                        (pair[0] == "-p" && module.package() == Some(pair[1]))
                            || (pair[0] == "--manifest-path" && module.manifest() == Some(pair[1]))
                    })
            })
            .collect();
        assert!(!selected.is_empty(), "{task:?}");
        let command: Vec<_> = args
            .iter()
            .enumerate()
            .filter(|(index, arg)| {
                if *index >= boundary {
                    return true;
                }
                !matches!(**arg, "--workspace" | "-p" | "--manifest-path")
                    && !(*index > 0 && matches!(args[index - 1], "-p" | "--manifest-path"))
            })
            .map(|(_, arg)| *arg)
            .collect();
        for module in selected {
            assert!(
                units.insert((module, *id, command.clone())),
                "duplicate Cargo coverage"
            );
        }
    }
    units
}

#[test]
fn individual_modules_form_a_disjoint_complete_cover_of_headless_commands() {
    for mode in [Mode::Tests, Mode::Performance, Mode::All] {
        let full = coverage(&plan(&request(&Module::ALL, mode)).unwrap());
        let mut union = BTreeSet::new();
        for module in Module::ALL {
            let tasks = plan(&request(&[module], mode)).unwrap();
            assert_eq!(
                tasks
                    .iter()
                    .filter(|task| matches!(task, Task::Governance))
                    .count(),
                1
            );
            for unit in coverage(&tasks) {
                assert_eq!(unit.0, module);
                assert!(union.insert(unit), "task ran in two modules");
            }
        }
        assert_eq!(union, full);
    }
}

#[test]
fn rendering_selection_preserves_performance_filters_and_serial_measurement() {
    // Exercise narrowing independently of maintenance edits to any real benchmark.
    let serialized = Task::Cargo {
        id: TaskId::Phase5Performance,
        label: "serialized performance regression",
        args: vec![
            "test",
            "--workspace",
            "--release",
            "--locked",
            "regression_filter",
            "--",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ],
    };
    let Some(Task::Cargo { args: narrowed, .. }) = project(serialized, &[Module::Render]).unwrap()
    else {
        panic!("selected performance task expected");
    };
    assert_eq!(
        narrowed,
        [
            "test",
            "--release",
            "--locked",
            "regression_filter",
            "-p",
            "stickymd-render",
            "--",
            "--ignored",
            "--nocapture",
            "--test-threads=1",
        ]
    );
    let tasks = plan(&request(&[Module::Render], Mode::Performance)).unwrap();
    let args = |id| {
        tasks
            .iter()
            .find_map(|task| match task {
                Task::Cargo {
                    id: actual, args, ..
                } if *actual == id => Some(args.as_slice()),
                _ => None,
            })
            .unwrap()
    };
    assert!(args(TaskId::SourceScrollbarPerformance).contains(&"scrollbar_release_baseline"));
    for id in [
        TaskId::SourceScrollbarPerformance,
        TaskId::Phase5Performance,
        TaskId::Phase11BPerformance,
        TaskId::Phase14Performance,
    ] {
        assert!(
            args(id).contains(&"--lib"),
            "{id:?} must not link unused integration targets"
        );
        assert!(
            args(id).contains(&"--test-threads=1"),
            "{id:?} measurements must be serial"
        );
    }
    assert_eq!(
        args(TaskId::Phase6Performance),
        [
            "test",
            "--release",
            "--locked",
            "phase6_",
            "-p",
            "stickymd-render",
            "--",
            "--ignored",
            "--nocapture",
            "--test-threads=1"
        ]
    );
    assert!(
        !tasks
            .iter()
            .any(|task| task.id() == TaskId::Phase2Performance)
    );
}

#[test]
fn requesting_several_modules_deduplicates_shared_commands() {
    let tasks = plan(&request(&[Module::Core, Module::Render], Mode::Tests)).unwrap();
    assert_eq!(tasks.len(), 2);
    let Task::Cargo { args, .. } = &tasks[1] else {
        panic!("Cargo tests expected");
    };
    assert_eq!(
        args,
        &[
            "test",
            "--locked",
            "-p",
            "stickymd-core",
            "-p",
            "stickymd-render"
        ]
    );
}

#[test]
fn unknown_or_mixed_package_selectors_are_rejected_instead_of_omitted() {
    for args in [
        vec!["test"],
        vec!["test", "-p", "unknown-crate"],
        vec!["test", "--manifest-path", "new/experiment/Cargo.toml"],
        vec!["test", "--workspace", "--exclude", "stickymd-smoke"],
        vec!["test", "--workspace", "-p", "stickymd-render"],
        vec!["test", "-p"],
        vec!["test", "--workspace", "-pnew-crate"],
    ] {
        let task = Task::Cargo {
            id: TaskId::WorkspaceTests,
            label: "test",
            args,
        };
        assert!(project(task, &[Module::Render]).is_err());
    }
    assert!(project(Task::NativeRuntimeDependencies, &Module::ALL).is_err());
}

#[test]
fn plans_are_explicitly_unexecuted_and_cannot_be_candidate_receipts() {
    let selected = request(&[Module::Render], Mode::Tests);
    let json = plan_json(&selected, &plan(&selected).unwrap());
    assert!(json.contains("\"kind\":\"selected-headless-plan\""));
    assert!(json.contains("\"status\":\"NOT_RUN\""));
    assert!(json.contains("\"modules\":[\"render\"]"));
    assert!(!json.contains("PASSED"));
    assert!(!json.contains("artifact_sha256"));
    let command =
        CommandLine::parse(["modules", "run", "render", "--plan"].map(str::to_owned)).unwrap();
    assert_eq!(command, CommandLine::Modules(Command::Run(selected)));
}

#[test]
fn workspace_membership_drift_fails_closed() {
    let listing = "stickymd-core v0.1.0 (path with spaces)\n\nstickymd-render v0.1.0\n\nstickymd-win v0.1.0\n\nstickymd-smoke v0.1.0\n";
    assert!(verify_workspace_listing(listing).is_ok());
    assert!(verify_workspace_listing("").is_err());
    assert!(verify_workspace_listing(&listing.replace("stickymd-core", "renamed-core")).is_err());
    assert!(verify_workspace_listing(&format!("{listing}new-crate v0.1.0\n")).is_err());
}
