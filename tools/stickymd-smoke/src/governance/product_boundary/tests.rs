//! The product rule against the real repository and against scratch workspaces that break one
//! condition each, every one refused for its own stated reason.

use super::*;
use stickymd_package::temporary::TemporaryDirectory;

#[test]
fn the_real_repository_keeps_the_product_boundary() {
    let root = crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
    verify(&root).unwrap();
}

/// A scratch Git workspace: `apps/ship` (the shipped bin) depends on the lib at `base`, and
/// `tools/aside` is a member outside its closure. `extra` adds or replaces files; files named in
/// `untracked` stay out of the index.
fn probe(base: &str, extra: &[(&str, &str)], untracked: &[&str]) -> TemporaryDirectory {
    let directory = TemporaryDirectory::new("product-probe").unwrap();
    let root = directory.path();
    let mut files: BTreeMap<String, String> = BTreeMap::new();
    let package = |name: &str, dependencies: &str| {
        format!(
            "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\n{dependencies}"
        )
    };
    files.insert(
        "Cargo.toml".to_owned(),
        format!(
            "[workspace]\nresolver = \"2\"\nmembers = [\"apps/ship\", \"{base}\", \"tools/aside\"]\n"
        ),
    );
    files.insert(
        "apps/ship/Cargo.toml".to_owned(),
        package("ship", &format!("base = {{ path = \"../../{base}\" }}\n")),
    );
    files.insert(
        "apps/ship/src/main.rs".to_owned(),
        "fn main() {\n    base::f();\n}\n".to_owned(),
    );
    files.insert(format!("{base}/Cargo.toml"), package("base", ""));
    files.insert(format!("{base}/src/lib.rs"), "pub fn f() {}\n".to_owned());
    files.insert("tools/aside/Cargo.toml".to_owned(), package("aside", ""));
    files.insert(
        "tools/aside/src/main.rs".to_owned(),
        "fn main() {}\n".to_owned(),
    );
    files.insert("docs/x.md".to_owned(), "one\n".to_owned());
    for (path, text) in extra {
        files.insert((*path).to_owned(), (*text).to_owned());
    }
    for (path, text) in &files {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    let run = |command: &mut std::process::Command| {
        let output = command.current_dir(root).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run(compiled::cargo().args(["generate-lockfile", "--offline"]));
    run(crate::release::platform_inputs::git(root).args(["init", "-q"]));
    let tracked: Vec<&String> = files
        .keys()
        .filter(|path| !untracked.contains(&path.as_str()))
        .collect();
    run(crate::release::platform_inputs::git(root)
        .args(["add", "Cargo.lock", "--"])
        .args(tracked));
    directory
}

/// The probe's build: every platform's metadata, the workspace selection, the macOS classifier.
const PROBE: Build = Build {
    platform: Platform::MacOsArm64,
    triple: None,
    selection: &["--workspace"],
    shipped: "ship",
    outside: &["aside"],
    pinned: &[],
};

/// The probe's files, or the reason the compilation or the files are refused.
fn check(root: &Path, build: &Build) -> Result<Vec<String>, String> {
    let files = compiled_files(root, build)?;
    check_files(&files, &git_index(root)?, build.platform)?;
    Ok(files)
}

#[test]
fn the_closure_s_registered_files_are_counted_tracked_files() {
    let directory = probe("crates/base", &[], &[]);
    let files = check(directory.path(), &PROBE).unwrap();
    assert_eq!(files, ["apps/ship/src/main.rs", "crates/base/src/lib.rs"]);
    directory.close().unwrap();
}

/// The gap the rule closes: a product include of an excluded file, which rustc writes as the code
/// wrote it.
#[test]
fn an_include_that_leaves_the_tracked_spelling_is_refused() {
    let lib = "pub fn f() {}\npub const X: &str = include_str!(\"../../../docs/x.md\");\n";
    let directory = probe("crates/base", &[("crates/base/src/lib.rs", lib)], &[]);
    let error = check(directory.path(), &PROBE).unwrap_err();
    assert!(
        error.contains("crates/base/src/../../../docs/x.md")
            && error.contains("does not track under that spelling"),
        "{error}"
    );
    directory.close().unwrap();
}

#[test]
fn an_untracked_file_is_refused() {
    let lib = "pub fn f() {}\npub const Y: &str = include_str!(\"y.txt\");\n";
    let directory = probe(
        "crates/base",
        &[
            ("crates/base/src/lib.rs", lib),
            ("crates/base/src/y.txt", "y"),
        ],
        &["crates/base/src/y.txt"],
    );
    let error = check(directory.path(), &PROBE).unwrap_err();
    assert!(
        error.contains("crates/base/src/y.txt") && error.contains("does not track"),
        "{error}"
    );
    directory.close().unwrap();
}

/// A closure member in an excluded tree: its files are tracked, spelled as Git records them, and
/// still refused, since the fingerprint does not count them.
#[test]
fn a_file_the_platform_does_not_count_is_refused() {
    let directory = probe("experiments/base", &[], &[]);
    let error = check(directory.path(), &PROBE).unwrap_err();
    assert!(
        error.contains("experiments/base/src/lib.rs")
            && error.contains("its fingerprint does not count"),
        "{error}"
    );
    directory.close().unwrap();
}

#[test]
fn an_environment_read_is_refused() {
    let lib = "pub fn f() {}\npub const V: Option<&str> = option_env!(\"STICKYMD_PROBE_UNSET\");\n";
    let directory = probe("crates/base", &[("crates/base/src/lib.rs", lib)], &[]);
    let error = check(directory.path(), &PROBE).unwrap_err();
    assert!(
        error.contains("STICKYMD_PROBE_UNSET") && error.contains("environment"),
        "{error}"
    );
    directory.close().unwrap();
}

/// The build script's own unit is checked too: a module it declares outside the tracked spelling.
#[test]
fn a_build_script_s_files_are_checked() {
    let directory = probe(
        "crates/base",
        &[("apps/ship/build.rs", "fn main() {}\n")],
        &[],
    );
    let files = check(directory.path(), &PROBE).unwrap();
    assert!(
        files.iter().any(|file| file == "apps/ship/build.rs"),
        "{files:?}"
    );
    directory.close().unwrap();
    let script = "#[path = \"../../docs/m.rs\"]\nmod m;\nfn main() {}\n";
    let directory = probe(
        "crates/base",
        &[("apps/ship/build.rs", script), ("docs/m.rs", "\n")],
        &[],
    );
    let error = check(directory.path(), &PROBE).unwrap_err();
    assert!(error.contains("apps/ship/../../docs/m.rs"), "{error}");
    directory.close().unwrap();
}

/// The partition is closed: a compiled member outside the closure must be listed, and a listed
/// member must not be in the closure.
#[test]
fn every_compiled_member_is_in_the_closure_or_listed_outside_it() {
    let directory = probe("crates/base", &[], &[]);
    let unlisted = Build {
        outside: &[],
        ..PROBE
    };
    let error = check(directory.path(), &unlisted).unwrap_err();
    assert!(
        error.contains("compiled aside") && error.contains("nor listed outside"),
        "{error}"
    );
    let inside = Build {
        outside: &["aside", "base"],
        ..PROBE
    };
    let error = check(directory.path(), &inside).unwrap_err();
    assert!(error.contains("base is in ship's closure"), "{error}");
    let unknown = Build {
        outside: &["aside", "ghost"],
        ..PROBE
    };
    let error = check(directory.path(), &unknown).unwrap_err();
    assert!(error.contains("ghost"), "{error}");
    directory.close().unwrap();
}

/// A unit reused from an earlier compilation of the same directory carries old dep-info.
#[test]
fn a_reused_unit_is_refused() {
    let directory = probe("crates/base", &[], &[]);
    let root = directory.path();
    let scope = scope(&cargo_metadata(root, None).unwrap(), &PROBE).unwrap();
    let build_directory = root.join("check");
    files_in(root, &PROBE, &scope, &build_directory).unwrap();
    let error = files_in(root, &PROBE, &scope, &build_directory).unwrap_err();
    assert!(error.contains("not compiled afresh"), "{error}");
    directory.close().unwrap();
}

#[test]
fn a_build_script_rule_names_its_own_directory() {
    let good = "/t/b/x-1/build_script_build-1.d: a/build.rs\n\n/t/b/x-1/build_script_build-1: a/build.rs\n\na/build.rs:\n";
    assert_eq!(
        compiled::parse_build_script(good, "/t/b/x-1").unwrap(),
        ["a/build.rs"]
    );
    for output in [
        "/t/b/x-2/build_script_build-1",
        "/t/b/x-1/other-1",
        "/t/b/x-1/sub/build_script_build-1",
    ] {
        let bad = format!("{output}: a/build.rs\n\na/build.rs:\n");
        assert!(
            compiled::parse_build_script(&bad, "/t/b/x-1").is_err(),
            "{output}"
        );
    }
    // A lib's rules stay as they were.
    assert!(compiled::parse(good).is_err());
}

#[test]
fn the_pinned_commands_must_still_run() {
    let directory = TemporaryDirectory::new("product-pins").unwrap();
    let root = directory.path();
    std::fs::create_dir_all(root.join(".github/workflows")).unwrap();
    std::fs::write(
        root.join(".github/workflows/release.yml"),
        "run: cargo build --workspace --locked\n",
    )
    .unwrap();
    let error = check_pinned(root, &WINDOWS).unwrap_err();
    assert!(
        error.contains("no longer runs `cargo build --workspace --release --locked`"),
        "{error}"
    );
    directory.close().unwrap();
}

/// The shipped binaries, the selections, the outside lists and the pinned commands are plan 11's.
#[test]
fn the_builds_are_the_contract_s() {
    let root = crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
    let plan = std::fs::read_to_string(root.join("docs/plan/11_testing_and_release.md")).unwrap();
    let marker = "**产品发布图";
    assert_eq!(plan.matches(marker).count(), 1);
    let start = plan.find(marker).unwrap();
    let bullet = &plan[start..start + plan[start..].find("**前提：**").unwrap()];
    // The text wraps lines; compare it as one line.
    let bullet: String = bullet.lines().map(str::trim).collect::<Vec<_>>().join("");
    let from = bullet
        .find("闭包外的 workspace 单元只能属于下列成员")
        .unwrap();
    let to = from + bullet[from..].find("其他单元一律拒绝").unwrap();
    let listed = &bullet[from..to];
    let windows: BTreeSet<&str> = listed
        .split_once("Windows 为 ")
        .unwrap()
        .1
        .split_once("，macOS 为空")
        .unwrap()
        .0
        .split('、')
        .map(|name| name.trim_matches('`'))
        .collect();
    assert_eq!(windows, WINDOWS.outside.iter().copied().collect());
    assert!(MACOS.outside.is_empty());
    for build in [&WINDOWS, &MACOS] {
        assert!(
            bullet.contains(&format!("`{}`", build.shipped)),
            "{}",
            build.shipped
        );
    }
    assert!(bullet.contains("`--workspace --release`"));
    assert_eq!(WINDOWS.selection, ["--workspace"]);
    let mac = format!(
        "`-p stickymd-mac --release --target {}`",
        MACOS.triple.unwrap()
    );
    assert!(bullet.contains(&mac), "{mac}");
    assert_eq!(
        MACOS.selection,
        ["-p", "stickymd-mac", "--target", MACOS.triple.unwrap()]
    );
    assert!(bullet.contains(&format!("`{}`", WINDOWS.pinned[0].1)));
    for build in [&WINDOWS, &MACOS] {
        assert_eq!(build.platform.target(), build.triple.unwrap());
        assert_eq!(build.platform.host_package(), build.shipped);
    }
}
