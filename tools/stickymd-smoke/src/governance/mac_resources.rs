//! Confine the new smoke measurement FFI to one file; host permissions are unchanged.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use std::path::Path;

pub(super) fn verify(root: &Path) -> Result<(), String> {
    let directory = root.join("tools/stickymd-smoke/src/mac_resources");
    let mut files = Vec::new();
    super::collect_files(&directory, "rs", &mut files)?;
    if !files
        .iter()
        .any(|path| path == &directory.join("native.rs"))
    {
        return Err("mac-resources native adapter is missing".into());
    }
    for path in files {
        check(
            path.file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default(),
            &super::read_text(&path)?,
        )
        .map_err(|error| format!("{}: {error}", path.display()))?;
    }
    let entry = super::read_text(&directory.join("mod.rs"))?;
    if !entry.contains("#[cfg(target_os = \"macos\")]\nmod native;") {
        return Err("mac-resources native adapter must remain macOS-only".into());
    }
    Ok(())
}

fn check(name: &str, source: &str) -> Result<(), String> {
    if !source
        .lines()
        .any(|line| line.starts_with("//! plan_ref: docs/plan/"))
    {
        return Err("missing plan_ref".into());
    }
    if name == "native.rs" {
        if let Some(line) = super::unsafe_block_without_safety(source) {
            return Err(format!("line {line}: unsafe block needs SAFETY invariant"));
        }
        for line in source
            .lines()
            .filter(|line| line.trim().starts_with("#[link("))
        {
            if ![
                "#[link(name = \"System\")]",
                "#[link(name = \"CoreGraphics\", kind = \"framework\")]",
                "#[link(name = \"CoreFoundation\", kind = \"framework\")]",
            ]
            .contains(&line.trim())
            {
                return Err("only libSystem, CoreGraphics and CoreFoundation are allowed".into());
            }
        }
    } else if source.lines().any(|line| {
        line.split("//")
            .next()
            .unwrap_or_default()
            .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
            .any(|word| word == "unsafe" || word == "extern")
    }) {
        return Err("native calls belong only in mac_resources/native.rs".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn native_permission_is_narrow_and_requires_safety_comments() {
        let prefix = "//! plan_ref: docs/plan/10_performance_reliability.md#macos-gates\n";
        let native = format!("{prefix}// SAFETY: valid SDK storage.\nunsafe {{ call() }}");
        assert!(super::check("native.rs", &native).is_ok());
        assert!(super::check("runner.rs", &native).is_err());
        assert!(super::check("native.rs", &format!("{prefix}unsafe {{ call() }}")).is_err());
        assert!(super::check("native.rs", &format!("{prefix}#[link(name = \"AppKit\")]")).is_err());
    }
}
