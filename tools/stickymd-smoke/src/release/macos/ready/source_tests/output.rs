//! Reading what Cargo and libtest print: the test binaries Cargo will run (its JSON build
//! messages and `cargo metadata`), and the per-binary summaries of one `cargo test` whose
//! stdout and stderr share a pipe, so each Cargo header precedes its binary's output.
//!
//! A section opens at Cargo's `Running <description> (<path>)` or `Doc-tests <crate>` line and
//! holds libtest's `running <n> tests` and `test result: …` lines. Anything else is ignored, so a
//! stray line can only make a section incomplete or doubled, which is reported, never a PASS.
//! plan_ref: docs/plan/11_testing_and_release.md#macos-readiness-check

use super::{Counts, PACKAGES, Run, Target};
use crate::release::json::{self, Value};

/// Who printed a section: a test binary (its file name) or a library's doc-tests (its crate).
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) enum Header {
    Binary(String),
    Doc(String),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in super::super) struct Section {
    pub header: Header,
    pub counts: Option<Counts>,
}

fn header(line: &str) -> Option<Header> {
    let line = line.trim();
    if let Some(rest) = line.strip_prefix("Running ") {
        // `cargo run` prints "Running `path`", without the parenthesized path: not a test.
        let path = rest.strip_suffix(')')?.rsplit_once(" (")?.1;
        let file = path.rsplit(['/', '\\']).next()?;
        return (!file.is_empty()).then(|| Header::Binary(file.to_owned()));
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
    if !finished.starts_with("finished in ") {
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

/// The sections of one `cargo test`, in order; every section must end with exactly one result.
pub(in super::super) fn sections(output: &str) -> Result<Vec<Section>, String> {
    let mut sections: Vec<Section> = Vec::new();
    let mut ran: Option<u64> = None;
    for line in output.lines() {
        if let Some(header) = header(line) {
            if let Some(open) = sections.last().filter(|s| s.counts.is_none()) {
                return Err(format!("{:?} printed no test result", open.header));
            }
            sections.push(Section {
                header,
                counts: None,
            });
            ran = None;
        } else if let Some(count) = running(line) {
            match sections.last() {
                Some(open) if open.counts.is_none() && ran.is_none() => ran = Some(count),
                _ => return Err(format!("{line:?} outside a test binary's output")),
            }
        } else if line.starts_with("test result: ") {
            let open = sections
                .last_mut()
                .filter(|s| s.counts.is_none())
                .ok_or_else(|| format!("{line:?} outside a test binary's output"))?;
            let count = ran
                .take()
                .ok_or_else(|| format!("{line:?} without a running line"))?;
            open.counts = Some(result(line, count)?);
        }
    }
    if let Some(open) = sections.last().filter(|s| s.counts.is_none()) {
        return Err(format!("{:?} printed no test result", open.header));
    }
    Ok(sections)
}

/// A test executable Cargo built, by package, kinds, target name and file name.
pub(in super::super) struct Executable {
    pub target: Target,
    pub file: String,
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
        let file = artifact
            .executable
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or("a test executable has no file name")?
            .to_owned();
        found.push(Executable {
            target: Target {
                package: artifact.package,
                kind: artifact.kind.join(","),
                name: artifact.name,
            },
            file,
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

/// Names each section by the binary or library it ran; a section that matches nothing Cargo
/// listed, or matches twice, is a problem rather than a guess.
pub(in super::super) fn attribute(
    sections: &[Section],
    executables: &[Executable],
    doc: &[Target],
) -> (Vec<Run>, Vec<String>) {
    let mut runs = Vec::new();
    let mut problems = Vec::new();
    for section in sections {
        let matches: Vec<&Target> = match &section.header {
            Header::Binary(file) => executables
                .iter()
                .filter(|e| &e.file == file)
                .map(|e| &e.target)
                .collect(),
            Header::Doc(name) => doc.iter().filter(|t| &t.name == name).collect(),
        };
        match (matches.as_slice(), section.counts) {
            ([target], Some(counts)) => runs.push(Run {
                target: (*target).clone(),
                counts,
            }),
            ([], _) => problems.push(format!("{:?} is not a listed test", section.header)),
            _ => problems.push(format!("{:?} is ambiguous", section.header)),
        }
    }
    (runs, problems)
}
