use super::{
    CiShard, CommandLine, GuidedSession, ManualCommand, ManualSelection, ManualSession, Options,
    Phase, QualificationCommand, ResourceModule, Selection, StandaloneRecord, WindowStressOptions,
    WindowStressScenario,
};

fn args<'a>(values: &'a [&'a str]) -> impl Iterator<Item = String> + 'a {
    values.iter().map(|value| (*value).to_owned())
}

/// `--case` accepts exactly the declared exact cases of each group, and on Windows names
/// each one by its declared id.
#[test]
fn exact_case_options_are_the_declared_cases() {
    use crate::qualification::exact_checks::{DeclaredCase, G3, G4, G5};
    fn accepted<T>(group: &str, parse: impl Fn(&str) -> Result<T, String>) -> Vec<String> {
        (0..100)
            .map(|number| format!("{group}-{number:02}"))
            .filter(|id| parse(id).is_ok())
            .collect()
    }
    let ids = |cases: &[DeclaredCase]| cases.iter().map(|case| case.id).collect::<Vec<_>>();
    assert_eq!(accepted("G3", super::G3Case::parse), ids(G3));
    assert_eq!(accepted("G4", super::G4Case::parse), ids(G4));
    assert_eq!(accepted("G5", super::G5Case::parse), ids(G5));
    #[cfg(windows)]
    for case in G3 {
        assert_eq!(super::G3Case::parse(case.id).unwrap().as_str(), case.id);
    }
    #[cfg(windows)]
    for case in G4 {
        assert_eq!(super::G4Case::parse(case.id).unwrap().as_str(), case.id);
    }
    #[cfg(windows)]
    for case in G5 {
        assert_eq!(super::G5Case::parse(case.id).unwrap().as_str(), case.id);
    }
}

#[test]
fn parses_phase_and_explicit_modes() {
    let options =
        Options::parse(args(&["phase", "03", "--performance", "--runtime"])).expect("valid CLI");
    assert_eq!(options.selection, Selection::Phase(Phase::P03));
    assert!(options.performance);
    assert!(options.runtime);
    assert!(!options.resources);
    assert!(!options.release);
    assert!(!options.package);
    assert!(!options.json);
}

#[test]
fn phase8_accepts_explicit_runtime_and_resource_modes() {
    let runtime =
        Options::parse(args(&["phase", "08", "--runtime"])).expect("Phase 8 runtime mode");
    assert_eq!(runtime.selection, Selection::Phase(Phase::P08));
    assert!(runtime.runtime);

    let resources =
        Options::parse(args(&["phase", "phase-08", "--resources"])).expect("Phase 8 resource mode");
    assert_eq!(resources.selection, Selection::Phase(Phase::P08));
    assert!(resources.resources);
}

#[test]
fn phase9_accepts_release_and_package_as_separate_modes() {
    let release =
        Options::parse(args(&["phase", "09", "--release"])).expect("Phase 9 release mode");
    assert_eq!(release.selection, Selection::Phase(Phase::P09));
    assert!(release.release);

    let package =
        Options::parse(args(&["phase", "phase-09", "--package"])).expect("Phase 9 package mode");
    assert!(package.package);

    let error = Options::parse(args(&["phase", "09", "--release", "--package"]))
        .expect_err("artifact-producing modes must not overlap");
    assert!(error.contains("must run separately"));
}

#[test]
fn rejects_environment_sensitive_ci_modes() {
    let error = Options::parse(args(&["all", "--ci", "--runtime"]))
        .expect_err("CI runtime smoke must be rejected");
    assert!(error.contains("cannot be combined"));
}

#[test]
fn ci_shards_are_explicit_and_require_the_all_ci_contract() {
    let tests = Options::parse(args(&["all", "--ci", "--ci-shard=tests"])).expect("tests CI shard");
    assert_eq!(tests.ci_shard, Some(CiShard::Tests));
    let performance = Options::parse(args(&["all", "--ci", "--ci-shard=performance"]))
        .expect("performance CI shard");
    assert_eq!(performance.ci_shard, Some(CiShard::Performance));
    assert!(
        Options::parse(args(&["all", "--ci-shard=tests"]))
            .expect_err("shard without CI")
            .contains("requires `all --ci`")
    );
}

#[test]
fn phase14_resources_can_select_one_measurement_module() {
    let options = Options::parse(args(&[
        "phase",
        "14",
        "--resources",
        "--resource-module=window",
    ]))
    .expect("targeted Phase 14 window resource module");
    assert_eq!(options.resource_module, Some(ResourceModule::Window));
    assert!(
        Options::parse(args(&[
            "phase",
            "08",
            "--resources",
            "--resource-module=window",
        ]))
        .expect_err("historical phase matrix must retain its own module")
        .contains("Phase 10 through 14")
    );
}

#[test]
fn resource_resume_requires_explicit_phase14_resources_and_evidence() {
    assert!(
        Options::parse(
            [
                "phase",
                "14",
                "--resources",
                "--resource-plan",
                "--evidence-file=target/plan.json"
            ]
            .map(str::to_owned)
        )
        .is_err()
    );
    let options = Options::parse(args(&[
        "phase",
        "14",
        "--resources",
        "--resource-resume",
        "--evidence-file=target/diagnostic.json",
    ]))
    .unwrap();
    assert!(options.resource_resume && options.json);
    for values in [
        vec![
            "phase",
            "14",
            "--resource-resume",
            "--evidence-file=target/a.json",
        ],
        vec!["phase", "14", "--resources", "--resource-resume"],
        vec![
            "phase",
            "13",
            "--resources",
            "--resource-resume",
            "--evidence-file=target/a.json",
        ],
        vec![
            "all",
            "--resources",
            "--resource-resume",
            "--evidence-file=target/a.json",
        ],
    ] {
        assert!(Options::parse(args(&values)).is_err());
    }
}

#[test]
fn failure_first_requires_explicit_diagnostic_resume() {
    let base = [
        "phase",
        "14",
        "--resources",
        "--resource-failure-first",
        "--evidence-file=target/diagnostic.json",
    ];
    assert!(
        Options::parse(args(&base))
            .unwrap_err()
            .contains("requires `--resource-resume`")
    );
    for suffix in [
        vec!["--resource-resume"],
        vec!["--resource-resume", "--resource-plan"],
    ] {
        let options =
            Options::parse(base.iter().chain(suffix.iter()).map(|s| s.to_string())).unwrap();
        assert!(options.resource_failure_first && options.resource_resume);
    }
}

#[test]
fn rejects_runtime_for_non_runtime_phase() {
    let error = Options::parse(args(&["phase", "02", "--runtime"]))
        .expect_err("Phase 02 has no runtime smoke");
    assert!(error.contains("Phase 03"));
}

#[test]
fn phase10_and_json_are_supported() {
    let options =
        Options::parse(args(&["phase", "10", "--json"])).expect("Phase 10 JSON evidence mode");
    assert_eq!(options.selection, Selection::Phase(Phase::P10));
    assert!(options.json);
}

#[test]
fn phase11_supports_every_local_convergence_mode() {
    for mode in [
        "--performance",
        "--runtime",
        "--resources",
        "--release",
        "--package",
    ] {
        let options =
            Options::parse(args(&["phase", "11", mode, "--json"])).expect("valid Phase 11 mode");
        assert_eq!(options.selection, Selection::Phase(Phase::P11));
        assert!(options.json);
    }
}

#[test]
fn phase11b_supports_every_local_amendment_mode() {
    for mode in [
        "--performance",
        "--runtime",
        "--resources",
        "--release",
        "--package",
    ] {
        let options = Options::parse(args(&["phase", "11-b", mode, "--json"]))
            .expect("valid Phase 11-B mode");
        assert_eq!(options.selection, Selection::Phase(Phase::P11B));
        assert!(options.json);
    }
}

#[test]
fn phase12_and_qualification_commands_are_explicit() {
    let phase = Options::parse(args(&["phase", "12", "--release", "--json"]))
        .expect("valid Phase 12 release mode");
    assert_eq!(phase.selection, Selection::Phase(Phase::P12));

    let source_freeze = CommandLine::parse(args(&["qualification", "source-freeze"]))
        .expect("valid Source Freeze command");
    assert_eq!(
        source_freeze,
        CommandLine::Qualification(QualificationCommand::SourceFreeze)
    );

    let readiness = CommandLine::parse(args(&["qualification", "readiness", "--explain"]))
        .expect("valid readiness command");
    assert_eq!(
        readiness,
        CommandLine::Qualification(QualificationCommand::Readiness { explain: true })
    );

    let modules = CommandLine::parse(args(&["qualification", "modules"]))
        .expect("valid module status command");
    assert_eq!(
        modules,
        CommandLine::Qualification(QualificationCommand::Modules)
    );

    let decision = CommandLine::parse(args(&[
        "qualification",
        "decision",
        "--key=RELEASE-VERSION",
        "--status=USER-APPROVED",
        "--evidence=USER-message",
    ]))
    .expect("valid decision command");
    assert_eq!(
        decision,
        CommandLine::Qualification(QualificationCommand::Decision {
            key: "RELEASE-VERSION".to_owned(),
            status: "USER-APPROVED".to_owned(),
            evidence: "USER-message".to_owned(),
            manual_scope: None,
        })
    );

    let manual =
        CommandLine::parse(args(&["acceptance", "manual"])).expect("valid manual recorder command");
    assert_eq!(
        manual,
        CommandLine::AcceptanceManual(ManualCommand::Run { session: None })
    );
    assert_eq!(
        CommandLine::parse(args(&[
            "acceptance",
            "manual",
            "export",
            "--select=G1",
            "--task=task.json",
        ]))
        .unwrap(),
        CommandLine::AcceptanceManual(ManualCommand::Export {
            selection: ManualSelection::Guided(Some(GuidedSession::G1)),
            task: std::path::PathBuf::from("task.json"),
        })
    );
    assert_eq!(
        CommandLine::parse(args(&["acceptance", "manual", "export", "--task=t.json"])).unwrap(),
        CommandLine::AcceptanceManual(ManualCommand::Export {
            selection: ManualSelection::Cases(None),
            task: std::path::PathBuf::from("t.json"),
        })
    );
    assert_eq!(
        CommandLine::parse(args(&["acceptance", "manual", "import", "--bundle=b.json"])).unwrap(),
        CommandLine::AcceptanceManual(ManualCommand::Import {
            bundle: std::path::PathBuf::from("b.json"),
        })
    );
    assert_eq!(
        CommandLine::parse(args(&[
            "acceptance",
            "manual",
            "record",
            "--zip=c.zip",
            "--task=t.json",
            "--bundle=b.json",
        ]))
        .unwrap(),
        CommandLine::ManualRecord(StandaloneRecord {
            task: std::path::PathBuf::from("t.json"),
            zip: std::path::PathBuf::from("c.zip"),
            bundle: std::path::PathBuf::from("b.json"),
        })
    );
    for invalid in [
        &["acceptance", "manual", "export"][..],
        &[
            "acceptance",
            "manual",
            "export",
            "--task=t.json",
            "--task=u.json",
        ],
        &[
            "acceptance",
            "manual",
            "export",
            "--task=t.json",
            "--select=M9",
        ],
        &["acceptance", "manual", "import"],
        &["acceptance", "manual", "import", "--bundle="],
        &[
            "acceptance",
            "manual",
            "record",
            "--task=t.json",
            "--zip=c.zip",
        ],
        &[
            "acceptance",
            "manual",
            "record",
            "--task=t.json",
            "--zip=c.zip",
            "--bundle=b",
            "--x=y",
        ],
    ] {
        assert!(CommandLine::parse(args(invalid)).is_err(), "{invalid:?}");
    }
    for selection in ["all", "guided", "M1", "M5", "G1", "G2"] {
        assert_eq!(
            ManualSelection::parse(selection).unwrap().as_str(),
            selection
        );
    }

    let native_runtime = CommandLine::parse(args(&[
        "qualification",
        "native-runtime",
        "--exe=target/release/stickymd-win.exe",
    ]))
    .expect("valid native-runtime dependency gate");
    assert_eq!(
        native_runtime,
        CommandLine::Qualification(QualificationCommand::NativeRuntime {
            executable: std::path::PathBuf::from("target/release/stickymd-win.exe"),
        })
    );
}

#[test]
fn phase13_environment_and_manual_sessions_are_explicit() {
    for mode in [
        "--performance",
        "--runtime",
        "--resources",
        "--release",
        "--package",
    ] {
        let options =
            Options::parse(args(&["phase", "13", mode, "--json"])).expect("valid Phase 13 mode");
        assert_eq!(options.selection, Selection::Phase(Phase::P13));
    }

    let environment = CommandLine::parse(args(&[
        "qualification",
        "environment",
        "--evidence-file=dist/evidence/qualification-environment.json",
    ]))
    .expect("valid environment preflight");
    assert_eq!(
        environment,
        CommandLine::Qualification(QualificationCommand::Environment {
            evidence_file: Some(std::path::PathBuf::from(
                "dist/evidence/qualification-environment.json"
            )),
        })
    );

    let manual = CommandLine::parse(args(&["acceptance", "manual", "run", "--session=M3"]))
        .expect("valid manual session");
    assert_eq!(
        manual,
        CommandLine::AcceptanceManual(ManualCommand::Run {
            session: Some(ManualSession::M3),
        })
    );
}

#[test]
fn phase14_and_guided_manual_sessions_are_explicit() {
    for mode in [
        "--performance",
        "--runtime",
        "--resources",
        "--release",
        "--package",
    ] {
        let options =
            Options::parse(args(&["phase", "14", mode, "--json"])).expect("valid Phase 14 mode");
        assert_eq!(options.selection, Selection::Phase(Phase::P14));
    }

    let guided = CommandLine::parse(args(&["acceptance", "manual", "guided", "--session=G2"]))
        .expect("valid guided manual session");
    assert_eq!(
        guided,
        CommandLine::AcceptanceManual(ManualCommand::Guided {
            session: Some(GuidedSession::G2),
        })
    );

    let g3 = CommandLine::parse(args(&[
        "qualification",
        "g3",
        "--zip=dist/candidate.zip",
        "--evidence-file=dist/evidence/g3.json",
    ]))
    .expect("valid exact-candidate G3 command");
    assert_eq!(
        g3,
        CommandLine::Qualification(QualificationCommand::G3Exact {
            zip: Some(std::path::PathBuf::from("dist/candidate.zip")),
            evidence_file: Some(std::path::PathBuf::from("dist/evidence/g3.json")),
            case: None,
        })
    );

    let targeted_g3 = CommandLine::parse(args(&["qualification", "g3", "--case=G3-05"]))
        .expect("valid targeted G3 command");
    assert_eq!(
        targeted_g3,
        CommandLine::Qualification(QualificationCommand::G3Exact {
            zip: None,
            evidence_file: None,
            case: Some(super::G3Case::G305),
        })
    );

    assert!(
        CommandLine::parse(args(&[
            "qualification",
            "g3",
            "--case=G3-04",
            "--case=G3-05",
        ]))
        .is_err()
    );

    let g4 = CommandLine::parse(args(&[
        "qualification",
        "g4",
        "--zip=dist/candidate.zip",
        "--evidence-file=dist/evidence/g4.json",
    ]))
    .expect("valid exact-candidate G4 command");
    assert_eq!(
        g4,
        CommandLine::Qualification(QualificationCommand::G4Exact {
            zip: Some(std::path::PathBuf::from("dist/candidate.zip")),
            evidence_file: Some(std::path::PathBuf::from("dist/evidence/g4.json")),
            case: None,
        })
    );

    let targeted_g4 = CommandLine::parse(args(&["qualification", "g4", "--case=G4-06"]))
        .expect("valid targeted G4 command");
    assert_eq!(
        targeted_g4,
        CommandLine::Qualification(QualificationCommand::G4Exact {
            zip: None,
            evidence_file: None,
            case: Some(super::G4Case::G406),
        })
    );

    let g5 = CommandLine::parse(args(&[
        "qualification",
        "g5",
        "--zip=dist/candidate.zip",
        "--evidence-file=dist/evidence/g5.json",
    ]))
    .expect("valid exact-candidate G5 command");
    assert_eq!(
        g5,
        CommandLine::Qualification(QualificationCommand::G5Exact {
            zip: Some(std::path::PathBuf::from("dist/candidate.zip")),
            evidence_file: Some(std::path::PathBuf::from("dist/evidence/g5.json")),
            case: None,
        })
    );

    let targeted_g5 = CommandLine::parse(args(&["qualification", "g5", "--case=G5-04"]))
        .expect("valid targeted G5 command");
    assert_eq!(
        targeted_g5,
        CommandLine::Qualification(QualificationCommand::G5Exact {
            zip: None,
            evidence_file: None,
            case: Some(super::G5Case::G504),
        })
    );
}

#[test]
fn phase14_window_stress_diagnostic_is_typed_and_bounded() {
    let command = CommandLine::parse(args(&[
        "qualification",
        "window-stress",
        "--scenario=combined",
        "--runs=10",
        "--collapse-cycles=1000",
        "--tray-cycles=100",
        "--control-cycles=100",
        "--view-mode-cycles=100",
        "--persistence-cycles=1",
    ]))
    .expect("valid window-stress diagnostic");
    assert_eq!(
        command,
        CommandLine::Qualification(QualificationCommand::WindowStress(WindowStressOptions {
            scenario: WindowStressScenario::Combined,
            runs: 10,
            collapse_cycles: 1000,
            tray_cycles: 100,
            control_cycles: 100,
            view_mode_cycles: 100,
            persistence_cycles: 1,
        }))
    );

    let error = CommandLine::parse(args(&[
        "qualification",
        "window-stress",
        "--scenario=tray",
        "--runs=0",
        "--collapse-cycles=0",
        "--tray-cycles=1",
        "--control-cycles=0",
        "--view-mode-cycles=0",
        "--persistence-cycles=0",
    ]))
    .expect_err("zero independent runs must be rejected");
    assert!(error.contains("outside 1..=1000"));

    let error = CommandLine::parse(args(&[
        "qualification",
        "window-stress",
        "--scenario=tray",
        "--runs=1001",
        "--collapse-cycles=0",
        "--tray-cycles=1",
        "--control-cycles=0",
        "--view-mode-cycles=0",
        "--persistence-cycles=0",
    ]))
    .expect_err("more than one thousand independent runs must be rejected");
    assert!(error.contains("outside 1..=1000"));
}

#[test]
fn evidence_file_enables_json_without_terminal_redirection() {
    let options = Options::parse(args(&[
        "phase",
        "11",
        "--performance",
        "--evidence-file=docs/report/evidence/phase-11.json",
    ]))
    .expect("valid evidence file option");
    assert!(options.json);
    assert_eq!(
        options.evidence_file.as_deref(),
        Some(std::path::Path::new("docs/report/evidence/phase-11.json"))
    );
}

#[test]
fn command_line_preserves_spaces_and_rejects_missing_or_extra_arguments() {
    let args = |values: &[&str]| {
        values
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        CommandLine::parse(args(&["package-path", "--directory", "folder with spaces"])).unwrap(),
        CommandLine::PackagePath(std::path::PathBuf::from("folder with spaces")),
    );
    for input in [
        vec!["package-path"],
        vec!["package-path", "--directory"],
        vec!["package-path", "--directory", ""],
        vec!["package-path", "--directory", "path", "extra"],
        vec!["package-path", "--unknown", "path"],
    ] {
        assert!(CommandLine::parse(args(&input)).is_err());
    }
}
