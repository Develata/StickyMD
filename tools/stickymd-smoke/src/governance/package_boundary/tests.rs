//! Each rule of the packaging boundary against the real repository and against bad samples, with
//! the reason each sample is refused.

use super::*;

#[test]
fn the_real_packaging_crate_keeps_its_boundary() {
    let root = crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
    verify(&root).unwrap();
}

fn metadata(packages: &str) -> Value {
    json::parse(&format!(r#"{{"packages":[{packages}]}}"#)).unwrap()
}

const PACKAGE: &str = r#"{"name":"stickymd-package","manifest_path":"/r/tools/stickymd-package/Cargo.toml","dependencies":[],"targets":[{"kind":["lib"],"src_path":"/r/tools/stickymd-package/src/lib.rs"}]}"#;
const WIN: &str = r#"{"name":"stickymd-win","manifest_path":"/r/apps/stickymd-win/Cargo.toml","dependencies":[],"targets":[{"kind":["custom-build"],"src_path":"/r/apps/stickymd-win/build.rs"}]}"#;

/// Cargo's decoded metadata decides, whatever the TOML spelling (`["dependencies"]`,
/// `"build" = ...`): any dependency or build script of the packaging crate is refused, and a
/// member's build script elsewhere than its `build.rs` too.
#[test]
fn dependencies_and_build_scripts_are_read_from_cargo_metadata() {
    check_metadata(&metadata(PACKAGE)).unwrap();
    check_metadata(&metadata(&format!("{PACKAGE},{WIN}"))).unwrap();
    let dependency = PACKAGE.replace(
        r#""dependencies":[]"#,
        r#""dependencies":[{"name":"stickymd-smoke"}]"#,
    );
    let error = check_metadata(&metadata(&dependency)).unwrap_err();
    assert!(error.contains("declares a dependency"), "{error}");
    let script = PACKAGE.replace(r#""kind":["lib"]"#, r#""kind":["custom-build"]"#);
    let error = check_metadata(&metadata(&script)).unwrap_err();
    assert!(error.contains("has a build script"), "{error}");
    let moved = WIN.replace(
        "apps/stickymd-win/build.rs",
        "apps/stickymd-win/scripts/build.rs",
    );
    let error = check_metadata(&metadata(&format!("{PACKAGE},{moved}"))).unwrap_err();
    assert!(error.contains("only the build.rs beside"), "{error}");
    let error = check_metadata(&metadata(WIN)).unwrap_err();
    assert!(error.contains("no stickymd-package"), "{error}");
}

#[test]
fn comments_are_told_apart_from_strings_and_characters() {
    let code = strip_comments(
        "let a = \"http://x\"; Command::new(\"stickymd-smoke\");\n// stickymd-smoke here\n/* \"stickymd-smoke\" /* nested */ */ let q = '\"'; let e = '\\''; let r = r#\"//\"#; f::<'a>();\n",
    )
    .unwrap();
    let error = check_names(&code).unwrap_err();
    assert!(error.starts_with("line 1 "), "{error}");
    let third = code.lines().nth(2).unwrap();
    assert!(!code.lines().nth(1).unwrap().contains("smoke"));
    assert!(!third.contains("smoke") && !third.contains("nested"));
    assert!(third.contains("'\"'") && third.contains("'\\''"));
    assert!(third.contains("r#\"//\"#") && third.contains("<'a>"));
    assert!(strip_comments("let s = \"open").is_err());
    assert!(strip_comments("/* open").is_err());
    // An include inside a comment is not an include.
    let commented = strip_comments("// include_str!(\"../../../docs/x.md\")").unwrap();
    assert!(
        includes("tools/stickymd-package/src/x.rs", &commented)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn includes_must_reach_counted_inputs() {
    let file = "tools/stickymd-package/src/package_content.rs";
    let check = |code: &str| includes(file, &strip_comments(code).unwrap());
    assert_eq!(
        check("include_str!(\"package_readme.txt\")").unwrap(),
        [(
            "include_str!",
            "tools/stickymd-package/src/package_readme.txt".to_owned()
        )]
    );
    // A counted shared file outside the crate is fine.
    check("include_str!(\"../../../assets/licenses/SIL-OFL-1.1.txt\")").unwrap();
    for bad in [
        "include_str!(\"../../stickymd-smoke/src/x.txt\")",
        "include_bytes!(\"../../../docs/plan/11_testing_and_release.md\")",
        "include_bytes!(\"/etc/x\")",
        "include_str!(\"../../../../outside.txt\")",
        "#[path = \"sub\\\\..\\\\..\\\\x.rs\"]\nmod x;",
    ] {
        let error = check(bad).unwrap_err();
        assert!(
            error.contains("not a build input of both platforms"),
            "{bad}: {error}"
        );
    }
    let error = check("include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/x.rs\"))").unwrap_err();
    assert!(error.contains("plain string literal"), "{error}");
    assert_eq!(resolve("a/b", "../c/./d.txt").as_deref(), Some("a/c/d.txt"));
    assert_eq!(resolve("a", "../../x"), None);
}

/// A Rust file reached by `include!` is checked in turn: its own include of a document and its
/// naming of the verification tool are refused, and the scripts it launches are collected.
#[test]
fn a_rust_file_reached_by_include_is_checked_in_turn() {
    let directory = stickymd_package::temporary::TemporaryDirectory::new("boundary-tree").unwrap();
    let root = directory.path();
    let write = |path: &str, text: &str| {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    };
    let start = || vec!["tools/stickymd-package/src/lib.rs".to_owned()];
    write(
        "tools/stickymd-package/src/lib.rs",
        "include!(\"../../../assets/fragment.rs\");\n",
    );
    write(
        "assets/fragment.rs",
        "const TEXT: &str = include_str!(\"../docs/fragment.txt\");\n",
    );
    let error = check_rust_tree(root, start()).unwrap_err();
    assert!(error.starts_with("assets/fragment.rs:"), "{error}");
    assert!(error.contains("docs/fragment.txt"), "{error}");
    write(
        "assets/fragment.rs",
        "fn f() { std::fs::read(\"tools/stickymd-smoke/tests/x\"); }\n",
    );
    let error = check_rust_tree(root, start()).unwrap_err();
    assert!(error.contains("names the verification tool"), "{error}");
    write(
        "assets/fragment.rs",
        "fn f() { windows::operation(root, shell, \"package-archive.ps1\", &[]); }\n",
    );
    assert_eq!(
        check_rust_tree(root, start()).unwrap(),
        ["package-archive.ps1"]
    );
    directory.close().unwrap();
}

#[test]
fn rust_code_never_names_the_verification_tool() {
    check_names("Err(\"belongs to stickymd-smoke release verify-package\")").unwrap();
    for bad in [
        "Command::new(\"cargo\").args([\"run\", \"-p\", \"stickymd-smoke\"])",
        "let x = \"cargo run -p stickymd-smoke -- release\";",
        "use stickymd_smoke::release;",
        "fs::read(root.join(\"tools/stickymd-smoke/tests/fixtures/notice.txt\"))",
    ] {
        let error = check_names(&strip_comments(bad).unwrap()).unwrap_err();
        assert!(
            error.contains("names the verification tool"),
            "{bad}: {error}"
        );
    }
    // A comment that mentions the tool is fine once comments are removed.
    check_names(&strip_comments("// see stickymd-smoke/README.md\n").unwrap()).unwrap();
    assert!(check_names("// see stickymd-smoke/README.md\n").is_err());
}

#[test]
fn scripts_follow_every_reference_and_refuse_unreadable_dot_sources() {
    check_script(
        "# invoke-smoke.ps1 is for verification\n<# stickymd-smoke\n#>\n. (Join-Path $PSScriptRoot 'invoke-package.ps1')\n",
    )
    .unwrap();
    for bad in [
        ". (Join-Path $PSScriptRoot 'invoke-smoke.ps1')\n",
        "Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments @('notices')\n",
        "cargo run --quiet -p stickymd-smoke --locked -- release macos-package\n",
        "./tools/smoke/all.ps1 -Ci\n",
    ] {
        let error = check_script(bad).unwrap_err();
        assert!(
            error.contains("calls the verification tool"),
            "{bad}: {error}"
        );
    }
    let error = check_script(". $helper\n").unwrap_err();
    assert!(error.contains("cannot follow"), "{error}");
    assert_eq!(
        ps1_references(
            ". \"$PSScriptRoot/notice-helper.ps1\"\n& (Join-Path $PSScriptRoot \"syft-fetch.ps1\") -Uri x\nrun: ./tools/release/package.ps1 -ExactCandidate\n"
        ),
        ["notice-helper.ps1", "syft-fetch.ps1", "package.ps1"]
    );
}

/// A script that references one outside `tools/release` cannot be followed and is refused; a
/// referenced helper that calls the verification tool is refused through the reference.
#[test]
fn referenced_scripts_are_checked_transitively() {
    let directory =
        stickymd_package::temporary::TemporaryDirectory::new("boundary-scripts").unwrap();
    let root = directory.path();
    std::fs::create_dir_all(root.join(SCRIPTS)).unwrap();
    let write =
        |name: &str, text: &str| std::fs::write(root.join(SCRIPTS).join(name), text).unwrap();
    write("package.ps1", ". \"$PSScriptRoot/notice-helper.ps1\"\n");
    write(
        "notice-helper.ps1",
        "Get-Content tools/stickymd-smoke/tests/x\n",
    );
    let error = check_scripts(root, vec!["package.ps1".to_owned()]).unwrap_err();
    assert!(error.contains("notice-helper.ps1"), "{error}");
    write("notice-helper.ps1", "Write-Output ok\n");
    check_scripts(root, vec!["package.ps1".to_owned()]).unwrap();
    write("package.ps1", "& ./elsewhere/missing.ps1\n");
    let error = check_scripts(root, vec!["package.ps1".to_owned()]).unwrap_err();
    assert!(error.contains("not in tools/release"), "{error}");
    directory.close().unwrap();
}

const WORKFLOW: &str = "name: x\njobs:\n  dependency-policy:\n    steps: []\n  self-check:\n    needs: dependency-policy\n    steps:\n      - run: cargo run -p stickymd-smoke -- release workspace-version\n  build-package:\n    needs: self-check\n    steps:\n      - name: Package\n# a comment at column zero does not end the job\n        run: ./tools/release/package.ps1 -ExactCandidate\n  verify-artifact:\n    needs: build-package\n    steps:\n      - uses: actions/download-artifact@0000000000000000000000000000000000000000\n      - run: ./tools/release/verify-package.ps1\n";

#[test]
fn the_build_job_runs_no_verification_code_and_the_jobs_are_ordered() {
    assert_eq!(check_workflow(WORKFLOW).unwrap(), ["package.ps1"]);
    let smoke = WORKFLOW.replace(
        "        run: ./tools/release/package.ps1 -ExactCandidate\n",
        "        run: ./tools/release/package.ps1 -ExactCandidate\n      - run: cargo run -p stickymd-smoke -- qualification native-runtime\n",
    );
    let error = check_workflow(&smoke).unwrap_err();
    assert!(error.contains("job `build-package`"), "{error}");
    let error = check_workflow(
        &WORKFLOW.replace("    needs: self-check\n", "    needs: dependency-policy\n"),
    )
    .unwrap_err();
    assert!(error.contains("needs: self-check"), "{error}");
    let error = check_workflow(&WORKFLOW.replace("actions/download-artifact@", "actions/cache@"))
        .unwrap_err();
    assert!(error.contains("download"), "{error}");
    let twice = format!("{WORKFLOW}  build-package:\n    needs: self-check\n");
    assert!(
        check_workflow(&twice)
            .unwrap_err()
            .contains("defined twice")
    );
    assert!(check_workflow(&WORKFLOW.replace("  verify-artifact:", "  verify:")).is_err());
}
