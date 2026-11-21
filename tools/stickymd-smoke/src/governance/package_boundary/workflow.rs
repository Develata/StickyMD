//! The release workflows' side of the packaging boundary: the dependency-policy job and the three
//! release jobs in order, the download step, and a `build-package` job free of the verification
//! tool.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use super::check_script;

/// The release workflow's jobs in order, the `build-package` job free of the verification tool;
/// returns the scripts that job runs, to be checked in turn. `needs` is read from the job's own
/// fields and the download from a step's own `uses`, never from comments, scalars or step bodies.
pub(super) fn check_workflow(text: &str) -> Result<Vec<String>, String> {
    let jobs = jobs(text)?;
    let job = |name: &str| {
        jobs.iter()
            .find(|(job, _)| job == name)
            .map(|(_, body)| body.as_str())
            .ok_or_else(|| format!("has no `{name}` job"))
    };
    job("dependency-policy")?;
    for (name, needs) in [
        ("self-check", "dependency-policy"),
        ("build-package", "self-check"),
        ("verify-artifact", "build-package"),
    ] {
        let declared = fields_at(job(name)?, 4).any(|field| field == format!("needs: {needs}"));
        if !declared {
            return Err(format!("job `{name}` must have `needs: {needs}`"));
        }
    }
    let downloads = step_fields(job("verify-artifact")?)
        .iter()
        .any(|field| field.starts_with("uses: actions/download-artifact@"));
    if !downloads {
        return Err("job `verify-artifact` must download the uploaded artifact".to_owned());
    }
    let build = job("build-package")?;
    check_script(build, "build-package").map_err(|e| format!("job `build-package`: {e}"))
}

/// The direct fields of a job's steps: the field on each `- ` item line (indent 6) and the lines
/// at indent 8 below it, inside the job's `steps:` (indent 4). A block scalar's lines are indented
/// more than its key, so none of them is read as a field.
fn step_fields(body: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut in_steps = false;
    for line in body.lines() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let indent = line.len() - trimmed.len();
        if indent <= 4 {
            in_steps = indent == 4 && without_comment(trimmed).trim() == "steps:";
            continue;
        }
        if !in_steps {
            continue;
        }
        let field = without_comment(trimmed).trim();
        if indent == 6 {
            if let Some(first) = field.strip_prefix("- ") {
                fields.push(first.trim().to_owned());
            }
        } else if indent == 8 {
            fields.push(field.to_owned());
        }
    }
    fields
}

/// The lines of `body` indented by exactly `indent` spaces, comments removed and trimmed.
fn fields_at(body: &str, indent: usize) -> impl Iterator<Item = &str> {
    body.lines().filter_map(move |line| {
        let field = line.get(indent..)?;
        (line[..indent].bytes().all(|b| b == b' ') && !field.starts_with(' '))
            .then(|| without_comment(field).trim())
    })
}

/// A YAML line without a trailing `#` comment (one preceded by whitespace or at the start).
fn without_comment(line: &str) -> &str {
    let bytes = line.as_bytes();
    for (at, byte) in bytes.iter().enumerate() {
        if *byte == b'#' && (at == 0 || bytes[at - 1].is_ascii_whitespace()) {
            return &line[..at];
        }
    }
    line
}

/// The jobs of a workflow: each `  <name>:` header directly under `jobs:` (a trailing comment
/// allowed) with the lines up to the next header. Blank and comment lines belong to the job they
/// sit in, whatever their indent; any other line indented less than a job ends the jobs. A job
/// name given twice is an error.
fn jobs(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut lines = text.lines().skip_while(|line| *line != "jobs:");
    if lines.next().is_none() {
        return Err("has no `jobs:` section".to_owned());
    }
    let mut jobs: Vec<(String, String)> = Vec::new();
    for line in lines {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            if let Some((_, body)) = jobs.last_mut() {
                body.push_str(line);
                body.push('\n');
            }
            continue;
        }
        if indent < 2 {
            break;
        }
        if indent == 2 {
            let name = without_comment(trimmed)
                .trim_end()
                .strip_suffix(':')
                .filter(|name| {
                    !name.is_empty()
                        && name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
                })
                .ok_or_else(|| format!("unexpected line under `jobs:`: {trimmed}"))?;
            if jobs.iter().any(|(job, _)| job == name) {
                return Err(format!("job `{name}` is defined twice"));
            }
            jobs.push((name.to_owned(), String::new()));
            continue;
        }
        let (_, body) = jobs
            .last_mut()
            .ok_or_else(|| "a step before the first job".to_owned())?;
        body.push_str(line);
        body.push('\n');
    }
    Ok(jobs)
}
