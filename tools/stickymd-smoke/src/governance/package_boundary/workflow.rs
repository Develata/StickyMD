//! The release workflows' side of the packaging boundary, by exact comparison only: the text
//! before `jobs:` and the `build-package` job equal pinned copies, no YAML anchor, alias or merge
//! key can bring other text into them, the jobs are exactly the four of plan 11, and the other
//! jobs' `needs` and the download step are the exact lines plan 11 names. Lines are compared after
//! line endings are made LF; nothing else is normalized or parsed.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

/// The pinned copies of a workflow: the text before its `jobs:` line, and its `build-package` job
/// from the job's header line up to the next job's.
pub(super) struct Pins {
    pub(super) header: &'static str,
    pub(super) build_package: &'static str,
}

pub(super) const RELEASE_WORKFLOWS: &[(&str, Pins)] = &[
    (
        ".github/workflows/release.yml",
        Pins {
            header: include_str!("pins/release.header.yml"),
            build_package: include_str!("pins/release.build-package.yml"),
        },
    ),
    (
        ".github/workflows/release-macos.yml",
        Pins {
            header: include_str!("pins/release-macos.header.yml"),
            build_package: include_str!("pins/release-macos.build-package.yml"),
        },
    ),
];

const JOBS: &[&str] = &[
    "  dependency-policy:",
    "  self-check:",
    "  build-package:",
    "  verify-artifact:",
];
const SELF_CHECK_NEEDS: &str = "    needs: dependency-policy";
const VERIFY_NEEDS: &str = "    needs: build-package";
const DOWNLOAD: &str =
    "        uses: actions/download-artifact@37930b1c2abaa49bbe596cd826c3c89aef350131 # v7.0.0";

fn is_comment(line: &str) -> bool {
    line.trim_start().starts_with('#')
}

/// A line with exactly two spaces of indentation that is not a comment: a job header position.
fn at_job_indent(line: &str) -> bool {
    line.starts_with("  ")
        && line.as_bytes().get(2).is_some_and(|b| *b != b' ')
        && !is_comment(line)
}

/// Whether `line` holds a YAML anchor (`&name`), alias (`*name`) or merge key (`<<:`): a `&` or
/// `*` at the start of the line's content or after a space, `[`, `{` or `,`, followed by a
/// letter, digit, `_` or `-`. A shell glob such as `*.zip` or an operator such as `&&` does not
/// match; a word like `*note` in a command does, which only makes the check stricter.
fn has_anchor(line: &str) -> bool {
    let bytes = line.as_bytes();
    line.contains("<<:")
        || bytes.iter().enumerate().any(|(at, byte)| {
            matches!(byte, b'&' | b'*')
                && (at == 0 || matches!(bytes[at - 1], b' ' | b'\t' | b'[' | b'{' | b','))
                && bytes
                    .get(at + 1)
                    .is_some_and(|next| next.is_ascii_alphanumeric() || matches!(next, b'_' | b'-'))
        })
}

/// Checks one release workflow against its pins and returns its `build-package` job's text.
pub(super) fn check(text: &str, pins: &Pins) -> Result<String, String> {
    let text = text.replace("\r\n", "\n");
    let lines: Vec<&str> = text.lines().collect();
    for (number, line) in lines.iter().enumerate() {
        if !is_comment(line) && has_anchor(line) {
            return Err(format!(
                "line {} holds a YAML anchor, alias or merge key, which could bring other text into a pinned job",
                number + 1
            ));
        }
    }
    let jobs: Vec<usize> = (0..lines.len()).filter(|&i| lines[i] == "jobs:").collect();
    let [jobs] = jobs[..] else {
        return Err(format!(
            "has {} `jobs:` lines; plan 11 expects exactly one",
            jobs.len()
        ));
    };
    let header: String = lines[..jobs]
        .iter()
        .map(|line| format!("{line}\n"))
        .collect();
    if header != pins.header.replace("\r\n", "\n") {
        return Err(
            "the text before `jobs:` differs from its pinned copy; review the change and update the pin in stickymd-smoke"
                .to_owned(),
        );
    }
    let mut headers = Vec::new();
    for (i, line) in lines.iter().enumerate().skip(jobs + 1) {
        if line.trim().is_empty() || is_comment(line) {
            continue;
        }
        if !line.starts_with("  ") {
            return Err(format!(
                "line {} follows `jobs:` without being inside it; plan 11 expects `jobs:` last",
                i + 1
            ));
        }
        if at_job_indent(line) {
            if !JOBS.contains(line) {
                return Err(format!(
                    "line {} is `{line}`; plan 11 names only the jobs {JOBS:?}",
                    i + 1
                ));
            }
            if headers.iter().any(|&(_, seen)| seen == *line) {
                return Err(format!("the job `{}` is defined twice", line.trim()));
            }
            headers.push((i, *line));
        }
    }
    let span = |job: &str| -> Result<Vec<&str>, String> {
        let position = headers
            .iter()
            .position(|&(_, line)| line == job)
            .ok_or_else(|| format!("has no `{}` job", job.trim().trim_end_matches(':')))?;
        let start = headers[position].0;
        let end = headers.get(position + 1).map_or(lines.len(), |&(i, _)| i);
        Ok(lines[start..end].to_vec())
    };
    for job in JOBS {
        span(job)?;
    }
    let build_package: String = span("  build-package:")?
        .iter()
        .map(|line| format!("{line}\n"))
        .collect();
    if build_package != pins.build_package.replace("\r\n", "\n") {
        return Err(
            "the `build-package` job differs from its pinned copy; review the change and update the pin in stickymd-smoke"
                .to_owned(),
        );
    }
    let needs = |job: &str| -> Result<Vec<&str>, String> {
        Ok(span(job)?
            .into_iter()
            .filter(|line| line.trim_start().starts_with("needs"))
            .collect())
    };
    for (job, expected) in [
        ("  self-check:", SELF_CHECK_NEEDS),
        ("  verify-artifact:", VERIFY_NEEDS),
    ] {
        if needs(job)? != [expected] {
            return Err(format!(
                "the `{}` job's `needs` lines are {:?}; plan 11 expects exactly `{}`",
                job.trim().trim_end_matches(':'),
                needs(job)?,
                expected.trim()
            ));
        }
    }
    if !span("  verify-artifact:")?.contains(&DOWNLOAD) {
        return Err(format!(
            "the `verify-artifact` job has no line `{}`",
            DOWNLOAD.trim()
        ));
    }
    Ok(build_package)
}
