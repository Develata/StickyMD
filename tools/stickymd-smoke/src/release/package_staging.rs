//! Prepare only new, privately owned package contents for the ZIP adapter.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use super::{
    cli::PackageInputOptions,
    notices,
    package_content::{self, Content, ordered_members},
    package_inputs,
    temporary::TemporaryDirectory,
};
use crate::{atomic_evidence, evidence::escape_json};
use std::{fs, path::Path};

pub(super) fn prepare(
    root: &Path,
    options: &PackageInputOptions,
    exe: &Path,
    directory: &Path,
) -> Result<(), String> {
    let inputs = package_inputs::resolve(root, options)?;
    let diagnostics = stage(root, exe, directory, &inputs)?;
    let members = ordered_members()
        .iter()
        .map(|member| format!("\"{}\"", member.name))
        .collect::<Vec<_>>()
        .join(",");
    let diagnostics = diagnostics
        .lines()
        .map(|line| format!("\"{}\"", escape_json(line)))
        .collect::<Vec<_>>()
        .join(",");
    println!(
        "{{\"inputs\":{},\"members\":[{members}],\"diagnostics\":[{diagnostics}]}}",
        inputs.json()
    );
    Ok(())
}

pub(super) fn stage(
    root: &Path,
    exe: &Path,
    directory: &Path,
    inputs: &package_inputs::Inputs,
) -> Result<String, String> {
    let directory = std::path::absolute(directory).map_err(|error| error.to_string())?;
    // CreateNew ownership: never overwrite, empty or recursively clean a caller's directory.
    let staging = TemporaryDirectory::create(directory)?;
    let mut diagnostics = Vec::new();
    for member in ordered_members() {
        let path = staging.path().join(member.name);
        fs::create_dir_all(path.parent().unwrap()).map_err(|error| error.to_string())?;
        let bytes = match member.content {
            Content::Executable => {
                fs::read(exe).map_err(|error| format!("cannot read release executable: {error}"))?
            }
            Content::Readme => package_content::readme(inputs)?.into_bytes(),
            Content::License(source) => {
                notices::normalized_license(&root.join(source))?.into_bytes()
            }
            Content::Notices => {
                notices::generate_with_output(root, &path, &mut diagnostics)?;
                continue;
            }
        };
        atomic_evidence::write_new(&path, &bytes)?;
    }
    let diagnostics = String::from_utf8(diagnostics).map_err(|error| error.to_string())?;
    staging.keep();
    Ok(diagnostics)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_license_normalization_preserves_bom_text_and_rejects_invalid_encoding() {
        let scratch = TemporaryDirectory::new("package-license").unwrap();
        let path = scratch.path().join("许可 space.txt");
        fs::write(&path, b"\xef\xbb\xbflicense\r\nnotice\rfinal\n").unwrap();
        assert_eq!(
            notices::normalized_license(&path).unwrap(),
            "license\nnotice\nfinal\n"
        );
        fs::write(&path, [0xff, 0xfe, 0x2d, 0x4e, 13, 0, 10, 0]).unwrap();
        assert_eq!(notices::normalized_license(&path).unwrap(), "中\n");
        fs::write(&path, [0xff, 0xfe, 0]).unwrap();
        assert!(notices::normalized_license(&path).is_err());
    }

    #[test]
    fn occupied_staging_and_missing_inputs_never_overwrite_or_leave_partial_contents() {
        let scratch = TemporaryDirectory::new("staging-test").unwrap();
        let inputs = package_inputs::Inputs {
            version: "0.1.1".into(),
            commit: "a".repeat(40),
            archive: String::new(),
            state: "DIRTY_VALIDATION",
        };
        let occupied = scratch.path().join("中文 space");
        fs::create_dir(&occupied).unwrap();
        fs::write(occupied.join("keep"), "user content").unwrap();
        assert!(
            stage(
                scratch.path(),
                &scratch.path().join("missing.exe"),
                &occupied,
                &inputs
            )
            .is_err()
        );
        assert_eq!(
            fs::read_to_string(occupied.join("keep")).unwrap(),
            "user content"
        );
        let output = scratch.path().join("new staging");
        let error = stage(
            scratch.path(),
            &scratch.path().join("missing.exe"),
            &output,
            &inputs,
        )
        .unwrap_err();
        assert!(error.contains("LICENSE"));
        assert!(!output.exists());
        fs::write(scratch.path().join("LICENSE"), "license\r\n").unwrap();
        let error = stage(
            scratch.path(),
            &scratch.path().join("missing.exe"),
            &output,
            &inputs,
        )
        .unwrap_err();
        assert!(error.contains("release executable"));
        assert!(!output.exists());
    }
}
