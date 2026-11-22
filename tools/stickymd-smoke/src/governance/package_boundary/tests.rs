//! Each rule of the packaging boundary against the real repository, and against samples that
//! break that rule alone, each refused for its own stated reason.

use super::*;

#[test]
fn the_real_repository_keeps_the_boundary() {
    let root = crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
    verify(&root).unwrap();
}

fn index_of(entries: &[(&str, &str)]) -> Index {
    entries
        .iter()
        .map(|(path, mode)| ((*path).to_owned(), (*mode).to_owned()))
        .collect()
}

fn metadata(packages: &str) -> Value {
    json::parse(&format!(
        r#"{{"workspace_root":"/r","packages":[{packages}]}}"#
    ))
    .unwrap()
}

const PACKAGING: &str = r#"{"name":"stickymd-package","manifest_path":"/r/tools/stickymd-package/Cargo.toml","dependencies":[],"features":{},"targets":[{"kind":["lib"],"name":"stickymd_package","src_path":"/r/tools/stickymd-package/src/lib.rs"},{"kind":["bin"],"name":"stickymd-package","src_path":"/r/tools/stickymd-package/src/main.rs"}]}"#;
const WIN: &str = r#"{"name":"stickymd-win","manifest_path":"/r/apps/stickymd-win/Cargo.toml","dependencies":[],"targets":[{"kind":["bin"],"name":"stickymd-win","src_path":"/r/apps/stickymd-win/src/main.rs"},{"kind":["custom-build"],"name":"build-script-build","src_path":"/r/apps/stickymd-win/build.rs"}]}"#;

fn member(name: &str, directory: &str, targets: &str, dependencies: &str) -> String {
    format!(
        r#"{{"name":"{name}","manifest_path":"/r/{directory}Cargo.toml","dependencies":[{dependencies}],"targets":[{targets}]}}"#
    )
}

/// Cargo's decoded metadata decides, whatever the TOML spelling.
#[test]
fn the_workspace_is_read_from_cargo_metadata() {
    let index = index_of(&[
        ("Cargo.toml", "100644"),
        ("tools/stickymd-package/Cargo.toml", "100644"),
        ("apps/stickymd-win/Cargo.toml", "100644"),
        ("crates/helper/Cargo.toml", "100644"),
        ("crates/other/Cargo.toml", "100644"),
        ("tools/stickymd-smoke/Cargo.toml", "100644"),
        ("docs/helper/Cargo.toml", "100644"),
    ]);
    let check = |packages: &str| check_metadata(&metadata(packages), &index, &|_| true);
    // A test target ships nothing and is not among the targets returned.
    let with_test = PACKAGING.replace(
        r#"]}"#,
        r#",{"kind":["test"],"name":"cli_exit","src_path":"/r/tools/stickymd-package/tests/cli_exit.rs"}]}"#,
    );
    let targets = check(&format!("{with_test},{WIN}")).unwrap();
    assert_eq!(
        targets
            .iter()
            .map(|t| (t.kind.as_str(), t.name.as_str(), t.source.as_str()))
            .collect::<Vec<_>>(),
        [
            (
                "lib",
                "stickymd_package",
                "tools/stickymd-package/src/lib.rs"
            ),
            (
                "bin",
                "stickymd-package",
                "tools/stickymd-package/src/main.rs"
            )
        ]
    );
    // A root package (manifest `Cargo.toml`) without a build script is an ordinary member.
    let root_package = member(
        "root",
        "",
        r#"{"kind":["lib"],"name":"root","src_path":"/r/src/lib.rs"}"#,
        "",
    );
    check(&format!("{PACKAGING},{WIN},{root_package}")).unwrap();
    let error = check_metadata(
        &metadata(&format!("{PACKAGING},{WIN},{root_package}")),
        &index,
        &|path| path != "Cargo.toml",
    )
    .unwrap_err();
    assert!(error.contains("counted by both platforms"), "{error}");

    let dependency = PACKAGING.replace(
        r#""dependencies":[]"#,
        r#""dependencies":[{"name":"stickymd-smoke"}]"#,
    );
    let feature = PACKAGING.replace(r#""features":{}"#, r#""features":{"extra":[]}"#);
    let packaging_script = PACKAGING.replace(r#""kind":["lib"]"#, r#""kind":["custom-build"]"#);
    let macro_member = member(
        "helper",
        "crates/helper/",
        r#"{"kind":["proc-macro"],"name":"helper","src_path":"/r/crates/helper/src/lib.rs"}"#,
        "",
    );
    let impostor = member(
        "other",
        "crates/other/",
        r#"{"kind":["bin"],"name":"StickyMD-Win","src_path":"/r/crates/other/src/main.rs"}"#,
        "",
    );
    let unicode = |name: &str, directory: &str| {
        member(
            name,
            directory,
            &format!(
                r#"{{"kind":["bin"],"name":"{name}","src_path":"/r/{directory}src/main.rs"}}"#
            ),
            "",
        )
    };
    let smoke_script = member(
        "stickymd-smoke",
        "tools/stickymd-smoke/",
        r#"{"kind":["custom-build"],"name":"build-script-build","src_path":"/r/tools/stickymd-smoke/build.rs"}"#,
        "",
    );
    let moved = WIN.replace(
        "apps/stickymd-win/build.rs",
        "apps/stickymd-win/tools/build.rs",
    );
    let unlisted = WIN.replace(r#""kind":["custom-build"]"#, r#""kind":["example"]"#);
    // The same script path, claimed by another member.
    let other_owner = member(
        "other",
        "crates/other/",
        r#"{"kind":["custom-build"],"name":"build-script-build","src_path":"/r/apps/stickymd-win/build.rs"}"#,
        "",
    );
    // Cargo's spelling of a manifest that Git records in another case.
    let case_variant = member(
        "helper",
        "Docs/helper/",
        r#"{"kind":["lib"],"name":"helper","src_path":"/r/Docs/helper/src/lib.rs"}"#,
        "",
    );
    for (packages, reason) in [
        (format!("{dependency},{WIN}"), "declares a dependency"),
        (format!("{feature},{WIN}"), "declares a feature"),
        (
            format!("{packaging_script},{WIN}"),
            "stickymd-package has a build script",
        ),
        (
            format!("{PACKAGING},{WIN},{macro_member}"),
            "procedural macro",
        ),
        (
            format!("{PACKAGING},{WIN},{impostor}"),
            "two workspace targets build the binary",
        ),
        // Windows compares names by its own upcase table: only ASCII names are admitted.
        (
            format!("{PACKAGING},{WIN},{}", unicode("Édit", "crates/helper/")),
            "is not ASCII letters",
        ),
        (
            format!("{PACKAGING},{WIN},{smoke_script}"),
            "plan 11 lists exactly",
        ),
        (format!("{PACKAGING},{moved}"), "plan 11 lists exactly"),
        (format!("{PACKAGING},{unlisted}"), "plan 11 lists exactly"),
        (
            format!("{PACKAGING},{unlisted},{other_owner}"),
            "plan 11 lists exactly",
        ),
        (
            format!("{PACKAGING},{WIN},{case_variant}"),
            "not a regular tracked file",
        ),
        (WIN.to_owned(), "lists no stickymd-package"),
    ] {
        let error = check(&packages).unwrap_err();
        assert!(error.contains(reason), "{reason}: {error}");
    }
}

#[test]
fn the_git_index_records_paths_with_their_modes() {
    let index = parse_index(
        b"100644 0000000000000000000000000000000000000000 0\ta.rs\x00120000 0000000000000000000000000000000000000000 0\tlink.rs\0",
    )
    .unwrap();
    assert_eq!(index.get("a.rs").map(String::as_str), Some("100644"));
    assert_eq!(index.get("link.rs").map(String::as_str), Some("120000"));
    // A file only on disk is not in the index.
    let directory = stickymd_package::temporary::TemporaryDirectory::new("boundary-index").unwrap();
    let root = directory.path();
    let git = |arguments: &[&str]| {
        assert!(
            crate::release::platform_inputs::git(root)
                .args(arguments)
                .output()
                .unwrap()
                .status
                .success()
        );
    };
    git(&["init", "-q"]);
    std::fs::write(root.join("tracked.txt"), "a").unwrap();
    std::fs::write(root.join("untracked.txt"), "b").unwrap();
    git(&["add", "tracked.txt"]);
    let index = git_index(root).unwrap();
    assert!(index.contains_key("tracked.txt") && !index.contains_key("untracked.txt"));
    directory.close().unwrap();
}

/// A scratch workspace with one lib and one bin, whose lib reads files every way rustc can.
fn probe() -> (stickymd_package::temporary::TemporaryDirectory, Vec<Target>) {
    let directory = stickymd_package::temporary::TemporaryDirectory::new("boundary-probe").unwrap();
    let workspace = directory.path().join("ws");
    std::fs::create_dir_all(workspace.join("src")).unwrap();
    std::fs::create_dir_all(directory.path().join("outside")).unwrap();
    let write = |path: &str, text: &str| std::fs::write(workspace.join(path), text).unwrap();
    write(
        "Cargo.toml",
        "[package]\nname = \"probe\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[workspace]\n",
    );
    write(
        "Cargo.lock",
        "version = 4\n\n[[package]]\nname = \"probe\"\nversion = \"0.1.0\"\n",
    );
    write(
        "src/lib.rs",
        "mod a;\n#[path = \"b.in\"]\nmod b;\ninclude!(\"c.in\");\nuse core::include_str as renamed;\npub const D: &str = renamed!(\"d.txt\");\nmacro_rules! m { () => { include_str!(\"f.txt\") } }\npub const F: &str = m!();\npub const E: &[u8] = include_bytes!(\"../../outside/e.bin\");\n#[cfg(debug_assertions)]\npub const P: &str = include_str!(\"dev.txt\");\n#[cfg(not(debug_assertions))]\npub const P: &str = include_str!(\"release.txt\");\n",
    );
    write("src/main.rs", "fn main() {}\n");
    write("src/a.rs", "pub fn a() {}\n");
    write("src/b.in", "pub fn b() {}\n");
    write("src/c.in", "pub fn c() {}\n");
    for file in ["d.txt", "f.txt", "dev.txt", "release.txt"] {
        write(&format!("src/{file}"), "x");
    }
    std::fs::write(directory.path().join("outside/e.bin"), "e").unwrap();
    let target = |kind: &str, source: &str| Target {
        kind: kind.to_owned(),
        name: "probe".to_owned(),
        source: source.to_owned(),
    };
    (
        directory,
        vec![target("lib", "src/lib.rs"), target("bin", "src/main.rs")],
    )
}

/// rustc registers, in its dep-info, the local files it reads: an out-of-line module, a `#[path]`
/// file, `include!`, a renamed `include_str!`, an include inside a macro, an `include_bytes!` that
/// leaves the crate (written as the code wrote it), and a file only one profile reads.
#[test]
fn rustc_registers_the_files_it_reads_in_each_profile() {
    let (directory, targets) = probe();
    let workspace = directory.path().join("ws");
    let files = compiled::files(&workspace, "probe", &targets).unwrap();
    for expected in [
        "src/lib.rs",
        "src/main.rs",
        "src/a.rs",
        "src/b.in",
        "src/c.in",
        "src/d.txt",
        "src/f.txt",
        "src/../../outside/e.bin",
        "src/dev.txt",
        "src/release.txt",
    ] {
        assert!(
            files.iter().any(|file| file == expected),
            "{expected}: {files:?}"
        );
    }
    assert_eq!(compiled::PROFILES, ["dev", "release"]);
    directory.close().unwrap();
}

/// Every expected target needs one fresh unit per profile: a target metadata lists but the check
/// skips, a unit metadata does not list, and a unit reused from an earlier compilation are refused.
#[test]
fn each_profile_has_one_fresh_unit_per_target() {
    let (directory, targets) = probe();
    let workspace = directory.path().join("ws");
    let mut ghost = targets.clone();
    ghost.push(Target {
        kind: "bin".to_owned(),
        name: "ghost".to_owned(),
        source: "src/ghost.rs".to_owned(),
    });
    let first = directory.path().join("first");
    let error = compiled::files_in(&workspace, "probe", &ghost, &first, "dev").unwrap_err();
    assert!(
        error.contains("compiled no unit for the bin `ghost`"),
        "{error}"
    );
    let second = directory.path().join("second");
    let error = compiled::files_in(&workspace, "probe", &targets[..1], &second, "dev").unwrap_err();
    assert!(error.contains("which metadata does not list"), "{error}");
    // The same private directory again: the units are fresh, so their dep-info is old.
    let error = compiled::files_in(&workspace, "probe", &targets, &first, "dev").unwrap_err();
    assert!(error.contains("not compiled afresh"), "{error}");
    directory.close().unwrap();
}

#[test]
fn dep_info_rules_and_file_lines_must_agree() {
    let good =
        "/t/x-1.d: a/lib.rs a/b.txt\n\n/t/libx-1.rmeta: a/lib.rs a/b.txt\n\na/lib.rs:\na/b.txt:\n";
    assert_eq!(compiled::parse(good).unwrap(), ["a/b.txt", "a/lib.rs"]);
    for (bad, reason) in [
        (
            "/t/x-1.d: a/lib.rs\n\na/lib.rs:\n#asset.txt:\n",
            "a dep-info rule lists",
        ),
        (
            "/t/x-1.d: a/lib.rs extra.txt\n\na/lib.rs:\n",
            "a dep-info rule lists",
        ),
        (
            "/t/x-1.d: a/lib.rs\n\na/lib.rs:\nstray line\n",
            "unexpected dep-info line",
        ),
        (
            "/t/x-1.o: a/lib.rs\n\na/lib.rs:\n",
            "unexpected dep-info rule",
        ),
        ("a/lib.rs:\n", "without a rule or a file"),
        ("/t/x-1.d: a\\ b.rs\n\na\\ b.rs:\n", "needs unescaping"),
        ("/t/x-1.d: a$$b.rs\n\na$$b.rs:\n", "needs unescaping"),
    ] {
        let error = compiled::parse(bad).unwrap_err();
        assert!(error.contains(reason), "{bad}: {error}");
    }
    // `env!("GITHUB_JOB")` in an include path: the job decides the file. The value stays out of
    // the diagnostic.
    let error = compiled::parse("/t/x-1.d: a/lib.rs\n\na/lib.rs:\n\n# env-dep:GITHUB_JOB=secret\n")
        .unwrap_err();
    assert!(
        error.contains("environment variable GITHUB_JOB") && !error.contains("secret"),
        "{error}"
    );
    // `\` separates paths only on Windows; elsewhere it is part of a name, refused.
    let backslash = "/t/x-1.d: a\\b.txt\n\na\\b.txt:\n";
    if cfg!(windows) {
        assert_eq!(compiled::parse(backslash).unwrap(), ["a/b.txt"]);
    } else {
        assert!(
            compiled::parse(backslash)
                .unwrap_err()
                .contains("backslash")
        );
    }
}

const LIB: &str = "tools/stickymd-package/src/lib.rs";

#[test]
fn compiled_files_are_regular_counted_files_of_the_crate_spelled_as_git_records_them() {
    let index = index_of(&[
        (LIB, "100644"),
        ("tools/stickymd-package/src/link.rs", "120000"),
        ("docs/plan/11_testing_and_release.md", "100644"),
    ]);
    let counted = |_: &str| true;
    let check = |file: &str| check_compiled(&[file.to_owned()], &index, &counted);
    check(LIB).unwrap();
    for (file, reason) in [
        (
            "docs/plan/11_testing_and_release.md",
            "outside tools/stickymd-package/",
        ),
        // Lexically the plan chapter, but not a path Git records.
        (
            "tools/stickymd-package/src/../../../docs/plan/11_testing_and_release.md",
            "does not track under that spelling",
        ),
        (
            "tools/stickymd-package/src/Lib.rs",
            "does not track under that spelling",
        ),
        ("tools/stickymd-package/src/link.rs", "not a regular file"),
    ] {
        let error = check(file).unwrap_err();
        assert!(error.contains(reason), "{file}: {error}");
    }
    let error = check_compiled(&[LIB.to_owned()], &index, &|_| false).unwrap_err();
    assert!(error.contains("does not count"), "{error}");
}

#[test]
fn the_listed_scripts_are_regular_tracked_files_counted_for_the_platform() {
    let index = index_of(&[
        ("tools/release/a.ps1", "100644"),
        ("tools/release/link.ps1", "120000"),
        ("apps/stickymd-mac/a.ps1", "100644"),
    ]);
    check_byte_scripts("tools/release/", &["a.ps1"], &index, Platform::WindowsX64).unwrap();
    for (directory, name, reason) in [
        ("tools/release/", "b.ps1", "not a regular file Git tracks"),
        ("tools/release/", "A.ps1", "not a regular file Git tracks"),
        (
            "tools/release/",
            "link.ps1",
            "not a regular file Git tracks",
        ),
        ("apps/stickymd-mac/", "a.ps1", "does not count"),
    ] {
        let error =
            check_byte_scripts(directory, &[name], &index, Platform::WindowsX64).unwrap_err();
        assert!(error.contains(reason), "{directory}{name}: {error}");
    }
}

#[test]
fn every_ps1_name_is_a_listed_script_named_bare() {
    let allowed = ["invoke-package.ps1", "package.ps1"];
    check_ps1_names(
        b". (Join-Path $PSScriptRoot 'invoke-package.ps1')\n# see package.ps1\n",
        &allowed,
    )
    .unwrap();
    for (text, reason) in [
        (
            &b"& (Join-Path $PSScriptRoot 'verify-package.ps1')"[..],
            "not one of the listed",
        ),
        (b"& ./tools/release/package.ps1", "after a path separator"),
        (b"& .\\package.ps1", "after a path separator"),
        (
            b"& (Join-Path $PSScriptRoot 'package.PS1')",
            "not one of the listed",
        ),
        (b"$name.ps1", "not one of the listed"),
        // `\xc3\xa9` is `é`: a letter of a longer name the ASCII run does not see.
        (
            b"& (Join-Path $PSScriptRoot '\xc3\xa9package.ps1')",
            "after a non-ASCII byte",
        ),
    ] {
        let error = check_ps1_names(text, &allowed).unwrap_err();
        assert!(
            error.contains(reason),
            "{}: {error}",
            String::from_utf8_lossy(text)
        );
    }
}

#[test]
fn a_build_script_names_no_parent_directory() {
    check_build_script(b"fs::read(manifest_dir.join(\"StickyMD.manifest\")); 0..SIDE").unwrap();
    for text in [
        &b"fs::read(\"../docs/x\")"[..],
        b"fs::read(\"src\\\\..\\\\x\")",
        b"dir.join(\"..\")",
    ] {
        let error = check_build_script(text).unwrap_err();
        assert!(
            error.contains("parent directory"),
            "{}: {error}",
            String::from_utf8_lossy(text)
        );
    }
}

#[test]
fn no_file_names_the_verification_tool_in_any_case_comments_included() {
    check_names(b"// the verification tool\nlet smoke = 1;").unwrap();
    for text in [
        &b"// see stickymd-smoke"[..],
        b"use STICKYMD_SMOKE::x;",
        b"# Invoke-Smoke.ps1",
        b"Invoke-StickyMdReleaseTool -Arguments @()",
        b"./tools/smoke/all.ps1",
        b".\\TOOLS\\SMOKE\\all.ps1",
    ] {
        let error = check_names(text).unwrap_err();
        assert!(
            error.contains("names the verification tool"),
            "{}: {error}",
            String::from_utf8_lossy(text)
        );
    }
}

const WORKFLOW: &str = "name: release\non:\n  workflow_dispatch:\njobs:\n  dependency-policy:\n    steps: []\n  self-check:\n    needs: dependency-policy\n    steps:\n      - run: cargo run -p stickymd-smoke -- release workspace-version\n  build-package:\n    needs: self-check\n    steps:\n      - run: ./tools/release/package.ps1\n  verify-artifact:\n    needs: build-package\n    steps:\n      - run: ./tools/release/verify-package.ps1\n";

/// The whole workflow equals its pin; the pin's `build-package` section is what the name search
/// reads, so the tool named elsewhere in the workflow is fine and named in that job is not.
#[test]
fn a_release_workflow_equals_its_pin_whole() {
    let section = workflow::check(WORKFLOW, WORKFLOW).unwrap();
    assert_eq!(
        section,
        "  build-package:\n    needs: self-check\n    steps:\n      - run: ./tools/release/package.ps1\n"
    );
    check_names(section.as_bytes()).unwrap();
    // Line endings are made LF on both sides.
    workflow::check(&WORKFLOW.replace('\n', "\r\n"), WORKFLOW).unwrap();
    let error =
        workflow::check(&WORKFLOW.replace("steps: []", "steps: [ ]"), WORKFLOW).unwrap_err();
    assert!(error.contains("differs from its pinned copy"), "{error}");
    let smoke = WORKFLOW.replace(
        "      - run: ./tools/release/package.ps1\n",
        "      - run: ./tools/release/package.ps1\n      - run: cargo test -p stickymd-smoke\n",
    );
    let section = workflow::check(&smoke, &smoke).unwrap();
    assert!(
        check_names(section.as_bytes())
            .unwrap_err()
            .contains("names the verification tool")
    );
    for (pin, reason) in [
        (
            WORKFLOW.replace("  build-package:\n", ""),
            "0 lines `  build-package:`",
        ),
        (
            WORKFLOW.replace(
                "  verify-artifact:\n",
                "  verify-artifact:\n  verify-artifact:\n",
            ),
            "2 lines `  verify-artifact:`",
        ),
        (
            WORKFLOW
                .replace("  build-package:\n", "  placeholder:\n")
                .replace("  verify-artifact:\n", "  build-package:\n")
                .replace("  placeholder:\n", "  verify-artifact:\n"),
            "before",
        ),
    ] {
        let error = workflow::check(&pin, &pin).unwrap_err();
        assert!(error.contains(reason), "{reason}: {error}");
    }
}

/// The pins are exactly the workflows the classifier counts: dropping one, or the classifier
/// counting another, is refused.
#[test]
fn the_pins_are_exactly_the_counted_workflows() {
    let counted =
        |path: &str| is_input(Platform::WindowsX64, path) || is_input(Platform::MacOsArm64, path);
    let index = index_of(&[
        (".github/workflows/ci.yml", "100644"),
        (".github/workflows/promote-release.yml", "100644"),
        (".github/workflows/release-macos.yml", "100644"),
        (".github/workflows/release.yml", "100644"),
    ]);
    workflow::check_pinned_set(&index, &counted).unwrap();
    let error =
        workflow::check_pinned_set(&index, &|path| counted(path) || path.ends_with("ci.yml"))
            .unwrap_err();
    assert!(error.contains("but stickymd-smoke pins"), "{error}");
    let error =
        workflow::check_pinned_set(&index, &|path| path.ends_with("/release.yml")).unwrap_err();
    assert!(error.contains("but stickymd-smoke pins"), "{error}");
}

/// The real pins hold the release workflows of plan 11: the dependency policy and the three
/// release jobs, in order, each needing the one before.
#[test]
fn the_pins_hold_the_jobs_of_plan_11() {
    for (path, pin) in workflow::RELEASE_WORKFLOWS {
        let order: Vec<usize> = [
            "  dependency-policy:",
            "  self-check:",
            "    needs: dependency-policy",
            "  build-package:",
            "    needs: self-check",
            "  verify-artifact:",
            "    needs: build-package",
            "        uses: actions/download-artifact@37930b1c2abaa49bbe596cd826c3c89aef350131 # v7.0.0",
        ]
        .iter()
        .map(|line| {
            pin.lines()
                .position(|candidate| candidate == *line)
                .unwrap_or_else(|| panic!("{path}: no line `{line}`"))
        })
        .collect();
        assert!(
            order.windows(2).all(|pair| pair[0] < pair[1]),
            "{path}: {order:?}"
        );
    }
}

/// The lists in the code are the lists plan 11 states, both ways: adding, dropping or changing an
/// entry on one side alone fails.
#[test]
fn the_lists_are_the_contract_s() {
    let root = crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
    let plan = std::fs::read_to_string(root.join("docs/plan/11_testing_and_release.md")).unwrap();
    let section = |from: &str, to: &str| -> String {
        assert_eq!(plan.matches(from).count(), 1, "{from}");
        let start = plan.find(from).unwrap();
        let end = start + plan[start..].find(to).unwrap();
        plan[start..end].to_owned()
    };
    let ticked = |text: &str| -> Vec<String> {
        text.split('`')
            .skip(1)
            .step_by(2)
            .map(str::to_owned)
            .collect()
    };
    let sorted = |mut items: Vec<String>| {
        items.sort();
        items.dedup();
        items
    };
    // Scripts: every `*.ps1` name of the scripts sub-bullet.
    let scripts = section("**脚本：**", "**workflow：**");
    let named = sorted(
        ticked(&scripts)
            .into_iter()
            .filter(|piece| piece.len() > 4 && piece.ends_with(".ps1"))
            .collect(),
    );
    assert_eq!(
        named,
        sorted(BYTE_SCRIPTS.iter().map(|s| (*s).to_owned()).collect())
    );
    // Build scripts: every "`member` 的 `path`" pair of the metadata sub-bullet.
    let metadata = section("**Cargo metadata：**", "**编译器读过的文件：**");
    let pieces = ticked(&metadata);
    let pairs: Vec<String> = metadata
        .match_indices("` 的 `")
        .map(|(at, _)| {
            let member = metadata[..at].rsplit('`').next().unwrap();
            let script = metadata[at + "` 的 `".len()..].split('`').next().unwrap();
            format!("{member} {script}")
        })
        .collect();
    assert!(pieces.iter().any(|p| p == "apps/stickymd-win/build.rs"));
    assert_eq!(
        sorted(pairs),
        sorted(
            BUILD_SCRIPTS
                .iter()
                .map(|(m, s)| format!("{m} {s}"))
                .collect()
        )
    );
    // Profiles: every "<name> profile" of the compiled-files sub-bullet.
    let compiled_text = section("**编译器读过的文件：**", "**构建脚本：**");
    let profiles = sorted(
        compiled_text
            .match_indices(" profile")
            .map(|(at, _)| {
                compiled_text[..at]
                    .rsplit(|c: char| !c.is_ascii_alphanumeric())
                    .next()
                    .unwrap()
                    .to_owned()
            })
            .filter(|name| !name.is_empty())
            .collect(),
    );
    assert_eq!(
        profiles,
        sorted(compiled::PROFILES.iter().map(|p| (*p).to_owned()).collect())
    );
    // Compile variables: every literal of the premise's parenthesis.
    let premise = section("固定副本不含下列字面串（", "），仓库内的");
    assert_eq!(
        sorted(ticked(&premise)),
        sorted(
            workflow::COMPILE_VARIABLES
                .iter()
                .map(|v| (*v).to_owned())
                .collect()
        )
    );
}

/// The premise that self-check and build-package compile alike: a pinned workflow sets none of the
/// variables that change a compilation, in any letter case; naming the compiler in a command is
/// fine.
#[test]
fn the_pins_set_no_compile_variables() {
    let with = |line: &str| {
        WORKFLOW.replace(
            "    needs: self-check\n",
            &format!("    needs: self-check\n    env:\n      {line}\n"),
        )
    };
    let fine = WORKFLOW.replace("./tools/release/package.ps1", "rustc -vV");
    workflow::check(&fine, &fine).unwrap();
    for (line, variable) in [
        ("rustflags: --cfg receipt_variant", "RUSTFLAGS"),
        ("CARGO_ENCODED_RUSTFLAGS: x", "RUSTFLAGS"),
        ("RUSTC_WRAPPER: x", "RUSTC_WRAPPER"),
        ("RUSTC_WORKSPACE_WRAPPER: x", "RUSTC_WORKSPACE_WRAPPER"),
        ("CARGO_BUILD_TARGET: x", "CARGO_BUILD_TARGET"),
        ("CARGO_BUILD_BUILD_DIR: x", "CARGO_BUILD_BUILD_DIR"),
        ("CARGO_TARGET_DIR: x", "CARGO_TARGET_DIR"),
        ("RUSTUP_TOOLCHAIN: nightly", "RUSTUP_TOOLCHAIN"),
        ("RUSTC: /opt/rustc", "RUSTC:"),
    ] {
        let text = with(line);
        let error = workflow::check(&text, &text).unwrap_err();
        assert!(error.contains(variable), "{line}: {error}");
    }
    let assigned = WORKFLOW.replace(
        "./tools/release/package.ps1",
        "$env:RUSTC = 'x'; cargo build",
    );
    assert!(
        workflow::check(&assigned, &assigned)
            .unwrap_err()
            .contains("env:RUSTC")
    );
}

#[test]
fn a_backslash_in_a_tracked_path_is_refused() {
    let error =
        parse_index(b"100644 0000000000000000000000000000000000000000 0\tapps/example/a\\b.txt\0")
            .unwrap_err();
    assert!(error.contains("holds a backslash"), "{error}");
}

/// Nothing in the resolved graph may depend on a member with a listed build script.
#[test]
fn cargo_lock_names_a_build_script_owner_only_as_itself() {
    let lock = "[[package]]\nname = \"stickymd-mac\"\nversion = \"0.2.0\"\n\n[[package]]\nname = \"stickymd-win\"\nversion = \"0.2.0\"\n";
    check_lock(lock).unwrap();
    for bad in [
        // A member, or a package outside the workspace, depending on it.
        lock.replace(
            "version = \"0.2.0\"\n\n",
            "version = \"0.2.0\"\ndependencies = [\n \"stickymd-win\",\n]\n\n",
        ),
        lock.replace("name = \"stickymd-win\"", "name = \"other\""),
    ] {
        let error = check_lock(&bad).unwrap_err();
        assert!(error.contains("nothing may depend on it"), "{error}");
    }
}

#[test]
fn a_listed_script_is_utf8_text() {
    check_script_encoding(b"param()\n& (Join-Path $PSScriptRoot 'package.ps1')\n").unwrap();
    let utf16: Vec<u8> = "& 'package.ps1'"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    for bytes in [
        [&[0xFF, 0xFE][..], &utf16].concat(),
        // Without its mark, ASCII in UTF-16 is valid UTF-8 with NULs between the letters.
        utf16.clone(),
        b"\xff\xfe".to_vec(),
    ] {
        let error = check_script_encoding(&bytes).unwrap_err();
        assert!(error.contains("is not UTF-8 text"), "{error}");
    }
}

#[test]
fn a_listed_name_may_start_the_file() {
    check_ps1_names(b"package.ps1 -ExactCandidate", &["package.ps1"]).unwrap();
}

#[test]
fn library_kinds_count_as_lib() {
    let kinds = |text: &str| json::parse(text).unwrap();
    for kind in [r#"["lib"]"#, r#"["rlib"]"#, r#"["rlib","cdylib"]"#] {
        assert_eq!(
            compiled::lib_or_bin(kinds(kind).array().unwrap()),
            Some("lib"),
            "{kind}"
        );
    }
    assert_eq!(
        compiled::lib_or_bin(kinds(r#"["bin"]"#).array().unwrap()),
        Some("bin")
    );
    assert_eq!(
        compiled::lib_or_bin(kinds(r#"["test"]"#).array().unwrap()),
        None
    );
}

/// An environment read is reported first and by name only, even when its value also shaped a
/// path that would fail another rule.
#[test]
fn an_environment_read_is_reported_first_without_its_value() {
    let text = "/t/x-1.d: my$secret.txt\n\nmy$secret.txt:\n\n# env-dep:SECRET_NAME=my$secret\n";
    let error = compiled::parse(text).unwrap_err();
    assert!(
        error.contains("SECRET_NAME") && !error.contains("secret.txt"),
        "{error}"
    );
}

#[test]
fn every_listed_rustc_spelling_is_refused() {
    let with = |run: &str| WORKFLOW.replace("./tools/release/package.ps1", run);
    for (run, literal) in [
        ("RUSTC=/opt/compiler cargo check", "RUSTC="),
        ("$env:RUSTC = 'x'; cargo build", "env:RUSTC"),
    ] {
        let text = with(run);
        let error = workflow::check(&text, &text).unwrap_err();
        assert!(error.contains(literal), "{run}: {error}");
    }
}
