//! Stable notices from the locked Windows normal dependency graph.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-verification-harness

mod graph;
mod licenses;
#[cfg(test)]
mod tests;

use crate::{atomic_evidence, integrity, repository};
use std::{fmt::Write, path::Path};

pub(super) fn generate(root: &Path, destination: &Path) -> Result<(), String> {
    let metadata = repository::command_text(
        root,
        "cargo",
        &[
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--filter-platform",
            "x86_64-pc-windows-msvc",
        ],
    )?;
    let (contents, count) = render(root, &metadata)?;
    let destination = std::path::absolute(destination).map_err(|error| error.to_string())?;
    atomic_evidence::write_new(&destination, contents.as_bytes())?;
    println!(
        "THIRD_PARTY_NOTICES={}\nRUNTIME_DEPENDENCY_COUNT={count}",
        destination.display()
    );
    Ok(())
}

fn render(root: &Path, metadata: &str) -> Result<(String, usize), String> {
    let packages = graph::runtime_packages(metadata)?;
    let mut output = licenses::text(&root.join("THIRD_PARTY_NOTICES.md"))?
        .trim_end()
        .replace("\r\n", "\n");
    output.push_str("\n\n## Generated Rust Runtime Dependency Notices\n\nThis section is generated from the Cargo.lock-resolved normal dependency graph for\nstickymd-win on x86_64-pc-windows-msvc. Build-only and development-only packages are excluded.\n");
    writeln!(
        output,
        "Cargo.lock SHA-256: {}",
        integrity::sha256(&root.join("Cargo.lock"))?
    )
    .unwrap();
    writeln!(output, "Runtime registry packages: {}", packages.len()).unwrap();
    for package in &packages {
        let repository = package
            .repository
            .as_deref()
            .filter(|value| !value.is_empty())
            .or(package
                .homepage
                .as_deref()
                .filter(|value| !value.is_empty()))
            .map(str::to_owned)
            .unwrap_or_else(|| {
                format!(
                    "https://crates.io/crates/{}/{}",
                    package.name, package.version
                )
            });
        writeln!(output, "\n================================================================================\nPACKAGE: {} {}\nDECLARED LICENSE: {}\nSOURCE: {repository}", package.name, package.version, package.license.as_deref().unwrap_or_default()).unwrap();
        for license in licenses::select(root, package)? {
            let text = licenses::text(&license)?.replace("\r\n", "\n");
            writeln!(output, "LICENSE FILE: {}\n--------------------------------------------------------------------------------\n{}", license.file_name().and_then(|name| name.to_str()).ok_or("license filename is not Unicode")?, text.trim_end()).unwrap();
        }
    }
    Ok((output, packages.len()))
}
