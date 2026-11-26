//! The Cargo processes of the runner's steps 3–5 (plan 11 #macos-readiness-check): target
//! discovery, each target's two listings, the main run and the explicit runs. Every one of them is
//! built here, and the runner gets its builder only from [`Steps::fresh`], which first empties
//! `target/macos-source-tests`: Cargo judges an artifact fresh by modification times, so a newer
//! test program another source left in a shared build directory could stand in for R's; in the
//! emptied one, all of them are built from R in this run (whole-gate review A2). The `phase 18`
//! check of the ignored-test table finds and lists the targets with the same commands in Cargo's
//! default build directory, which it never empties.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::output::{self, Executable, Listed};
use super::{Target, command, explicit_command};
use std::ffi::OsStr;
use std::path::Path;
use std::process::{Command, Stdio};

/// Where steps 3–5 build, relative to the checkout's root: emptied before step 3.
pub(super) const TEST_TARGET: &str = "target/macos-source-tests";

/// Where they keep their intermediate artifacts: inside [`TEST_TARGET`], so emptying it leaves
/// none behind even when the shell sets a shared `CARGO_BUILD_BUILD_DIR` (fix-round review R1-1).
const TEST_BUILD_DIR: &str = "target/macos-source-tests/build";

/// The builder of the Cargo processes of steps 3–5 in one checkout.
pub(super) struct Steps<'a> {
    root: &'a Path,
    /// `CARGO_TARGET_DIR` for every process (with `CARGO_BUILD_BUILD_DIR` inside it), or `None`
    /// for Cargo's default directories.
    target: Option<&'static str>,
}

impl<'a> Steps<'a> {
    /// The runner's: [`TEST_TARGET`], emptied first, so nothing built before this run is reused.
    /// A link there, or the directory this program runs from, is refused, not emptied.
    pub(super) fn fresh(root: &'a Path) -> Result<Self, String> {
        let running = std::env::current_exe()
            .map_err(|error| format!("cannot name the running program: {error}"))?;
        empty_build_directory(root, TEST_TARGET, Some(&running))?;
        Ok(Self {
            root,
            target: Some(TEST_TARGET),
        })
    }

    /// The `phase 18` check's: Cargo's default build directory, kept as it is. It judges which
    /// tests are ignored, not what passed, so its builds need not be fresh; never the runner's.
    pub(super) fn in_default_directory(root: &'a Path) -> Self {
        Self { root, target: None }
    }

    /// `program` in the checkout, with no input, colourless Cargo output (colour codes would hide
    /// Cargo's headers) and the build directory of these steps.
    fn process(&self, program: impl AsRef<OsStr>) -> Command {
        let mut process = Command::new(program);
        process
            .current_dir(self.root)
            .env("CARGO_TERM_COLOR", "never")
            .stdin(Stdio::null());
        if let Some(target) = self.target {
            process
                .env("CARGO_TARGET_DIR", target)
                .env("CARGO_BUILD_BUILD_DIR", TEST_BUILD_DIR);
        }
        process
    }

    /// Step 3: the prescribed command, building only, with Cargo's JSON messages.
    pub(super) fn discovery(&self) -> Command {
        let argv = command();
        let mut process = self.process(&argv[0]);
        process
            .args(&argv[1..])
            .args(["--no-run", "--message-format=json"]);
        process
    }

    /// Step 3: the packages' targets as Cargo describes them (it builds nothing).
    pub(super) fn metadata(&self) -> Command {
        let mut process = self.process("cargo");
        process.args(["metadata", "--no-deps", "--format-version", "1", "--locked"]);
        process
    }

    /// Step 3: one target's listing, `--ignored` or not: a test executable with no arguments, or
    /// `cargo test --locked -p <package> --doc --` for a library's doc-tests.
    pub(super) fn listing(&self, program: &Path, argv: &[&str], ignored: bool) -> Command {
        let mut process = self.process(program);
        process.args(argv).args(["--list", "--format", "terse"]);
        if ignored {
            process.arg("--ignored");
        }
        process
    }

    /// Step 4: the prescribed command, its text unchanged.
    pub(super) fn main_run(&self) -> Command {
        let argv = command();
        let mut process = self.process(&argv[0]);
        process.args(&argv[1..]);
        process
    }

    /// Step 5: the explicit run of `tests`, its text unchanged.
    pub(super) fn explicit_run(&self, tests: &[&str]) -> Command {
        let argv = explicit_command(tests);
        let mut process = self.process(&argv[0]);
        process.args(&argv[1..]);
        process
    }
}

/// Removes `relative` under `root` so a build there starts from nothing; absence is fine. A link
/// there or at any level between it and `root` is refused (it would empty what it points to; a
/// file in the way is an error too), and so is the directory that holds `running`, the program
/// doing this.
pub(super) fn empty_build_directory(
    root: &Path,
    relative: &str,
    running: Option<&Path>,
) -> Result<(), String> {
    // Every level below the root, not only the last, must be a real directory: a link at
    // `target` would lead the removal into whatever it points to (fix-round review R1-7).
    let mut directory = root.to_path_buf();
    for component in Path::new(relative).components() {
        directory.push(component);
        if !super::refuse_link(&directory)? {
            return Ok(());
        }
    }
    if let Some(running) = running
        && holds(&directory, running)?
    {
        return Err(format!(
            "{} holds this program ({}); run it from another build directory",
            directory.display(),
            running.display()
        ));
    }
    std::fs::remove_dir_all(&directory)
        .map_err(|error| format!("cannot empty {}: {error}", directory.display()))
}

/// Whether `file` lies inside `directory`, both resolved.
fn holds(directory: &Path, file: &Path) -> Result<bool, String> {
    let resolve = |path: &Path| {
        std::fs::canonicalize(path)
            .map_err(|error| format!("cannot resolve {}: {error}", path.display()))
    };
    Ok(resolve(file)?.starts_with(resolve(directory)?))
}

/// The stdout of a process of these steps that must succeed.
fn stdout_of(mut process: Command, what: &str) -> Result<String, String> {
    let output = process
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("cannot run {what}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{what} failed"));
    }
    String::from_utf8(output.stdout).map_err(|_| format!("{what} printed non-UTF-8"))
}

/// What Cargo will run: the test executables of the build and the doc-tested libraries.
pub(super) fn discover(steps: &Steps) -> Result<(Vec<Executable>, Vec<Target>), String> {
    let executables = output::executables(&stdout_of(steps.discovery(), "the discovery build")?)?;
    let metadata = stdout_of(steps.metadata(), "cargo metadata")?;
    Ok((executables, output::doc_libraries(&metadata)?))
}

/// One listing: `argv` with `--list --format terse`, and again with `--ignored` added; a library's
/// doc-tests (`doc`) are read as rustdoc prints them.
fn list(
    steps: &Steps,
    program: &Path,
    argv: &[&str],
    doc: bool,
) -> Result<(Vec<String>, Vec<String>), String> {
    let once = |ignored: bool| -> Result<Vec<String>, String> {
        let output = steps
            .listing(program, argv, ignored)
            .stderr(Stdio::null())
            .output()
            .map_err(|error| format!("cannot list {}: {error}", program.display()))?;
        if !output.status.success() {
            return Err(format!("listing {} failed", program.display()));
        }
        let text = std::str::from_utf8(&output.stdout).map_err(|_| "a listing is not UTF-8")?;
        if doc {
            output::doc_listing(text)
        } else {
            output::listing(text)
        }
    };
    Ok((once(false)?, once(true)?))
}

/// Every target's own listing of its tests: each test executable's `--list`, and each
/// doc-tested library's `cargo test --doc -- --list`. A target whose listing fails is a problem
/// of that target alone; the others keep theirs (pre-freeze item 4).
pub(super) fn listings(
    steps: &Steps,
    executables: &[Executable],
    doc: &[Target],
) -> (Vec<Listed>, Vec<String>) {
    gather(executables, doc, |program, argv, is_doc| {
        list(steps, program, argv, is_doc)
    })
}

/// [`listings`] over any lister, `(program, argv, doc)` to `(all, ignored)`.
pub(super) fn gather(
    executables: &[Executable],
    doc: &[Target],
    list: impl Fn(&Path, &[&str], bool) -> Result<(Vec<String>, Vec<String>), String>,
) -> (Vec<Listed>, Vec<String>) {
    let mut out = Vec::new();
    let mut problems = Vec::new();
    let mut take =
        |target: &Target, listed: Result<(Vec<String>, Vec<String>), String>| match listed {
            Ok((all, ignored)) => out.push(Listed {
                target: target.clone(),
                all,
                ignored,
            }),
            Err(error) => problems.push(format!("the listing of {target:?}: {error}")),
        };
    for executable in executables {
        take(&executable.target, list(&executable.path, &[], false));
    }
    for library in doc {
        let argv = ["test", "--locked", "-p", &library.package, "--doc", "--"];
        take(library, list(Path::new("cargo"), &argv, true));
    }
    (out, problems)
}

#[cfg(test)]
mod tests {
    use super::{Steps, TEST_TARGET};
    use std::ffi::OsStr;
    use std::path::Path;
    use std::process::Command;

    fn environment<'c>(process: &'c Command, key: &str) -> Option<Option<&'c OsStr>> {
        process
            .get_envs()
            .find(|(name, _)| *name == OsStr::new(key))
            .map(|(_, value)| value)
    }

    fn argv(process: &Command) -> Vec<String> {
        std::iter::once(process.get_program())
            .chain(process.get_args())
            .map(|part| part.to_string_lossy().into_owned())
            .collect()
    }

    /// Whole-gate review A2: the runner's builder exists only once the directory is emptied, and
    /// every process of steps 3–5 it builds (discovery, metadata, both listings of a binary and
    /// of a library's doc-tests, the main run and an explicit run) builds there, in the checkout,
    /// with the prescribed command text unchanged.
    #[test]
    fn every_process_of_steps_3_to_5_builds_in_the_emptied_directory() {
        let scratch = crate::release::temporary::TemporaryDirectory::new("test-target").unwrap();
        let root = scratch.path();
        let stale = root.join(TEST_TARGET).join("debug/deps/stickymd_core-0");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(&stale, "a test program another source built").unwrap();
        let steps = Steps::fresh(root).unwrap();
        assert!(!root.join(TEST_TARGET).exists());
        let doc = ["test", "--locked", "-p", "stickymd-core", "--doc", "--"];
        let processes = [
            steps.discovery(),
            steps.metadata(),
            steps.listing(Path::new("/r/target/debug/deps/x-1"), &[], false),
            steps.listing(Path::new("/r/target/debug/deps/x-1"), &[], true),
            steps.listing(Path::new("cargo"), &doc, false),
            steps.listing(Path::new("cargo"), &doc, true),
            steps.main_run(),
            steps.explicit_run(&super::super::S06_TESTS),
        ];
        for process in &processes {
            assert_eq!(
                environment(process, "CARGO_TARGET_DIR"),
                Some(Some(OsStr::new("target/macos-source-tests"))),
                "{:?}",
                argv(process)
            );
            // Fix-round review R1-1: the intermediate artifacts too, whatever the shell sets.
            assert_eq!(
                environment(process, "CARGO_BUILD_BUILD_DIR"),
                Some(Some(OsStr::new("target/macos-source-tests/build"))),
                "{:?}",
                argv(process)
            );
            assert_eq!(process.get_current_dir(), Some(root));
        }
        assert_eq!(argv(&processes[6]), super::command());
        assert_eq!(
            argv(&processes[7]),
            super::explicit_command(&super::super::S06_TESTS)
        );
        assert_eq!(
            argv(&processes[0])[..super::command().len()],
            super::command()[..]
        );
        // The phase 18 check keeps Cargo's default directory.
        let default = Steps::in_default_directory(root);
        assert_eq!(environment(&default.main_run(), "CARGO_TARGET_DIR"), None);
        assert_eq!(
            environment(&default.main_run(), "CARGO_BUILD_BUILD_DIR"),
            None
        );
    }

    /// Whole-gate review A2: a link at the directory, and a directory holding the running
    /// program, are refused and left as they are.
    #[test]
    fn a_linked_build_directory_or_the_running_one_is_not_emptied() {
        let scratch = crate::release::temporary::TemporaryDirectory::new("test-target").unwrap();
        let root = scratch.path();
        let elsewhere = root.join("elsewhere");
        std::fs::create_dir_all(&elsewhere).unwrap();
        std::fs::write(elsewhere.join("kept"), "").unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();
        std::os::unix::fs::symlink(&elsewhere, root.join(TEST_TARGET)).unwrap();
        let refused = super::empty_build_directory(root, TEST_TARGET, None).unwrap_err();
        assert!(refused.contains("symbolic link"), "{refused}");
        assert!(elsewhere.join("kept").exists());
        std::fs::remove_file(root.join(TEST_TARGET)).unwrap();

        // Fix-round review R1-7: a link above it (here `target` itself, pointing at a shared
        // cache) is refused too, and the directory it leads to is kept.
        std::fs::remove_dir(root.join("target")).unwrap();
        let shared = root.join("shared-cache");
        std::fs::create_dir_all(shared.join("macos-source-tests")).unwrap();
        std::fs::write(shared.join("macos-source-tests/kept"), "").unwrap();
        std::os::unix::fs::symlink(&shared, root.join("target")).unwrap();
        let refused = super::empty_build_directory(root, TEST_TARGET, None).unwrap_err();
        assert!(refused.contains("symbolic link"), "{refused}");
        assert!(shared.join("macos-source-tests/kept").exists());
        std::fs::remove_file(root.join("target")).unwrap();
        // A file where a directory belongs is refused, not removed.
        std::fs::write(root.join("target"), "").unwrap();
        assert!(super::empty_build_directory(root, TEST_TARGET, None).is_err());
        assert!(root.join("target").is_file());
        std::fs::remove_file(root.join("target")).unwrap();
        // Nothing there yet is nothing to empty.
        super::empty_build_directory(root, TEST_TARGET, None).unwrap();
        std::fs::create_dir_all(root.join("target")).unwrap();

        let running = root.join(TEST_TARGET).join("release/stickymd-smoke");
        std::fs::create_dir_all(running.parent().unwrap()).unwrap();
        std::fs::write(&running, "").unwrap();
        let refused = super::empty_build_directory(root, TEST_TARGET, Some(&running)).unwrap_err();
        assert!(refused.contains("holds this program"), "{refused}");
        assert!(running.exists());
        let other = root.join("target/release/stickymd-smoke");
        std::fs::create_dir_all(other.parent().unwrap()).unwrap();
        std::fs::write(&other, "").unwrap();
        super::empty_build_directory(root, TEST_TARGET, Some(&other)).unwrap();
        assert!(!root.join(TEST_TARGET).exists());
    }

    /// Pre-freeze item 4 (fix-round review 15): one target whose listing fails is a problem of
    /// that target alone; the other targets keep their listings, so their runs are still judged.
    #[test]
    fn a_failed_listing_drops_only_its_own_target() {
        use super::super::Target;
        use super::super::output::Executable;
        let target = |name: &str, kind: &str| Target {
            package: "p".to_owned(),
            kind: kind.to_owned(),
            name: name.to_owned(),
        };
        let executables = [
            Executable {
                target: target("good", "bin"),
                path: "target/debug/deps/good-1".into(),
            },
            Executable {
                target: target("bad", "bin"),
                path: "target/debug/deps/bad-1".into(),
            },
        ];
        let doc = [target("p", "doctest")];
        let (listed, problems) = super::gather(&executables, &doc, |program, _, is_doc| {
            if program.ends_with("bad-1") {
                Err("listing bad-1 failed".to_owned())
            } else {
                Ok((
                    vec![format!("{}::t", if is_doc { "doc" } else { "bin" })],
                    Vec::new(),
                ))
            }
        });
        let names: Vec<_> = listed.iter().map(|l| l.target.name.as_str()).collect();
        assert_eq!(names, ["good", "p"]);
        assert_eq!(listed[1].all, ["doc::t"]);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("bad") && problems[0].contains("listing bad-1 failed"));
    }
}
