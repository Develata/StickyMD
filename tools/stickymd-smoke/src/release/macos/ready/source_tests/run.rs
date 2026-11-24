//! Runs the source tests on R on a Mac and writes the receipt (plan 11 #macos-readiness-check).
//! It starts only on a clean checkout of R, takes the run lock and removes any old receipt before
//! anything else can fail, so an interrupted, refused or failed run never leaves an earlier PASS
//! standing. Everything after the start is recorded as it happened; the gate, not this runner,
//! decides what the receipt proves.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::output::{self, Executable};
use super::{Host, Receipt, SameKey, Side, Target};
use super::{PACKAGES, RECEIPT, SAME_KEY_TESTS, command, same_key_command};
use crate::evidence::SourceIdentity;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};

/// More output than any test run here prints; beyond it the run is stopped and recorded.
const OUTPUT_LIMIT: usize = 64 << 20;

/// Whether the Git index hides a change from `git status`: an entry marked assume-unchanged
/// (a lowercase tag in `git ls-files -v`) or skip-worktree (`S`) can differ from the tree.
fn hidden_changes(root: &Path) -> Result<bool, String> {
    let listing = query(root, "git", &["ls-files", "-v"])?;
    Ok(listing.lines().any(|line| {
        line.chars()
            .next()
            .is_some_and(|tag| tag.is_ascii_lowercase() || tag == 'S')
    }))
}

fn side(root: &Path, identity: &SourceIdentity) -> Side {
    Side {
        head: identity.commit.to_ascii_lowercase(),
        clean: !identity.worktree_dirty && matches!(hidden_changes(root), Ok(false)),
    }
}

/// P18-L08's default-set test checks a name differing only in case on the temporary volume and
/// skips that check on a case-sensitive one; the release proof needs it run, so the temporary
/// volume must be case-insensitive (the case-sensitive side has its own image test).
fn temporary_volume_is_case_insensitive() -> Result<bool, String> {
    let directory =
        std::env::temp_dir().join(format!("stickymd-case-probe-{}", std::process::id()));
    std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let probe = std::fs::write(directory.join("case-probe"), b"")
        .map(|()| directory.join("CASE-PROBE").exists())
        .map_err(|error| error.to_string());
    let _ = std::fs::remove_dir_all(&directory);
    probe
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
/// stderr; returns the exit code (`None` for a signal) and the output.
fn run_merged(root: &Path, argv: &[String]) -> Result<(Option<i32>, String), String> {
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
                    let _ = child.kill();
                    break Err(format!("{argv:?} printed more than {OUTPUT_LIMIT} bytes"));
                }
            }
            Err(error) => break Err(format!("cannot read {argv:?}: {error}")),
        }
    };
    let status = child
        .wait()
        .map_err(|error| format!("cannot wait for {argv:?}: {error}"))?;
    read?;
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

/// The same-key run: one binary, one result.
fn same_key(root: &Path, problems: &mut Vec<String>) -> SameKey {
    let argv = same_key_command();
    let (exit_code, counts) = match run_merged(root, &argv) {
        Ok((exit, text)) => match output::sections(&text) {
            Ok(sections) => match sections.as_slice() {
                [only] => (exit, only.counts),
                _ => {
                    problems.push(format!(
                        "the same-key run printed {} test results",
                        sections.len()
                    ));
                    (exit, None)
                }
            },
            Err(error) => {
                problems.push(format!("the same-key output: {error}"));
                (exit, None)
            }
        },
        Err(error) => {
            problems.push(error);
            (None, None)
        }
    };
    SameKey {
        command: argv,
        tests: SAME_KEY_TESTS.map(str::to_owned).to_vec(),
        counts,
        exit_code,
    }
}

/// The whole run; returns the receipt it wrote.
pub(in super::super) fn run(root: &Path, release: &str) -> Result<Receipt, String> {
    crate::integrity::validate_hex(release, 40, "release commit")?;
    let release = release.to_ascii_lowercase();
    let start = SourceIdentity::capture(root);
    if start.commit.to_ascii_lowercase() != release || start.worktree_dirty {
        return Err(format!(
            "the source tests run on a clean checkout of {release}; HEAD is {} (dirty {})",
            start.commit, start.worktree_dirty
        ));
    }
    let _lock = super::lock(root, true)?.ok_or(
        "another source-test run or a readiness check holds the lock; wait for it to finish",
    )?;
    let path = root.join(RECEIPT);
    match std::fs::remove_file(&path) {
        Ok(()) => eprintln!("removed the previous receipt; it stands for no run from now on"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot remove {}: {error}", path.display())),
    }
    if hidden_changes(root)? {
        return Err(
            "the Git index marks files assume-unchanged or skip-worktree, so a clean status proves nothing; clear the flags".to_owned(),
        );
    }
    if !temporary_volume_is_case_insensitive()? {
        return Err(
            "the temporary volume is case-sensitive, so P18-L08's case-insensitive check would be skipped; run with TMPDIR on a case-insensitive volume".to_owned(),
        );
    }
    let host = host(root)?;
    let mut problems = Vec::new();
    let (executables, doc) = discover(root).unwrap_or_else(|error| {
        problems.push(error);
        (Vec::new(), Vec::new())
    });
    let argv = command();
    let (exit_code, binaries) = match run_merged(root, &argv) {
        Ok((exit, text)) => match output::sections(&text) {
            Ok(sections) => {
                let (runs, more) = output::attribute(&sections, &executables, &doc);
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
    let same_key = same_key(root, &mut problems);
    let end = SourceIdentity::capture(root);
    let receipt = Receipt {
        release_source: release,
        host,
        // The index flags were refused before the run, so the start is clean exactly when Git
        // said so; the end is checked again.
        start: Side {
            head: start.commit.to_ascii_lowercase(),
            clean: !start.worktree_dirty,
        },
        end: side(root, &end),
        command: argv,
        exit_code,
        expected: executables
            .into_iter()
            .map(|executable| executable.target)
            .chain(doc)
            .collect(),
        binaries,
        same_key,
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
