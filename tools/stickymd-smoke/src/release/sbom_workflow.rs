//! SBOM sequencing and publication; PowerShell provides download/extraction/Syft adapters.
//! plan_ref: docs/plan/11_testing_and_release.md#release-artifact-authority

use super::{
    cli::{SbomBuildOptions, SbomOptions, SbomPreparationOptions},
    sbom, sbom_preparation, syft,
    temporary::TemporaryDirectory,
    windows,
};
use std::{fs, io::Write, path::Path, time::Duration};

pub(crate) fn generate(
    root: &Path,
    options: &SbomBuildOptions,
    log: &mut dyn Write,
) -> Result<(), String> {
    let directory = std::path::absolute(
        options
            .directory
            .clone()
            .unwrap_or_else(|| root.join("dist")),
    )
    .map_err(|e| e.to_string())?;
    let temporary = TemporaryDirectory::new("sbom")?;
    let staging = temporary.path().join("verified-syft");
    let prepared = sbom_preparation::prepare(
        root,
        &SbomPreparationOptions {
            package_directory: directory.clone(),
            zip: options.zip.clone(),
            syft: options.syft.clone(),
            directory: staging.clone(),
        },
    )?;
    let shell = options.powershell.as_deref().unwrap_or(Path::new("pwsh"));
    let (executable, archive) = if let Some(exe) = &options.syft {
        (std::path::absolute(exe).map_err(|e| e.to_string())?, None)
    } else {
        for download in &prepared.syft.downloads {
            download_pinned(root, shell, temporary.path(), download, log)?;
        }
        let archive = match prepared.verified {
            Some(path) => path,
            None => syft::snapshot(&prepared.syft.archive, &prepared.syft.checksums, &staging)?,
        };
        (temporary.path().join("syft/syft.exe"), Some(archive))
    };
    let context = temporary.path().join("context");
    fs::create_dir(&context).map_err(|e| e.to_string())?;
    for name in ["Cargo.lock", "Cargo.toml"] {
        let bytes = fs::read(root.join(name)).map_err(|e| e.to_string())?;
        crate::atomic_evidence::write_new(&context.join(name), &bytes)?;
    }
    let output = temporary.path().join("generated.spdx.json");
    let mut arguments = vec![
        "-Context".into(),
        context.to_string_lossy().into_owned(),
        "-ZipPath".into(),
        prepared.zip.to_string_lossy().into_owned(),
        "-SyftPath".into(),
        executable.to_string_lossy().into_owned(),
        "-WorkspaceVersion".into(),
        prepared.version,
        "-SyftVersion".into(),
        prepared.syft.version.clone(),
        "-OutputPath".into(),
        output.to_string_lossy().into_owned(),
    ];
    if let Some(archive) = archive {
        arguments.extend([
            "-SyftArchive".into(),
            archive.to_string_lossy().into_owned(),
        ]);
    }
    windows::operation(
        root,
        shell,
        "syft-execute.ps1",
        &arguments.iter().map(String::as_str).collect::<Vec<_>>(),
        log,
    )?;
    sbom::publish_with_output(
        &SbomOptions {
            input: output,
            output: options
                .output
                .clone()
                .unwrap_or_else(|| directory.join("SBOM.spdx.json")),
            zip: prepared.zip,
            checksums: directory.join("SHA256SUMS.txt"),
        },
        log,
    )?;
    temporary.close()?;
    writeln!(log, "SYFT_VERSION={}", prepared.syft.version).map_err(|e| e.to_string())
}

fn download_pinned(
    root: &Path,
    shell: &Path,
    temporary: &Path,
    download: &syft::Download,
    log: &mut dyn Write,
) -> Result<(), String> {
    download_with(
        download,
        temporary,
        |path| {
            windows::operation(
                root,
                shell,
                "syft-fetch.ps1",
                &[
                    "-Uri",
                    &download.uri,
                    "-OutputPath",
                    &path.to_string_lossy(),
                ],
                log,
            )
            .and_then(|()| syft::publish_cached(root, download.kind, path))
        },
        |seconds| std::thread::sleep(Duration::from_secs(seconds)),
    )
}

fn download_with(
    download: &syft::Download,
    temporary: &Path,
    mut attempt_download: impl FnMut(&Path) -> Result<(), String>,
    mut wait: impl FnMut(u64),
) -> Result<(), String> {
    let mut last_failure = String::new();
    for attempt in 1..=syft::DOWNLOAD_ATTEMPTS {
        // Each request owns its staging directory; retry never reuses incomplete bytes.
        let partial = temporary.join(format!(
            "download-{}-{attempt}.partial",
            download.kind.label().replace(' ', "-")
        ));
        let result = attempt_download(&partial);
        // Cleanup failure is itself a failed operation; never leave reusable partial bytes.
        if partial.exists() {
            fs::remove_file(&partial).map_err(|e| format!("cannot remove Syft partial: {e}"))?;
        }
        match result {
            Ok(()) => return Ok(()),
            Err(error) => last_failure = error,
        }
        if attempt < syft::DOWNLOAD_ATTEMPTS {
            wait(u64::from(attempt));
        }
    }
    Err(format!(
        "{} download failed after {} attempts: {last_failure}",
        download.kind.label(),
        syft::DOWNLOAD_ATTEMPTS
    ))
}

#[cfg(all(test, windows))]
mod diagnostics;
#[cfg(test)]
mod tests;
