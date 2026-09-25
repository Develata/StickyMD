//! Verify supplied promotion inputs without creating a candidate or receipt.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use super::{cli::PromotionOptions, identity, temporary::TemporaryDirectory, windows};
use crate::{integrity, repository};
use std::{fs, path::Path};

pub(super) fn verify(root: &Path, options: &PromotionOptions) -> Result<(), String> {
    let source = identity::source(root)?;
    let version = repository::workspace_version(root)?;
    identity::approved_source(&source, &options.source, &version, &options.tag)?;
    integrity::validate_sha256(&options.zip_hash, "expected ZIP SHA-256")?;
    integrity::validate_sha256(&options.sbom_hash, "expected SBOM SHA-256")?;
    let zip_name = format!("StickyMD-{version}-windows-x64-portable.zip");
    integrity::artifact_name(&zip_name)?;
    let directory = std::path::absolute(&options.directory).map_err(|error| error.to_string())?;
    let zip = directory.join(&zip_name);
    for name in [&zip_name, "SBOM.spdx.json", "SHA256SUMS.txt"] {
        if !directory.join(name).is_file() {
            return Err(format!(
                "Exact promotion input is missing: {}",
                directory.join(name).display()
            ));
        }
    }
    for entry in fs::read_dir(&directory).map_err(|error| error.to_string())? {
        let entry = entry.map_err(|error| error.to_string())?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.path().is_file()
            && name.to_ascii_lowercase().ends_with(".zip")
            && !name.eq_ignore_ascii_case(&zip_name)
        {
            return Err("Exact promotion input contains an unexpected ZIP".to_owned());
        }
    }
    let temporary = TemporaryDirectory::new("verify-promoted")?;
    let copied_zip = temporary.path().join("input.zip");
    fs::copy(&zip, &copied_zip)
        .map_err(|error| format!("cannot snapshot promoted ZIP: {error}"))?;
    let zip_hash = integrity::sha256(&copied_zip)?;
    let sbom_hash = integrity::sha256(&directory.join("SBOM.spdx.json"))?;
    for (kind, actual, expected) in [
        ("ZIP", &zip_hash, &options.zip_hash),
        ("SBOM", &sbom_hash, &options.sbom_hash),
    ] {
        if !actual.eq_ignore_ascii_case(expected) {
            return Err(format!(
                "{kind} hash {actual} differs from approved {expected}"
            ));
        }
    }
    integrity::verify_checksum_manifest(&directory, &zip_name, &zip_hash, &sbom_hash)?;
    let names = windows::names(root, &copied_zip)?;
    let readme = readme_index(&names)?;
    identity::readme_source(&windows::read_entry(root, &copied_zip, readme)?, &source)?;
    temporary.close()?;
    println!(
        "PROMOTION_INPUT=PASS\nPROMOTION_ZIP={}\nPROMOTION_ZIP_SHA256={zip_hash}\nPROMOTION_SBOM_SHA256={sbom_hash}",
        zip.display()
    );
    Ok(())
}

fn readme_index(names: &[String]) -> Result<usize, String> {
    let mut matches = names
        .iter()
        .enumerate()
        .filter(|(_, name)| name.eq_ignore_ascii_case("StickyMD/README.txt"));
    match (matches.next(), matches.next()) {
        (Some((index, _)), None) => Ok(index),
        _ => Err("Promoted ZIP must contain exactly one StickyMD/README.txt".to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn promoted_readme_is_required_and_unique() {
        assert_eq!(
            readme_index(&["StickyMD/README.txt".to_owned()]).unwrap(),
            0
        );
        assert!(readme_index(&[]).is_err());
        assert!(
            readme_index(&[
                "StickyMD/README.txt".to_owned(),
                "stickymd/readme.txt".to_owned()
            ])
            .is_err()
        );
    }
}
