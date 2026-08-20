//! Repository-contract validation used by every phase smoke.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::cli::{Phase, Selection};

const REQUIRED_FILES: &[&str] = &[
    "AGENTS.md",
    "Cargo.toml",
    "Cargo.lock",
    "LICENSE",
    "THIRD_PARTY_NOTICES.md",
    ".github/workflows/ci.yml",
    "docs/AGENTS.md",
    "docs/coverage-matrix.md",
    "docs/features/00_v1_product_behavior.md",
    "docs/acceptance-cases/00_v1_acceptance.md",
    "docs/plan/00_engineering_constitution.md",
    "docs/plan/01_terminology.md",
    "docs/plan/02_positioning_and_scope.md",
    "docs/plan/03_system_architecture.md",
    "docs/plan/04_runtime_state_model.md",
    "docs/plan/05_document_persistence.md",
    "docs/plan/06_markdown_math_rendering.md",
    "docs/plan/07_editor_and_ime.md",
    "docs/plan/08_assets_and_export.md",
    "docs/plan/09_windows_shell.md",
    "docs/plan/10_performance_reliability.md",
    "docs/plan/11_testing_and_release.md",
    "tools/stickymd-smoke/Cargo.toml",
    "assets/licenses/SIL-OFL-1.1.txt",
    "assets/licenses/KaTeX-fonts-NOTICE.txt",
];

const FORBIDDEN_PACKAGES: &[&str] = &[
    "async-std",
    "cef",
    "curl",
    "egui",
    "gtk",
    "html5ever",
    "hyper",
    "iced",
    "onig",
    "onig_sys",
    "reqwest",
    "ratex-cairo",
    "ratex-ffi",
    "ratex-gtk4",
    "ratex-pdf",
    "ratex-render",
    "ratex-svg",
    "ratex-wasm",
    "rusqlite",
    "slint",
    "syntect",
    "tauri",
    "tokio",
    "ureq",
    "wgpu",
    "webview2",
    "wry",
];

#[derive(Debug)]
struct MatrixRow {
    line: usize,
    mode: String,
    status: String,
}

pub(crate) fn find_repository_root(start: &Path) -> Result<PathBuf, String> {
    for candidate in start.ancestors() {
        if candidate.join("Cargo.toml").is_file() && candidate.join("AGENTS.md").is_file() {
            return Ok(candidate.to_path_buf());
        }
    }
    Err(format!(
        "cannot locate repository root above {}",
        start.display()
    ))
}

pub(crate) fn verify(root: &Path) -> Result<(), String> {
    verify_required_files(root)?;
    verify_global_acceptance_sequence(root)?;
    verify_phase_artifacts(root)?;
    verify_plan_refs(root)?;
    verify_local_markdown_links(root)?;
    verify_forbidden_packages(root)?;
    Ok(())
}

pub(crate) fn verify_ready_status(root: &Path, selection: Selection) -> Result<(), String> {
    let phases: Vec<_> = match selection {
        Selection::Phase(phase) => vec![phase],
        Selection::All => Phase::ALL.to_vec(),
    };
    for phase in phases {
        let path = matrix_path(root, phase);
        for row in read_matrix_rows(&path)? {
            if row.mode == "Automated" && row.status != "AUTOMATED PASS" {
                return Err(format!(
                    "{}:{} automated row is `{}` instead of `AUTOMATED PASS`",
                    path.display(),
                    row.line,
                    row.status
                ));
            }
        }
    }
    Ok(())
}

fn verify_required_files(root: &Path) -> Result<(), String> {
    for relative in REQUIRED_FILES {
        if !root.join(relative).is_file() {
            return Err(format!("required file is missing: {relative}"));
        }
    }
    Ok(())
}

fn verify_global_acceptance_sequence(root: &Path) -> Result<(), String> {
    let path = root.join("docs/acceptance-cases/00_v1_acceptance.md");
    let content = read_text(&path)?;
    let mut observed = Vec::new();
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("## AC-") {
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let number = digits
                .parse::<u8>()
                .map_err(|error| format!("invalid acceptance heading `{line}`: {error}"))?;
            observed.push(number);
        }
    }
    let expected: Vec<u8> = (1..=30).collect();
    if observed != expected {
        return Err(format!(
            "global acceptance IDs must be exactly AC-001..AC-030; observed {observed:?}"
        ));
    }
    Ok(())
}

fn verify_phase_artifacts(root: &Path) -> Result<(), String> {
    for phase in Phase::ALL {
        let matrix = matrix_path(root, phase);
        if !matrix.is_file() {
            return Err(format!("phase matrix is missing: {}", matrix.display()));
        }
        let rows = read_matrix_rows(&matrix)?;
        if rows.is_empty() || !rows.iter().any(|row| row.mode == "Automated") {
            return Err(format!(
                "{} must contain at least one automated row",
                matrix.display()
            ));
        }

        let script = root.join(format!("tools/smoke/phase-{}.ps1", phase.number()));
        let content = read_text(&script)?;
        for needle in [
            "stickymd-smoke",
            "'phase'",
            &format!("'{}'", phase.number()),
        ] {
            if !content.contains(needle) {
                return Err(format!(
                    "{} does not route through Rust CLI token `{needle}`",
                    script.display()
                ));
            }
        }
    }
    let all_script = root.join("tools/smoke/all.ps1");
    let content = read_text(&all_script)?;
    if !content.contains("stickymd-smoke") || !content.contains("'all'") {
        return Err(format!(
            "{} must route through `stickymd-smoke all`",
            all_script.display()
        ));
    }
    Ok(())
}

fn matrix_path(root: &Path, phase: Phase) -> PathBuf {
    root.join(format!("docs/acceptance-cases/phase-{}.md", phase.number()))
}

fn read_matrix_rows(path: &Path) -> Result<Vec<MatrixRow>, String> {
    let content = read_text(path)?;
    let mut rows = Vec::new();
    for (index, line) in content.lines().enumerate() {
        if !line.trim_start().starts_with("| P") {
            continue;
        }
        let cells: Vec<_> = line
            .trim()
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() != 5 {
            return Err(format!(
                "{}:{} phase matrix row must contain 5 columns",
                path.display(),
                index + 1
            ));
        }
        let mode = cells[2];
        let evidence = cells[3];
        let status = cells[4];
        match mode {
            "Automated" if matches!(status, "AUTOMATED PASS" | "BLOCKED") => {}
            "Manual" if matches!(status, "MANUAL PASS" | "NOT TESTED" | "BLOCKED") => {}
            _ => {
                return Err(format!(
                    "{}:{} invalid mode/status pair `{mode}` / `{status}`",
                    path.display(),
                    index + 1
                ));
            }
        }
        if evidence.is_empty() {
            return Err(format!(
                "{}:{} evidence cell must not be empty",
                path.display(),
                index + 1
            ));
        }
        if status == "MANUAL PASS" && !evidence.contains("receipt:") {
            return Err(format!(
                "{}:{} MANUAL PASS requires a checked-in `receipt:` reference",
                path.display(),
                index + 1
            ));
        }
        rows.push(MatrixRow {
            line: index + 1,
            mode: mode.to_owned(),
            status: status.to_owned(),
        });
    }
    Ok(rows)
}

fn verify_plan_refs(root: &Path) -> Result<(), String> {
    let mut rust_files = Vec::new();
    collect_files(&root.join("apps"), "rs", &mut rust_files)?;
    collect_files(&root.join("crates"), "rs", &mut rust_files)?;
    let mut count = 0usize;
    for rust_file in rust_files {
        let content = read_text(&rust_file)?;
        for (index, line) in content.lines().enumerate() {
            let Some(reference) = line.trim().strip_prefix("//! plan_ref: ") else {
                continue;
            };
            count += 1;
            let (relative, anchor) = reference.split_once('#').ok_or_else(|| {
                format!(
                    "{}:{} plan_ref has no stable anchor",
                    rust_file.display(),
                    index + 1
                )
            })?;
            if !relative.starts_with("docs/plan/") {
                return Err(format!(
                    "{}:{} plan_ref is outside docs/plan",
                    rust_file.display(),
                    index + 1
                ));
            }
            let plan_path = root.join(relative);
            let plan = read_text(&plan_path)?;
            let marker = format!("<a id=\"{anchor}\"></a>");
            if !plan.contains(&marker) {
                return Err(format!(
                    "{}:{} missing stable anchor `{marker}` in {}",
                    rust_file.display(),
                    index + 1,
                    plan_path.display()
                ));
            }
        }
    }
    if count < 30 {
        return Err(format!(
            "expected at least 30 production plan_ref declarations, found {count}"
        ));
    }
    Ok(())
}

fn verify_local_markdown_links(root: &Path) -> Result<(), String> {
    let mut files = vec![root.join("README.md"), root.join("AGENTS.md")];
    collect_files(&root.join("docs"), "md", &mut files)?;
    for file in files {
        if file.starts_with(root.join("docs/phases")) {
            continue;
        }
        let content = read_text(&file)?;
        for (line, destination) in markdown_links(&content) {
            if is_external_or_anchor(&destination) {
                continue;
            }
            let without_fragment = destination
                .split_once('#')
                .map_or(destination.as_str(), |(path, _)| path)
                .trim();
            if without_fragment.is_empty() {
                continue;
            }
            let target = file
                .parent()
                .ok_or_else(|| format!("{} has no parent", file.display()))?
                .join(without_fragment.replace('/', std::path::MAIN_SEPARATOR_STR));
            if !target.exists() {
                return Err(format!(
                    "{}:{line} local Markdown link does not resolve: {destination}",
                    file.display()
                ));
            }
        }
    }
    Ok(())
}

fn markdown_links(content: &str) -> Vec<(usize, String)> {
    let mut links = Vec::new();
    let mut offset = 0usize;
    while let Some(relative_close) = content[offset..].find("](") {
        let close = offset + relative_close;
        let Some(relative_end) = content[close + 2..].find(')') else {
            break;
        };
        let end = close + 2 + relative_end;
        let open = content[..close].rfind('[');
        let is_image = open.is_some_and(|open| open > 0 && content.as_bytes()[open - 1] == b'!');
        if !is_image {
            let raw = content[close + 2..end].trim();
            let destination = raw.trim_matches(['<', '>']).to_owned();
            let line = content[..close]
                .bytes()
                .filter(|byte| *byte == b'\n')
                .count()
                + 1;
            links.push((line, destination));
        }
        offset = end + 1;
    }
    links
}

fn is_external_or_anchor(destination: &str) -> bool {
    let lower = destination.to_ascii_lowercase();
    destination.starts_with('#')
        || destination.starts_with('/')
        || destination.get(1..3) == Some(":/")
        || lower.starts_with("http://")
        || lower.starts_with("https://")
        || lower.starts_with("mailto:")
        || lower.starts_with("file:")
}

fn verify_forbidden_packages(root: &Path) -> Result<(), String> {
    let lock = read_text(&root.join("Cargo.lock"))?;
    let forbidden: BTreeSet<_> = FORBIDDEN_PACKAGES.iter().copied().collect();
    for line in lock.lines() {
        let trimmed = line.trim();
        let Some(name) = trimmed
            .strip_prefix("name = \"")
            .and_then(|value| value.strip_suffix('"'))
        else {
            continue;
        };
        if forbidden.contains(name) {
            return Err(format!("forbidden package appears in Cargo.lock: {name}"));
        }
    }
    Ok(())
}

fn collect_files(
    directory: &Path,
    extension: &str,
    output: &mut Vec<PathBuf>,
) -> Result<(), String> {
    if !directory.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(directory)
        .map_err(|error| format!("cannot read {}: {error}", directory.display()))?
    {
        let entry = entry.map_err(|error| format!("cannot read directory entry: {error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("cannot inspect {}: {error}", entry.path().display()))?;
        if file_type.is_dir() {
            if entry.file_name() != "target" {
                collect_files(&entry.path(), extension, output)?;
            }
        } else if file_type.is_file()
            && entry.path().extension().and_then(|value| value.to_str()) == Some(extension)
        {
            output.push(entry.path());
        }
    }
    Ok(())
}

fn read_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("cannot read {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::markdown_links;

    #[test]
    fn markdown_link_parser_ignores_images() {
        let links = markdown_links("[plan](../plan/a.md#x) ![image](missing.png)");
        assert_eq!(links, vec![(1, "../plan/a.md#x".to_owned())]);
    }

    #[test]
    fn markdown_link_parser_reports_line_numbers() {
        let links = markdown_links("first\n[second](b.md)\n");
        assert_eq!(links, vec![(2, "b.md".to_owned())]);
    }
}
