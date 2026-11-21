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

/// Cargo's decoded metadata decides, whatever the TOML spelling: any dependency, build script or
/// out-of-crate target of the packaging crate, a member's build script elsewhere than its
/// `build.rs` or in a directory Windows does not count, a procedural macro and a second binary
/// with the product's name are refused.
#[test]
fn dependencies_targets_build_scripts_macros_and_binary_names_are_read_from_cargo_metadata() {
    assert_eq!(
        check_metadata(&metadata(&format!("{PACKAGE},{WIN}"))).unwrap(),
        Workspace {
            package_targets: vec![
                "tools/stickymd-package/src/lib.rs".to_owned(),
                "tools/stickymd-package/src/main.rs".to_owned()
            ],
            build_scripts: vec!["apps/stickymd-win/build.rs".to_owned()],
        }
    );
    let dependency = PACKAGE.replace(
        r#""dependencies":[]"#,
        r#""dependencies":[{"name":"stickymd-smoke"}]"#,
    );
    let script = PACKAGE.replace(r#""kind":["lib"]"#, r#""kind":["custom-build"]"#);
    let outside = PACKAGE.replace(
        "/r/tools/stickymd-package/src/lib.rs",
        "/r/tools/stickymd-smoke/src/lib.rs",
    );
    let moved = WIN.replace(
        "apps/stickymd-win/build.rs",
        "apps/stickymd-win/scripts/build.rs",
    );
    let smoke_script = r#"{"name":"stickymd-smoke","manifest_path":"/r/tools/stickymd-smoke/Cargo.toml","dependencies":[],"targets":[{"kind":["custom-build"],"name":"build-script-build","src_path":"/r/tools/stickymd-smoke/build.rs"}]}"#;
    let mac_script = smoke_script
        .replace("stickymd-smoke", "stickymd-mac")
        .replace("tools/", "apps/");
    let impostor = r#"{"name":"stickymd-smoke","manifest_path":"/r/tools/stickymd-smoke/Cargo.toml","dependencies":[],"targets":[{"kind":["bin"],"name":"StickyMD-Win","src_path":"/r/tools/stickymd-smoke/src/bin/x.rs"}]}"#;
    let macro_member = r#"{"name":"helper","manifest_path":"/r/crates/helper/Cargo.toml","dependencies":[],"targets":[{"kind":["proc-macro"],"name":"helper","src_path":"/r/crates/helper/src/lib.rs"}]}"#;
    for (packages, reason) in [
        (dependency, "declares a dependency"),
        (script, "has a build script"),
        (outside, "outside the packaging crate"),
        (format!("{PACKAGE},{moved}"), "only the build.rs beside"),
        (
            format!("{PACKAGE},{smoke_script}"),
            "not counted for Windows",
        ),
        (format!("{PACKAGE},{mac_script}"), "not counted for Windows"),
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

/// The Git index alone says what is tracked: a file on disk that was never added is not.
#[test]
fn the_git_index_leaves_out_untracked_files() {
    let directory = stickymd_package::temporary::TemporaryDirectory::new("boundary-index").unwrap();
    let root = directory.path();
    let git = |arguments: &[&str]| {
        assert!(
            std::process::Command::new("git")
                .current_dir(root)
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
    assert!(index.contains("tracked.txt"));
    assert!(!index.contains("untracked.txt"));
    directory.close().unwrap();
}

#[test]
fn the_lexer_tells_comments_from_literals_and_masks_literal_contents() {
    let text = "let a = \"http://x\"; Command::new(\"other\");\n// stickymd-smoke here\n/* \"stickymd-smoke\" /* nested */ */ let q = '\"'; let e = '\\''; let b = b'\"'; let r = r#\"//\"#; let br = br##\"a \"# b\"##; let c = c\"é\"; f::<'a>();\nlet s = \"include_str!(\\\"x\\\")\"; // é ü\nlet t = \"tail\";\n";
    let lexed = lex(text).unwrap();
    assert_eq!(lexed.code.len(), text.len());
    assert_eq!(lexed.tokens.len(), text.len());
    check_names(&lexed.code).unwrap();
    let third = lexed.code.lines().nth(2).unwrap();
    assert!(!lexed.code.lines().nth(1).unwrap().contains("smoke"));
    assert!(!third.contains("smoke") && !third.contains("nested"));
    assert!(third.contains("r#\"//\"#") && third.contains("br##\"a \"# b\"##"));
    assert!(third.contains("c\"é\"") && third.contains("<'a>"));
    // Literal contents are masked in the tokens, and offsets after multi-byte text line up.
    assert!(!lexed.tokens.contains("include_str!") && !lexed.tokens.contains("tail"));
    let at = lexed.code.find("let t").unwrap();
    assert_eq!(&lexed.tokens[at..at + 5], "let t");
    assert!(lexed.strings.contains(&"tail".to_owned()));
    for bad in ["let s = \"open", "/* open", "let r = r#\"open\";"] {
        assert!(lex(bad).is_err(), "{bad}");
    }
}

fn check(file: &str, code: &str, source: Source, tracked: &[&str]) -> Result<(), String> {
    let lexed = lex(code).unwrap();
    check_includes(file, &lexed, source, &|path| tracked.contains(&path))
}

const FILE: &str = "tools/stickymd-package/src/package_content.rs";
const TRACKED: &[&str] = &[
    "tools/stickymd-package/src/package_readme.txt",
    "tools/stickymd-package/src/other.rs",
    "assets/licenses/SIL-OFL-1.1.txt",
    "docs/plan/11_testing_and_release.md",
];

#[test]
fn data_includes_must_reach_counted_inputs_spelled_as_git_records_them() {
    let ok = |code: &str| check(FILE, code, Source::Packaging, TRACKED).unwrap();
    ok("include_str!(\"package_readme.txt\")");
    ok("include_str !(\n    \"package_readme.txt\"\n)");
    ok("std::include_bytes![\"../../../assets/licenses/SIL-OFL-1.1.txt\"]");
    // Not a built-in macro, an attribute or a module path: a string, a custom macro, a plain
    // identifier, a predicate that mentions `path`.
    ok("let s = \"include!(\\\"../../../outside.rs\\\")\";");
    ok("my_include_str!(\"../../../docs/x.md\"); let include = 1; let path = 2;");
    ok("#[cfg_attr(path, allow(dead_code))] fn f() {}");
    // A tracked file that both platforms exclude, reached under every spelling of the macro.
    let excluded = "\"../../../docs/plan/11_testing_and_release.md\"";
    for spelling in [
        "include_str!",
        "core::include_str!",
        "r#include_str!",
        "std::r#include_bytes!",
        "include_bytes !",
    ] {
        let error = check(
            FILE,
            &format!("{spelling}({excluded})"),
            Source::Packaging,
            TRACKED,
        )
        .unwrap_err();
        assert!(
            error.contains("not a tracked build input"),
            "{spelling}: {error}"
        );
    }
    for (bad, reason) in [
        (
            "include_str!(\"../../stickymd-smoke/src/x.txt\")",
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
        (
            "include_str!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/x.txt\"))",
            "plain string literal",
        ),
    ] {
        let error = check(FILE, bad, Source::Packaging, TRACKED).unwrap_err();
        assert!(error.contains(reason), "{bad}: {error}");
    }
}

/// The packaging crate's Rust is its own `.rs` files: `include!` and `#[path]` are refused in any
/// spelling, so no file escapes the enumeration, however it is named or wherever it sits.
#[test]
fn no_rust_is_brought_in_by_path() {
    for bad in [
        "include!(\"other.rs\");",
        "r#include!(\"notice.in\");",
        "core::include!(\"other.rs\");",
        "#[path = \"other.rs\"] mod x;",
        "# [ path = \"other.rs\" ] mod x;",
        "mod layout { #[path = \"part.in\"] mod part; }",
        "#![path = \"other.rs\"]",
        "# ! [ r#path = \"other.rs\" ]",
    ] {
        let error = check(FILE, bad, Source::Packaging, TRACKED).unwrap_err();
        assert!(error.contains("brings in Rust by path"), "{bad}: {error}");
    }
    for bad in [
        "#[cfg_attr(windows, path = \"other.rs\")]\nmod x;",
        "#[cfg_attr(all(a, b), cfg_attr(windows, path = \"other.rs\"))]\nmod x;",
        "#[cfg_attr(windows, doc = \"[x]\", path = \"other.rs\")]\nmod x;",
        "#[cfg_attr(windows, r#path = \"other.rs\")]\nmod x;",
    ] {
        let error = check(FILE, bad, Source::Packaging, TRACKED).unwrap_err();
        assert!(error.contains("cfg_attr"), "{bad}: {error}");
    }
    // An out-of-line module of the crate is an ordinary enumerated file.
    check(FILE, "mod child;", Source::Packaging, TRACKED).unwrap();
}

/// A member's build script runs in the Windows workspace build: it keeps its code in itself
/// (inline modules allowed, raw names included), names no path leaving its member and includes
/// data only from counted paths.
#[test]
fn a_build_script_is_self_contained() {
    let file = "apps/stickymd-win/build.rs";
    let tracked = [
        "assets/licenses/SIL-OFL-1.1.txt",
        "apps/stickymd-win/StickyMD.manifest",
    ];
    let ok = |code: &str| check(file, code, Source::BuildScript, &tracked).unwrap();
    ok("mod helper { pub fn run() {} } fn main() { fs::read(\"StickyMD.manifest\"); }");
    ok("fn main() { let r#mod = 1; }");
    ok("fn main() { println!(\"cargo:rerun-if-changed=StickyMD.manifest\"); let r = \"a..b\"; }");
    // Both platforms count every member's build.rs, so what it includes must be counted by both:
    // a shared crate's own data is, the Windows host's manifest is not.
    check(
        "crates/stickymd-render/build.rs",
        "fn main() { include_bytes!(\"data/table.bin\"); }",
        Source::BuildScript,
        &["crates/stickymd-render/data/table.bin"],
    )
    .unwrap();
    for (bad, reason) in [
        (
            "fn main() { include_str!(\"StickyMD.manifest\"); }",
            "not a tracked build input",
        ),
        // Counted by both platforms, but outside the member: the path rule refuses it.
        (
            "fn main() { include_str!(\"../../assets/licenses/SIL-OFL-1.1.txt\"); }",
            "leaves the member's directory",
        ),
        (
            "mod helper;\nfn main() { helper::run(); }",
            "out-of-line module",
        ),
        ("mod r#type;\nfn main() {}", "out-of-line module"),
        ("pub(crate) mod helper ;", "out-of-line module"),
        ("include!(\"helper.rs\");", "brings in Rust by path"),
        (
            "#[path = \"helper.rs\"] mod h { }",
            "brings in Rust by path",
        ),
        (
            "fn main() { include_str!(\"src/fixture.txt\"); }",
            "not a tracked build input",
        ),
        (
            "fn main() { fs::read(\"../../docs/plan/x.md\"); }",
            "leaves the member's directory",
        ),
        (
            "fn main() { fs::read(\"src\\\\..\\\\..\\\\x\"); }",
            "leaves the member's directory",
        ),
        // A `..` after `=` or `:`, and absolute paths in either separator style.
        (
            "fn main() { println!(\"cargo:rerun-if-changed=../x\"); }",
            "leaves the member's directory",
        ),
        (
            "fn main() { fs::read(\"/etc/x\"); }",
            "leaves the member's directory",
        ),
        (
            "fn main() { fs::read(\"C:\\\\x\"); }",
            "leaves the member's directory",
        ),
        (
            "fn main() { println!(\"cargo:rustc-env=X=C:/x\"); }",
            "leaves the member's directory",
        ),
    ] {
        let error = check(file, bad, Source::BuildScript, &tracked).unwrap_err();
        assert!(error.contains(reason), "{bad}: {error}");
    }
}

#[test]
fn rust_code_never_names_the_verification_tool_and_launches_scripts_by_name() {
    for bad in [
        "Command::new(\"cargo\").args([\"run\", \"-p\", \"stickymd-smoke\"])",
        "Err(\"belongs to stickymd-smoke release verify-package\")",
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
    for bad in [
        "run(\"../smoke/helper.ps1\")",
        "run(\"Package-Archive.PS1\")",
    ] {
        let error = scripts_launched(&lex(bad).unwrap()).unwrap_err();
        assert!(error.contains("by its file name"), "{bad}: {error}");
    }
}

#[test]
fn script_references_are_bound_to_tools_release_or_refused() {
    let origin = "tools/release/package.ps1";
    assert_eq!(
        check_script(
            "# invoke-smoke.ps1 is for verification; old-helper.ps1 is gone\n<# stickymd-smoke\n#>\n. (Join-Path $PSScriptRoot 'invoke-package.ps1')\n. \"$PSScriptRoot/notice-helper.ps1\"\n& (Join-Path $PSScriptRoot \"syft-fetch.ps1\") -Uri x # helper.ps1 in a comment\n<# a note #> & (Join-Path $PSScriptRoot 'syft-execute.ps1')\n",
            origin
        )
        .unwrap(),
        ["invoke-package.ps1", "notice-helper.ps1", "syft-fetch.ps1", "syft-execute.ps1"]
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
        // Code after a block comment on the same line is code.
        (
            "<# note #> ./tools/smoke/all.ps1\n",
            "calls the verification tool",
        ),
        (". $helper\n", "cannot follow"),
        // Another bound name on the line does not stand for the dot-source's own target.
        (
            ". $helper; Write-Output (Join-Path $PSScriptRoot 'a.ps1')\n",
            "cannot follow",
        ),
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
        // The name as Git records it: no other letter case.
        ("& (Join-Path $PSScriptRoot 'Helper.PS1')\n", "cannot bind"),
    ] {
        let error = check_script(bad, origin).unwrap_err();
        assert!(error.contains(reason), "{bad}: {error}");
    }
    // Outside tools/release only a repository path binds; multi-byte spaces do not upset the scan.
    let error = check_script("& \"$PSScriptRoot/package.ps1\"\n", "build-package").unwrap_err();
    assert!(error.contains("cannot bind"), "{error}");
    let error = check_script("echo\u{3000}helper.ps1\n", "build-package").unwrap_err();
    assert!(error.contains("cannot bind"), "{error}");
}

/// A referenced helper is checked through the reference; one that Git does not track under the
/// spelling used, or that the calling platform does not count (macOS: none of tools/release), is
/// refused.
#[test]
fn referenced_scripts_are_checked_transitively_and_counted_for_the_caller() {
    let directory =
        stickymd_package::temporary::TemporaryDirectory::new("boundary-scripts").unwrap();
    let root = directory.path();
    std::fs::create_dir_all(root.join(SCRIPTS)).unwrap();
    let write =
        |name: &str, text: &str| std::fs::write(root.join(SCRIPTS).join(name), text).unwrap();
    let tracked = [
        "tools/release/package.ps1",
        "tools/release/notice-helper.ps1",
    ];
    let is_tracked = |path: &str| tracked.contains(&path);
    let windows = |name: &str| {
        check_scripts(
            root,
            &is_tracked,
            Platform::WindowsX64,
            vec![name.to_owned()],
        )
    };
    write("package.ps1", ". \"$PSScriptRoot/notice-helper.ps1\"\n");
    write(
        "notice-helper.ps1",
        "Get-Content tools/stickymd-smoke/tests/x\n",
    );
    let error = windows("package.ps1").unwrap_err();
    assert!(
        error.contains("notice-helper.ps1") && error.contains("verification tool"),
        "{error}"
    );
    write("notice-helper.ps1", "Write-Output ok\n");
    windows("package.ps1").unwrap();
    write("package.ps1", ". (Join-Path $PSScriptRoot 'missing.ps1')\n");
    let error = windows("package.ps1").unwrap_err();
    assert!(error.contains("does not track"), "{error}");
    let error = check_scripts(
        root,
        &is_tracked,
        Platform::MacOsArm64,
        vec!["notice-helper.ps1".to_owned()],
    )
    .unwrap_err();
    assert!(error.contains("does not count"), "{error}");
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
    let download = "        uses: actions/download-artifact@0000000000000000000000000000000000000000 # v7.0.0\n";
    let in_run_text = WORKFLOW.replace(
        &format!("      - name: Download\n{download}"),
        "      - run: |\n          uses: actions/download-artifact@0000000000000000000000000000000000000000\n",
    );
    // The action named only in a job-level environment scalar, at the steps' field indent.
    let in_env = WORKFLOW.replace(
        &format!("    steps:\n      - name: Download\n{download}"),
        "    env:\n      NOTE: |\n        uses: actions/download-artifact@0000000000000000000000000000000000000000\n    steps:\n",
    );
    for (bad, reason) in [
        (
            WORKFLOW.replace("    needs: self-check\n", "    needs: dependency-policy\n"),
            "needs: self-check",
        ),
        (
            WORKFLOW.replace(
                "    needs: self-check\n",
                "      - run: echo\n        needs: self-check\n",
            ),
            "needs: self-check",
        ),
        (
            WORKFLOW.replace(
                "        uses: actions/download-artifact@",
                "        # uses: actions/download-artifact@",
            ),
            "download",
        ),
        (in_run_text, "download"),
        (in_env, "download"),
        (
            format!("{WORKFLOW}  build-package:\n    needs: self-check\n"),
            "defined twice",
        ),
        (
            WORKFLOW.replace("  verify-artifact:", "  verify:"),
            "no `verify-artifact` job",
        ),
        (
            WORKFLOW.replace("  dependency-policy:", "  policy:"),
            "no `dependency-policy` job",
        ),
    ] {
        let error = check_workflow(&bad).unwrap_err();
        assert!(error.contains(reason), "{reason}: {error}");
    }
}
