//! Local packaging coordinator: Rust rules, one archive adapter, no recursive CLI launch.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use super::{
    cli::{PackageBuildOptions, PackagePublishOptions},
    package_content, package_inputs, package_publish, package_staging,
    temporary::TemporaryDirectory,
    windows,
};
use crate::atomic_evidence;
use std::{fs, io::Write, path::Path};

pub(crate) fn build(
    root: &Path,
    options: &PackageBuildOptions,
    log: &mut dyn Write,
) -> Result<(), String> {
    let exe = options
        .exe
        .clone()
        .unwrap_or_else(|| root.join("target/release/stickymd-win.exe"));
    if !exe.is_file() {
        return Err(format!(
            "Release executable does not exist: {}",
            exe.display()
        ));
    }
    let directory = std::path::absolute(
        options
            .directory
            .clone()
            .unwrap_or_else(|| root.join("dist")),
    )
    .map_err(|e| e.to_string())?;
    let inputs = package_inputs::resolve(root, &options.inputs)?;
    let temporary = TemporaryDirectory::new("package")?;
    let staging = temporary.path().join("contents");
    let diagnostics = package_staging::stage(root, &exe, &staging, &inputs)?;
    log.write_all(diagnostics.as_bytes())
        .map_err(|e| e.to_string())?;
    fs::create_dir_all(&directory).map_err(|e| e.to_string())?;
    let members = package_content::ordered_members()
        .iter()
        .map(|m| format!("\"{}\"", m.name))
        .collect::<Vec<_>>()
        .join(",");
    let inventory = temporary.path().join("members.json");
    atomic_evidence::write_new(&inventory, format!("[{members}]").as_bytes())?;
    let archive = temporary.path().join("completed.zip");
    let text = |path: &Path| path.to_string_lossy().into_owned();
    windows::operation(
        root,
        options.powershell.as_deref().unwrap_or(Path::new("pwsh")),
        "package-archive.ps1",
        &[
            "-StagingDirectory",
            &text(&staging),
            "-Inventory",
            &text(&inventory),
            "-OutputPath",
            &text(&archive),
        ],
        log,
    )?;
    let output = directory.join(&inputs.archive);
    let hash = package_publish::publish(&PackagePublishOptions {
        input: archive,
        output: output.clone(),
        checksums: directory.join("SHA256SUMS.txt"),
    })?;
    temporary.close()?;
    writeln!(
        log,
        "PACKAGE_PATH={}\nPACKAGE_SHA256={hash}\nSOURCE_TREE_STATE={}",
        output.display(),
        inputs.state
    )
    .map_err(|e| e.to_string())
}
