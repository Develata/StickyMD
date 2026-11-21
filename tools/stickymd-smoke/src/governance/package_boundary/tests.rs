//! Each rule of the packaging boundary against the real repository and against bad samples, with
//! the reason each sample is refused.

use super::*;

#[test]
fn the_real_packaging_crate_keeps_its_boundary() {
    let root = crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
    verify(&root).unwrap();
}

fn metadata(packages: &str) -> Value {
    json::parse(&format!(
        r#"{{"workspace_root":"/r","packages":[{packages}]}}"#
    ))
    .unwrap()
}

const PACKAGE: &str = r#"{"name":"stickymd-package","manifest_path":"/r/tools/stickymd-package/Cargo.toml","dependencies":[],"targets":[{"kind":["lib"],"name":"stickymd_package","src_path":"/r/tools/stickymd-package/src/lib.rs"},{"kind":["bin"],"name":"stickymd-package","src_path":"/r/tools/stickymd-package/src/main.rs"}]}"#;
const WIN: &str = r#"{"name":"stickymd-win","manifest_path":"/r/apps/stickymd-win/Cargo.toml","dependencies":[],"targets":[{"kind":["bin"],"name":"stickymd-win","src_path":"/r/apps/stickymd-win/src/main.rs"},{"kind":["custom-build"],"name":"build-script-build","src_path":"/r/apps/stickymd-win/build.rs"}]}"#;

/// Cargo's decoded metadata decides, whatever the TOML spelling (`["dependencies"]`,
/// `"build" = ...`): any dependency or build script of the packaging crate, a member's build
/// script elsewhere than its `build.rs`, a procedural macro and a second binary with the
/// product's name are refused; the members' build scripts are returned for their own check.
#[test]
fn dependencies_build_scripts_macros_and_binary_names_are_read_from_cargo_metadata() {
    assert_eq!(
        check_metadata(&metadata(&format!("{PACKAGE},{WIN}"))).unwrap(),
        ["apps/stickymd-win/build.rs"]
    );
    let dependency = PACKAGE.replace(
        r#""dependencies":[]"#,
        r#""dependencies":[{"name":"stickymd-smoke"}]"#,
    );
    let script = PACKAGE.replace(r#""kind":["lib"]"#, r#""kind":["custom-build"]"#);
    let moved = WIN.replace(
        "apps/stickymd-win/build.rs",
        "apps/stickymd-win/scripts/build.rs",
    );
    let impostor = r#"{"name":"stickymd-smoke","manifest_path":"/r/tools/stickymd-smoke/Cargo.toml","dependencies":[],"targets":[{"kind":["bin"],"name":"StickyMD-Win","src_path":"/r/tools/stickymd-smoke/src/bin/x.rs"}]}"#;
    let macro_member = r#"{"name":"helper","manifest_path":"/r/tools/helper/Cargo.toml","dependencies":[],"targets":[{"kind":["proc-macro"],"name":"helper","src_path":"/r/tools/helper/src/lib.rs"}]}"#;
    for (packages, reason) in [
        (dependency, "declares a dependency"),
        (script, "has a build script"),
        (format!("{PACKAGE},{moved}"), "only the build.rs beside"),
        (
            format!("{PACKAGE},{WIN},{impostor}"),
            "two workspace targets build the binary",
        ),
        (format!("{PACKAGE},{macro_member}"), "procedural macro"),
        (WIN.to_owned(), "no stickymd-package"),
    ] {
        let error = check_metadata(&metadata(&packages)).unwrap_err();
        assert!(error.contains(reason), "{reason}: {error}");
    }
}

#[test]
fn the_lexer_tells_comments_from_literals_and_masks_literal_contents() {
    let text = "let a = \"http://x\"; Command::new(\"stickymd-smoke\");\n// stickymd-smoke here\n/* \"stickymd-smoke\" /* nested */ */ let q = '\"'; let e = '\\''; let b = b'\"'; let r = r#\"//\"#; let br = br##\"a \"# b\"##; let c = c\"é\"; f::<'a>();\nlet s = \"include_str!(\\\"x\\\")\"; // é ü\nlet t = \"tail\";\n";
    let lexed = lex(text).unwrap();
    assert_eq!(lexed.code.len(), text.len());
    assert_eq!(lexed.tokens.len(), text.len());
    let error = check_names(&lexed.code).unwrap_err();
    assert!(error.starts_with("line 1 "), "{error}");
    let third = lexed.code.lines().nth(2).unwrap();
    assert!(!lexed.code.lines().nth(1).unwrap().contains("smoke"));
    assert!(!third.contains("smoke") && !third.contains("nested"));
    assert!(third.contains("r#\"//\"#") && third.contains("br##\"a \"# b\"##"));
    assert!(third.contains("c\"é\"") && third.contains("<'a>"));
    // Literal contents are masked in the tokens: no include hides in a string, and offsets
    // after multi-byte text still line up between code and tokens.
    assert!(!lexed.tokens.contains("include_str!"));
    let tail = lexed.tokens.find("tail").map(|_| ()).is_none();
    assert!(tail, "the literal `tail` is masked");
    let at = lexed.code.find("let t").unwrap();
    assert_eq!(&lexed.tokens[at..at + 5], "let t");
    assert!(lexed.strings.contains(&"tail".to_owned()));
    for bad in ["let s = \"open", "/* open", "let r = r#\"open\";"] {
        assert!(lex(bad).is_err(), "{bad}");
    }
}

fn check(file: &str, code: &str, source: Source, tracked: &[&str]) -> Result<Vec<String>, String> {
    let lexed = lex(code).unwrap();
    check_includes(file, &lexed, source, &|path| tracked.contains(&path))
}

const FILE: &str = "tools/stickymd-package/src/package_content.rs";
const TRACKED: &[&str] = &[
    "tools/stickymd-package/src/package_readme.txt",
    "tools/stickymd-package/src/other.rs",
    "tools/stickymd-package/src/notice.in",
    "assets/licenses/SIL-OFL-1.1.txt",
    "assets/fragment.rs",
    "docs/plan/11_testing_and_release.md",
];

#[test]
fn includes_must_reach_counted_inputs_spelled_as_git_records_them() {
    let ok = |code: &str| check(FILE, code, Source::Packaging, TRACKED).unwrap();
    assert!(ok("include_str!(\"package_readme.txt\")").is_empty());
    assert!(ok("include_str !(\n    \"package_readme.txt\"\n)").is_empty());
    assert!(ok("std::include_bytes![\"../../../assets/licenses/SIL-OFL-1.1.txt\"]").is_empty());
    assert_eq!(
        ok("include!(\"notice.in\");"),
        ["tools/stickymd-package/src/notice.in"]
    );
    assert_eq!(
        ok("# [ path = \"other.rs\" ] mod x;"),
        ["tools/stickymd-package/src/other.rs"]
    );
    // Not the built-in macros: a string, a custom macro, a plain identifier.
    assert!(ok("let s = \"include!(\\\"../../../outside.rs\\\")\";").is_empty());
    assert!(ok("my_include_str!(\"../../../docs/x.md\"); let include = 1;").is_empty());
    // A predicate that mentions `path` is not a path attribute.
    assert!(ok("#[cfg_attr(path, allow(dead_code))] fn f() {}").is_empty());
    for (bad, reason) in [
        (
            "include_str!(\"../../stickymd-smoke/src/x.txt\")",
            "not a tracked build input",
        ),
        (
            "include_bytes!(\"../../../docs/plan/11_testing_and_release.md\")",
            "not a tracked build input",
        ),
        // The same file on a case-insensitive volume, under a spelling Git does not record.
        (
            "include_str!(\"../../../Assets/licenses/SIL-OFL-1.1.txt\")",
            "not a tracked build input",
        ),
        (
            "include_str!(\"untracked.txt\")",
            "not a tracked build input",
        ),
        ("include_bytes!(\"/etc/x\")", "outside the repository"),
        (
            "include_str!(\"../../../../outside.txt\")",
            "outside the repository",
        ),
        // Tracked, but Rust outside the packaging crate.
        (
            "include!(\"../../../assets/fragment.rs\")",
            "a tracked file of the packaging crate",
        ),
        (
            "#[path = \"../../../assets/fragment.rs\"]\nmod x;",
            "a tracked file of the packaging crate",
        ),
        (
            "#[cfg_attr(windows, path = \"other.rs\")]\nmod x;",
            "cfg_attr",
        ),
        (
            "#[cfg_attr(all(a, b), cfg_attr(windows, path = \"other.rs\"))]\nmod x;",
            "cfg_attr",
        ),
        (
            "#[cfg_attr(windows, doc = \"[x]\", path = \"other.rs\")]\nmod x;",
            "cfg_attr",
        ),
        (
            "include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/x.rs\"))",
            "plain string literal",
        ),
    ] {
        let error = check(FILE, bad, Source::Packaging, TRACKED).unwrap_err();
        assert!(error.contains(reason), "{bad}: {error}");
    }
}

/// A fragment reached by `include!` is checked in turn (the real check queues it), and it may not
/// declare an out-of-line module, whose file the crate walk would not see.
#[test]
fn a_fragment_reached_by_include_is_checked_in_turn() {
    let fragment = "tools/stickymd-package/src/notice.in";
    let error = check(
        fragment,
        "const T: &str = include_str!(\"../../../docs/plan/11_testing_and_release.md\");",
        Source::Fragment,
        TRACKED,
    )
    .unwrap_err();
    assert!(error.contains("not a tracked build input"), "{error}");
    let error = check(fragment, "mod child;", Source::Fragment, TRACKED).unwrap_err();
    assert!(error.contains("out-of-line module"), "{error}");
    // In an enumerated `.rs` file an out-of-line module is an ordinary crate file.
    assert!(
        check(FILE, "mod child;", Source::Packaging, TRACKED)
            .unwrap()
            .is_empty()
    );
}

/// A member's build script runs in the Windows workspace build: it may include counted data and
/// hold inline modules, but declares no out-of-line module (raw names included) and includes no
/// Rust, so excluded code cannot ride along.
#[test]
fn a_build_script_is_self_contained() {
    let file = "tools/stickymd-smoke/build.rs";
    let tracked = ["assets/licenses/SIL-OFL-1.1.txt"];
    let ok = |code: &str| check(file, code, Source::BuildScript, &tracked).unwrap();
    ok("fn main() { include_str!(\"../../assets/licenses/SIL-OFL-1.1.txt\"); }");
    ok("mod helper { pub fn run() {} } fn main() { let model = 1; }");
    for (bad, reason) in [
        (
            "mod helper;\nfn main() { helper::run(); }",
            "out-of-line module",
        ),
        ("mod r#type;\nfn main() {}", "out-of-line module"),
        ("pub(crate) mod helper ;", "out-of-line module"),
        ("include!(\"helper.rs\");", "keep its code in build.rs"),
        (
            "#[path = \"helper.rs\"] mod h { }",
            "keep its code in build.rs",
        ),
        (
            "fn main() { include_str!(\"src/fixture.txt\"); }",
            "not a tracked build input",
        ),
    ] {
        let error = check(file, bad, Source::BuildScript, &tracked).unwrap_err();
        assert!(error.contains(reason), "{bad}: {error}");
    }
}

#[test]
fn rust_code_never_names_the_verification_tool_and_launches_scripts_by_name() {
    check_names("Err(\"belongs to stickymd-smoke release verify-package\")").unwrap();
    for bad in [
        "Command::new(\"cargo\").args([\"run\", \"-p\", \"stickymd-smoke\"])",
        "let x = \"cargo run -p stickymd-smoke -- release\";",
        "use stickymd_smoke::release;",
        "fs::read(root.join(\"tools/stickymd-smoke/tests/fixtures/notice.txt\"))",
    ] {
        let error = check_names(&lex(bad).unwrap().code).unwrap_err();
        assert!(
            error.contains("names the verification tool"),
            "{bad}: {error}"
        );
    }
    // A comment that mentions the tool is fine once comments are removed.
    check_names(&lex("// see stickymd-smoke/README.md\n").unwrap().code).unwrap();
    // A character literal holding a quote does not shift which literals are strings.
    let launched =
        lex("let q = b'\"'; windows::operation(root, shell, \"package-archive.ps1\", &[]);")
            .unwrap();
    assert_eq!(
        scripts_launched(&launched).unwrap(),
        ["package-archive.ps1"]
    );
    let error = scripts_launched(&lex("run(\"../smoke/helper.ps1\")").unwrap()).unwrap_err();
    assert!(error.contains("by a path"), "{error}");
}

#[test]
fn script_references_are_bound_to_tools_release_or_refused() {
    let origin = "tools/release/package.ps1";
    assert_eq!(
        check_script(
            "# invoke-smoke.ps1 is for verification; old-helper.ps1 is gone\n<# stickymd-smoke\n#>\n. (Join-Path $PSScriptRoot 'invoke-package.ps1')\n. \"$PSScriptRoot/notice-helper.ps1\"\n& (Join-Path $PSScriptRoot \"syft-fetch.ps1\") -Uri x\n",
            origin
        )
        .unwrap(),
        ["invoke-package.ps1", "notice-helper.ps1", "syft-fetch.ps1"]
    );
    assert_eq!(
        check_script(
            "run: ./tools/release/package.ps1 -ExactCandidate\n",
            "build-package"
        )
        .unwrap(),
        ["package.ps1"]
    );
    for (bad, reason) in [
        (
            ". (Join-Path $PSScriptRoot 'invoke-smoke.ps1')\n",
            "calls the verification tool",
        ),
        (
            "Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments @('notices')\n",
            "calls the verification tool",
        ),
        (
            "cargo run --quiet -p stickymd-smoke --locked -- release macos-package\n",
            "calls the verification tool",
        ),
        ("./tools/smoke/all.ps1 -Ci\n", "calls the verification tool"),
        (". $helper\n", "cannot follow"),
        // A same-named helper elsewhere is not the one in tools/release.
        ("& ./other/package-archive.ps1\n", "cannot bind"),
        (
            ". (Join-Path $PSScriptRoot '..\\shared\\helper.ps1')\n",
            "cannot bind",
        ),
        // `$PSScriptRoot` on the line does not bind a name to it.
        (
            "$a = $PSScriptRoot; & (Join-Path $other 'helper.ps1')\n",
            "cannot bind",
        ),
    ] {
        let error = check_script(bad, origin).unwrap_err();
        assert!(error.contains(reason), "{bad}: {error}");
    }
    // Outside tools/release only a repository path binds.
    let error = check_script("& \"$PSScriptRoot/package.ps1\"\n", "build-package").unwrap_err();
    assert!(error.contains("cannot bind"), "{error}");
}

/// A referenced helper that calls the verification tool is refused through the reference, and a
/// reference to a script that does not exist in tools/release is refused.
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
    write("package.ps1", ". (Join-Path $PSScriptRoot 'missing.ps1')\n");
    let error = check_scripts(root, vec!["package.ps1".to_owned()]).unwrap_err();
    assert!(error.contains("does not exist"), "{error}");
    directory.close().unwrap();
}

const WORKFLOW: &str = "name: x\njobs:\n  dependency-policy:\n    steps: []\n  self-check: # runs the verification tool\n    needs: dependency-policy\n    steps:\n      - run: cargo run -p stickymd-smoke -- release workspace-version\n  build-package:\n    needs: self-check\n    steps:\n      - name: Package\n# a comment at column zero does not end the job\n        run: ./tools/release/package.ps1 -ExactCandidate\n  verify-artifact:\n    needs: build-package\n    steps:\n      - name: Download\n        uses: actions/download-artifact@0000000000000000000000000000000000000000 # v7.0.0\n      - run: ./tools/release/verify-package.ps1\n";

#[test]
fn the_build_job_runs_no_verification_code_and_the_jobs_are_ordered() {
    assert_eq!(check_workflow(WORKFLOW).unwrap(), ["package.ps1"]);
    let dash = WORKFLOW.replace("      - name: Download\n        uses:", "      - uses:");
    check_workflow(&dash).unwrap();
    let smoke = WORKFLOW.replace(
        "        run: ./tools/release/package.ps1 -ExactCandidate\n",
        "        run: ./tools/release/package.ps1 -ExactCandidate\n      - run: cargo run -p stickymd-smoke -- qualification native-runtime\n",
    );
    let error = check_workflow(&smoke).unwrap_err();
    assert!(error.contains("job `build-package`"), "{error}");
    let in_run_text = WORKFLOW.replace(
        "      - name: Download\n        uses: actions/download-artifact@0000000000000000000000000000000000000000 # v7.0.0\n",
        "      - run: |\n          uses: actions/download-artifact@0000000000000000000000000000000000000000\n",
    );
    for (bad, reason) in [
        (
            WORKFLOW.replace("    needs: self-check\n", "    needs: dependency-policy\n"),
            "needs: self-check",
        ),
        // `needs` only inside a step body is not the job's field.
        (
            WORKFLOW.replace(
                "    needs: self-check\n",
                "      - run: echo\n        needs: self-check\n",
            ),
            "needs: self-check",
        ),
        // A download step that is commented out, or the action named only in a run text, is no
        // download.
        (
            WORKFLOW.replace(
                "        uses: actions/download-artifact@",
                "        # uses: actions/download-artifact@",
            ),
            "download",
        ),
        (in_run_text, "download"),
        (
            format!("{WORKFLOW}  build-package:\n    needs: self-check\n"),
            "defined twice",
        ),
        (
            WORKFLOW.replace("  verify-artifact:", "  verify:"),
            "no `verify-artifact` job",
        ),
    ] {
        let error = check_workflow(&bad).unwrap_err();
        assert!(error.contains(reason), "{reason}: {error}");
    }
}
