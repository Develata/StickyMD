//! Runs the source tests on R on a Mac and writes the receipt (plan 11 #macos-readiness-check).
//! It starts only on a clean checkout of R (as committed, no index flag hiding a change) with no
//! obvious Cargo runner or wrapper in the environment (USER 2026-11-24, decision (d)); it then
//! takes the run lock and retires any old receipt (synced to disk) before anything else can
//! fail, so a run that fails after its start never leaves an earlier PASS standing. Everything
//! after the start is recorded as it happened; the gate, not this runner, decides what the
//! receipt proves.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::super::identity;
use super::output::{self, Executable, Listed, Section};
use super::{EXPLICIT, Explicit, Host, PACKAGES, RECEIPT, Receipt, Side, Target};
use super::{command, explicit_command, explicit_target};
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

/// Runs `argv` in `root` with stdout and stderr on one pipe, echoing it to this process's
/// stderr; returns the exit code (`None` for a signal) and the output. A read that fails for any
/// reason but an interruption ends the child before waiting for it, so the pipe cannot fill
/// while this process waits.
fn run_merged(
    root: &Path,
    argv: &[String],
    report: Option<&Path>,
) -> Result<(Option<i32>, String), String> {
    let (mut reader, writer) =
        std::io::pipe().map_err(|error| format!("cannot open a pipe: {error}"))?;
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(root)
        // Colour codes would hide Cargo's headers; this changes only how output is drawn.
        .env("CARGO_TERM_COLOR", "never")
        .stdin(Stdio::null())
        .stdout(
            writer
                .try_clone()
                .map_err(|error| format!("cannot share the pipe: {error}"))?,
        )
        .stderr(writer);
    if let Some(report) = report {
        command.env("STICKYMD_VOLUME_REPORT", report);
    }
    let mut child = command
        .spawn()
        .map_err(|error| format!("cannot start {argv:?}: {error}"))?;
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
    if let Err(error) = read {
        drop(reader);
        let killed = child.kill().err();
        let _ = child.wait();
        return Err(match killed {
            Some(kill) => format!("{error}; it could not be stopped: {kill}"),
            None => error,
        });
    }
    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for {argv:?}: {error}"))?;
    Ok((status.code(), String::from_utf8_lossy(&bytes).into_owned()))
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

/// One listing: `argv` with `--list --format terse`, and again with `--ignored` added.
fn list(root: &Path, program: &Path, argv: &[&str]) -> Result<(Vec<String>, Vec<String>), String> {
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
        output::listing(std::str::from_utf8(&output.stdout).map_err(|_| "a listing is not UTF-8")?)
    };
    Ok((once(false)?, once(true)?))
}

/// Every target's own listing of its tests: each test executable's `--list`, and each
/// doc-tested library's `cargo test --doc -- --list`.
fn listings(
    root: &Path,
    executables: &[Executable],
    doc: &[Target],
) -> Result<Vec<Listed>, String> {
    let mut out = Vec::new();
    for executable in executables {
        let (all, ignored) = list(root, &executable.path, &[])?;
        out.push(Listed {
            target: executable.target.clone(),
            all,
            ignored,
        });
    }
    for library in doc {
        let (all, ignored) = list(
            root,
            Path::new("cargo"),
            &["test", "--locked", "-p", &library.package, "--doc", "--"],
        )?;
        out.push(Listed {
            target: library.clone(),
            all,
            ignored,
        });
    }
    Ok(out)
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
        exit_code: None,
        temporary_volume: None,
        image_volume: None,
    };
    let (exit, text) = match run_merged(root, &argv, report) {
        Ok(found) => found,
        Err(error) => {
            problems.push(format!("the {item} run: {error}"));
            return entry;
        }
    };
    entry.exit_code = exit;
    let sections: Vec<Section> = match output::sections(&text) {
        Ok(sections) => sections,
        Err(error) => {
            problems.push(format!("the {item} output: {error}"));
            return entry;
        }
    };
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
    retire(&path)?;
    // No probe of this process's own temporary directory: P18-L08's tests measure the volumes
    // where their assertions run, and the receipt records that (review round 3, B4).
    let host = host(root)?;
    let mut problems = Vec::new();
    let (executables, doc) = discover(root).unwrap_or_else(|error| {
        problems.push(error);
        (Vec::new(), Vec::new())
    });
    let listed = listings(root, &executables, &doc).unwrap_or_else(|error| {
        problems.push(error);
        Vec::new()
    });
    let argv = command();
    let (exit_code, binaries) = match run_merged(root, &argv, None) {
        Ok((exit, text)) => match output::sections(&text) {
            Ok(sections) => {
                let (runs, more) = output::attribute(&sections, &executables, &doc, &listed);
                problems.extend(more);
                (exit, runs)
            }
            Err(error) => {
                problems.push(format!("the test output: {error}"));
                (exit, Vec::new())
            }
        },
        Err(error) => {
            problems.push(error);
            (None, Vec::new())
        }
    };
    // P18-L08's tests report the volumes they measured into a file of this run's own directory.
    let reports = crate::release::temporary::TemporaryDirectory::new("volume-report")?;
    let explicit = EXPLICIT
        .iter()
        .map(|(item, tests)| {
            let report = (*item == "same-key").then(|| reports.path().join("volumes.txt"));
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
    if let Err(error) = reports.close() {
        problems.push(error);
    }
    let end = identity::capture(root).unwrap_or(identity::Identity {
        head: "UNKNOWN".to_owned(),
        clean: false,
        reason: Some("the end identity could not be read".to_owned()),
    });
    let receipt = Receipt {
        release_source: release,
        host,
        start: side(&start),
        end: side(&end),
        command: argv,
        exit_code,
        expected: executables
            .into_iter()
            .map(|executable| executable.target)
            .chain(doc)
            .collect(),
        binaries,
        explicit,
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
