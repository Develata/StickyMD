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
    verify_gui(root)
}

fn verify_gui(root: &Path) -> Result<(), String> {
    let base = root.join("tools/stickymd-smoke/src");
    for directory in ["mac_gui", "mac_common"] {
        let mut files = Vec::new();
        super::collect_files(&base.join(directory), "rs", &mut files)?;
        for path in files {
            let relative = path.strip_prefix(&base).map_err(|e| e.to_string())?;
            let name = relative.to_string_lossy().replace('\\', "/");
            check_gui(&name, &super::read_text(&path)?).map_err(|e| format!("{name}: {e}"))?;
        }
    }
    for (path, declaration) in [
        ("main.rs", "mod mac_common;"),
        ("mac_gui/mod.rs", "mod native;"),
    ] {
        if !super::read_text(&base.join(path))?
            .contains(&format!("#[cfg(target_os = \"macos\")]\n{declaration}"))
        {
            return Err(format!("{path}: {declaration} must remain macOS-only"));
        }
    }
    Ok(())
}

fn check_gui(path: &str, source: &str) -> Result<(), String> {
    let links: &[&str] = match path {
        "mac_common/native.rs" => &["System"],
        "mac_gui/native/events.rs" => &["CoreGraphics", "System"],
        "mac_gui/native/windows.rs" => &["CoreGraphics"],
        "mac_gui/native/cf.rs" => &["CoreFoundation"],
        "mac_gui/native/accessibility.rs" => &["ApplicationServices"],
        "mac_gui/native/permissions.rs" => &["ApplicationServices", "CoreGraphics"],
        _ => &[],
    };
    if !source
        .lines()
        .any(|s| s == "//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host")
    {
        return Err("missing exact Phase 16 plan_ref".into());
    }
    if links.is_empty() {
        return check("orchestration.rs", source);
    }
    if let Some(line) = super::unsafe_block_without_safety(source) {
        return Err(format!("line {line}: unsafe block needs SAFETY invariant"));
    }
    for line in source.lines().filter(|s| s.trim().starts_with("#[link(")) {
        if !links.iter().any(|name| {
            let expected = if *name == "System" {
                format!("#[link(name = \"{name}\")]")
            } else {
                format!("#[link(name = \"{name}\", kind = \"framework\")]")
            };
            line.trim() == expected
        }) {
            return Err("framework not allowed for this native concern".into());
        }
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
    fn gui_ffi_is_path_and_framework_specific() {
        let prefix = "//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host\n";
        let source = format!("{prefix}// SAFETY: SDK storage.\nunsafe {{ call() }}");
        assert!(super::check_gui("mac_gui/native/events.rs", &source).is_ok());
        for path in [
            "mac_gui/scenarios.rs",
            "mac_gui/nested/native.rs",
            "mac_common/session.rs",
        ] {
            assert!(super::check_gui(path, &source).is_err());
        }
        assert!(
            super::check_gui(
                "mac_gui/native/events.rs",
                &format!("{prefix}#[link(name = \"AppKit\", kind = \"framework\")]")
            )
            .is_err()
        );
        assert!(
            super::check_gui(
                "mac_gui/native/windows.rs",
                &format!("{prefix}#[link(name = \"ApplicationServices\", kind = \"framework\")]")
            )
            .is_err()
        );
        assert!(
            super::check_gui(
                "mac_gui/native/permissions.rs",
                &format!("{prefix}unsafe {{ call() }}")
            )
            .is_err()
        );
    }
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
