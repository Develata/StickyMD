//! Choose the actual measured artifact without rebuilding an unused local EXE.
//! plan_ref: docs/plan/11_testing_and_release.md#resource-module-qualification

use super::*;

pub(super) fn execution_plan(root: &Path, options: &Options) -> Result<Vec<Task>, String> {
    let mut tasks = build_plan(options)?;
    let formal = options.evidence_file.as_deref().is_some_and(|path| {
        crate::qualification::smoke_scope::is_formal_measurement_path(root, path)
    });
    if !options.ci
        && !options.release
        && !options.package
        && (options.runtime || options.performance || options.resources)
        && (formal || crate::qualification::requires_promoted_candidate(root))
    {
        use_promoted_candidate(&mut tasks);
    }
    Ok(tasks)
}

pub(super) fn use_promoted_candidate(tasks: &mut Vec<Task>) {
    if tasks.iter().any(|task| task.id() == TaskId::ReleaseBuild) {
        tasks.retain(|task| task.id() != TaskId::ReleaseBuild);
        // Validate before headless benchmarks or GUI/environment work can begin.
        let index = usize::from(matches!(tasks.first(), Some(Task::Governance)));
        tasks.insert(index, Task::PromotedCandidate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_or_malformed_candidate_fails_without_running_the_local_build() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "stickymd-candidate-plan-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("target/release")).unwrap();
        std::fs::create_dir_all(root.join("dist/evidence")).unwrap();
        let local = root.join("target/release/stickymd-win.exe");
        std::fs::write(&local, b"unused local executable").unwrap();
        let options = Options::parse(["phase", "14", "--runtime"].map(str::to_owned)).unwrap();
        for receipt in [
            "dist/evidence/release-source-freeze.json",
            "dist/evidence/release-candidate.json",
        ] {
            std::fs::write(root.join(receipt), "{}").unwrap();
            let tasks = execution_plan(&root, &options).unwrap();
            assert_eq!(tasks[1], Task::PromotedCandidate);
            assert!(!tasks.iter().any(|task| task.id() == TaskId::ReleaseBuild));
            let mut results = Vec::new();
            let mut environment = None;
            // An invalid later Cargo task makes accidental continuation observable.
            let tasks = [
                tasks[1].clone(),
                cargo(
                    TaskId::ReleaseBuild,
                    "must not run",
                    &["not-a-real-command"],
                ),
            ];
            let outcome = execute_tasks(
                &root,
                &options,
                "phase-14",
                &tasks,
                &mut results,
                &mut environment,
            );
            assert!(outcome.is_err());
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].id, task_label(&Task::PromotedCandidate));
            assert_eq!(results[0].status, EvidenceStatus::Failed);
            assert_eq!(std::fs::read(&local).unwrap(), b"unused local executable");
        }
        for mode in ["--package", "--release", "--ci"] {
            let options = Options::parse(["phase", "14", mode].map(str::to_owned)).unwrap();
            assert_eq!(
                execution_plan(&root, &options).unwrap(),
                build_plan(&options).unwrap()
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn formal_measurements_validate_the_candidate_instead_of_building() {
        let root = std::env::temp_dir();
        for mode in ["runtime", "performance", "resources"] {
            let options = Options::parse([
                "phase".into(),
                "14".into(),
                format!("--{mode}"),
                format!("--evidence-file=dist/evidence/{mode}-qualification.json"),
            ])
            .unwrap();
            let tasks = execution_plan(&root, &options).unwrap();
            assert_eq!(tasks[1], Task::PromotedCandidate);
            assert!(!tasks.iter().any(|task| task.id() == TaskId::ReleaseBuild));
            if mode != "resources" {
                let mut labels: Vec<_> = tasks.iter().map(task_label).collect();
                labels.push("acceptance readiness");
                assert_eq!(labels, formal_task_labels(mode == "runtime").unwrap());
            }
        }
    }

    #[test]
    fn preflight_and_source_only_plans_keep_their_original_builds() {
        let root = std::env::temp_dir();
        for mode in [
            "--runtime",
            "--performance",
            "--resources",
            "--package",
            "--release",
            "--ci",
        ] {
            let options = Options::parse(["phase", "14", mode].map(str::to_owned)).unwrap();
            assert_eq!(
                execution_plan(&root, &options).unwrap(),
                build_plan(&options).unwrap()
            );
        }
    }
}
