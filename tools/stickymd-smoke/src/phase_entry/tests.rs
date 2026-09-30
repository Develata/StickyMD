use super::*;

fn strings(values: &[&str]) -> Vec<String> {
    values.iter().map(|v| (*v).into()).collect()
}

#[test]
fn legacy_entry_capabilities_and_all_shards_preserve_canonical_validation() {
    for phase in Phase::ALL {
        let args = strings(&[phase.number()]);
        let expected = strings(&["phase", phase.number()]);
        assert_eq!(route(&args).unwrap(), expected);
        assert_eq!(
            parse(&args, false).unwrap(),
            CommandLine::parse(expected).unwrap()
        );
    }
    assert_eq!(
        routed(&["all", "--Ci=true", "--CiShard=tests", "--Json=true"]),
        strings(&["all", "--ci", "--ci-shard=tests", "--json"])
    );
    for args in [
        vec!["00", "--Performance=false"],
        vec!["02", "--Runtime=true"],
        vec!["04", "--Resources=true"],
        vec!["08", "--Package=true"],
        vec!["09", "--Json=true"],
        vec!["10", "--EvidenceFile=x"],
        vec!["11-b", "--Ci=true"],
        vec!["11", "--ResourceModule=window"],
        vec!["all", "--EvidenceFile=x"],
        vec!["all", "--CiShard=tests"],
        vec!["all", "--Ci=true", "--CiShard=unknown"],
        vec!["10", "--ResourceModule=window"],
        vec!["09", "--Release=true", "--Package=true"],
    ] {
        assert!(parse(&strings(&args), true).is_err(), "{args:?}");
    }
}
fn routed(values: &[&str]) -> Vec<String> {
    route(&strings(values)).unwrap()
}

#[test]
fn every_retained_action_routes_through_the_canonical_parser() {
    for phase in ["12", "13", "14"] {
        for (input, expected) in [
            (
                vec![
                    "--Json=true",
                    "--EvidenceFile=中文 evidence.json",
                    "--Ci=false",
                ],
                vec![
                    "phase",
                    phase,
                    "--json",
                    "--evidence-file=中文 evidence.json",
                ],
            ),
            (
                vec!["--Readiness=true", "--Explain=true"],
                vec!["qualification", "readiness", "--explain"],
            ),
            (
                vec!["--RemoteRunId=123", "--RemoteAttempt=2"],
                vec!["qualification", "remote", "--run-id=123", "--attempt=2"],
            ),
            (
                vec!["--DownloadedZip=中文 zip.zip"],
                vec!["qualification", "downloaded", "--zip=中文 zip.zip"],
            ),
            (
                vec![
                    "--DecisionKey=key",
                    "--DecisionStatus=status",
                    "--DecisionEvidence=中文 evidence",
                ],
                vec![
                    "qualification",
                    "decision",
                    "--key=key",
                    "--status=status",
                    "--evidence=中文 evidence",
                ],
            ),
        ] {
            let arguments = [&[phase][..], &input].concat();
            assert_eq!(routed(&arguments), strings(&expected));
            assert_eq!(
                parse(&strings(&arguments), false).unwrap(),
                CommandLine::parse(strings(&expected)).unwrap()
            );
        }
        let mut expected = vec!["acceptance", "manual"];
        if phase != "12" {
            expected.push("run");
        }
        assert_eq!(routed(&[phase, "--Manual=true"]), strings(&expected));
    }
    for phase in ["13", "14"] {
        for (name, expected) in [
            ("--Environment=true", vec!["qualification", "environment"]),
            ("--Campaign=true", vec!["qualification", "local"]),
            (
                "--ManualSession=M3",
                vec!["acceptance", "manual", "run", "--session=M3"],
            ),
            ("--ManualList=true", vec!["acceptance", "manual", "list"]),
            (
                "--ManualStatus=true",
                vec!["acceptance", "manual", "status"],
            ),
        ] {
            assert_eq!(routed(&[phase, name]), strings(&expected));
        }
    }
    for (name, expected) in [
        (
            "--SourceFreeze=true",
            vec!["qualification", "source-freeze"],
        ),
        ("--Attribution=true", vec!["qualification", "attribution"]),
        ("--Guided=true", vec!["acceptance", "manual", "guided"]),
        (
            "--GuidedSession=G2",
            vec!["acceptance", "manual", "guided", "--session=G2"],
        ),
    ] {
        assert_eq!(routed(&["14", name]), strings(&expected));
    }
    for name in ["G3", "G4", "G5"] {
        let args = vec![
            "14".into(),
            format!("--{name}=true"),
            format!("--{name}Zip=中文.zip"),
            format!("--{name}Case={name}-01"),
            "--EvidenceFile=target/diagnostic.json".into(),
        ];
        assert!(parse(&args, false).is_ok());
        assert_eq!(
            route(&args).unwrap(),
            vec![
                "qualification".into(),
                name.to_lowercase(),
                "--zip=中文.zip".into(),
                "--evidence-file=target/diagnostic.json".into(),
                format!("--case={name}-01")
            ]
        );
    }
    assert_eq!(
        routed(&["14", "--WindowStress=true", "--TrayCycles=0"]),
        strings(&[
            "qualification",
            "window-stress",
            "--scenario=combined",
            "--runs=10",
            "--collapse-cycles=1000",
            "--tray-cycles=0",
            "--control-cycles=100",
            "--view-mode-cycles=100",
            "--persistence-cycles=100"
        ])
    );
}

#[test]
fn diagnostic_and_conflicting_requests_fail_before_any_dispatch() {
    for mode in ["ResourcePlan", "ResourceFailureFirst", "ResourceResume"] {
        for action in ["SourceFreeze", "Environment", "WindowStress", "Campaign"] {
            let mut args = strings(&[
                "14",
                "--Resources=true",
                "--ResourceResume=true",
                "--EvidenceFile=target/keep.json",
            ]);
            if mode != "ResourceResume" {
                args.push(format!("--{mode}=true"));
            }
            args.push(format!("--{action}=true"));
            assert!(parse(&args, false).unwrap_err().starts_with(mode));
        }
    }
    for args in [
        vec!["14", "--Manual=true", "--ManualSession=M1"],
        vec!["14", "--Guided=true", "--GuidedSession=G1"],
        vec!["14", "--G3Zip=a.zip"],
        vec!["14", "--G4Case=G4-01"],
        vec!["12", "--DecisionKey=key"],
        vec!["13", "--RemoteRunId=123"],
        vec!["14", "--ResourcePlan=true"],
        vec!["14", "--ResourceResume=true"],
        vec!["14", "--WindowStressRuns=0"],
        vec!["14", "--ControlCycles=10001"],
        vec!["14", "--WindowStressScenario=unknown"],
        vec!["13", "--ManualSession=M6"],
        vec!["14", "--ResourceModule=unknown"],
        vec!["14", "--G5Case=G5-05"],
        vec!["12", "--SourceFreeze=true"],
        vec!["14", "--Candidate=true"],
        vec!["12", "--Candidate=true"], // Existing unsupported legacy command must not authorize a freeze.
        vec!["14", "--Ci=true", "--ci=false"],
        vec!["14", "--Ci=maybe"],
    ] {
        assert!(parse(&strings(&args), false).is_err(), "{args:?}");
    }
}

#[test]
fn text_values_are_not_coerced_to_shell_booleans_and_plan_does_not_dispatch() {
    assert_eq!(
        routed(&["14", "--DownloadedZip=false"]),
        strings(&["qualification", "downloaded", "--zip=false"])
    );
    assert_eq!(
        routed(&["14", "--DownloadedZip=0"]),
        strings(&["qualification", "downloaded", "--zip=0"])
    );
    assert_eq!(
        routed(&["12", "--RemoteRunId=0", "--Json=false"]),
        strings(&["phase", "12"])
    );
    assert_eq!(
        parse(&strings(&["14", "--SourceFreeze=true"]), true).unwrap(),
        CommandLine::PhaseEntryPlan(strings(&["qualification", "source-freeze"]))
    );
}
