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

/// A receipt that judges PASS on both rows: every listed target ran.
fn good() -> Receipt {
    let expected = listed();
    Receipt {
        release_source: R.to_owned(),
        host: Host {
            product_name: "macOS".into(),
            product_version: "27.2".into(),
            build_version: "27C55".into(),
            arch: "arm64".into(),
            virtual_machine: "0".into(),
            cargo: "cargo 1.97.1".into(),
            rustc: "rustc 1.97.1".into(),
        },
        start: Side {
            head: R.into(),
            clean: true,
        },
        end: Side {
            head: R.into(),
            clean: true,
        },
        command: command(),
        exit_code: Some(0),
        binaries: expected
            .iter()
            .map(|target| Run {
                target: target.clone(),
                counts: counts(10, 1),
            })
            .collect(),
        expected,
        same_key: SameKey {
            command: same_key_command(),
            tests: SAME_KEY_TESTS.map(str::to_owned).to_vec(),
            counts: Some(Counts {
                ran: 2,
                passed: 2,
                filtered_out: 40,
                ..Counts::default()
            }),
            exit_code: Some(0),
        },
        problems: Vec::new(),
    }
}

fn verdicts(receipt: &Receipt) -> (Outcome, Outcome) {
    judge(receipt, R, &listed())
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
        },
        output::Executable {
            target: target("stickymd-package", "bin", "stickymd-package"),
            file: "stickymd_package-8fbe866c61699946".into(),
        },
    ];
    let doc = vec![target("stickymd-package", "doctest", "stickymd_package")];
    let (runs, problems) = output::attribute(&sections, &executables, &doc);
    assert_eq!(runs.len(), 3);
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
    let older = text.replacen("\"schema_version\":1", "\"schema_version\":0", 1);
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
    same_key.same_key.command.pop();
    not_tested(&verdicts(&same_key).1, "same-key run is not");
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
    not_tested(&verdicts(&uneven).0, "add up");
}

#[test]
fn a_recorded_failure_is_a_fail() {
    let mut failing = good();
    failing.binaries[2].counts.failed = 1;
    failing.binaries[2].counts.ran += 1;
    failed(&verdicts(&failing).0, "1 failed");
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
    renamed.same_key.counts = Some(Counts {
        filtered_out: 42,
        ..Counts::default()
    });
    not_tested(&verdicts(&renamed).1, "passed 0 of its 2");
    let mut skipped = good();
    skipped.same_key.counts = Some(Counts {
        ran: 2,
        passed: 1,
        ignored: 1,
        filtered_out: 40,
        ..Counts::default()
    });
    not_tested(&verdicts(&skipped).1, "1 ignored");
    let mut failing = good();
    failing.same_key.counts = Some(Counts {
        ran: 2,
        passed: 1,
        failed: 1,
        filtered_out: 40,
        ..Counts::default()
    });
    failing.same_key.exit_code = Some(101);
    failed(&verdicts(&failing).1, "1 failed");
    let mut silent = good();
    silent.same_key.counts = None;
    not_tested(&verdicts(&silent).1, "no test result");
    // The main row does not depend on the same-key run.
    assert_eq!(verdicts(&failing).0, Outcome::Pass);
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
    not_tested(&verdicts(&wrapped).0, "add up");
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
        receipt.same_key.counts = Some(counts);
        not_tested(&verdicts(&receipt).1, "of its 2");
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

/// Every `#[ignore]` test of the eight packages, by file, and what it is for. A Source row whose
/// test is ignored is not proven by the default run (plan 11 #macos-release-gate): a new ignored
/// test here must be classified before this list changes (P16-S06 is `IgnoredSource`).
#[test]
fn every_ignored_test_of_the_eight_packages_is_classified() {
    const IGNORED: &[(&str, usize, &str)] = &[
        (
            "apps/stickymd-mac/src/paths.rs",
            1,
            "P16-S06 (IgnoredSource)",
        ),
        (
            "apps/stickymd-mac/src/note_root/tests.rs",
            1,
            "P18-L08 (same-key run)",
        ),
        ("apps/stickymd-mac/src/export/mod.rs", 1, "timing"),
        ("apps/stickymd-mac/src/assets/transaction.rs", 1, "timing"),
        (
            "crates/stickymd-posix/src/user.rs",
            1,
            "run by another test in a child process",
        ),
        ("crates/stickymd-core/src/assets.rs", 1, "timing"),
        ("crates/stickymd-render/src/image.rs", 1, "timing"),
        ("crates/stickymd-render/src/math_action.rs", 2, "timing"),
        (
            "crates/stickymd-render/src/source/scrollbar.rs",
            1,
            "timing",
        ),
        (
            "crates/stickymd-render/src/source/projection/startup_tests.rs",
            1,
            "diagnostic",
        ),
        ("crates/stickymd-render/src/math/engine.rs", 1, "timing"),
        (
            "crates/stickymd-render/src/preview/text_layout.rs",
            1,
            "timing",
        ),
        (
            "crates/stickymd-render/src/preview/semantic_conversion.rs",
            1,
            "timing",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline.rs",
            3,
            "timing",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/audit_tests.rs",
            2,
            "benchmark",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/resource_tests.rs",
            3,
            "attribution",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/image_refresh_tests.rs",
            1,
            "benchmark",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/text_reuse_tests.rs",
            1,
            "measurement",
        ),
        (
            "crates/stickymd-render/src/preview/pipeline/horizontal_tests.rs",
            1,
            "diagnostic",
        ),
        ("crates/stickymd-app/src/source_search.rs", 1, "timing"),
        (
            "crates/stickymd-app/src/execution/preview_worker.rs",
            1,
            "timing",
        ),
        (
            "crates/stickymd-app/src/flow/window/reducer.rs",
            1,
            "timing",
        ),
        ("crates/stickymd-desktop/src/shell/mod.rs", 1, "timing"),
        ("crates/stickymd-desktop/src/controls/tests.rs", 1, "timing"),
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
            let count = std::fs::read_to_string(&path)
                .unwrap()
                .lines()
                .filter(|line| line.trim_start().starts_with("#[ignore"))
                .count();
            if count > 0 {
                let relative = path
                    .strip_prefix(&root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/");
                found.push((relative, count));
            }
        }
    }
    found.sort();
    let mut listed: Vec<(String, usize)> = IGNORED
        .iter()
        .map(|(file, count, _)| ((*file).to_owned(), *count))
        .collect();
    listed.sort();
    assert_eq!(
        found, listed,
        "classify every ignored test before changing this list"
    );
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
