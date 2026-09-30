//! Read-only timing observations from existing receipts and stderr logs.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod cargo;
mod input;
mod log;
mod report;

use std::{collections::BTreeSet, fs, io::Read, path::PathBuf};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    inputs: Vec<PathBuf>,
    json: bool,
}

#[derive(Debug, PartialEq)]
struct Observation {
    kind: &'static str,
    label: String,
    metric: String,
    seconds: Option<f64>,
    status: Option<String>,
    origin: Option<String>,
    line: Option<usize>,
}

#[derive(Debug, Default)]
struct Summary {
    format: &'static str,
    status: String,
    suite: Option<String>,
    source: Option<String>,
    observations: Vec<Observation>,
    missing_task_timings: Vec<String>,
}

impl Summary {
    fn record(
        &mut self,
        kind: &'static str,
        label: &str,
        metric: &str,
        seconds: Option<f64>,
    ) -> &mut Observation {
        self.observations.push(Observation {
            kind,
            label: label.into(),
            metric: metric.into(),
            seconds,
            status: None,
            origin: None,
            line: None,
        });
        self.observations.last_mut().expect("just appended")
    }
}

pub(crate) fn parse(arguments: &[String]) -> Result<Options, String> {
    let mut options = Options {
        inputs: Vec::new(),
        json: false,
    };
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        match argument.as_str() {
            "--input" => {
                index += 1;
                let path = arguments
                    .get(index)
                    .filter(|value| !value.is_empty() && !value.starts_with("--"))
                    .ok_or("timings --input requires a path")?;
                options.inputs.push(PathBuf::from(path));
            }
            "--json" if !options.json => options.json = true,
            _ => return Err(format!("unknown or repeated timings option: {argument}")),
        }
        index += 1;
    }
    if options.inputs.is_empty() {
        return Err("timings requires at least one --input path".into());
    }
    Ok(options)
}

pub(crate) fn execute(options: &Options) -> Result<(), String> {
    // Read every input before writing stdout: a later invalid input cannot leave a
    // plausible partial summary. Each file remains a separate observation scope.
    let summaries = read_inputs(&options.inputs)?;
    println!("{}", report::render(&summaries, options.json));
    Ok(())
}

fn read_inputs(paths: &[PathBuf]) -> Result<Vec<(PathBuf, Summary)>, String> {
    let mut seen = BTreeSet::new();
    let mut summaries = Vec::new();
    for path in paths {
        // Inspect the path before opening: ordinary Unix FIFOs block in open(),
        // so validating only the resulting handle is too late for such inputs.
        let metadata = fs::metadata(path)
            .map_err(|error| format!("cannot inspect timing input {}: {error}", path.display()))?;
        validate_file(&metadata, path)?;
        let canonical = fs::canonicalize(path)
            .map_err(|error| format!("cannot open timing input {}: {error}", path.display()))?;
        if !seen.insert(canonical) {
            return Err(format!("duplicate timing input: {}", path.display()));
        }
        let file = fs::File::open(path)
            .map_err(|error| format!("cannot open timing input {}: {error}", path.display()))?;
        let metadata = file
            .metadata()
            .map_err(|error| format!("cannot inspect timing input {}: {error}", path.display()))?;
        // Recheck the opened handle if the file changed after the path check.
        validate_file(&metadata, path)?;
        let mut text = String::new();
        file.take(64 * 1024 * 1024 + 1)
            .read_to_string(&mut text)
            .map_err(|error| {
                format!("cannot read UTF-8 timing input {}: {error}", path.display())
            })?;
        if text.len() > 64 * 1024 * 1024 {
            return Err("timing input exceeds 64 MiB".into());
        }
        summaries.push((
            path.clone(),
            summarize(&text)
                .map_err(|error| format!("invalid timing input {}: {error}", path.display()))?,
        ));
    }
    Ok(summaries)
}

fn validate_file(metadata: &fs::Metadata, path: &std::path::Path) -> Result<(), String> {
    if !metadata.is_file() || metadata.len() > 64 * 1024 * 1024 {
        return Err(format!(
            "timing input must be a regular file of at most 64 MiB: {}",
            path.display()
        ));
    }
    Ok(())
}

fn summarize(text: &str) -> Result<Summary, String> {
    let text = text.trim_start_matches('\u{feff}').trim_start();
    if text.starts_with('{') || text.starts_with('[') {
        input::summarize(&crate::release::json::parse(text)?)
    } else {
        log::summarize(text)
    }
}

fn seconds(value: &str) -> Result<f64, String> {
    let value: f64 = value.parse().map_err(|_| "invalid seconds value")?;
    if value.is_finite() && value >= 0.0 {
        Ok(value)
    } else {
        Err("seconds must be finite and nonnegative".into())
    }
}

#[cfg(test)]
mod tests;
