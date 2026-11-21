//! The packaging crate's boundary (plan 11 #platform-artifact-reuse). The Platform Build Input
//! Fingerprint counts `tools/stickymd-package` and not this verification tool, which is sound
//! only while no release byte can depend on verification code. The crate dependency makes the
//! compiler refuse any reference from the packaging crate to this one; this check covers the edges
//! the compiler cannot see:
//! - the packaging crate's manifest declares no dependency and no build script;
//! - every `include_str!`, `include_bytes!`, `include!` and `#[path]` in it names a plain string
//!   literal that resolves, from its file's directory, to a path inside the repository that the
//!   fingerprint counts for both platforms;
//! - no workspace member's manifest names a build script (Cargo then runs only `build.rs`, which
//!   the fingerprint counts), and no member's `build.rs` names the verification tool;
//! - its code (comments aside) never names the verification tool, its package or its directory,
//!   so it can neither spawn it nor read its files;
//! - the scripts it launches, and the byte-producing scripts and steps of the release workflows
//!   (with every script they dot-source), call the packaging tool and never the verification
//!   tool.
//!
//! The checks are lexical. Code built to evade them (a name assembled at run time, say) is left to
//! review; what they catch is the ordinary way such an edge is written.
//! plan_ref: docs/plan/11_testing_and_release.md#platform-artifact-reuse

use std::path::Path;

const CRATE: &str = "tools/stickymd-package";
const SCRIPTS: &str = "tools/release";
/// The Windows scripts that build release bytes, called by `release` or by other scripts.
const BYTE_SCRIPTS: &[&str] = &[
    "package.ps1",
    "generate-sbom.ps1",
    "generate-third-party-notices.ps1",
    "invoke-package.ps1",
];
/// The steps of each release workflow that build release bytes, by name.
const BYTE_STEPS: &[(&str, &[&str])] = &[
    (
        ".github/workflows/release.yml",
        &["Package", "Generate verified SPDX SBOM and checksums"],
    ),
    (
        ".github/workflows/release-macos.yml",
        &[
            "Build the ad-hoc signed bundle",
            "Package",
            "Generate verified SPDX SBOM and checksums",
        ],
    ),
];
/// How code names the verification tool: its package, its crate and its directory.
const TOOL_NAMES: &[&str] = &[
    "\"stickymd-smoke\"",
    "-p stickymd-smoke",
    "stickymd_smoke",
    "stickymd-smoke/",
];

pub(super) fn verify(root: &Path) -> Result<(), String> {
    let manifest_path = format!("{CRATE}/Cargo.toml");
    check_manifest(&read(root, &manifest_path)?).map_err(|e| format!("{manifest_path}: {e}"))?;
    if root.join(CRATE).join("build.rs").exists() {
        return Err(format!(
            "{CRATE}/build.rs: the packaging crate has no build script"
        ));
    }
    let mut sources = Vec::new();
    collect_rust(&root.join(CRATE), &mut sources)?;
    if sources.is_empty() {
        return Err(format!(
            "{CRATE} has no Rust sources; a check of nothing proves nothing"
        ));
    }
    let mut launched = Vec::new();
    for path in &sources {
        let relative = path
            .strip_prefix(root)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .replace('\\', "/");
        let text = std::fs::read_to_string(path).map_err(|e| format!("{relative}: {e}"))?;
        check_source(&relative, &text).map_err(|e| format!("{relative}: {e}"))?;
        launched.extend(scripts_named(&text));
    }
    let mut scripts: Vec<String> = BYTE_SCRIPTS.iter().map(|name| (*name).to_owned()).collect();
    scripts.extend(launched);
    let mut checked = std::collections::BTreeSet::new();
    while let Some(name) = scripts.pop() {
        if !checked.insert(name.clone()) {
            continue;
        }
        let relative = format!("{SCRIPTS}/{name}");
        let text = read(root, &relative)?;
        check_script(&text).map_err(|e| format!("{relative}: {e}"))?;
        scripts.extend(dot_sourced(&text));
    }
    check_build_scripts(root)?;
    let build_app = "apps/stickymd-mac/packaging/build-app.sh";
    check_script(&read(root, build_app)?).map_err(|e| format!("{build_app}: {e}"))?;
    for (workflow, steps) in BYTE_STEPS {
        let text = read(root, workflow)?;
        for step in *steps {
            let body = step_body(&text, step)
                .ok_or_else(|| format!("{workflow} has no step named `{step}`"))?;
            check_script(body).map_err(|e| format!("{workflow} step `{step}`: {e}"))?;
        }
    }
    Ok(())
}

fn read(root: &Path, relative: &str) -> Result<String, String> {
    std::fs::read_to_string(root.join(relative)).map_err(|e| format!("{relative}: {e}"))
}

fn collect_rust(directory: &Path, into: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    for entry in
        std::fs::read_dir(directory).map_err(|e| format!("{}: {e}", directory.display()))?
    {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.is_dir() {
            collect_rust(&path, into)?;
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            into.push(path);
        }
    }
    into.sort();
    Ok(())
}

/// Every table whose name ends in `dependencies` (plain, dev, build, or per target) is empty, and
/// `[package]` names no build script.
fn check_manifest(text: &str) -> Result<(), String> {
    let mut table = String::new();
    for (index, line) in text.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if let Some(name) = line.strip_prefix('[') {
            table = name
                .trim_matches(|c| c == '[' || c == ']')
                .trim()
                .to_owned();
            if table.ends_with("dependencies") || table.contains("dependencies.") {
                // An inline table header such as `[dependencies.x]` declares a dependency itself.
                if table.contains("dependencies.") {
                    return Err(format!("line {}: declares a dependency", index + 1));
                }
            }
            if table == "patch" || table.starts_with("patch.") || table == "replace" {
                return Err(format!("line {}: overrides a dependency", index + 1));
            }
            continue;
        }
        if table.ends_with("dependencies") {
            return Err(format!(
                "line {}: declares a dependency; the packaging crate is std-only",
                index + 1
            ));
        }
        if table == "package"
            && line
                .split('=')
                .next()
                .is_some_and(|key| key.trim() == "build")
        {
            return Err(format!("line {}: names a build script", index + 1));
        }
    }
    Ok(())
}

/// The code of `text` with `//` comments removed (a `//` inside a string literal ends the line
/// early, which can only hide code from the name check, never add a false include).
fn code_lines(text: &str) -> impl Iterator<Item = (usize, &str)> {
    text.lines().enumerate().map(|(index, line)| {
        let code = line.find("//").map_or(line, |at| &line[..at]);
        (index + 1, code)
    })
}

/// Names of the verification tool are refused in code; `include_*!` and `#[path]` must name a
/// plain literal that resolves, from the directory of `file` (repository-relative), to a path the
/// fingerprint counts for both platforms.
fn check_source(file: &str, text: &str) -> Result<(), String> {
    check_names(text)?;
    let directory = file.rsplit_once('/').map_or("", |(directory, _)| directory);
    for (number, line) in text.lines().enumerate() {
        for marker in ["include_str!", "include_bytes!", "include!", "#[path"] {
            let mut rest = line;
            while let Some(at) = rest.find(marker) {
                let after = &rest[at + marker.len()..];
                let argument = after
                    .trim_start_matches(['(', ' ', '='])
                    .strip_prefix('"')
                    .and_then(|value| value.split_once('"'))
                    .map(|(value, _)| value)
                    .ok_or_else(|| {
                        format!(
                            "line {}: `{marker}` takes a plain string literal here",
                            number + 1
                        )
                    })?;
                let resolved = resolve(directory, argument).filter(|path| {
                    use crate::release::platform::Platform;
                    use crate::release::platform_inputs::is_input;
                    is_input(Platform::WindowsX64, path) && is_input(Platform::MacOsArm64, path)
                });
                if resolved.is_none() {
                    return Err(format!(
                        "line {}: `{marker}` reaches `{argument}`, which is not a build input of both platforms",
                        number + 1
                    ));
                }
                rest = after;
            }
        }
    }
    Ok(())
}

/// Refuses code (comments aside) that names the verification tool.
fn check_names(text: &str) -> Result<(), String> {
    for (number, line) in code_lines(text) {
        for name in TOOL_NAMES {
            if line.contains(name) {
                return Err(format!(
                    "line {number} names the verification tool (`{name}`); release bytes may not depend on it"
                ));
            }
        }
    }
    Ok(())
}

/// `relative` joined to the repository directory `directory` and normalized, or `None` when it
/// is absolute, uses `\\`, or climbs out of the repository.
fn resolve(directory: &str, relative: &str) -> Option<String> {
    if relative.starts_with('/') || relative.contains('\\') || relative.contains(':') {
        return None;
    }
    let mut parts: Vec<&str> = directory
        .split('/')
        .filter(|part| !part.is_empty())
        .collect();
    for part in relative.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    (!parts.is_empty()).then(|| parts.join("/"))
}

/// No workspace member's manifest names a build script, so Cargo runs only an auto-discovered
/// `build.rs`, which the fingerprint counts; and no member's `build.rs` names the verification
/// tool, so a build script cannot read its files.
fn check_build_scripts(root: &Path) -> Result<(), String> {
    for module in crate::headless::Module::ALL {
        if module.package().is_none() {
            continue;
        }
        let manifest = format!("{}Cargo.toml", module.root());
        let mut table = String::new();
        for (index, line) in read(root, &manifest)?.lines().enumerate() {
            let line = line.split('#').next().unwrap_or("").trim();
            if let Some(name) = line.strip_prefix('[') {
                table = name
                    .trim_matches(|c| c == '[' || c == ']')
                    .trim()
                    .to_owned();
            } else if table == "package"
                && line
                    .split('=')
                    .next()
                    .is_some_and(|key| key.trim() == "build")
            {
                return Err(format!(
                    "{manifest}:{}: names a build script; only an auto-discovered build.rs is counted",
                    index + 1
                ));
            }
        }
        let script = format!("{}build.rs", module.root());
        if root.join(&script).is_file() {
            check_names(&read(root, &script)?).map_err(|e| format!("{script}: {e}"))?;
        }
    }
    Ok(())
}

/// The PowerShell script names the code passes as string literals.
fn scripts_named(text: &str) -> Vec<String> {
    let mut names = Vec::new();
    for (_, line) in code_lines(text) {
        for piece in line.split('"').skip(1).step_by(2) {
            if piece.ends_with(".ps1")
                && !piece.is_empty()
                && piece
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            {
                names.push(piece.to_owned());
            }
        }
    }
    names
}

/// The scripts `text` dot-sources from its own directory.
fn dot_sourced(text: &str) -> Vec<String> {
    text.lines()
        .filter(|line| line.trim_start().starts_with(". "))
        .filter_map(|line| {
            let quoted = line.split('\'').nth(1)?;
            quoted.ends_with(".ps1").then(|| quoted.to_owned())
        })
        .collect()
}

fn check_script(text: &str) -> Result<(), String> {
    for (index, line) in text.lines().enumerate() {
        let code = line.trim_start();
        if code.starts_with('#') {
            continue;
        }
        for name in [
            "stickymd-smoke",
            "invoke-smoke.ps1",
            "Invoke-StickyMdReleaseTool",
        ] {
            if code.contains(name) {
                return Err(format!(
                    "line {} calls the verification tool (`{name}`); building release bytes calls stickymd-package",
                    index + 1
                ));
            }
        }
    }
    Ok(())
}

/// The lines of the workflow step named `name`: from its `- name:` line up to the next step.
fn step_body<'a>(workflow: &'a str, name: &str) -> Option<&'a str> {
    let header = format!("- name: {name}\n");
    let start = workflow.find(&header)?;
    let indent = workflow[..start].rsplit('\n').next()?.len();
    let body_start = start + header.len();
    let rest = &workflow[body_start..];
    let mut end = rest.len();
    let mut offset = 0;
    for line in rest.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let line_indent = line.len() - trimmed.len();
        if !trimmed.trim().is_empty() && line_indent <= indent {
            end = offset;
            break;
        }
        offset += line.len();
    }
    Some(&rest[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_real_packaging_crate_keeps_its_boundary() {
        let root =
            crate::governance::find_repository_root(&std::env::current_dir().unwrap()).unwrap();
        verify(&root).unwrap();
    }

    #[test]
    fn any_dependency_or_build_script_in_the_manifest_is_refused() {
        let clean = "[package]\nname = \"stickymd-package\"\n\n[dependencies]\n# none\n\n[lib]\npath = \"src/lib.rs\"\n";
        check_manifest(clean).unwrap();
        for bad in [
            "[dependencies]\nstickymd-smoke = { path = \"../stickymd-smoke\" }\n",
            "[dev-dependencies]\nx = \"1\"\n",
            "[build-dependencies]\nx = \"1\"\n",
            "[target.'cfg(windows)'.dependencies]\nx = \"1\"\n",
            "[dependencies.stickymd-smoke]\npath = \"../stickymd-smoke\"\n",
            "[patch.crates-io]\nx = { path = \"x\" }\n",
            "[package]\nname = \"x\"\nbuild = \"build.rs\"\n",
        ] {
            assert!(check_manifest(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn includes_must_reach_counted_inputs_and_the_tool_is_never_named() {
        let file = "tools/stickymd-package/src/package_content.rs";
        check_source(
            file,
            "const README: &str = include_str!(\"package_readme.txt\");\n",
        )
        .unwrap();
        // A counted shared file outside the crate is fine.
        check_source(
            "tools/stickymd-package/src/notices/licenses.rs",
            "include_str!(\"../../../../assets/licenses/SIL-OFL-1.1.txt\")",
        )
        .unwrap();
        check_source(
            file,
            "// stickymd-smoke calls this crate; a comment may say so\n",
        )
        .unwrap();
        check_source(
            file,
            "Err(\"belongs to stickymd-smoke release verify-package\")\n",
        )
        .unwrap();
        for bad in [
            "include_str!(\"../../stickymd-smoke/src/x.txt\")",
            "include_bytes!(\"../../../docs/plan/11_testing_and_release.md\")",
            "include_bytes!(\"/etc/x\")",
            "include_str!(\"../../../../../outside.txt\")",
            "include!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"/x.rs\"))",
            "#[path = \"../../stickymd-smoke/src/release/package.rs\"]\nmod x;",
            "#[path = \"sub\\\\..\\\\..\\\\x.rs\"]\nmod x;",
            "Command::new(\"cargo\").args([\"run\", \"-p\", \"stickymd-smoke\"])",
            "let x = \"cargo run -p stickymd-smoke -- release\";",
            "use stickymd_smoke::release;",
            // A helper that reads the verification tool's fixtures at run time.
            "fs::read(root.join(\"tools/stickymd-smoke/tests/fixtures/notice.txt\"))",
        ] {
            assert!(check_source(file, bad).is_err(), "{bad}");
        }
        assert_eq!(resolve("a/b", "../c/./d.txt").as_deref(), Some("a/c/d.txt"));
        assert_eq!(resolve("a", "../../x"), None);
    }

    /// A build script Cargo discovers by itself runs in the Windows workspace build: one that
    /// names the verification tool is refused here, and one added anywhere is counted by the
    /// fingerprint (`platform_inputs` tests).
    #[test]
    fn a_build_script_may_not_name_the_verification_tool() {
        check_names("fn main() { println!(\"cargo:rerun-if-changed=app.manifest\"); }").unwrap();
        assert!(
            check_names("fn main() { let _ = std::fs::read(\"../../tools/stickymd-smoke/x\"); }")
                .is_err()
        );
    }

    #[test]
    fn scripts_and_steps_that_build_bytes_never_call_the_verification_tool() {
        check_script("# invoke-smoke.ps1 is for verification\n. (Join-Path $PSScriptRoot 'invoke-package.ps1')\n").unwrap();
        for bad in [
            ". (Join-Path $PSScriptRoot 'invoke-smoke.ps1')\n",
            "Invoke-StickyMdReleaseTool -RepoRoot $repoRoot -Arguments @('notices')\n",
            "cargo run --quiet -p stickymd-smoke --locked -- release macos-package\n",
        ] {
            assert!(check_script(bad).is_err(), "{bad}");
        }
        assert_eq!(
            dot_sourced(". (Join-Path $PSScriptRoot 'invoke-package.ps1')\n$x = 1\n"),
            ["invoke-package.ps1"]
        );
        assert_eq!(
            scripts_named("windows::operation(root, shell, \"package-archive.ps1\", &[])"),
            ["package-archive.ps1"]
        );
        let workflow = "    steps:\n      - name: Package\n        run: |\n          cargo run -p stickymd-smoke -- release macos-package\n      - name: Verify package\n        run: cargo run -p stickymd-smoke -- release macos-verify-package\n";
        let body = step_body(workflow, "Package").unwrap();
        assert!(body.contains("macos-package") && !body.contains("verify-package"));
        assert!(check_script(body).is_err());
        assert!(step_body(workflow, "Missing").is_none());
    }
}
