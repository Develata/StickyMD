//! One local SBOM preparation request composes existing selection and cache authorities.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use super::cli::SbomPreparationOptions;
use crate::{evidence::escape_json, package_path, repository};
use std::path::{Path, PathBuf};

pub(super) struct Prepared {
    pub version: String,
    pub zip: PathBuf,
    pub syft: super::syft::Plan,
    pub verified: Option<PathBuf>,
}

pub(super) fn execute(root: &Path, options: &SbomPreparationOptions) -> Result<(), String> {
    let Prepared {
        version,
        zip,
        syft,
        verified,
    } = prepare(root, options)?;
    let plan = syft.json();
    let archive = verified.map_or_else(
        || "null".into(),
        |path| format!("\"{}\"", escape_json(&path.to_string_lossy())),
    );
    println!(
        "{{\"workspace_version\":\"{}\",\"zip_path\":\"{}\",\"syft\":{plan},\"verified_archive\":{archive}}}",
        escape_json(&version),
        escape_json(&zip.to_string_lossy())
    );
    Ok(())
}

pub(super) fn prepare(root: &Path, options: &SbomPreparationOptions) -> Result<Prepared, String> {
    let version = repository::workspace_version(root)?;
    let zip = match &options.zip {
        Some(path) => std::path::absolute(path).map_err(|error| error.to_string())?,
        None => package_path::resolve(root, &options.package_directory)?,
    };
    if !zip.is_file() {
        return Err(format!("ZIP does not exist: {}", zip.display()));
    }
    // This selects a path; archive contents and candidate qualification remain separate.
    let (plan, verified) = super::syft::prepare(root, options.syft.as_deref(), &options.directory)?;
    Ok(Prepared {
        version,
        zip,
        syft: plan,
        verified,
    })
}
