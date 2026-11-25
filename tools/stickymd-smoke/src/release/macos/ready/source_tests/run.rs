//! Runs the source tests on R on a Mac and writes the receipt (plan 11 #macos-readiness-check).
//! It starts only on a clean checkout of R (as committed, no index flag hiding a change) with no
//! obvious Cargo runner or wrapper in the environment (USER 2026-11-24, decision (d)); it then
//! takes the run lock, records its own identity and retires any old receipt (synced to disk)
//! before anything else can fail, so a run that fails after its start never leaves an earlier
//! PASS standing: the receipt the gate accepts names the run the record names (review round 4).
//! It also builds the `mac-gui` driver on R the documented way and records its SHA-256, so the
//! gate can tell R's driver from any other (review round 4, BLOCKER 1). Everything
//! after the start is recorded as it happened; the gate, not this runner, decides what the
//! receipt proves.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::super::identity;
use super::output::{self, Executable, Listed, Section};
use super::{
    DRIVER_TARGET, Driver, EXPLICIT, Exit, Explicit, Host, PACKAGES, RECEIPT, RUN, Receipt, Side,
    Target,
};
use super::{command, driver_command, explicit_command, explicit_target};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// More output than any test run here prints; beyond it the run is stopped and recorded.
const OUTPUT_LIMIT: usize = 64 << 20;

/// The environment entries that would put another program between Cargo and the tests or the
/// compiler (decision (d)); the rest of Cargo's configuration (its home and the parent
/// directories' files) is a precondition this runner does not prove.
fn runner_entries() -> Vec<String> {
    std::env::vars_os()
        .filter_map(|(key, _)| key.into_string().ok())
        .filter(|key| {
            (key.starts_with("CARGO_TARGET_") && key.ends_with("_RUNNER"))
                || matches!(
                    key.as_str(),
                    "CARGO_BUILD_RUSTC_WRAPPER"
                        | "RUSTC_WRAPPER"
                        | "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"
                        | "RUSTC_WORKSPACE_WRAPPER"
                )
        })
        .collect()
}

/// The trimmed stdout of a short query that must succeed.
fn query(root: &Path, program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(root)
        .stdin(Stdio::null())
        .output()
        .map_err(|error| format!("cannot run {program}: {error}"))?;
    if !output.status.success() {
        return Err(format!("{program} {arguments:?} failed"));
    }
    String::from_utf8(output.stdout)
        .map(|text| text.trim().to_owned())
        .map_err(|_| format!("{program} printed non-UTF-8"))
}

fn host(root: &Path) -> Result<Host, String> {
    let versions = query(root, "/usr/bin/sw_vers", &[])?;
    let field = |key: &str| -> Result<String, String> {
        versions
            .lines()
            .find_map(|line| line.strip_prefix(key))
            .map(|value| value.trim().to_owned())
            .ok_or_else(|| format!("sw_vers printed no {key}"))
    };
    Ok(Host {
        product_name: field("ProductName:")?,
        product_version: field("ProductVersion:")?,
        build_version: field("BuildVersion:")?,
        arch: query(root, "/usr/bin/uname", &["-m"])?,
        virtual_machine: query(root, "/usr/sbin/sysctl", &["-n", "kern.hv_vmm_present"])?,
        cargo: query(root, "cargo", &["-V"])?,
        rustc: query(root, "rustc", &["-V"])?,
    })
}

/// A finished command: how it ended, what it printed, and what could not be established.
struct Merged {
    exit: Exit,
    output: String,
    problem: Option<String>,
}

/// Runs `argv` in `root` with stdout and stderr on one pipe, echoing it to this process's
/// stderr. A command that cannot start ends `NotStarted`; one whose output cannot be read to its
/// end is stopped and ends `Unknown`, so the pipe cannot fill while this process waits; otherwise
/// it ends with its exit code or a signal (review round 4).
fn run_merged(root: &Path, argv: &[String], report: Option<&Path>) -> Merged {
    let not_started = |problem: String| Merged {
        exit: Exit::NotStarted,
        output: String::new(),
        problem: Some(problem),
    };
    let (mut reader, writer) = match std::io::pipe() {
        Ok(pipe) => pipe,
        Err(error) => return not_started(format!("cannot open a pipe: {error}")),
    };
    let shared = match writer.try_clone() {
        Ok(shared) => shared,
        Err(error) => return not_started(format!("cannot share the pipe: {error}")),
    };
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(root)
        // Colour codes would hide Cargo's headers; this changes only how output is drawn.
        .env("CARGO_TERM_COLOR", "never")
        .stdin(Stdio::null())
        .stdout(shared)
        .stderr(writer);
    if let Some(report) = report {
        command.env("STICKYMD_VOLUME_REPORT", report);
    }
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(error) => return not_started(format!("cannot start {argv:?}: {error}")),
    };
    // The command holds the pipe's write ends; drop them so the read ends with the child.
    drop(command);
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut echo = std::io::stderr();
    let read = loop {
        match reader.read(&mut buffer) {
            Ok(0) => break Ok(()),
            Ok(count) => {
                let _ = echo.write_all(&buffer[..count]);
                bytes.extend_from_slice(&buffer[..count]);
                if bytes.len() > OUTPUT_LIMIT {
                    break Err(format!("{argv:?} printed more than {OUTPUT_LIMIT} bytes"));
                }
            }
            Err(error) if error.kind() == ErrorKind::Interrupted => {}
            Err(error) => break Err(format!("cannot read {argv:?}: {error}")),
        }
    };
    let output = String::from_utf8_lossy(&bytes).into_owned();
    if let Err(error) = read {
        drop(reader);
        let killed = child.kill().err();
        let _ = child.wait();
        return Merged {
            exit: Exit::Unknown,
            output,
            problem: Some(match killed {
                Some(kill) => format!("{error}; it could not be stopped: {kill}"),
                None => error,
            }),
        };
    }
    match child.wait() {
        Ok(status) => Merged {
            exit: status.code().map_or(Exit::Signal, Exit::Code),
            output,
            problem: None,
        },
        Err(error) => Merged {
            exit: Exit::Unknown,
            output,
            problem: Some(format!("cannot wait for {argv:?}: {error}")),
        },
    }
}

/// The path of the `stickymd-smoke` binary a `cargo build --message-format=json` built, from its
/// messages: exactly one such artifact, and a successful build.
fn driver_path(stdout: &str) -> Result<std::path::PathBuf, String> {
    use crate::release::json::{self, Value};
    let mut found = Vec::new();
    let mut finished = false;
    for line in stdout.lines().filter(|line| !line.is_empty()) {
        let message = json::parse(line).map_err(|error| format!("Cargo message: {error}"))?;
        match message.field("reason")?.string()? {
            "build-finished" => finished = message.field("success")? == &Value::Bool(true),
            "compiler-artifact" => {
                let target = message.field("target")?;
                let bin = target
                    .field("kind")?
                    .array()?
                    .iter()
                    .any(|kind| kind.string() == Ok("bin"));
                if bin
                    && target.field("name")?.string()? == "stickymd-smoke"
                    && let Some(executable) = message.field("executable")?.optional_string()?
                {
                    found.push(std::path::PathBuf::from(executable));
                }
            }
            _ => {}
        }
    }
    if !finished {
        return Err("the driver build did not finish successfully".to_owned());
    }
    match <[_; 1]>::try_from(found) {
        Ok([path]) => Ok(path),
        Err(found) => Err(format!(
            "the driver build named {} stickymd-smoke binaries",
            found.len()
        )),
    }
}

/// Of the explicit run's `tests`, each name the binary's own listing holds, as often as it holds
/// it: a test registered twice is listed twice (fix round, BLOCKER 5).
fn listed_of(all: &[String], tests: &[&str]) -> Vec<String> {
    all.iter()
        .filter(|name| tests.contains(&name.as_str()))
        .cloned()
        .collect()
}

/// Empties the driver's own build directory: nothing a build of another commit left there can be
/// reused (fix round, BLOCKER 1).
fn empty_driver_target(root: &Path) -> Result<(), String> {
    let directory = root.join(DRIVER_TARGET);
    if super::refuse_link(&directory)? {
        std::fs::remove_dir_all(&directory)
            .map_err(|error| format!("cannot empty {}: {error}", directory.display()))?;
    }
    Ok(())
}

/// Builds the `mac-gui` driver on R as documented, into its own emptied directory, and hashes
/// what Cargo built there.
fn driver(root: &Path) -> Driver {
    let command = driver_command();
    let sha256 = (|| -> Result<String, String> {
        empty_driver_target(root)?;
        let (environment, argv) = command.split_first().ok_or("an empty driver command")?;
        let (key, value) = environment
            .split_once('=')
            .ok_or("the driver command names no environment")?;
        let output = Command::new(&argv[0])
            .args(&argv[1..])
            .env(key, value)
            .current_dir(root)
            .stdin(Stdio::null())
            .stderr(Stdio::inherit())
            .output()
            .map_err(|error| format!("cannot run the driver build: {error}"))?;
        if !output.status.success() {
            return Err("the driver build failed".to_owned());
        }
        let path = driver_path(
            std::str::from_utf8(&output.stdout).map_err(|_| "Cargo printed non-UTF-8")?,
        )?;
        crate::integrity::sha256(&path)
            .map(|sha256| sha256.to_ascii_lowercase())
            .map_err(|error| format!("cannot hash {}: {error}", path.display()))
    })();
    Driver { command, sha256 }
}

/// An identity no other run of this checkout shares: the time it started and this process.
fn run_identity() -> Result<String, String> {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| format!("the clock is before 1970: {error}"))?;
    Ok(format!("{}-{}", since.as_nanos(), std::process::id()))
}

/// What Cargo will run: the test executables of the build and the doc-tested libraries.
fn discover(root: &Path) -> Result<(Vec<Executable>, Vec<Target>), String> {
    let mut argv = command();
    argv.extend(["--no-run", "--message-format=json"].map(str::to_owned));
    let output = Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(root)
        .stdin(Stdio::null())
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("cannot run the discovery build: {error}"))?;
    if !output.status.success() {
        return Err("the discovery build failed".to_owned());
    }
    let executables = output::executables(
        std::str::from_utf8(&output.stdout).map_err(|_| "Cargo printed non-UTF-8")?,
    )?;
    let metadata = query(
        root,
        "cargo",
        &["metadata", "--no-deps", "--format-version", "1", "--locked"],
    )?;
    Ok((executables, output::doc_libraries(&metadata)?))
}

/// One listing: `argv` with `--list --format terse`, and again with `--ignored` added; a library's
/// doc-tests (`doc`) are read as rustdoc prints them.
fn list(
    root: &Path,
    program: &Path,
    argv: &[&str],
    doc: bool,
) -> Result<(Vec<String>, Vec<String>), String> {
    let once = |ignored: bool| -> Result<Vec<String>, String> {
        let mut arguments: Vec<&str> = argv.to_vec();
        arguments.extend(["--list", "--format", "terse"]);
        if ignored {
            arguments.push("--ignored");
        }
        let output = Command::new(program)
            .args(&arguments)
            .current_dir(root)
            .stdin(Stdio::null())
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
fn listings(root: &Path, executables: &[Executable], doc: &[Target]) -> (Vec<Listed>, Vec<String>) {
    gather(executables, doc, |program, argv, is_doc| {
        list(root, program, argv, is_doc)
    })
}

/// [`listings`] over any lister, `(program, argv, doc)` to `(all, ignored)`.
fn gather(
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

/// The volumes P18-L08's tests reported (`<volume> <case behaviour>` lines): exactly one line for
/// each of `temporary` and `image`, or what is wrong.
fn volumes(report: &str) -> Result<(String, String), String> {
    let mut temporary = None;
    let mut image = None;
    for line in report.lines() {
        let (volume, kind) = line
            .split_once(' ')
            .filter(|(_, kind)| matches!(*kind, "case-insensitive" | "case-sensitive"))
            .ok_or_else(|| format!("unreadable volume report line {line:?}"))?;
        let slot = match volume {
            "temporary" => &mut temporary,
            "image" => &mut image,
            _ => return Err(format!("unknown volume in {line:?}")),
        };
        if slot.replace(kind.to_owned()).is_some() {
            return Err(format!("the {volume} volume was reported twice"));
        }
    }
    Ok((
        temporary.ok_or("the temporary volume was not reported")?,
        image.ok_or("the image volume was not reported")?,
    ))
}

/// One explicit run: one section, of stickymd-mac's binary, with its printed tests named. With
/// `report`, the tests write the volumes they measured to that file.
fn explicit(
    root: &Path,
    item: &str,
    tests: &[&str],
    executables: &[Executable],
    listings: &[Listed],
    report: Option<&Path>,
    problems: &mut Vec<String>,
) -> Explicit {
    let argv = explicit_command(tests);
    let mut entry = Explicit {
        item: item.to_owned(),
        command: argv.clone(),
        tests: tests.iter().map(|test| (*test).to_owned()).collect(),
        target: None,
        counts: None,
        cases: Vec::new(),
        // The binary's own listing, taken before any run (review round 4).
        listed: listings
            .iter()
            .find(|l| l.target == explicit_target())
            .map(|l| listed_of(&l.all, tests))
            .unwrap_or_default(),
        exit: Exit::NotStarted,
        temporary_volume: None,
        image_volume: None,
    };
    let merged = run_merged(root, &argv, report);
    entry.exit = merged.exit;
    if let Some(problem) = merged.problem {
        problems.push(format!("the {item} run: {problem}"));
    }
    if merged.exit == Exit::NotStarted {
        return entry;
    }
    // What was read before any break is kept: a printed failure counts (fix round, BLOCKER 3).
    let parsed = output::parse(&merged.output);
    if let Some(error) = &parsed.error {
        problems.push(format!("the {item} output: {error}"));
    }
    let sections: Vec<Section> = parsed.sections;
    let [only] = sections.as_slice() else {
        problems.push(format!(
            "the {item} run printed {} test results",
            sections.len()
        ));
        return entry;
    };
    entry.target = output::target_of(only, executables, &[]).cloned();
    entry.counts = only.counts;
    let listed = listings.iter().find(|l| l.target == explicit_target());
    let printed_names: Vec<String> = only.tests.iter().map(|(name, _)| name.clone()).collect();
    let (cases, _) = output::cases(
        &printed_names,
        listed.map_or(&[][..], |l| l.ignored.as_slice()),
        &only.tests,
    );
    entry.cases = cases;
    if let Some(report) = report {
        match std::fs::read_to_string(report)
            .map_err(|error| format!("cannot read the volume report: {error}"))
            .and_then(|text| volumes(&text))
        {
            Ok((temporary, image)) => {
                entry.temporary_volume = Some(temporary);
                entry.image_volume = Some(image);
            }
            Err(error) => problems.push(format!("the {item} run: {error}")),
        }
    }
    entry
}

fn side(identity: &identity::Identity) -> Side {
    Side {
        head: identity.head.clone(),
        clean: identity.clean,
    }
}

/// Whether the file at the receipt's path, if any, is a source-test receipt of some schema: an
/// unrelated file put there by mistake is kept and reported, never removed (review round 4).
fn recognize(path: &Path) -> Result<(), String> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    let receipt = crate::release::json::parse(&text).is_ok_and(|value| {
        value.field("kind").and_then(|kind| {
            kind.string()
                .map(|kind| kind == "macos-release-source-tests")
        }) == Ok(true)
    });
    if receipt {
        Ok(())
    } else {
        Err(format!(
            "{} is not a source-test receipt; move it away and run again",
            path.display()
        ))
    }
}

/// Removes the old receipt and syncs its directory, so the removal survives a crash that follows.
fn retire(path: &Path) -> Result<(), String> {
    match std::fs::remove_file(path) {
        Ok(()) => eprintln!("removed the previous receipt; it stands for no run from now on"),
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(format!("cannot remove {}: {error}", path.display())),
    }
    if let Some(parent) = path.parent() {
        std::fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| format!("cannot sync {}: {error}", parent.display()))?;
    }
    Ok(())
}

/// The whole run; returns the receipt it wrote.
pub(in super::super) fn run(root: &Path, release: &str) -> Result<Receipt, String> {
    crate::integrity::validate_hex(release, 40, "release commit")?;
    let release = release.to_ascii_lowercase();
    let start = identity::require(root, &release, "macos-source-tests")?;
    let entries = runner_entries();
    if !entries.is_empty() {
        return Err(format!(
            "the environment sets {entries:?}, which would run the tests or the compiler through another program; unset it"
        ));
    }
    let _lock = super::lock_for_run(root)?.ok_or(
        "another source-test run or a readiness check holds the lock; wait for it to finish",
    )?;
    let path = root.join(RECEIPT);
    super::refuse_link(&path)?;
    recognize(&path)?;
    // This run's identity, before the old receipt goes: from here on only a receipt naming it
    // counts, so an interrupted run cannot leave an older one standing (review round 4).
    let run = run_identity()?;
    let marker = root.join(RUN);
    super::refuse_link(&marker)?;
    crate::atomic_evidence::write(&marker, format!("{run}\n").as_bytes())?;
    retire(&path)?;
    // No probe of this process's own temporary directory: P18-L08's tests measure the volumes
    // where their assertions run, and the receipt records that (review round 3, B4).
    let host = host(root)?;
    let mut problems = Vec::new();
    let (executables, doc) = discover(root).unwrap_or_else(|error| {
        problems.push(error);
        (Vec::new(), Vec::new())
    });
    let (listed, unlisted) = listings(root, &executables, &doc);
    problems.extend(unlisted);
    let argv = command();
    let merged = run_merged(root, &argv, None);
    problems.extend(merged.problem);
    // What was read before any break is kept, attributed to listed names, so a printed failure
    // survives output cut short; only a whole output can pass (fix round, BLOCKER 3).
    let binaries = if merged.exit == Exit::NotStarted {
        Vec::new()
    } else {
        let parsed = output::parse(&merged.output);
        if let Some(error) = parsed.error {
            problems.push(format!("the test output: {error}"));
        }
        let (runs, more) = output::attribute(&parsed.sections, &executables, &doc, &listed);
        problems.extend(more);
        runs
    };
    // P18-L08's tests report the volumes they measured into a file of this run's own directory;
    // without one the explicit run records no volumes, and the results already taken are still
    // published (review round 4).
    let reports = crate::release::temporary::TemporaryDirectory::new("volume-report")
        .map_err(|error| problems.push(format!("the volume report directory: {error}")))
        .ok();
    let explicit = EXPLICIT
        .iter()
        .map(|(item, tests)| {
            let report = reports
                .as_ref()
                .filter(|_| *item == "same-key")
                .map(|reports| reports.path().join("volumes.txt"));
            explicit(
                root,
                item,
                tests,
                &executables,
                &listed,
                report.as_deref(),
                &mut problems,
            )
        })
        .collect();
    if let Some(Err(error)) = reports.map(|reports| reports.close()) {
        problems.push(error);
    }
    let driver = driver(root);
    let end = identity::capture(root).unwrap_or_else(|error| {
        // Kept where the operator reads it (fix round, SHOULD_FIX 8).
        problems.push(format!("the end identity could not be read: {error}"));
        identity::Identity {
            head: "UNKNOWN".to_owned(),
            clean: false,
            reason: Some(error),
        }
    });
    let receipt = Receipt {
        release_source: release,
        run,
        host,
        start: side(&start),
        end: side(&end),
        command: argv,
        exit: merged.exit,
        expected: executables
            .into_iter()
            .map(|executable| executable.target)
            .chain(doc)
            .collect(),
        binaries,
        explicit,
        driver,
        problems,
    };
    crate::atomic_evidence::write(&path, super::render(&receipt).as_bytes())?;
    eprintln!(
        "wrote {} ({} packages selected)",
        path.display(),
        PACKAGES.len()
    );
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    /// Review round 4: only a file of the receipt's own kind is retired; anything else at its
    /// path is kept and reported.
    #[test]
    fn only_a_receipt_is_retired_from_the_receipt_path() {
        let scratch = crate::release::temporary::TemporaryDirectory::new("retire").unwrap();
        let path = scratch.path().join("receipt.json");
        assert!(super::recognize(&path).is_ok());
        std::fs::write(
            &path,
            "{\"kind\":\"macos-release-source-tests\",\"schema_version\":1}",
        )
        .unwrap();
        assert!(super::recognize(&path).is_ok());
        // A single valid JSON of another kind alone is refused too (fix round, SHOULD_FIX 10).
        for other in [
            "",
            "{\"kind\":\"automated\"}\n{\"kind\":\"automated\"}\n",
            "{}",
            "{\"kind\":\"macos-release-source-test\"}",
        ] {
            std::fs::write(&path, other).unwrap();
            assert!(
                super::recognize(&path)
                    .unwrap_err()
                    .contains("not a source-test receipt")
            );
        }
    }

    /// Review round 4, BLOCKER 1: the driver is the one `stickymd-smoke` binary of a build that
    /// finished.
    #[test]
    fn the_driver_is_the_one_binary_a_finished_build_names() {
        let artifact = |name: &str, kind: &str| {
            format!(
                r#"{{"reason":"compiler-artifact","target":{{"kind":["{kind}"],"name":"{name}"}},"executable":"/r/target/release/{name}"}}"#
            )
        };
        let finished = r#"{"reason":"build-finished","success":true}"#;
        let built = format!(
            "{}\n{}\n{finished}\n",
            artifact("stickymd-package", "lib"),
            artifact("stickymd-smoke", "bin")
        );
        assert_eq!(
            super::driver_path(&built).unwrap(),
            std::path::PathBuf::from("/r/target/release/stickymd-smoke")
        );
        let failed = built.replace("\"success\":true", "\"success\":false");
        assert!(super::driver_path(&failed).is_err());
        let none = format!("{}\n{finished}\n", artifact("stickymd-package", "lib"));
        assert!(
            super::driver_path(&none)
                .unwrap_err()
                .contains("0 stickymd-smoke")
        );
        // Fix round, SHOULD_FIX 10: the name alone, and the kind alone, each decide.
        let lib = format!("{}\n{finished}\n", artifact("stickymd-smoke", "lib"));
        assert!(super::driver_path(&lib).is_err());
        let other = format!("{}\n{finished}\n", artifact("stickymd-mac", "bin"));
        assert!(super::driver_path(&other).is_err());
    }

    /// Pre-freeze item 4 (fix-round review 15): one target whose listing fails is a problem of
    /// that target alone; the other targets keep their listings, so their runs are still judged.
    #[test]
    fn a_failed_listing_drops_only_its_own_target() {
        use super::super::Target;
        use super::output::Executable;
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

    /// Fix round, BLOCKER 5: the listing keeps a name as often as the binary lists it.
    #[test]
    fn the_listing_keeps_a_doubled_test_twice() {
        let all = ["a::b", "a::b", "a::c"].map(str::to_owned);
        assert_eq!(super::listed_of(&all, &["a::b"]), ["a::b", "a::b"]);
    }

    /// Fix round, BLOCKER 1: the driver's directory starts empty, whatever a build left there.
    #[test]
    fn the_driver_directory_is_emptied_before_the_build() {
        let scratch = crate::release::temporary::TemporaryDirectory::new("driver-target").unwrap();
        let root = scratch.path();
        let stale = root
            .join(super::DRIVER_TARGET)
            .join("release/stickymd-smoke");
        std::fs::create_dir_all(stale.parent().unwrap()).unwrap();
        std::fs::write(&stale, "a build of another commit").unwrap();
        super::empty_driver_target(root).unwrap();
        assert!(!root.join(super::DRIVER_TARGET).exists());
        super::empty_driver_target(root).unwrap();
    }

    /// Review round 3, B4: the volume report holds exactly one line for each volume.
    #[test]
    fn the_volume_report_names_each_volume_once() {
        assert_eq!(
            super::volumes("temporary case-insensitive\nimage case-sensitive\n").unwrap(),
            ("case-insensitive".to_owned(), "case-sensitive".to_owned())
        );
        assert!(
            super::volumes("temporary case-insensitive\n")
                .unwrap_err()
                .contains("image")
        );
        assert!(
            super::volumes(
                "temporary case-insensitive\ntemporary case-sensitive\nimage case-sensitive\n"
            )
            .unwrap_err()
            .contains("twice")
        );
        assert!(super::volumes("other case-sensitive\n").is_err());
        assert!(super::volumes("image maybe\n").is_err());
    }
}
