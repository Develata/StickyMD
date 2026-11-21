//! Each rule of the packaging boundary against the real repository, and against samples that
//! break that rule alone, each refused for its own stated reason.

use super::*;

#[test]
fn the_real_repository_keeps_the_boundary() {
    let root = crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
    verify(&root).unwrap();
}

fn metadata(packages: &str) -> Value {
    json::parse(&format!(
        r#"{{"workspace_root":"/r","packages":[{packages}]}}"#
    ))
    .unwrap()
}

const PACKAGING: &str = r#"{"name":"stickymd-package","manifest_path":"/r/tools/stickymd-package/Cargo.toml","dependencies":[],"targets":[{"kind":["lib"],"name":"stickymd_package","src_path":"/r/tools/stickymd-package/src/lib.rs"},{"kind":["bin"],"name":"stickymd-package","src_path":"/r/tools/stickymd-package/src/main.rs"}]}"#;
const WIN: &str = r#"{"name":"stickymd-win","manifest_path":"/r/apps/stickymd-win/Cargo.toml","dependencies":[],"targets":[{"kind":["bin"],"name":"stickymd-win","src_path":"/r/apps/stickymd-win/src/main.rs"},{"kind":["custom-build"],"name":"build-script-build","src_path":"/r/apps/stickymd-win/build.rs"}]}"#;

/// Cargo's decoded metadata decides, whatever the TOML spelling.
#[test]
fn the_workspace_is_read_from_cargo_metadata() {
    let counted = |_: &str| true;
    let check = |packages: &str| check_metadata(&metadata(packages), &counted);
    // A test target ships nothing and is not among the targets returned.
    let with_test = PACKAGING.replace(
        r#"]}"#,
        r#",{"kind":["test"],"name":"cli_exit","src_path":"/r/tools/stickymd-package/tests/cli_exit.rs"}]}"#,
    );
    assert_eq!(
        check(&format!("{with_test},{WIN}")).unwrap(),
        [
            "tools/stickymd-package/src/lib.rs",
            "tools/stickymd-package/src/main.rs"
        ]
    );
    // A root package (manifest `Cargo.toml`) without a build script is an ordinary member.
    let root_package = r#"{"name":"root","manifest_path":"/r/Cargo.toml","dependencies":[],"targets":[{"kind":["lib"],"name":"root","src_path":"/r/src/lib.rs"}]}"#;
    check(&format!("{PACKAGING},{WIN},{root_package}")).unwrap();
    // A member whose manifest a platform does not count.
    let error = check_metadata(
        &metadata(&format!("{PACKAGING},{WIN},{root_package}")),
        &|path| path != "Cargo.toml",
    )
    .unwrap_err();
    assert!(error.contains("not counted by both platforms"), "{error}");
    let dependency = PACKAGING.replace(
        r#""dependencies":[]"#,
        r#""dependencies":[{"name":"stickymd-smoke"}]"#,
    );
    let packaging_script = PACKAGING.replace(r#""kind":["lib"]"#, r#""kind":["custom-build"]"#);
    let macro_member = r#"{"name":"helper","manifest_path":"/r/crates/helper/Cargo.toml","dependencies":[],"targets":[{"kind":["proc-macro"],"name":"helper","src_path":"/r/crates/helper/src/lib.rs"}]}"#;
    let impostor = r#"{"name":"other","manifest_path":"/r/crates/other/Cargo.toml","dependencies":[],"targets":[{"kind":["bin"],"name":"StickyMD-Win","src_path":"/r/crates/other/src/main.rs"}]}"#;
    let smoke_script = r#"{"name":"stickymd-smoke","manifest_path":"/r/tools/stickymd-smoke/Cargo.toml","dependencies":[],"targets":[{"kind":["custom-build"],"name":"build-script-build","src_path":"/r/tools/stickymd-smoke/build.rs"}]}"#;
    let moved = WIN.replace(
        "apps/stickymd-win/build.rs",
        "apps/stickymd-win/tools/build.rs",
    );
    let unlisted = WIN.replace(r#""kind":["custom-build"]"#, r#""kind":["example"]"#);
    for (packages, reason) in [
        (format!("{dependency},{WIN}"), "declares a dependency"),
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
        (
            format!("{PACKAGING},{WIN},{smoke_script}"),
            "plan 11 lists exactly",
        ),
        (format!("{PACKAGING},{moved}"), "plan 11 lists exactly"),
        (format!("{PACKAGING},{unlisted}"), "plan 11 lists exactly"),
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

/// rustc registers, in its dep-info, the local files it reads: an out-of-line module, a `#[path]`
/// file, `include!`, a renamed `include_str!`, an include inside a macro, and an `include_bytes!`
/// that leaves the crate, written as the code wrote it.
#[test]
fn rustc_registers_the_files_it_reads() {
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
        "mod a;\n#[path = \"b.in\"]\nmod b;\ninclude!(\"c.in\");\nuse core::include_str as renamed;\npub const D: &str = renamed!(\"d.txt\");\nmacro_rules! m { () => { include_str!(\"f.txt\") } }\npub const F: &str = m!();\npub const E: &[u8] = include_bytes!(\"../../outside/e.bin\");\n",
    );
    write("src/a.rs", "pub fn a() {}\n");
    write("src/b.in", "pub fn b() {}\n");
    write("src/c.in", "pub fn c() {}\n");
    write("src/d.txt", "d");
    write("src/f.txt", "f");
    std::fs::write(directory.path().join("outside/e.bin"), "e").unwrap();
    let files = compiled::files(&workspace, "probe", &workspace.join("target")).unwrap();
    for expected in [
        "src/lib.rs",
        "src/a.rs",
        "src/b.in",
        "src/c.in",
        "src/d.txt",
        "src/f.txt",
        "src/../../outside/e.bin",
    ] {
        assert!(
            files.iter().any(|file| file == expected),
            "{expected}: {files:?}"
        );
    }
    directory.close().unwrap();
}

#[test]
fn dep_info_is_read_line_by_line_without_unescaping() {
    let text = "/t/deps/x-1.d: a/lib.rs a/b.txt\n\n/t/deps/libx-1.rmeta: a/lib.rs a/b.txt\n\na/lib.rs:\na\\b.txt:\n\n# env-dep:CARGO_PKG_NAME=x\n";
    assert_eq!(compiled::parse(text).unwrap(), ["a/lib.rs", "a/b.txt"]);
    for bad in ["a\\ b.rs:\n", "a$$b.rs:\n", "a/lib.rs\n"] {
        assert!(compiled::parse(bad).is_err(), "{bad}");
    }
}

const LIB: &str = "tools/stickymd-package/src/lib.rs";

fn index_of(entries: &[(&str, &str)]) -> BTreeMap<String, String> {
    entries
        .iter()
        .map(|(path, mode)| ((*path).to_owned(), (*mode).to_owned()))
        .collect()
}

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
fn every_packaging_target_is_a_compiled_file() {
    check_targets(&[LIB.to_owned()], &[LIB.to_owned()]).unwrap();
    let error = check_targets(
        &["tools/stickymd-package/src/entry.in".to_owned()],
        &[LIB.to_owned()],
    )
    .unwrap_err();
    assert!(error.contains("not among the files rustc read"), "{error}");
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
    assert!(contains(b"run X.PS1", b".ps1"));
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
    assert!(check_names(section.as_bytes()).is_err());
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

/// The real pins are the release workflows of plan 11: the dependency policy and the three
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
