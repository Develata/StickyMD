use super::*;
use crate::headless::Module;

fn plan(paths: Result<Vec<String>, String>, registry: Result<(), String>) -> Plan {
    prepare(
        git::Facts {
            head: Some("a".repeat(40)),
            paths,
        },
        &Command {
            mode: Mode::Tests,
            plan_only: true,
        },
        || registry,
    )
    .unwrap()
}

#[test]
fn local_command_defaults_and_rejects_unavailable_evidence_or_conflicting_flags() {
    assert_eq!(
        Command::parse(&[]).unwrap(),
        Command {
            mode: Mode::Tests,
            plan_only: false
        }
    );
    for mode in ["tests", "performance", "all"] {
        let args = ["--plan".to_owned(), format!("--mode={mode}")];
        assert!(Command::parse(&args).unwrap().plan_only);
    }
    for args in [
        vec!["--plan", "--plan"],
        vec!["--mode=tests", "--mode=all"],
        vec!["--mode=runtime"],
        vec!["--evidence-file=receipt.json"],
        vec!["--ci"],
        vec!["--unknown"],
    ] {
        assert!(Command::parse(&args.into_iter().map(str::to_owned).collect::<Vec<_>>()).is_err());
    }
}

#[test]
fn local_git_failures_unknown_shared_inputs_and_registry_drift_select_all() {
    for plan in [
        plan(Err("unreadable index".to_owned()), Ok(())),
        plan(Ok(vec!["new-crate/src/lib.rs".to_owned()]), Ok(())),
        plan(Ok(vec!["Cargo.lock".to_owned()]), Ok(())),
        plan(
            Ok(vec!["crates/stickymd-core/src/lib.rs".to_owned()]),
            Err("new member".to_owned()),
        ),
    ] {
        assert!(plan.selection.full);
        assert_eq!(plan.selection.modules, Module::ALL);
        let descriptions = plan.tasks.describe().unwrap();
        assert!(
            descriptions.iter().any(
                |task| task.args.first() == Some(&"test") && task.args.contains(&"--workspace")
            )
        );
    }
}

#[test]
fn local_plan_reuses_path_ownership_and_explains_reverse_dependencies() {
    let plan = plan(
        Ok(vec!["crates/stickymd-core/src/中文 space.rs".to_owned()]),
        Ok(()),
    );
    assert_eq!(
        plan.selection.modules,
        [Module::Core, Module::Render, Module::Windows]
    );
    let descriptions = plan.tasks.describe().unwrap();
    let tests = descriptions
        .iter()
        .find(|task| task.args.first() == Some(&"test"))
        .unwrap();
    let reasons = projection::task_reasons(&plan.selection, tests);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.contains("中文 space.rs"))
    );
    assert!(
        reasons
            .iter()
            .any(|reason| reason == "reverse dependency: render depends on core")
    );
    assert!(
        reasons
            .iter()
            .any(|reason| reason == "reverse dependency: windows depends on render")
    );
    let json = projection::json(
        &plan,
        &Command {
            mode: Mode::Tests,
            plan_only: true,
        },
    )
    .unwrap();
    assert!(json.contains("\"status\":\"NOT_RUN\""));
    assert!(json.contains("\"qualification_receipt\":false"));
    assert!(!json.contains("PASSED"));
    assert!(!json.contains("artifact_sha256"));
}

#[test]
fn local_empty_and_documentation_plans_skip_code_checks_and_keep_shared_checks() {
    for paths in [
        vec![],
        vec!["README.md".to_owned(), "docs/report/local.md".to_owned()],
    ] {
        let plan = prepare(
            git::Facts {
                head: Some("a".repeat(40)),
                paths: Ok(paths),
            },
            &Command {
                mode: Mode::Tests,
                plan_only: true,
            },
            || panic!("documentation does not require a Cargo graph query"),
        )
        .unwrap();
        assert!(!plan.selection.full);
        assert!(plan.selection.modules.is_empty());
        assert_eq!(plan.tasks.describe().unwrap().len(), 2);
        assert!(!plan.checks.dependency && !plan.checks.quality && !plan.checks.headless);
    }
}

#[test]
fn local_json_escapes_newline_and_quote_paths_without_lossy_transport() {
    let plan = plan(
        Ok(vec!["crates/stickymd-render/new\n\"line.rs".to_owned()]),
        Ok(()),
    );
    let json = projection::json(
        &plan,
        &Command {
            mode: Mode::Tests,
            plan_only: true,
        },
    )
    .unwrap();
    assert!(json.contains("new\\n\\\"line.rs"));
    assert!(!json.contains("new\n"));
    assert!(json.contains("\"worktree_dirty\":true"));
}
