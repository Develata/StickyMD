//! The receipt's format, the output reader, and the judgement against bad samples: each sample
//! changes one recorded fact of a good receipt and must lose its PASS for that reason.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::output::{self, Header};
use super::*;

const R: &str = "1111111111111111111111111111111111111111";

/// Merged `cargo test` output of this toolchain (Rust 1.97.1, macOS), reduced.
const OUTPUT: &str = "   Compiling stickymd-package v0.2.0
    Finished `test` profile [unoptimized + debuginfo] target(s) in 3.10s
     Running unittests src/lib.rs (target/debug/deps/stickymd_package-492cd95c54ae281b)

running 83 tests
test json::tests::nested ... ok
test result: ok. 83 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s

     Running unittests src/main.rs (target/debug/deps/stickymd_package-8fbe866c61699946)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

     Running tests/cli_exit.rs (target/debug/deps/cli_exit-837ce8600e84f4d0)

running 1 test
test exit_code ... ok

test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.35s

   Doc-tests stickymd_package

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
";

fn counts(passed: u64, ignored: u64) -> Counts {
    Counts {
        ran: passed + ignored,
        passed,
        ignored,
        ..Counts::default()
    }
}

fn target(package: &str, kind: &str, name: &str) -> Target {
    Target {
        package: package.to_owned(),
        kind: kind.to_owned(),
        name: name.to_owned(),
    }
}

/// R's test targets as `cargo metadata` lists them for this workspace (reduced to what the
/// packages hold; `stickymd-mac` and `stickymd-smoke` are binaries only).
fn listed() -> Vec<Target> {
    let mut targets = Vec::new();
    for (package, lib, extra) in [
        ("stickymd-core", true, &["test:property"][..]),
        (
            "stickymd-render",
            true,
            &["test:phase5_semantics", "test:table_math_pipes"][..],
        ),
        ("stickymd-app", true, &[][..]),
        ("stickymd-desktop", true, &[][..]),
        ("stickymd-posix", true, &[][..]),
        ("stickymd-mac", false, &["bin:stickymd-mac"][..]),
        (
            "stickymd-package",
            true,
            &["bin:stickymd-package", "test:cli_exit"][..],
        ),
        (
            "stickymd-smoke",
            false,
            &["bin:stickymd-smoke", "test:headless"][..],
        ),
    ] {
        let library = package.replace('-', "_");
        if lib {
            targets.push(target(package, "lib", &library));
            targets.push(target(package, "doctest", &library));
        }
        for item in extra {
            let (kind, name) = item.split_once(':').unwrap();
            targets.push(target(package, kind, name));
        }
    }
    targets
}

/// A receipt that judges PASS on every verdict: every listed target ran, test by test.
fn good() -> Receipt {
    fixture(R, listed())
}

fn verdicts(receipt: &Receipt) -> (Outcome, Outcome) {
    let verdicts = judge(receipt, R, &listed());
    (verdicts.tests, verdicts.same_key)
}

fn s06(receipt: &Receipt) -> Outcome {
    judge(receipt, R, &listed()).s06
}

fn not_tested(outcome: &Outcome, words: &str) {
    match outcome {
        Outcome::NotTested(reason) => assert!(reason.contains(words), "{reason}"),
        other => panic!("expected NOT_TESTED ({words}), got {other:?}"),
    }
}

fn failed(outcome: &Outcome, words: &str) {
    match outcome {
        Outcome::Fail(reason) => assert!(reason.contains(words), "{reason}"),
        other => panic!("expected FAIL ({words}), got {other:?}"),
    }
}

#[test]
fn the_output_is_read_per_binary_and_doc_test() {
    let sections = output::sections(OUTPUT).unwrap();
    assert_eq!(
        sections
            .iter()
            .map(|section| section.header.clone())
            .collect::<Vec<_>>(),
        [
            Header::Binary("target/debug/deps/stickymd_package-492cd95c54ae281b".into()),
            Header::Binary("target/debug/deps/stickymd_package-8fbe866c61699946".into()),
            Header::Binary("target/debug/deps/cli_exit-837ce8600e84f4d0".into()),
            Header::Doc("stickymd_package".into()),
        ]
    );
    assert_eq!(sections[0].counts, Some(counts(83, 0)));
    assert_eq!(sections[2].counts, Some(counts(1, 0)));
}

#[test]
fn output_that_cannot_be_read_whole_is_refused() {
    // A binary that crashed before its summary.
    let crashed = OUTPUT.replace(
        "test result: ok. 83 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.52s\n",
        "",
    );
    assert!(
        output::sections(&crashed)
            .unwrap_err()
            .contains("no test result")
    );
    // A summary with a field missing, or outside any binary.
    let short = OUTPUT.replace("0 measured; ", "");
    assert!(output::sections(&short).unwrap_err().contains("unreadable"));
    let stray = format!("running 2 tests\n{OUTPUT}");
    assert!(output::sections(&stray).unwrap_err().contains("outside"));
    // `cargo run`'s Running line names no test binary and is ignored.
    let run = format!("     Running `target/debug/stickymd-smoke`\n{OUTPUT}");
    assert_eq!(output::sections(&run).unwrap().len(), 4);
    let failed = OUTPUT.replace(
        "test result: ok. 1 passed; 0 failed",
        "test result: FAILED. 0 passed; 1 failed",
    );
    let sections = output::sections(&failed).unwrap();
    assert_eq!(sections[2].counts.unwrap().failed, 1);
}

#[test]
fn sections_are_named_by_what_cargo_listed_and_nothing_else() {
    let sections = output::sections(OUTPUT).unwrap();
    let executables = vec![
        output::Executable {
            target: target("stickymd-package", "lib", "stickymd_package"),
            path: "target/debug/deps/stickymd_package-492cd95c54ae281b".into(),
        },
        output::Executable {
            target: target("stickymd-package", "bin", "stickymd-package"),
            path: "target/debug/deps/stickymd_package-8fbe866c61699946".into(),
        },
    ];
    let doc = vec![target("stickymd-package", "doctest", "stickymd_package")];
    let listed = |t: &Target, all: &[&str]| output::Listed {
        target: t.clone(),
        all: all.iter().map(|n| (*n).to_owned()).collect(),
        ignored: Vec::new(),
    };
    let listings = vec![
        listed(&executables[0].target, &["json::tests::nested"]),
        listed(&executables[1].target, &[]),
        listed(&doc[0], &[]),
    ];
    let (runs, problems) = output::attribute(&sections, &executables, &doc, &listings);
    assert_eq!(runs.len(), 3);
    assert_eq!(runs[0].cases.len(), 1);
    assert_eq!(runs[0].cases[0].outcome, CaseOutcome::Ok);
    assert_eq!(runs[2].target, doc[0]);
    assert_eq!(problems.len(), 1);
    assert!(problems[0].contains("cli_exit"), "{problems:?}");
}

#[test]
fn the_doc_tested_libraries_come_from_cargo_metadata() {
    let metadata = r#"{"packages":[
        {"name":"stickymd-core","targets":[
            {"kind":["lib"],"name":"stickymd_core","doctest":true},
            {"kind":["test"],"name":"property","doctest":false}]},
        {"name":"stickymd-win","targets":[{"kind":["bin"],"name":"stickymd-win","doctest":false}]},
        {"name":"stickymd-smoke","targets":[{"kind":["bin"],"name":"stickymd-smoke","doctest":false}]}]}"#;
    assert_eq!(
        output::doc_libraries(metadata).unwrap(),
        [target("stickymd-core", "doctest", "stickymd_core")]
    );
}

#[test]
fn the_receipt_round_trips_and_is_read_strictly() {
    let receipt = good();
    let text = render(&receipt);
    assert_eq!(parse(&text).unwrap(), receipt);
    let extra = text.replacen("\"problems\":[]", "\"problems\":[],\"passed\":true", 1);
    assert!(parse(&extra).unwrap_err().contains("exactly the fields"));
    let missing = text.replacen(",\"exit\":0,\"expected\"", ",\"expected\"", 1);
    assert_ne!(missing, text);
    assert!(parse(&missing).is_err());
    let mistyped = text.replacen("\"clean\":true", "\"clean\":\"true\"", 1);
    assert!(parse(&mistyped).is_err());
    let duplicated = text.replacen(
        "\"release_source\":",
        "\"release_source\":\"x\",\"release_source\":",
        1,
    );
    assert!(parse(&duplicated).unwrap_err().contains("duplicate"));
    assert!(parse(&format!("{text}{{}}")).is_err());
    // Schema 2 (no run identity, no driver, no ending) and schema 1 are refused (review round 4).
    for older in [1, 2] {
        let older = text.replacen(
            "\"schema_version\":3",
            &format!("\"schema_version\":{older}"),
            1,
        );
        assert_ne!(older, text);
        assert!(parse(&older).unwrap_err().contains("schema 3"));
    }
}

#[test]
fn a_good_receipt_passes_both_rows() {
    assert_eq!(verdicts(&good()), (Outcome::Pass, Outcome::Pass));
}

#[test]
fn a_receipt_for_another_commit_or_an_unclean_checkout_proves_nothing() {
    let mut other = good();
    other.release_source = "2".repeat(40);
    not_tested(&verdicts(&other).0, "not 1111");
    let mut moved = good();
    moved.end.head = "2".repeat(40);
    let (tests, same_key) = verdicts(&moved);
    not_tested(&tests, "at the end");
    not_tested(&same_key, "at the end");
    let mut dirty = good();
    dirty.start.clean = false;
    not_tested(&verdicts(&dirty).0, "at the start");
    let mut linux = good();
    linux.host.product_name = "Debian".into();
    not_tested(&verdicts(&linux).0, "not macOS arm64");
}

#[test]
fn a_filtered_or_narrowed_command_proves_nothing() {
    let mut filtered = good();
    filtered.command.extend(["--".into(), "note_root".into()]);
    not_tested(&verdicts(&filtered).0, "not the row's command");
    let mut narrowed = good();
    narrowed.command.truncate(narrowed.command.len() - 2);
    not_tested(&verdicts(&narrowed).0, "not the row's command");
    let mut same_key = good();
    same_key.explicit[0].command.pop();
    not_tested(
        &verdicts(&same_key).1,
        "same-key is not the required command",
    );
}

#[test]
fn a_package_or_binary_that_did_not_run_leaves_the_row_open() {
    let mut missing_package = good();
    missing_package
        .expected
        .retain(|target| target.package != "stickymd-smoke");
    missing_package
        .binaries
        .retain(|run| run.target.package != "stickymd-smoke");
    not_tested(&verdicts(&missing_package).0, "not R's");
    let mut unrun = good();
    unrun.binaries.pop();
    not_tested(&verdicts(&unrun).0, "covered 19 of the 20");
    let mut doubled = good();
    let again = doubled.binaries[0].clone();
    doubled.binaries.push(again);
    not_tested(&verdicts(&doubled).0, "twice");
    let mut problem = good();
    problem.problems.push("the test output: unreadable".into());
    not_tested(&verdicts(&problem).0, "unreadable");
}

#[test]
fn filtered_or_inconsistent_counts_leave_the_row_open() {
    let mut filtered = good();
    filtered.binaries[3].counts.as_mut().unwrap().filtered_out = 1;
    not_tested(&verdicts(&filtered).0, "filtered");
    let mut uneven = good();
    uneven.binaries[3].counts.as_mut().unwrap().ran += 1;
    not_tested(&verdicts(&uneven).0, "are not the");
}

#[test]
fn a_recorded_failure_is_a_fail() {
    let mut failing = good();
    failing.binaries[2].counts.as_mut().unwrap().failed = 1;
    failing.binaries[2].counts.as_mut().unwrap().ran += 1;
    failed(&verdicts(&failing).0, "a test failed");
    let mut exited = good();
    exited.exit = Exit::Code(101);
    failed(&verdicts(&exited).0, "Code(101)");
    let mut killed = good();
    killed.exit = Exit::Signal;
    failed(&verdicts(&killed).0, "Signal");
    // Review round 3, NEEDS_VERIFICATION: a failed test alone, the counts left as they are.
    let mut case = good();
    case.binaries[2].cases[0].outcome = CaseOutcome::Failed;
    failed(&verdicts(&case).0, "a test failed");
}

#[test]
fn both_same_key_tests_must_pass() {
    let mut renamed = good();
    renamed.explicit[0].cases[1].name = "note_root::tests::renamed".into();
    not_tested(&verdicts(&renamed).1, "not exactly");
    let mut skipped = good();
    skipped.explicit[0].cases[1].outcome = CaseOutcome::Ignored;
    skipped.explicit[0].counts = Some(Counts {
        ran: 2,
        passed: 1,
        ignored: 1,
        filtered_out: 10,
        ..Counts::default()
    });
    not_tested(&verdicts(&skipped).1, "not exactly");
    let mut failing = good();
    failing.explicit[0].cases[1].outcome = CaseOutcome::Failed;
    failing.explicit[0].exit = Exit::Code(101);
    failed(&verdicts(&failing).1, "recorded a failure");
    let mut silent = good();
    silent.explicit[0].counts = None;
    not_tested(&verdicts(&silent).1, "printed no result");
    // The main row does not depend on the same-key run.
    assert_eq!(verdicts(&failing).0, Outcome::Pass);
}

/// Review round 2, finding 3: the same-key verdict holds only for stickymd-mac's binary running
/// exactly its two tests on the probed temporary volume, not for any two passing tests.
#[test]
fn the_same_key_run_must_be_stickymd_mac_s_two_tests_on_the_probed_volume() {
    let mut elsewhere = good();
    elsewhere.explicit[0].target = Some(target("stickymd-core", "lib", "stickymd_core"));
    not_tested(&verdicts(&elsewhere).1, "not attributed");
    // The binary's own listing, as the runner took it before any run, lacks one of the two.
    let mut unlisted = good();
    unlisted.explicit[0]
        .listed
        .retain(|name| name != SAME_KEY_TESTS[1]);
    not_tested(&verdicts(&unlisted).1, "stickymd-mac's binary lists");
    let mut volume = good();
    volume.explicit[0].temporary_volume = None;
    not_tested(&verdicts(&volume).1, "did not measure");
}

/// Decision (a): P16-S06 holds on its explicit run of the exFAT test, exactly one pass.
#[test]
fn s06_holds_on_its_explicit_run_only() {
    assert_eq!(s06(&good()), Outcome::Pass);
    let mut missing = good();
    missing.explicit.pop();
    not_tested(&s06(&missing), "explicit runs are");
    let mut none_ran = good();
    none_ran.explicit[1].cases.clear();
    none_ran.explicit[1].counts = Some(Counts {
        filtered_out: 11,
        ..Counts::default()
    });
    not_tested(&s06(&none_ran), "not exactly");
}

/// Review round 2, finding 2: a binary that lists tests but ran none, or whose printed outcomes
/// disagree with its summary, proves nothing.
#[test]
fn a_binary_that_lists_tests_but_ran_none_is_not_a_pass() {
    let mut silent = good();
    let run = &mut silent.binaries[0];
    run.counts = Some(Counts::default());
    for case in &mut run.cases {
        case.outcome = CaseOutcome::NotRun;
    }
    not_tested(&verdicts(&silent).0, "listed but did not run");
    let mut ignored = good();
    ignored.binaries[0].cases[0].outcome = CaseOutcome::Ignored;
    ignored.binaries[0].counts.as_mut().unwrap().passed -= 1;
    ignored.binaries[0].counts.as_mut().unwrap().ignored += 1;
    not_tested(&verdicts(&ignored).0, "its listing says ignored=false");
    let mut contradicted = good();
    contradicted.binaries[0].cases[0].outcome = CaseOutcome::Failed;
    failed(&verdicts(&contradicted).0, "a test failed");
}

#[test]
fn test_lines_and_listings_are_read_exactly() {
    let text = "     Running unittests src/lib.rs (target/debug/deps/x-1)

running 3 tests
test a::b ... ok
test a::c - should panic ... ok
test a::d ... ignored, mounts a disk image

test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.01s
";
    let sections = output::sections(text).unwrap();
    assert_eq!(
        sections[0].tests,
        [
            ("a::b".to_owned(), CaseOutcome::Ok),
            ("a::c".to_owned(), CaseOutcome::Ok),
            ("a::d".to_owned(), CaseOutcome::Ignored),
        ]
    );
    assert_eq!(
        output::listing("a::b: test\na::c: test\n\n").unwrap(),
        ["a::b", "a::c"]
    );
    assert!(output::listing("a::b: test\n3 tests, 0 benchmarks\n").is_err());
    let (cases, unlisted) = output::cases(
        &["a::b".to_owned(), "a::e".to_owned()],
        &[],
        &sections[0].tests,
    );
    assert_eq!(cases[1].outcome, CaseOutcome::NotRun);
    assert_eq!(
        unlisted,
        ["a::c ran but is not listed", "a::d ran but is not listed"]
    );
}

fn root() -> std::path::PathBuf {
    crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap()
}

/// The `run` text of the step named `name` in a workflow, its folded lines joined by spaces.
fn step_command(workflow: &str, name: &str) -> Option<String> {
    let mut lines = workflow
        .lines()
        .skip_while(|line| line.trim() != format!("- name: {name}"));
    lines.next()?;
    let run = lines.next()?.trim();
    if let Some(single) = run.strip_prefix("run: ").filter(|rest| *rest != ">-") {
        return Some(single.to_owned());
    }
    let folded: Vec<&str> = lines
        .take_while(|line| line.starts_with("          "))
        .map(str::trim)
        .collect();
    Some(folded.join(" "))
}

/// The row's command is exactly the self-check step's, so a change to one shows in the other.
#[test]
fn the_command_is_the_release_workflow_self_check() {
    let workflow =
        std::fs::read_to_string(root().join(".github/workflows/release-macos.yml")).unwrap();
    let step = "Test the shared crates, the macOS host and the release tooling";
    assert_eq!(step_command(&workflow, step), Some(command().join(" ")));
    // A filter appended to the step no longer matches.
    let filtered = workflow.replace("-p stickymd-smoke\n", "-p stickymd-smoke -- note_root\n");
    assert_ne!(step_command(&filtered, step), Some(command().join(" ")));
}

#[test]
fn a_target_dropped_from_both_lists_is_caught() {
    let mut narrowed = good();
    let table = target("stickymd-render", "test", "table_math_pipes");
    narrowed.expected.retain(|t| *t != table);
    narrowed.binaries.retain(|run| run.target != table);
    not_tested(&verdicts(&narrowed).0, "table_math_pipes");
}

#[test]
fn contradictory_or_overflowing_counts_are_refused() {
    let lying = OUTPUT.replace(
        "test result: ok. 1 passed; 0 failed",
        "test result: ok. 0 passed; 1 failed",
    );
    assert!(
        output::sections(&lying)
            .unwrap_err()
            .contains("contradicts")
    );
    let silent = OUTPUT.replace(
        "test result: ok. 1 passed; 0 failed",
        "test result: FAILED. 1 passed; 0 failed",
    );
    assert!(
        output::sections(&silent)
            .unwrap_err()
            .contains("contradicts")
    );
    let mut wrapped = good();
    wrapped.binaries[0].counts = Some(Counts {
        ran: 0,
        passed: u64::MAX,
        ignored: 1,
        ..Counts::default()
    });
    not_tested(&verdicts(&wrapped).0, "are not the");
    // Review round 3, NEEDS_VERIFICATION: a measured test alone breaks only the sum.
    let mut measured = good();
    measured.binaries[0].counts.as_mut().unwrap().measured = 1;
    not_tested(&verdicts(&measured).0, "are not the");
}

/// Review round 3, NEEDS_VERIFICATION: each count condition of an explicit run alone, the
/// cases left as they are. An ignored or measured test needs no check of its own: with
/// `ran == passed == 2` the sum leaves none, so those samples break the sum alone.
#[test]
fn the_same_key_run_is_exactly_its_two_tests() {
    let counts = |ran, passed, ignored, measured| Counts {
        ran,
        passed,
        ignored,
        measured,
        filtered_out: 40,
        ..Counts::default()
    };
    for (counts, breaks) in [
        (counts(3, 2, 1, 0), "ran"),
        (counts(2, 1, 1, 0), "passed"),
        (counts(2, 2, 1, 0), "the sum, by an ignored test"),
        (counts(2, 2, 0, 1), "the sum, by a measured test"),
    ] {
        let mut receipt = good();
        receipt.explicit[0].counts = Some(counts);
        assert!(
            matches!(verdicts(&receipt).1, Outcome::NotTested(ref reason) if reason.contains("not exactly")),
            "{breaks}"
        );
    }
}

#[test]
fn the_listed_targets_come_from_cargo_metadata() {
    let package =
        |name: &str, targets: &str| format!(r#"{{"name":"{name}","targets":[{targets}]}}"#);
    let lib =
        |name: &str| format!(r#"{{"kind":["lib"],"name":"{name}","test":true,"doctest":true}}"#);
    let mut packages: Vec<String> = PACKAGES
        .iter()
        .filter(|p| **p != "stickymd-mac")
        .map(|p| package(p, &lib(&p.replace('-', "_"))))
        .collect();
    packages.push(package(
        "stickymd-mac",
        r#"{"kind":["bin"],"name":"stickymd-mac","test":true,"doctest":false},
           {"kind":["custom-build"],"name":"build-script-build","test":false,"doctest":false},
           {"kind":["bench"],"name":"frame","test":false,"doctest":false}"#,
    ));
    let metadata = format!(r#"{{"packages":[{}]}}"#, packages.join(","));
    let targets = output::listed_targets(&metadata).unwrap();
    assert!(targets.contains(&target("stickymd-mac", "bin", "stickymd-mac")));
    assert!(targets.contains(&target("stickymd-core", "doctest", "stickymd_core")));
    assert!(
        !targets
            .iter()
            .any(|t| t.name == "frame" || t.name == "build-script-build")
    );
    assert_eq!(targets.len(), 7 * 2 + 1);
    let without_mac = metadata.replace(
        r#""name":"stickymd-mac","test":true"#,
        r#""name":"stickymd-mac","test":false"#,
    );
    assert!(
        output::listed_targets(&without_mac)
            .unwrap_err()
            .contains("stickymd-mac")
    );
}

/// The classified ignored tests (review round 4, BLOCKER 3): no test twice, and each explicit
/// run's ignored tests carry exactly that run's role, in stickymd-mac's binary.
#[test]
fn the_ignored_table_names_each_test_once_and_the_explicit_runs_by_role() {
    use super::ignored::{IGNORED, Role, role};
    let mut keys: Vec<(&str, &str, &str, &str)> = IGNORED
        .iter()
        .map(|(p, k, t, n, _)| (*p, *k, *t, *n))
        .collect();
    keys.sort_unstable();
    assert!(keys.windows(2).all(|pair| pair[0] != pair[1]));
    for (item, tests) in EXPLICIT {
        for test in tests.iter().filter(|test| **test != SAME_KEY_TESTS[0]) {
            assert_eq!(
                role(&explicit_target(), test),
                Some(Role::Explicit(item)),
                "{test}"
            );
        }
    }
    for (_, _, _, name, role) in IGNORED {
        if let Role::Explicit(item) = role {
            assert!(
                EXPLICIT
                    .iter()
                    .any(|(i, tests)| *i == item && tests.contains(&name)),
                "{name}"
            );
        }
    }
}

/// Review round 4, BLOCKER 3: a test libtest lists as ignored that the table does not classify
/// (here as a multi-line `cfg_attr` would leave it) keeps the default tests open, though it was
/// reported faithfully as ignored.
#[test]
fn an_unclassified_ignored_test_leaves_the_default_tests_open() {
    let mut receipt = good();
    let run = &mut receipt.binaries[0];
    run.cases.push(Case {
        name: "persistence::storage::tests::a_failed_directory_sync_is_reported".into(),
        ignored: true,
        outcome: CaseOutcome::Ignored,
    });
    run.counts.as_mut().unwrap().ran += 1;
    run.counts.as_mut().unwrap().ignored += 1;
    not_tested(&verdicts(&receipt).0, "not classified");
}

/// Review round 4, SHOULD_FIX: a process that never started, or whose end could not be followed,
/// is not a result; a failure it printed still is.
#[test]
fn a_run_without_a_result_is_open_and_a_printed_failure_still_fails() {
    for exit in [Exit::NotStarted, Exit::Unknown] {
        let mut receipt = good();
        receipt.exit = exit;
        not_tested(&verdicts(&receipt).0, "has no result");
        receipt.binaries[2].cases[0].outcome = CaseOutcome::Failed;
        failed(&verdicts(&receipt).0, "a test failed");
        let mut explicit = good();
        explicit.explicit[1].exit = exit;
        assert!(matches!(s06(&explicit), Outcome::NotTested(ref r) if r.contains("has no result")));
    }
}

/// Review round 4, BLOCKER 1: the driver the run on R built counts as R's only when the receipt
/// is R's and the build was the documented one.
#[test]
fn the_receipt_names_r_s_driver_only_for_r_and_the_documented_build() {
    use super::driver_of;
    assert_eq!(driver_of(&good(), R).unwrap(), "1".repeat(64));
    assert!(driver_of(&good(), &"f".repeat(40)).is_err());
    let mut other = good();
    other.driver.command.pop();
    assert!(driver_of(&other, R).unwrap_err().contains("documented"));
    let mut none = good();
    none.driver.sha256 = Err("the driver build failed".into());
    assert!(driver_of(&none, R).unwrap_err().contains("built no driver"));
}

/// Review round 4, SHOULD_FIX: libtest's mode suffixes, an ignore reason holding ` ... `, and a
/// doc-test section with rustdoc's two groups (merged, then standalone), as rustc 1.99 prints
/// them.
#[test]
fn modes_reasons_and_two_doc_test_groups_are_read() {
    let output = "     Running unittests src/lib.rs (target/debug/deps/doctry-0123456789abcdef)

running 3 tests
test tests::slow ... ignored, slow ... really
test tests::plain ... ok
test tests::panics - should panic ... ok

test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.00s

   Doc-tests doctry

running 3 tests
test src/lib.rs - (line 17) - compile ... ok
test src/lib.rs - (line 9) - should panic ... ok
test src/lib.rs - (line 1) ... ok

test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s


running 3 tests
test src/lib.rs - (line 21) ... ignored
test src/lib.rs - (line 5) - compile fail ... ok
test src/lib.rs - (line 13) ... ok

test result: ok. 2 passed; 0 failed; 1 ignored; 0 measured; 0 filtered out; finished in 0.45s

all doctests ran in 0.96s; merged doctests compilation took 0.17s
";
    let sections = output::sections(output).unwrap();
    assert_eq!(sections.len(), 2);
    let names =
        |i: usize| -> Vec<&str> { sections[i].tests.iter().map(|(n, _)| n.as_str()).collect() };
    assert_eq!(names(0), ["tests::slow", "tests::plain", "tests::panics"]);
    assert_eq!(sections[0].tests[0].1, CaseOutcome::Ignored);
    assert_eq!(
        names(1),
        [
            "src/lib.rs - (line 17)",
            "src/lib.rs - (line 9)",
            "src/lib.rs - (line 1)",
            "src/lib.rs - (line 21)",
            "src/lib.rs - (line 5)",
            "src/lib.rs - (line 13)",
        ]
    );
    assert_eq!(
        sections[1].counts,
        Some(Counts {
            ran: 6,
            passed: 5,
            ignored: 1,
            ..Counts::default()
        })
    );
    // A test binary still holds one group only.
    let doubled = output.replacen("   Doc-tests doctry\n", "", 1);
    assert!(output::sections(&doubled).is_err());
}

/// Review round 4: a name printed `ok` and then `FAILED` stays a failure, besides the problem.
#[test]
fn a_failure_printed_after_an_ok_is_kept() {
    let printed = vec![
        ("a::b".to_owned(), CaseOutcome::Ok),
        ("a::b".to_owned(), CaseOutcome::Failed),
    ];
    let (cases, problems) = output::cases(&["a::b".to_owned()], &[], &printed);
    assert_eq!(cases[0].outcome, CaseOutcome::Failed);
    assert_eq!(problems, ["a::b is printed twice"]);
}

/// The same-key tests exist under these names in `stickymd-mac`, the second one ignored.
#[test]
fn the_same_key_tests_exist_where_the_command_names_them() {
    let sources =
        std::fs::read_to_string(root().join("apps/stickymd-mac/src/note_root/tests.rs")).unwrap();
    let main = std::fs::read_to_string(root().join("apps/stickymd-mac/src/main.rs")).unwrap();
    let note_root =
        std::fs::read_to_string(root().join("apps/stickymd-mac/src/note_root.rs")).unwrap();
    assert!(main.contains("\nmod note_root;"));
    assert!(note_root.contains("\nmod tests;"));
    let [default, ignored] = SAME_KEY_TESTS.map(|path| path.rsplit("::").next().unwrap());
    let at = |name: &str| sources.find(&format!("fn {name}()")).unwrap();
    let before = |name: &str| &sources[..at(name)];
    assert!(before(default).trim_end().ends_with("#[test]"));
    assert!(
        before(ignored)
            .trim_end()
            .rsplit('\n')
            .next()
            .unwrap()
            .starts_with("#[ignore")
    );
}

/// Review round 3, B3: a test printed twice (first ok, then FAILED), listed twice, or listed
/// ignored without being listed is a problem, never folded into its first entry.
#[test]
fn duplicate_or_unknown_names_are_problems_not_first_entries() {
    let names = |list: &[&str]| list.iter().map(|n| (*n).to_owned()).collect::<Vec<_>>();
    let printed = vec![
        ("a::b".to_owned(), CaseOutcome::Ok),
        ("a::b".to_owned(), CaseOutcome::Failed),
    ];
    let (_, problems) = output::cases(&names(&["a::b"]), &[], &printed);
    assert_eq!(problems, ["a::b is printed twice"]);
    let (_, problems) = output::cases(&names(&["a::b", "a::b"]), &[], &[]);
    assert_eq!(problems, ["a::b is listed twice"]);
    let (_, problems) = output::cases(&names(&["a::b"]), &names(&["a::z"]), &[]);
    assert_eq!(problems, ["a::z is listed ignored but not listed"]);
}

/// Review round 3, B4: P18-L08 holds only on both volumes as its tests measured them; each
/// volume alone decides.
#[test]
fn the_same_key_run_needs_both_measured_volumes() {
    let mut no_image = good();
    no_image.explicit[0].image_volume = None;
    not_tested(&verdicts(&no_image).1, "did not measure");
    let mut sensitive_temporary = good();
    sensitive_temporary.explicit[0].temporary_volume = Some("case-sensitive".into());
    not_tested(&verdicts(&sensitive_temporary).1, "did not measure");
    let mut insensitive_image = good();
    insensitive_image.explicit[0].image_volume = Some("case-insensitive".into());
    not_tested(&verdicts(&insensitive_image).1, "did not measure");
}

/// Review round 3, S2 and the single-fact rule: an explicit run fails on its exit code alone, on
/// a failed test alone, and on a failure count alone.
#[test]
fn an_explicit_run_fails_on_each_recorded_failure_alone() {
    let mut exited = good();
    exited.explicit[1].exit = Exit::Code(101);
    assert!(matches!(s06(&exited), Outcome::Fail(_)));
    let mut case = good();
    case.explicit[1].cases[0].outcome = CaseOutcome::Failed;
    assert!(matches!(s06(&case), Outcome::Fail(_)));
    let mut counted = good();
    if let Some(counts) = &mut counted.explicit[1].counts {
        counts.failed = 1;
    }
    assert!(matches!(s06(&counted), Outcome::Fail(_)));
}

/// Review round 3, S4: a binary is matched by its whole printed path, and a summary line cut
/// short is not a result.
#[test]
fn a_same_named_binary_elsewhere_or_a_cut_summary_is_refused() {
    let sections = output::sections(OUTPUT).unwrap();
    let elsewhere = vec![output::Executable {
        target: target("stickymd-package", "lib", "stickymd_package"),
        path: "/other/target/release/stickymd_package-492cd95c54ae281b".into(),
    }];
    assert!(output::target_of(&sections[0], &elsewhere, &[]).is_none());
    let here = vec![output::Executable {
        target: target("stickymd-package", "lib", "stickymd_package"),
        path: "/repo/target/debug/deps/stickymd_package-492cd95c54ae281b".into(),
    }];
    assert!(output::target_of(&sections[0], &here, &[]).is_some());
    let cut = OUTPUT.replace("finished in 0.52s", "finished in 0.");
    assert!(output::sections(&cut).unwrap_err().contains("unreadable"));
}

/// Fix round, BLOCKER 4: a classified test moved to a target of the same package and name but
/// another kind (a `lib` test moved to `tests/`) is a new ignored test until it is classified.
#[test]
fn an_ignored_test_moved_to_a_target_of_another_kind_is_unclassified() {
    use super::ignored::{IGNORED, role};
    let (package, kind, name, test, _) = IGNORED[0];
    assert!(role(&target(package, kind, name), test).is_some());
    let moved = if kind == "test" { "lib" } else { "test" };
    assert!(role(&target(package, moved, name), test).is_none());
}

/// Fix round, BLOCKER 5: a test registered twice is listed twice; the explicit run, which runs
/// it once, does not hold.
#[test]
fn an_explicit_test_listed_twice_does_not_hold() {
    let mut doubled = good();
    doubled.explicit[1].listed.push(S06_TESTS[0].to_owned());
    assert!(matches!(s06(&doubled), Outcome::NotTested(ref r) if r.contains("twice")));
}

/// Fix round, BLOCKER 3: a `FAILED` printed before the output broke off (no summary, say past
/// the size cap) is kept from the raw output to the verdict, and a `running 3 tests` in a failed
/// test's captured output is not read as a control line.
#[test]
fn a_failure_printed_before_the_output_breaks_off_is_a_fail() {
    let head =
        "     Running unittests src/lib.rs (target/debug/deps/stickymd_package-492cd95c54ae281b)

running 2 tests
test json::tests::nested ... FAILED
test json::tests::flat ... ok

failures:

---- json::tests::nested stdout ----
running 3 tests
test inner ... ok
thread 'json::tests::nested' panicked at src/json.rs:9:5
";
    let library = target("stickymd-package", "lib", "stickymd_package");
    let executables = vec![output::Executable {
        target: library.clone(),
        path: "/repo/target/debug/deps/stickymd_package-492cd95c54ae281b".into(),
    }];
    let listings = vec![output::Listed {
        target: library.clone(),
        all: vec!["json::tests::nested".into(), "json::tests::flat".into()],
        ignored: Vec::new(),
    }];
    // Whole, the captured lines are not tests and the counts hold.
    let whole = format!(
        "{head}\nfailures:\n    json::tests::nested\n\ntest result: FAILED. 1 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.01s\n"
    );
    let sections = output::sections(&whole).unwrap();
    assert_eq!(sections[0].tests.len(), 2);
    // Broken mid-stream by an unreadable turn (another binary starts before this result): the
    // sections read so far are kept.
    let tests_only = &head[..head.find("\nfailures:").unwrap()];
    let broken = output::parse(&format!(
        "{tests_only}\n     Running unittests src/main.rs (target/debug/deps/other-0123456789abcdef)\n"
    ));
    assert!(broken.error.is_some());
    assert_eq!(broken.sections[0].tests[0].1, CaseOutcome::Failed);
    // Cut short: an error, and the failure stays.
    let parsed = output::parse(head);
    assert!(parsed.error.is_some());
    let (runs, _) = output::attribute(&parsed.sections, &executables, &[], &listings);
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0].counts, None);
    let mut receipt = good();
    let at = receipt
        .binaries
        .iter()
        .position(|run| run.target == library)
        .unwrap();
    receipt.binaries[at] = runs[0].clone();
    receipt.exit = Exit::Unknown;
    receipt
        .problems
        .push("the test output was cut short".into());
    failed(&verdicts(&receipt).0, "a test failed");
    // Without a failure, a section without its summary never passes.
    receipt.binaries[at].cases[0].outcome = CaseOutcome::Ok;
    receipt.exit = Exit::Code(0);
    receipt.problems.clear();
    not_tested(&verdicts(&receipt).0, "printed no test result");
}

/// Fix round, SHOULD_FIX 9: schema 3 values have their forms: a SHA-256 for the driver, the
/// runner's own run identity, and volumes only on P18-L08's run, as one of two words.
#[test]
fn schema_3_values_have_their_forms() {
    let text = render(&good());
    for (from, to) in [
        (
            format!("\"sha256\":\"{}\"", "1".repeat(64)),
            "\"sha256\":\"1111\"".to_owned(),
        ),
        (
            "\"run\":\"1764000000000000000-4242\"".to_owned(),
            "\"run\":\"\"".to_owned(),
        ),
        (
            "\"run\":\"1764000000000000000-4242\"".to_owned(),
            "\"run\":\"yesterday\"".to_owned(),
        ),
        (
            "\"temporary_volume\":\"case-insensitive\"".to_owned(),
            "\"temporary_volume\":\"maybe\"".to_owned(),
        ),
    ] {
        let bad = text.replacen(&from, &to, 1);
        assert_ne!(bad, text, "{from}");
        assert!(parse(&bad).is_err(), "{to}");
    }
    let mut s06 = good();
    s06.explicit[1].image_volume = Some("case-sensitive".into());
    assert!(
        parse(&render(&s06))
            .unwrap_err()
            .contains("measures no volume")
    );
}
