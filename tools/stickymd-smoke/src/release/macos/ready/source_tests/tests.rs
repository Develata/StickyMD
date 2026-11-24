//! The receipt's format, the output reader, and the judgement against bad samples: each sample
//! changes one recorded fact of a good receipt and must lose its PASS for that reason.

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
            Header::Binary("stickymd_package-492cd95c54ae281b".into()),
            Header::Binary("stickymd_package-8fbe866c61699946".into()),
            Header::Binary("cli_exit-837ce8600e84f4d0".into()),
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
            file: "stickymd_package-492cd95c54ae281b".into(),
            path: "target/debug/deps/stickymd_package-492cd95c54ae281b".into(),
        },
        output::Executable {
            target: target("stickymd-package", "bin", "stickymd-package"),
            file: "stickymd_package-8fbe866c61699946".into(),
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
    let missing = text.replacen(",\"exit_code\":0,\"expected\"", ",\"expected\"", 1);
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
    let older = text.replacen("\"schema_version\":2", "\"schema_version\":1", 1);
    assert!(parse(&older).is_err());
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
    filtered.binaries[3].counts.filtered_out = 1;
    not_tested(&verdicts(&filtered).0, "filtered");
    let mut uneven = good();
    uneven.binaries[3].counts.ran += 1;
    not_tested(&verdicts(&uneven).0, "are not the");
}

#[test]
fn a_recorded_failure_is_a_fail() {
    let mut failing = good();
    failing.binaries[2].counts.failed = 1;
    failing.binaries[2].counts.ran += 1;
    failed(&verdicts(&failing).0, "a test failed");
    let mut exited = good();
    exited.exit_code = Some(101);
    failed(&verdicts(&exited).0, "Some(101)");
    let mut killed = good();
    killed.exit_code = None;
    failed(&verdicts(&killed).0, "None");
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
    failing.explicit[0].exit_code = Some(101);
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
    let mut unlisted = good();
    let mac = unlisted
        .binaries
        .iter_mut()
        .find(|run| run.target == explicit_target())
        .unwrap();
    mac.cases.retain(|c| c.name != SAME_KEY_TESTS[1]);
    mac.counts.ran -= 1;
    mac.counts.ignored -= 1;
    not_tested(
        &verdicts(&unlisted).1,
        "not a test stickymd-mac's binary lists",
    );
    let mut volume = good();
    volume.explicit[0].temporary_volume = None;
    not_tested(&verdicts(&volume).1, "not shown case-insensitive");
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
    run.counts = Counts::default();
    for case in &mut run.cases {
        case.outcome = CaseOutcome::NotRun;
    }
    not_tested(&verdicts(&silent).0, "listed but did not run");
    let mut ignored = good();
    ignored.binaries[0].cases[0].outcome = CaseOutcome::Ignored;
    ignored.binaries[0].counts.passed -= 1;
    ignored.binaries[0].counts.ignored += 1;
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
    assert_eq!(unlisted, ["a::c", "a::d"]);
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
    wrapped.binaries[0].counts = Counts {
        ran: 0,
        passed: u64::MAX,
        ignored: 1,
        ..Counts::default()
    };
    not_tested(&verdicts(&wrapped).0, "are not the");
}

#[test]
fn the_same_key_run_is_exactly_its_two_tests() {
    for counts in [
        Counts {
            ran: 3,
            passed: 2,
            ignored: 1,
            filtered_out: 40,
            ..Counts::default()
        },
        Counts {
            ran: 3,
            passed: 2,
            measured: 1,
            filtered_out: 40,
            ..Counts::default()
        },
    ] {
        let mut receipt = good();
        receipt.explicit[0].counts = Some(counts);
        not_tested(&verdicts(&receipt).1, "not exactly");
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

/// Every `#[ignore]` test of the eight packages, by file and name, and what it is for. A Source
/// row whose test is ignored is not proven by the default run (plan 11 #macos-release-gate): it
/// needs an explicit run in the receipt (P16-S06, P18-L08). A new or moved ignored test fails this
/// until it is classified here (review round 2, SHOULD_FIX 9).
#[test]
fn every_ignored_test_of_the_eight_packages_is_classified() {
    const IGNORED: &[(&str, &str, &str)] = &[
        (
            "apps/stickymd-mac/src/assets/transaction.rs",
            "image_paste_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "apps/stickymd-mac/src/export/mod.rs",
            "export_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "apps/stickymd-mac/src/note_root/tests.rs",
            "on_a_case_sensitive_volume_case_makes_another_key",
            "P18-L08, explicit run",
        ),
        (
            "apps/stickymd-mac/src/paths.rs",
            "an_exfat_volume_is_refused_and_keeps_no_probe_file",
            "P16-S06, explicit run",
        ),
        (
            "crates/stickymd-app/src/execution/preview_worker.rs",
            "phase6_source_edit_during_math_build_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-app/src/flow/window/reducer.rs",
            "phase8_performance_window_reducer_and_geometry",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-app/src/source_search.rs",
            "phase14_one_mib_unicode_case_insensitive_search_p95_is_bounded",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-core/src/assets.rs",
            "phase7_managed_scan_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-desktop/src/controls/tests.rs",
            "phase8_performance_control_layout_and_hit_test",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-desktop/src/shell/mod.rs",
            "phase3_source_pipeline_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-posix/src/user.rs",
            "print_home_directory",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/image.rs",
            "phase7_image_decode_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/math/engine.rs",
            "phase6_formula_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/math_action.rs",
            "phase11b_performance_math_action_one_mib",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/math_action.rs",
            "phase11b_performance_math_action_stays_linear_in_shifted_paragraphs",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline.rs",
            "phase10_zoom_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline.rs",
            "phase5_preview_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline.rs",
            "phase6_math_document_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/audit_tests.rs",
            "phase5_preview_release_baseline_image_adapter_scroll",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/audit_tests.rs",
            "phase5_preview_release_baseline_long_code_scroll",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/horizontal_tests.rs",
            "phase5_preview_release_baseline_horizontal_long_line",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/image_refresh_tests.rs",
            "phase7_image_band_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/resource_tests.rs",
            "phase10_preview_resource_cache_retention_diagnostic",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/resource_tests.rs",
            "phase10_preview_resource_glyph_growth_diagnostic",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/resource_tests.rs",
            "phase6_preview_resource_formula_budget_diagnostic",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/text_reuse_tests.rs",
            "phase5_preview_release_baseline_text_reuse",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/semantic_conversion.rs",
            "phase11b_performance_one_mib_with_one_thousand_math_nodes",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/preview/text_layout.rs",
            "phase14_preview_selection_geometry_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/source/projection/startup_tests.rs",
            "source_initialization_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "crates/stickymd-render/src/source/scrollbar.rs",
            "scrollbar_release_baseline",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-package/src/sbom_workflow/diagnostics.rs",
            "compare_in_process_and_legacy_dispatch",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g3.rs",
            "c2d_crash_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g3.rs",
            "export_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g3.rs",
            "fake_asset_safety_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g3.rs",
            "image_paste_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g3.rs",
            "user_asset_safety_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g4/preflight.rs",
            "c2d_dock_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g4/preflight.rs",
            "c2d_identity_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g4/preflight.rs",
            "c2d_ime_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g4/preflight.rs",
            "c2d_search_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g4/preflight.rs",
            "c2d_tray_failure_restores_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g4/preflight.rs",
            "c2d_tray_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g4/preflight.rs",
            "c2d_two_copies_empty_note_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g4/preflight.rs",
            "c2d_two_copies_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g4/preflight.rs",
            "math_conversion_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g5/preflight.rs",
            "acceptance_automation_compact_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g5/preflight.rs",
            "acceptance_automation_presentation_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g5/preflight.rs",
            "acceptance_automation_rendering_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/g5/preflight.rs",
            "acceptance_automation_shell_local_preflight",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/module_ledger/fingerprint/tests.rs",
            "module_planning_profile",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/module_ledger/reuse_tests.rs",
            "lookup_all_profile",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/module_ledger/store/tests.rs",
            "link_scan_profile",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/resource_diagnostics/batch_tests.rs",
            "native_diagnostic_batch_identity_profile",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/resource_diagnostics/tests.rs",
            "native_diagnostic_identity_lookup_profile",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/qualification/resource_diagnostics/tests.rs",
            "native_diagnostic_resource_identity_is_stable",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/release/package_diagnostics.rs",
            "compare_package_dispatch_paths",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/release/package_diagnostics.rs",
            "direct_package_runtime_preserves_output_and_cleanup",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/runner/headless/local_build.rs",
            "actual_locked_local_build_ignores_stale_qualification_receipts",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/runtime/native_diagnostics.rs",
            "native_startup_cpu_diagnostic",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/runtime/native_diagnostics.rs",
            "native_window_interaction_diagnostic",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/runtime/resources/native_diagnostics.rs",
            "native_window_comparison_diagnostic",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/runtime/resources/native_diagnostics.rs",
            "native_zoom_cpu_diagnostic",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/runtime/resources/native_reuse_tests.rs",
            "native_resource_alias_and_batch_reuse",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/runtime/resources/probe.rs",
            "native_resource_desktop_probe",
            "timing or diagnostic; no gate row",
        ),
        (
            "tools/stickymd-smoke/src/runtime/startup_focus.rs",
            "native_startup_focus_diagnostic",
            "timing or diagnostic; no gate row",
        ),
    ];
    let root = root();
    let mut found = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = [
        "crates/stickymd-core",
        "crates/stickymd-render",
        "crates/stickymd-app",
        "crates/stickymd-desktop",
        "crates/stickymd-posix",
        "apps/stickymd-mac",
        "tools/stickymd-package",
        "tools/stickymd-smoke",
    ]
    .iter()
    .map(|dir| root.join(dir))
    .collect();
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).unwrap() {
                stack.push(entry.unwrap().path());
            }
        } else if path.extension().is_some_and(|e| e == "rs") {
            let text = std::fs::read_to_string(&path).unwrap();
            let lines: Vec<&str> = text.lines().collect();
            let relative = path
                .strip_prefix(&root)
                .unwrap()
                .to_string_lossy()
                .replace('\\', "/");
            for (at, line) in lines.iter().enumerate() {
                if !line.trim_start().starts_with("#[ignore") {
                    continue;
                }
                let name = lines[at..lines.len().min(at + 6)]
                    .iter()
                    .find_map(|line| {
                        let rest = line.split("fn ").nth(1)?;
                        Some(rest.split('(').next()?.trim().to_owned())
                    })
                    .unwrap_or_default();
                found.push((relative.clone(), name));
            }
        }
    }
    found.sort();
    let mut listed: Vec<(String, String)> = IGNORED
        .iter()
        .map(|(file, name, _)| ((*file).to_owned(), (*name).to_owned()))
        .collect();
    listed.sort();
    assert_eq!(
        found, listed,
        "classify every ignored test before changing this list"
    );
    for (item, tests) in EXPLICIT {
        for test in tests {
            let name = test.rsplit("::").next().unwrap();
            let role = IGNORED
                .iter()
                .find(|(_, n, _)| *n == name)
                .map(|(_, _, r)| *r);
            if *test != SAME_KEY_TESTS[0] {
                assert!(
                    role.is_some_and(|r| r.contains(item.trim_start_matches("same-key"))),
                    "{test}"
                );
            }
        }
    }
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
