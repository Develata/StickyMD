//! Read-only, bounded analysis of optional startup shell substeps.
//! plan_ref: docs/plan/10_performance_reliability.md#initial-engineering-targets

mod analysis;

use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Options {
    trace: PathBuf,
    details: PathBuf,
    json: bool,
}

pub(crate) fn parse(arguments: &[String]) -> Result<Options, String> {
    let (mut trace, mut details, mut json) = (None, None, false);
    let mut index = 0;
    while let Some(argument) = arguments.get(index) {
        match argument.as_str() {
            "--trace" | "--details" => {
                let slot = if argument == "--trace" {
                    &mut trace
                } else {
                    &mut details
                };
                if slot.is_some() {
                    return Err(format!("repeated startup-details option: {argument}"));
                }
                index += 1;
                let value = arguments
                    .get(index)
                    .filter(|value| !value.is_empty() && !value.starts_with("--"))
                    .ok_or_else(|| format!("startup-details {argument} requires a path"))?;
                *slot = Some(PathBuf::from(value));
            }
            "--json" if !json => json = true,
            _ => {
                return Err(format!(
                    "unknown or repeated startup-details option: {argument}"
                ));
            }
        }
        index += 1;
    }
    Ok(Options {
        trace: trace.ok_or("startup-details requires --trace <v2-path>")?,
        details: details.ok_or("startup-details requires --details <sidecar-path>")?,
        json,
    })
}

pub(crate) fn execute(options: &Options) -> Result<(), String> {
    let trace = read(&options.trace)?;
    let details = read(&options.details)?;
    // No partial output, process launch, evidence write or readiness aggregation.
    let analysis = analysis::analyze(&trace, &details)?;
    println!("{}", analysis.render(options.json));
    Ok(())
}

fn read(path: &Path) -> Result<String, String> {
    const LIMIT: u64 = 8 * 1024;
    let validate = |metadata: &fs::Metadata| {
        if metadata.is_file() && metadata.len() <= LIMIT {
            Ok(())
        } else {
            Err(format!(
                "startup diagnostic input must be a regular file of at most 8 KiB: {}",
                path.display()
            ))
        }
    };
    // Check before open to reject ordinary FIFOs, then recheck the actual handle.
    let metadata = fs::metadata(path)
        .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?;
    validate(&metadata)?;
    let file =
        fs::File::open(path).map_err(|error| format!("cannot open {}: {error}", path.display()))?;
    validate(
        &file
            .metadata()
            .map_err(|error| format!("cannot inspect {}: {error}", path.display()))?,
    )?;
    let mut content = String::new();
    file.take(LIMIT + 1)
        .read_to_string(&mut content)
        .map_err(|error| {
            format!(
                "cannot read UTF-8 startup input {}: {error}",
                path.display()
            )
        })?;
    if content.len() as u64 > LIMIT {
        return Err("startup diagnostic input exceeds 8 KiB".into());
    }
    Ok(content)
}

#[cfg(test)]
mod tests;
