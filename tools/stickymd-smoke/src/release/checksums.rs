//! Checksum generation and output-path guards shared by local release writers.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use super::cli::ChecksumOptions;
use crate::{atomic_evidence, integrity};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

pub(super) fn generate(options: &ChecksumOptions) -> Result<(), String> {
    let output = destination(&options.output)?;
    let mut paths = vec![options.zip.as_path(), output.as_path()];
    if let Some(sbom) = &options.sbom {
        paths.push(sbom);
    }
    distinct_paths(&paths)?;
    let zip_hash = integrity::sha256(&options.zip)?;
    let sbom_hash = options.sbom.as_deref().map(integrity::sha256).transpose()?;
    let text = manifest(
        &options.zip,
        &zip_hash,
        options.sbom.as_deref().zip(sbom_hash.as_deref()),
    )?;
    atomic_evidence::write(&output, text.as_bytes())?;
    // The PowerShell staging adapter keeps its existing KEY=value output interface.
    println!(
        "{{\"zip_sha256\":\"{zip_hash}\",\"sbom_sha256\":{}}}",
        sbom_hash.map_or_else(|| "null".to_owned(), |hash| format!("\"{hash}\""))
    );
    Ok(())
}

pub(super) fn manifest(
    zip: &Path,
    zip_hash: &str,
    sbom: Option<(&Path, &str)>,
) -> Result<String, String> {
    let mut artifacts = vec![(basename(zip)?, zip_hash)];
    if let Some((path, hash)) = sbom {
        artifacts.push((basename(path)?, hash));
    }
    integrity::render_checksums(&artifacts)
}

fn basename(path: &Path) -> Result<&str, String> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or("artifact name is not Unicode")?;
    integrity::artifact_name(name)?;
    Ok(name)
}

pub(super) fn destination(path: &Path) -> Result<PathBuf, String> {
    let path = std::path::absolute(path).map_err(|error| error.to_string())?;
    basename(&path)?;
    if !path.parent().is_some_and(Path::is_dir) {
        return Err(format!(
            "destination directory does not exist: {}",
            path.display()
        ));
    }
    match fs::symlink_metadata(&path) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(format!(
                "release output is not a regular file: {}",
                path.display()
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(format!("cannot inspect output {}: {error}", path.display())),
    }
    Ok(path)
}

pub(super) fn distinct_paths(paths: &[&Path]) -> Result<(), String> {
    let mut seen = BTreeSet::new();
    for path in paths {
        let absolute = std::path::absolute(path).map_err(|error| error.to_string())?;
        let normalized = if absolute.exists() {
            absolute.canonicalize().map_err(|error| error.to_string())?
        } else {
            absolute
                .parent()
                .ok_or("artifact has no parent")?
                .canonicalize()
                .map_err(|error| error.to_string())?
                .join(basename(&absolute)?)
        };
        let key = normalized.to_str().ok_or("artifact path is not Unicode")?;
        #[cfg(windows)]
        let key = key.to_lowercase();
        if !seen.insert(key.to_owned()) {
            return Err("Release input and output paths must be distinct".to_owned());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::release::temporary::TemporaryDirectory;

    #[test]
    fn generation_preserves_old_manifest_on_missing_inputs_or_path_aliases() {
        let temp = TemporaryDirectory::new("checksums-test").unwrap();
        let zip = temp.path().join("中文 archive.zip");
        let output = temp.path().join("SHA256SUMS.txt");
        fs::write(&zip, b"abc").unwrap();
        fs::write(&output, b"previous").unwrap();
        let options = ChecksumOptions {
            zip: zip.clone(),
            sbom: Some(temp.path().join("missing.json")),
            output: output.clone(),
        };
        assert!(generate(&options).is_err());
        assert_eq!(fs::read(&output).unwrap(), b"previous");
        let options = ChecksumOptions {
            sbom: None,
            ..options
        };
        assert!(
            generate(&ChecksumOptions {
                output: zip.clone(),
                ..options.clone()
            })
            .is_err()
        );
        assert_eq!(fs::read(&zip).unwrap(), b"abc");
        generate(&options).unwrap();
        assert_eq!(
            fs::read_to_string(&output).unwrap(),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad *中文 archive.zip\n"
        );
        assert_eq!(fs::read_dir(temp.path()).unwrap().count(), 2);
    }

    #[test]
    fn checksum_generation_uses_strict_verifier_rules() {
        let hash = "A".repeat(64);
        assert!(
            manifest(
                Path::new("../a.zip"),
                &hash,
                Some((Path::new("a.ZIP"), &hash))
            )
            .is_err()
        );
        for name in ["unsafe:zip", "unsafe.zip.", "unsafe.zip ", "unsafe\nzip"] {
            assert!(manifest(Path::new(name), &hash, None).is_err());
        }
        assert!(manifest(Path::new("a.zip"), "short", None).is_err());
    }
}
