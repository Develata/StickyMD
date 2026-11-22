//! The product rule against the real repository and against scratch workspaces that break one
//! condition each, every one refused for its own stated reason.

use super::*;
use stickymd_package::temporary::TemporaryDirectory;

#[test]
fn the_real_repository_keeps_the_product_boundary() {
    let root = crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
    verify(&root).unwrap();
}

fn package(name: &str, rest: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n{rest}")
}

/// A scratch workspace: `apps/ship` (the shipped bin) depends on the lib `crates/base`, and
/// `tools/aside` is a member outside its closure.
fn workspace() -> BTreeMap<String, String> {
    [
        (
            "Cargo.toml",
            "[workspace]\nresolver = \"2\"\nmembers = [\"apps/ship\", \"crates/base\", \"tools/aside\"]\n"
                .to_owned(),
        ),
        (
            "apps/ship/Cargo.toml",
            package(
                "ship",
                "[dependencies]\nbase = { path = \"../../crates/base\" }\n",
            ),
        ),
        (
            "apps/ship/src/main.rs",
            "fn main() {\n    base::f();\n}\n".to_owned(),
        ),
        ("crates/base/Cargo.toml", package("base", "")),
        ("crates/base/src/lib.rs", "pub fn f() {}\n".to_owned()),
        ("tools/aside/Cargo.toml", package("aside", "")),
        ("tools/aside/src/main.rs", "fn main() {}\n".to_owned()),
        ("docs/x.md", "one\n".to_owned()),
    ]
    .into_iter()
    .map(|(path, text)| (path.to_owned(), text))
    .collect()
}

/// Git for the scratch repository alone: an inherited `GIT_DIR` or `GIT_INDEX_FILE` (a hook, a
/// caller's environment) would point it at another repository.
fn scratch_git(root: &Path) -> std::process::Command {
    let mut command = crate::release::platform_inputs::git(root);
    for variable in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_COMMON_DIR",
    ] {
        command.env_remove(variable);
    }
    command
}

/// Writes `files` into a new Git repository, with a lock file, and adds all but `untracked`.
fn probe(files: &BTreeMap<String, String>, untracked: &[&str]) -> TemporaryDirectory {
    let directory = TemporaryDirectory::new("product-probe").unwrap();
    let root = directory.path();
    for (path, text) in files {
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
    run(scratch_git(root).args(["init", "-q"]));
    let tracked: Vec<&String> = files
        .keys()
        .filter(|path| !untracked.contains(&path.as_str()))
        .collect();
    run(scratch_git(root)
        .args(["add", "Cargo.lock", "--"])
        .args(tracked));
    directory
}

/// The probe workspace with `changes` applied.
fn probe_with(changes: &[(&str, &str)], untracked: &[&str]) -> TemporaryDirectory {
    let mut files = workspace();
    for (path, text) in changes {
        files.insert((*path).to_owned(), (*text).to_owned());
    }
    probe(&files, untracked)
}

/// The probe's build: every platform's metadata, the workspace selection, the macOS classifier.
const PROBE: Build = Build {
    platform: Platform::MacOsArm64,
    triple: None,
    selection: &["--workspace"],
    shipped: "ship",
    outside: &["aside"],
    command: ("", ""),
};

/// The probe's files, or the reason the compilation or the files are refused.
fn check(root: &Path, build: &Build) -> Result<Vec<String>, String> {
    let files = compiled_files(root, build)?;
    // The scratch repository's own index, whatever the caller's Git environment.
    let output = scratch_git(root)
        .args(["ls-files", "-z", "--cached", "--stage"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let index = super::super::package_boundary::parse_index(&output.stdout)?;
    check_files(&files, &index, build.platform)?;
    Ok(files)
}

#[test]
fn the_closure_s_registered_files_are_counted_tracked_files() {
    let directory = probe(&workspace(), &[]);
    let files = check(directory.path(), &PROBE).unwrap();
    assert_eq!(files, ["apps/ship/src/main.rs", "crates/base/src/lib.rs"]);
    directory.close().unwrap();
}

#[test]
fn the_shipped_member_s_lib_is_checked_with_its_bin() {
    let directory = probe_with(&[("apps/ship/src/lib.rs", "pub fn g() {}\n")], &[]);
    let files = check(directory.path(), &PROBE).unwrap();
    assert!(
        files.iter().any(|file| file == "apps/ship/src/lib.rs"),
        "{files:?}"
    );
    directory.close().unwrap();
}

/// The gap the rule closes: a product include of an excluded file, which rustc writes as the code
/// wrote it.
#[test]
fn an_include_that_leaves_the_tracked_spelling_is_refused() {
    let lib = "pub fn f() {}\npub const X: &str = include_str!(\"../../../docs/x.md\");\n";
    let directory = probe_with(&[("crates/base/src/lib.rs", lib)], &[]);
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
    let directory = probe_with(
        &[
            ("crates/base/src/lib.rs", lib),
            ("crates/base/src/y.txt", "y"),
        ],
        &["crates/base/src/y.txt"],
    );
    let error = check(directory.path(), &PROBE).unwrap_err();
    assert!(
        error.contains("crates/base/src/y.txt")
            && error.contains("does not track under that spelling"),
        "{error}"
    );
    directory.close().unwrap();
}

/// A closure member in an excluded tree: its files are tracked, spelled as Git records them, and
/// still refused, since the fingerprint does not count them.
#[test]
fn a_file_the_platform_does_not_count_is_refused() {
    let mut files = workspace();
    files.retain(|path, _| !path.starts_with("crates/base/"));
    files.insert(
        "Cargo.toml".to_owned(),
        "[workspace]\nresolver = \"2\"\nmembers = [\"apps/ship\", \"experiments/base\", \"tools/aside\"]\n"
            .to_owned(),
    );
    files.insert(
        "apps/ship/Cargo.toml".to_owned(),
        package(
            "ship",
            "[dependencies]\nbase = { path = \"../../experiments/base\" }\n",
        ),
    );
    files.insert(
        "experiments/base/Cargo.toml".to_owned(),
        package("base", ""),
    );
    files.insert(
        "experiments/base/src/lib.rs".to_owned(),
        "pub fn f() {}\n".to_owned(),
    );
    let directory = probe(&files, &[]);
    let error = check(directory.path(), &PROBE).unwrap_err();
    assert!(
        error.contains("experiments/base/src/lib.rs")
            && error.contains("its fingerprint does not count"),
        "{error}"
    );
    directory.close().unwrap();
}

/// A local package the workspace excludes is linked into the product but is no member: refused
/// before anything compiles.
#[test]
fn a_local_package_outside_the_workspace_is_refused() {
    let directory = probe_with(
        &[
            (
                "Cargo.toml",
                "[workspace]\nresolver = \"2\"\nmembers = [\"apps/ship\", \"crates/base\", \"tools/aside\"]\nexclude = [\"experiments/vendor\"]\n",
            ),
            (
                "apps/ship/Cargo.toml",
                &package(
                    "ship",
                    "[dependencies]\nbase = { path = \"../../crates/base\" }\nvendor = { path = \"../../experiments/vendor\" }\n",
                ),
            ),
            ("experiments/vendor/Cargo.toml", &package("vendor", "")),
            ("experiments/vendor/src/lib.rs", "pub fn v() {}\n"),
        ],
        &[],
    );
    let error = check(directory.path(), &PROBE).unwrap_err();
    assert!(
        error.contains("vendor") && error.contains("local package outside the workspace"),
        "{error}"
    );
    directory.close().unwrap();
}

#[test]
fn an_environment_read_is_refused() {
    let lib = "pub fn f() {}\npub const V: Option<&str> = option_env!(\"STICKYMD_PROBE_UNSET\");\n";
    let directory = probe_with(&[("crates/base/src/lib.rs", lib)], &[]);
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
    let directory = probe_with(&[("apps/ship/build.rs", "fn main() {}\n")], &[]);
    let files = check(directory.path(), &PROBE).unwrap();
    assert!(
        files.iter().any(|file| file == "apps/ship/build.rs"),
        "{files:?}"
    );
    directory.close().unwrap();
    let script = "#[path = \"../../docs/m.rs\"]\nmod m;\nfn main() {}\n";
    let directory = probe_with(&[("apps/ship/build.rs", script), ("docs/m.rs", "\n")], &[]);
    let error = check(directory.path(), &PROBE).unwrap_err();
    assert!(
        error.contains("apps/ship/../../docs/m.rs")
            && error.contains("does not track under that spelling"),
        "{error}"
    );
    directory.close().unwrap();
}

/// The packaging step takes the binary by the member's name: a renamed bin, or a second one, is
/// refused rather than checked in its place.
#[test]
fn the_shipped_member_has_exactly_one_bin_named_like_it() {
    let renamed = package(
        "ship",
        "[[bin]]\nname = \"other\"\npath = \"src/main.rs\"\n\n[dependencies]\nbase = { path = \"../../crates/base\" }\n",
    );
    let directory = probe_with(&[("apps/ship/Cargo.toml", &renamed)], &[]);
    let error = check(directory.path(), &PROBE).unwrap_err();
    assert!(
        error.contains("[\"other\"]") && error.contains("exactly one, named ship"),
        "{error}"
    );
    directory.close().unwrap();
    let directory = probe_with(&[("apps/ship/src/bin/extra.rs", "fn main() {}\n")], &[]);
    let error = check(directory.path(), &PROBE).unwrap_err();
    assert!(error.contains("exactly one, named ship"), "{error}");
    directory.close().unwrap();
}

/// The partition is closed: a compiled member outside the closure must be listed, and a listed
/// member must exist and not be in the closure.
#[test]
fn every_compiled_member_is_in_the_closure_or_listed_outside_it() {
    let directory = probe(&workspace(), &[]);
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
    assert!(
        error.contains("ghost") && error.contains("not a workspace member"),
        "{error}"
    );
    directory.close().unwrap();
}

/// A selection that does not build the shipped binary leaves an expected unit uncompiled.
#[test]
fn every_expected_unit_must_be_compiled() {
    let directory = probe(&workspace(), &[]);
    let partial = Build {
        selection: &["-p", "base"],
        ..PROBE
    };
    let error = check(directory.path(), &partial).unwrap_err();
    assert!(
        error.contains("compiled no unit for the bin `ship`"),
        "{error}"
    );
    directory.close().unwrap();
}

/// A member that is also a build dependency compiles a second unit of the same lib (the build
/// profile differs), which the rule refuses rather than check one and skip the other.
#[test]
fn a_target_compiled_twice_is_refused() {
    let manifest = package(
        "ship",
        "[dependencies]\nbase = { path = \"../../crates/base\" }\n\n[build-dependencies]\nbase = { path = \"../../crates/base\" }\n",
    );
    let directory = probe_with(
        &[
            ("apps/ship/Cargo.toml", &manifest),
            ("apps/ship/build.rs", "fn main() {\n    base::f();\n}\n"),
        ],
        &[],
    );
    let error = check(directory.path(), &PROBE).unwrap_err();
    assert!(error.contains("the lib `base` of base twice"), "{error}");
    directory.close().unwrap();
}

/// A unit reused from an earlier compilation of the same directory carries old dep-info.
#[test]
fn a_reused_unit_is_refused() {
    let directory = probe(&workspace(), &[]);
    let root = directory.path();
    let scope = scope(&cargo_metadata(root, None).unwrap(), &PROBE).unwrap();
    let build_directory = root.join("check");
    files_in(root, &PROBE, &scope, &build_directory).unwrap();
    let error = files_in(root, &PROBE, &scope, &build_directory).unwrap_err();
    assert!(error.contains("not compiled afresh"), "{error}");
    directory.close().unwrap();
}

#[test]
fn a_dep_info_must_name_its_unit_s_entry() {
    let paths = vec!["a/src/lib.rs".to_owned(), "a/src/x.rs".to_owned()];
    assert!(require_entry(paths.clone(), "a/src/lib.rs").is_ok());
    let error = require_entry(paths, "b/src/lib.rs").unwrap_err();
    assert!(
        error.contains("does not name its entry b/src/lib.rs"),
        "{error}"
    );
}

#[test]
fn a_build_script_rule_names_only_its_dep_info_and_executable() {
    let dep_info = "/t/b/x-1/build_script_build-1.d";
    let rules = |executable: &str| {
        format!("{dep_info}: a/build.rs\n\n{executable}: a/build.rs\n\na/build.rs:\n")
    };
    // The executable as this platform names it, and the other platform's name, refused.
    let (own, other) = if cfg!(windows) {
        (
            "/t/b/x-1/build_script_build-1.exe",
            "/t/b/x-1/build_script_build-1",
        )
    } else {
        (
            "/t/b/x-1/build_script_build-1",
            "/t/b/x-1/build_script_build-1.exe",
        )
    };
    let good = rules(own);
    assert_eq!(
        compiled::parse_build_script(&good, dep_info).unwrap(),
        ["a/build.rs"]
    );
    assert!(compiled::parse_build_script(&rules(other), dep_info).is_err());
    let good = good.as_str();
    for output in [
        "/t/b/x-1/build_script_build-2",
        "/t/b/x-1/build_script_build-other.txt",
        "/t/b/x-2/build_script_build-1",
        "/t/b/x-1/sub/build_script_build-1",
    ] {
        let bad = format!("{dep_info}: a/build.rs\n\n{output}: a/build.rs\n\na/build.rs:\n");
        assert!(
            compiled::parse_build_script(&bad, dep_info).is_err(),
            "{output}"
        );
    }
    assert!(compiled::parse_build_script(good, "/t/b/x-1/build_script_build-1").is_err());
    // A lib's rules stay as they were.
    assert!(compiled::parse(good).is_err());
}

#[test]
fn the_build_command_is_the_one_repeated() {
    let command = "clean cargo build -p x --release";
    let target = Some("target=t");
    let good = "#!/bin/sh\ntarget=t\n  clean cargo build -p x --release\n";
    assert!(check_command(good, command, target).is_ok());
    assert!(check_command(&good.replace('\n', "\r\n"), command, target).is_ok());
    for (bad, why) in [
        (
            "target=t\nclean cargo build -p x --release --features y\n",
            "a flag on the same line",
        ),
        (
            "target=t\n# clean cargo build -p x --release\nclean cargo build -p y --release\n",
            "a comment keeping the old command",
        ),
        (
            "target=t\nclean cargo build -p x --release\ncargo build -p z\n",
            "a second build",
        ),
        ("target=t\n", "no build"),
        (
            "target=u\nclean cargo build -p x --release\n",
            "another target",
        ),
        (
            "target=t\ntarget=u\nclean cargo build -p x --release\n",
            "a second target",
        ),
    ] {
        assert!(check_command(bad, command, target).is_err(), "{why}");
    }
}

#[test]
fn a_pinned_file_must_equal_its_copy_up_to_line_ends() {
    assert!(check_pin("f", "a\r\nb\r\n", "a\nb\n").is_ok());
    let error = check_pin("f", "a\nc\n", "a\nb\n").unwrap_err();
    assert!(error.contains("f differs from its pinned copy"), "{error}");
}

#[test]
fn the_cargo_configuration_is_the_reviewed_file_alone() {
    let index = |entries: &[(&str, &str)]| -> Index {
        entries
            .iter()
            .map(|(path, mode)| ((*path).to_owned(), (*mode).to_owned()))
            .collect()
    };
    let good = index(&[(".cargo/config.toml", "100644"), ("Cargo.toml", "100644")]);
    assert!(check_cargo_config(&good, CARGO_CONFIG_PIN).is_ok());
    assert!(check_cargo_config(&good, &CARGO_CONFIG_PIN.replace('\n', "\r\n")).is_ok());
    let changed = format!("{CARGO_CONFIG_PIN}include = [\"../docs/release.toml\"]\n");
    let error = check_cargo_config(&good, &changed).unwrap_err();
    assert!(error.contains("differs from its pinned copy"), "{error}");
    for bad in [
        index(&[
            (".cargo/config.toml", "100644"),
            (".cargo/config", "100644"),
        ]),
        index(&[
            (".cargo/config.toml", "100644"),
            (".CARGO/config", "100644"),
        ]),
        index(&[(".Cargo/config.toml", "100644")]),
        index(&[(".cargo/config.toml", "120000")]),
        index(&[("Cargo.toml", "100644")]),
    ] {
        let error = check_cargo_config(&bad, CARGO_CONFIG_PIN).unwrap_err();
        assert!(error.contains("admits exactly the regular file"), "{error}");
    }
}

/// The shipped binaries, the selections, the outside lists and the commands are plan 11's, and
/// each command is the pinned build the selection repeats.
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
    let windows: BTreeSet<&str> = bullet[from..to]
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
        assert_eq!(build.platform.target(), build.triple.unwrap());
        assert_eq!(build.platform.host_package(), build.shipped);
    }
    // Windows: the workflow's build line is the selection, with `--release --locked`.
    assert_eq!(WINDOWS.selection, ["--workspace"]);
    assert!(bullet.contains("`--workspace --release`"));
    assert_eq!(
        WINDOWS.command.1,
        format!(
            "run: cargo build {} --release --locked",
            WINDOWS.selection.join(" ")
        )
    );
    assert!(bullet.contains(&format!(
        "`{}`",
        WINDOWS.command.1.trim_start_matches("run: ")
    )));
    // Both pinned build lines and the macOS target line, verbatim.
    for line in [WINDOWS.command.1, MACOS.command.1, MACOS_TARGET_LINE] {
        assert!(bullet.contains(&format!("`{line}`")), "{line}");
    }
    // macOS: the script builds the one package for the target its target line names.
    let triple = MACOS.triple.unwrap();
    assert_eq!(MACOS.selection, ["-p", "stickymd-mac", "--target", triple]);
    assert!(bullet.contains(&format!("`-p stickymd-mac --release --target {triple}`")));
    assert_eq!(MACOS_TARGET_LINE, format!("target={triple}"));
    assert!(
        MACOS.command.1.starts_with(
            "clean cargo build -p stickymd-mac --release --locked --target \"$target\" "
        )
    );
    // The workflow equals its pin (the packaging boundary); the script's pin is checked here.
    check_command(
        &read_text(&root, WINDOWS.command.0).unwrap(),
        WINDOWS.command.1,
        None,
    )
    .unwrap();
    check_command(BUILD_APP_PIN, MACOS.command.1, Some(MACOS_TARGET_LINE)).unwrap();
    assert_eq!(MACOS.command.0, BUILD_APP);
}
