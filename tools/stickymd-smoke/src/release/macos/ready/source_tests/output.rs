//! Reading what Cargo and libtest print: the test binaries Cargo will run (its JSON build
//! messages and `cargo metadata`), and the per-binary summaries of one `cargo test` whose
//! stdout and stderr share a pipe, so each Cargo header precedes its binary's output.
//!
//! A section opens at Cargo's `Running <description> (<path>)` or `Doc-tests <crate>` line and
//! holds libtest's `running <n> tests` line, one `test <name> ... <outcome>` line per test, and
//! its `test result: …` line; a doc-test section may hold several such groups (rustdoc runs the
//! merged doc-tests, then the standalone ones), whose counts add up. Anything else is ignored, so a stray line can only make a section
//! incomplete, doubled or self-contradictory, which is reported, never a PASS. Each binary's own
//! `--list` names the tests it holds; a run is checked against that listing test by test.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::{Case, CaseOutcome, Counts, PACKAGES, Run, Target};
use crate::release::json::{self, Value};

/// Who printed a section: a test binary (its path as Cargo printed it) or a library's doc-tests
/// (its crate).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum Header {
    Binary(String),
    Doc(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct Section {
    pub header: Header,
    pub counts: Option<Counts>,
    /// Each `test <name> ... <outcome>` line, in order.
    pub tests: Vec<(String, CaseOutcome)>,
}

/// The suffixes libtest prints after a test's name for its mode (rustc 1.99 `test::types`); the
/// listing names the test without them.
const MODES: [&str; 3] = [" - compile fail", " - compile", " - should panic"];

/// `test <name> ... ok|FAILED|ignored[, <reason>]`, with libtest's mode suffix taken off the name.
/// A name holds no ` ... `, a reason may (review round 4), so the line splits at the first one.
fn test_line(line: &str) -> Option<(String, CaseOutcome)> {
    let rest = line.strip_prefix("test ")?;
    let (name, outcome) = rest.split_once(" ... ")?;
    let outcome = match outcome {
        "ok" => CaseOutcome::Ok,
        "FAILED" => CaseOutcome::Failed,
        "ignored" => CaseOutcome::Ignored,
        other if other.starts_with("ignored, ") => CaseOutcome::Ignored,
        _ => return None,
    };
    let name = MODES
        .iter()
        .find_map(|mode| name.strip_suffix(mode))
        .unwrap_or(name);
    (!name.is_empty()).then(|| (name.to_owned(), outcome))
}

/// The test names of a `--list --format terse` listing (`<name>: test`); any other line is an
/// error, so a listing cannot be misread as shorter than it is.
pub(in super::super) fn listing(output: &str) -> Result<Vec<String>, String> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            line.strip_suffix(": test")
                .map(str::to_owned)
                .ok_or_else(|| format!("unreadable test listing line {line:?}"))
        })
        .collect()
}

/// rustdoc's closing line after merged doc-tests, which it prints even for `--list` (rustc 1.99.1,
/// 2024 edition): `all doctests ran in <s>s; merged doctests compilation took <s>s`, each `<s>` a
/// decimal number.
fn doc_timing(line: &str) -> bool {
    let seconds = |text: &str| {
        text.strip_suffix('s')
            .and_then(|number| number.split_once('.'))
            .is_some_and(|(whole, fraction)| {
                !whole.is_empty()
                    && !fraction.is_empty()
                    && whole
                        .bytes()
                        .chain(fraction.bytes())
                        .all(|b| b.is_ascii_digit())
            })
    };
    line.strip_prefix("all doctests ran in ")
        .and_then(|rest| rest.split_once("; merged doctests compilation took "))
        .is_some_and(|(ran, took)| seconds(ran) && seconds(took))
}

/// A library's doc-test listing (`cargo test --doc -- --list --format terse`): a [`listing`] that
/// may end with rustdoc's timing line, once, as its last line; anywhere else that line is an error
/// like any other (pre-freeze item 4).
pub(in super::super) fn doc_listing(output: &str) -> Result<Vec<String>, String> {
    let mut lines: Vec<&str> = output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    if lines.last().is_some_and(|line| doc_timing(line)) {
        lines.pop();
    }
    listing(&lines.join("\n"))
}

/// The cases of one binary: every listed test, with whether it is listed as ignored and the
/// outcome the run printed for it (`NotRun` when it printed none); a printed test the listing
/// does not hold is returned apart.
pub(in super::super) fn cases(
    all: &[String],
    ignored: &[String],
    printed: &[(String, CaseOutcome)],
) -> (Vec<Case>, Vec<String>) {
    // Every name once: a listing, an ignored listing or a run that names a test twice, or an
    // ignored test the full listing lacks, is a problem, never folded into its first entry
    // (review round 3, B3).
    let mut problems = Vec::new();
    let twice = |names: Vec<&String>, what: &str, problems: &mut Vec<String>| {
        let mut sorted = names;
        sorted.sort();
        for pair in sorted.windows(2) {
            if pair[0] == pair[1] {
                problems.push(format!("{} is {what} twice", pair[0]));
            }
        }
    };
    twice(all.iter().collect(), "listed", &mut problems);
    twice(ignored.iter().collect(), "listed ignored", &mut problems);
    twice(
        printed.iter().map(|(name, _)| name).collect(),
        "printed",
        &mut problems,
    );
    problems.extend(
        ignored
            .iter()
            .filter(|name| !all.contains(name))
            .map(|name| format!("{name} is listed ignored but not listed")),
    );
    let cases = all
        .iter()
        .map(|name| Case {
            name: name.clone(),
            ignored: ignored.contains(name),
            // A name printed twice is a problem above; a FAILED among its results is kept, so a
            // failure is never folded away (review round 4).
            outcome: {
                let mut found = printed.iter().filter(|(printed, _)| printed == name);
                let first = found.next().map(|(_, outcome)| *outcome);
                if found.any(|(_, outcome)| *outcome == CaseOutcome::Failed) {
                    CaseOutcome::Failed
                } else {
                    first.unwrap_or(CaseOutcome::NotRun)
                }
            },
        })
        .collect();
    problems.extend(
        printed
            .iter()
            .filter(|(name, _)| !all.contains(name))
            .map(|(name, _)| format!("{name} ran but is not listed")),
    );
    (cases, problems)
}

fn header(line: &str) -> Option<Header> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("Running ") {
        // `cargo run` prints "Running `path`", without the parenthesized path: not a test.
        let path = rest.strip_suffix(')')?.rsplit_once(" (")?.1;
        return (!path.is_empty()).then(|| Header::Binary(path.to_owned()));
    }
    let name = line.strip_prefix("Doc-tests ")?;
    (!name.is_empty() && !name.contains(char::is_whitespace)).then(|| Header::Doc(name.to_owned()))
}

/// `running 3 tests` / `running 1 test`.
fn running(line: &str) -> Option<u64> {
    let rest = line.strip_prefix("running ")?;
    let number = rest
        .strip_suffix(" tests")
        .or_else(|| rest.strip_suffix(" test"))?;
    number.parse().ok()
}

/// `test result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in
/// 0.00s`, with `FAILED` for a failed run; every field present, in libtest's order.
fn result(line: &str, ran: u64) -> Result<Counts, String> {
    let (ok, rest) = if let Some(rest) = line.strip_prefix("test result: ok. ") {
        (true, rest)
    } else if let Some(rest) = line.strip_prefix("test result: FAILED. ") {
        (false, rest)
    } else {
        return Err(format!("unreadable test result {line:?}"));
    };
    let parts: Vec<&str> = rest.split("; ").collect();
    let [passed, failed, ignored, measured, filtered, finished] = parts.as_slice() else {
        return Err(format!("unreadable test result {line:?}"));
    };
    // The whole trailer, `finished in <seconds>s`: a line cut short is not a result.
    let complete = finished
        .strip_prefix("finished in ")
        .and_then(|rest| rest.strip_suffix('s'))
        .is_some_and(|seconds| {
            seconds.split_once('.').is_some_and(|(whole, part)| {
                !whole.is_empty()
                    && !part.is_empty()
                    && whole
                        .bytes()
                        .chain(part.bytes())
                        .all(|b| b.is_ascii_digit())
            })
        });
    if !complete {
        return Err(format!("unreadable test result {line:?}"));
    }
    let number = |part: &str, word: &str| -> Result<u64, String> {
        part.strip_suffix(word)
            .and_then(|number| number.parse().ok())
            .ok_or_else(|| format!("unreadable test result {line:?}"))
    };
    let counts = Counts {
        ran,
        passed: number(passed, " passed")?,
        failed: number(failed, " failed")?,
        ignored: number(ignored, " ignored")?,
        measured: number(measured, " measured")?,
        filtered_out: number(filtered, " filtered out")?,
    };
    // libtest prints `ok` exactly when nothing failed; anything else contradicts itself.
    if ok != (counts.failed == 0) {
        return Err(format!("the test result contradicts itself: {line:?}"));
    }
    Ok(counts)
}

/// Two groups' counts as one, or `None` when a sum overflows.
fn add(a: Counts, b: Counts) -> Option<Counts> {
    Some(Counts {
        ran: a.ran.checked_add(b.ran)?,
        passed: a.passed.checked_add(b.passed)?,
        failed: a.failed.checked_add(b.failed)?,
        ignored: a.ignored.checked_add(b.ignored)?,
        measured: a.measured.checked_add(b.measured)?,
        filtered_out: a.filtered_out.checked_add(b.filtered_out)?,
    })
}

/// What one `cargo test` printed, as far as it could be read: the sections in order (the last one
/// possibly without its result) and, when the output broke off or could not be read, why. What was
/// read before the break is kept, so a `FAILED` already printed is never lost (fix round,
/// BLOCKER 3); a complete result is still needed for anything to pass.
pub(in super::super) struct Parsed {
    pub sections: Vec<Section>,
    pub error: Option<String>,
}

/// The sections of one `cargo test`, or why the output cannot be read whole.
#[cfg(test)]
pub(in super::super) fn sections(output: &str) -> Result<Vec<Section>, String> {
    let parsed = parse(output);
    match parsed.error {
        Some(error) => Err(error),
        None => Ok(parsed.sections),
    }
}

/// The reader's state: the sections so far, the `running` count of the group open in the last
/// one, and whether libtest is printing its failures (from `failures:` to the result line: the
/// failed tests' captured output, which may hold any line, `running 3 tests` included).
struct Reader {
    sections: Vec<Section>,
    open: Option<u64>,
    diagnostics: bool,
}

impl Reader {
    /// One line. A section's group opens at its `running` line and closes at its result; a test
    /// binary holds exactly one group, a doc-test section one or more.
    fn line(&mut self, line: &str) -> Result<(), String> {
        if self.diagnostics && !line.starts_with("test result: ") {
            return Ok(());
        }
        self.diagnostics = false;
        if self.open.is_some() && line == "failures:" {
            self.diagnostics = true;
        } else if let Some(header) = header(line) {
            if let Some(last) = self
                .sections
                .last()
                .filter(|s| s.counts.is_none() || self.open.is_some())
            {
                return Err(format!("{:?} printed no test result", last.header));
            }
            self.sections.push(Section {
                header,
                counts: None,
                tests: Vec::new(),
            });
        } else if let Some(count) = running(line) {
            match self.sections.last() {
                Some(last)
                    if self.open.is_none()
                        && (last.counts.is_none() || matches!(last.header, Header::Doc(_))) =>
                {
                    self.open = Some(count);
                }
                _ => return Err(format!("{line:?} outside a test binary's output")),
            }
        } else if !line.starts_with("test result: ")
            && let Some(test) = test_line(line)
        {
            match self.sections.last_mut() {
                Some(last) if self.open.is_some() => last.tests.push(test),
                _ => return Err(format!("{line:?} outside a test binary's output")),
            }
        } else if line.starts_with("test result: ") {
            let ran = self
                .open
                .take()
                .ok_or_else(|| format!("{line:?} outside a test binary's output"))?;
            let last = self
                .sections
                .last_mut()
                .ok_or_else(|| format!("{line:?} outside a test binary's output"))?;
            let group = result(line, ran)?;
            last.counts = Some(match last.counts {
                None => group,
                Some(before) => add(before, group)
                    .ok_or_else(|| format!("{:?}: the counts overflow", last.header))?,
            });
        }
        Ok(())
    }
}

/// The sections of one `cargo test`, in order, as far as they can be read.
pub(in super::super) fn parse(output: &str) -> Parsed {
    let mut reader = Reader {
        sections: Vec::new(),
        open: None,
        diagnostics: false,
    };
    for line in output.lines() {
        if let Err(error) = reader.line(line) {
            return Parsed {
                sections: reader.sections,
                error: Some(error),
            };
        }
    }
    let error = reader
        .sections
        .last()
        .filter(|s| s.counts.is_none() || reader.open.is_some())
        .map(|last| format!("{:?} printed no test result", last.header));
    Parsed {
        sections: reader.sections,
        error,
    }
}

/// A test executable Cargo built, by package, kinds, target name and file name.
pub(in super::super) struct Executable {
    pub target: Target,
    /// Where Cargo built it: the section's printed path must be its tail, and it is asked for its
    /// own test listing.
    pub path: std::path::PathBuf,
}

/// The test executables of the eight packages from `cargo test … --no-run
/// --message-format=json` (the same parser the Windows owner check uses).
pub(in super::super) fn executables(stdout: &str) -> Result<Vec<Executable>, String> {
    let mut found = Vec::new();
    for artifact in crate::unit_owners::parse_artifacts(stdout)? {
        if !PACKAGES.contains(&artifact.package.as_str()) {
            return Err(format!(
                "Cargo built a test of {}, which the row does not select",
                artifact.package
            ));
        }
        found.push(Executable {
            target: Target {
                package: artifact.package,
                kind: artifact.kind.join(","),
                name: artifact.name,
            },
            path: artifact.executable,
        });
    }
    Ok(found)
}

/// The libraries of the eight packages whose doc-tests `cargo test` runs, from `cargo metadata
/// --no-deps --format-version 1`: library targets with `doctest` set.
pub(in super::super) fn doc_libraries(metadata: &str) -> Result<Vec<Target>, String> {
    let document = json::parse(metadata)?;
    let mut found = Vec::new();
    for package in document.field("packages")?.array()? {
        let name = package.field("name")?.string()?;
        if !PACKAGES.contains(&name) {
            continue;
        }
        for target in package.field("targets")?.array()? {
            let kinds: Vec<&str> = target
                .field("kind")?
                .array()?
                .iter()
                .map(Value::string)
                .collect::<Result<_, _>>()?;
            let doctest = matches!(target.field("doctest")?, Value::Bool(true));
            if doctest
                && kinds
                    .iter()
                    .any(|kind| matches!(*kind, "lib" | "rlib" | "proc-macro"))
            {
                found.push(Target {
                    package: name.to_owned(),
                    kind: "doctest".to_owned(),
                    name: target.field("name")?.string()?.to_owned(),
                });
            }
        }
    }
    Ok(found)
}

/// The test binaries and doc-tested libraries `cargo test` runs for the eight packages, from
/// `cargo metadata --no-deps --format-version 1` on R: every target with `test` set (a library,
/// binary or test target; a build script is not a test), and the doc-tested libraries. The gate
/// checks the receipt against this list, made apart from the run (plan 11
/// #macos-readiness-check).
pub(in super::super) fn listed_targets(metadata: &str) -> Result<Vec<Target>, String> {
    let document = json::parse(metadata)?;
    let mut found = Vec::new();
    for package in document.field("packages")?.array()? {
        let name = package.field("name")?.string()?;
        if !PACKAGES.contains(&name) {
            continue;
        }
        for target in package.field("targets")?.array()? {
            let kinds: Vec<&str> = target
                .field("kind")?
                .array()?
                .iter()
                .map(Value::string)
                .collect::<Result<_, _>>()?;
            if matches!(target.field("test")?, Value::Bool(true)) && kinds != ["custom-build"] {
                found.push(Target {
                    package: name.to_owned(),
                    kind: kinds.join(","),
                    name: target.field("name")?.string()?.to_owned(),
                });
            }
        }
    }
    for package in PACKAGES {
        if !found.iter().any(|target| target.package == package) {
            return Err(format!("cargo metadata lists no test target of {package}"));
        }
    }
    found.extend(doc_libraries(metadata)?);
    Ok(found)
}

/// The target a section ran, by what Cargo listed: an executable's file name, or a doc-tested
/// library's name. `None` when it matches nothing or more than one.
pub(in super::super) fn target_of<'a>(
    section: &Section,
    executables: &'a [Executable],
    doc: &'a [Target],
) -> Option<&'a Target> {
    let matches: Vec<&Target> = match &section.header {
        // The printed path, relative to the workspace, must be the tail of the executable Cargo
        // built, component by component: a file of the same name elsewhere is not it.
        Header::Binary(path) => executables
            .iter()
            .filter(|e| e.path.ends_with(std::path::Path::new(path)))
            .map(|e| &e.target)
            .collect(),
        Header::Doc(name) => doc.iter().filter(|t| &t.name == name).collect(),
    };
    match matches.as_slice() {
        [one] => Some(one),
        _ => None,
    }
}

/// A target's listing: all its tests and those of them listed as ignored.
pub(in super::super) struct Listed {
    pub target: Target,
    pub all: Vec<String>,
    pub ignored: Vec<String>,
}

/// Names each section by the target it ran and joins it with that target's listing; a section
/// that matches nothing Cargo listed, a target without a listing, or a printed test the listing
/// does not hold is a problem rather than a guess.
pub(in super::super) fn attribute(
    sections: &[Section],
    executables: &[Executable],
    doc: &[Target],
    listings: &[Listed],
) -> (Vec<Run>, Vec<String>) {
    let mut runs = Vec::new();
    let mut problems = Vec::new();
    for section in sections {
        // A section without its result is kept: its printed failures count (fix round,
        // BLOCKER 3); with no summary it can never pass.
        let Some(target) = target_of(section, executables, doc) else {
            problems.push(format!(
                "{:?} is not one listed test target",
                section.header
            ));
            continue;
        };
        let Some(listed) = listings.iter().find(|l| &l.target == target) else {
            problems.push(format!(
                "{} {} {} has no test listing",
                target.package, target.kind, target.name
            ));
            continue;
        };
        let (cases, unlisted) = cases(&listed.all, &listed.ignored, &section.tests);
        problems.extend(
            unlisted
                .into_iter()
                .map(|problem| format!("{}: {problem}", target.name)),
        );
        runs.push(Run {
            target: target.clone(),
            counts: section.counts,
            cases,
        });
    }
    (runs, problems)
}
