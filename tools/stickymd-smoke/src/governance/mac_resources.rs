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
            &path
                .strip_prefix(directory.parent().ok_or("missing source root")?)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .replace('\\', "/"),
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
    check_native(path, source, links)
}

fn check(name: &str, source: &str) -> Result<(), String> {
    if !source
        .lines()
        .any(|line| line.starts_with("//! plan_ref: docs/plan/"))
    {
        return Err("missing plan_ref".into());
    }
    let links: &[&str] = if name == "mac_resources/native.rs" {
        &["System", "CoreGraphics", "CoreFoundation"]
    } else {
        &[]
    };
    check_native(name, source, links)
}

fn check_native(path: &str, source: &str, links: &[&str]) -> Result<(), String> {
    let tokens = tokens(source)?;
    if links.is_empty() && tokens.iter().any(|t| matches!(*t, "unsafe" | "extern")) {
        return Err(format!(
            "{path}: native calls require an exact allowed native path"
        ));
    }
    if let Some(line) = super::unsafe_block_without_safety(source) {
        return Err(format!("line {line}: unsafe block needs SAFETY invariant"));
    }
    for (i, token) in tokens.iter().enumerate() {
        if *token != "#" || tokens.get(i + 1) != Some(&"[") {
            continue;
        }
        if tokens.get(i + 2) == Some(&"path") {
            return Err("#[path] can escape the scanned source trees".into());
        }
        if tokens.get(i + 2) == Some(&"cfg_attr") {
            let end = tokens[i..]
                .iter()
                .position(|t| *t == "]")
                .ok_or("unclosed conditional attribute")?
                + i;
            if tokens[i..=end]
                .iter()
                .any(|t| matches!(*t, "link" | "path"))
            {
                return Err("conditional native link/path attributes are not allowed".into());
            }
        }
        if tokens.get(i + 2) != Some(&"link") {
            continue;
        }
        let end = tokens[i..]
            .iter()
            .position(|t| *t == "]")
            .ok_or("unclosed link attribute")?
            + i;
        let attribute = tokens[i..=end].concat();
        if !links.iter().any(|name| {
            attribute
                == if *name == "System" {
                    format!("#[link(name=\"{name}\")]")
                } else {
                    format!("#[link(name=\"{name}\",kind=\"framework\")]")
                }
        }) {
            return Err(format!("{path}: native link is not allowed: {attribute}"));
        }
    }
    Ok(())
}

// Keep strings indivisible: comment markers inside them cannot hide later code.
// Nested block comments and line comments carry no native permissions.
fn tokens(source: &str) -> Result<Vec<&str>, String> {
    let bytes = source.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i].is_ascii_whitespace() {
            i += 1;
            continue;
        }
        if bytes[i..].starts_with(b"//") {
            i += bytes[i..]
                .iter()
                .position(|b| *b == b'\n')
                .unwrap_or(bytes.len() - i);
            continue;
        }
        if bytes[i..].starts_with(b"/*") {
            i += 2;
            let mut depth = 1;
            while i < bytes.len() && depth > 0 {
                if bytes[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if bytes[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
            if depth != 0 {
                return Err("unterminated block comment".into());
            }
            continue;
        }
        let start = i;
        if bytes[i] == b'\'' {
            let mut end = i + 1;
            if bytes.get(end) == Some(&b'\\') {
                end += 2;
                if bytes.get(end - 1) == Some(&b'u') && bytes.get(end) == Some(&b'{') {
                    end += source[end..].find('}').ok_or("unclosed char escape")? + 1;
                } else if bytes.get(end - 1) == Some(&b'x') {
                    end += 2;
                }
            } else if let Some(c) = source[end..].chars().next() {
                end += c.len_utf8();
            }
            if bytes.get(end) == Some(&b'\'') {
                i = end + 1;
                out.push(&source[start..i]);
                continue;
            }
        }
        let raw = i + usize::from(matches!(bytes[i], b'b' | b'c'));
        if bytes.get(raw) == Some(&b'r') {
            let mut quote = raw + 1;
            while bytes.get(quote) == Some(&b'#') {
                quote += 1;
            }
            if bytes.get(quote) == Some(&b'"') {
                let terminator = format!("\"{}", "#".repeat(quote - raw - 1));
                i = quote
                    + 1
                    + source[quote + 1..]
                        .find(&terminator)
                        .ok_or("unterminated raw string")?
                    + terminator.len();
                out.push(&source[start..i]);
                continue;
            }
        }
        if bytes[i] == b'"' {
            i += 1;
            loop {
                match bytes.get(i) {
                    Some(b'\\') => i += 2,
                    Some(b'"') => {
                        i += 1;
                        break;
                    }
                    Some(_) => i += 1,
                    None => return Err("unterminated string".into()),
                }
            }
        } else if bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' {
            i += 1;
            while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
        } else {
            i += source[i..]
                .chars()
                .next()
                .ok_or("missing character")?
                .len_utf8();
        }
        out.push(&source[start..i]);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn rejects_nested_native_inline_links_and_path_escapes() {
        let prefix = "//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host\n";
        let native = format!("{prefix}// SAFETY: SDK storage.\nunsafe {{ call() }}");
        assert!(super::check("mac_resources/nested/native.rs", &native).is_err());
        for path in [
            "mac_resources/native.rs",
            "mac_gui/native/events.rs",
            "mac_common/native.rs",
        ] {
            let check = |s: &str| {
                if path.starts_with("mac_resources/") {
                    super::check(path, &format!("{prefix}{s}"))
                } else {
                    super::check_gui(path, &format!("{prefix}{s}"))
                }
            };
            for bad in [
                "mod x { #[link(name = \"Foo\")] extern \"C\" {} }",
                "#[link(name = \"System\")] #[link(name = \"Foo\")] extern \"C\" {}",
                "#[path = \"../../elsewhere.rs\"] mod outside;",
                "#[ /* comment */ path = \"../../elsewhere.rs\"] mod outside;",
                "let x = \"/*\"; #[link(name = \"Foo\")] extern \"C\" {}",
                "#[cfg_attr(target_os = \"macos\", path = \"escape.rs\")] mod outside;",
                "#[cfg_attr(target_os = \"macos\", link(name = \"Foo\"))] extern \"C\" {}",
                "let s = br#\"\\\" //\"#; #[link(name = \"Foo\")] extern \"C\" {}",
            ] {
                assert!(check(bad).is_err(), "{path}: {bad}");
            }
            assert!(
                check("// #[link(name = \"Foo\")]\n/* #[path = \"foo\"] /* nested */ */").is_ok()
            );
            assert!(check("mod x { #[link(name = \"System\")] extern \"C\" {} }").is_ok());
            assert!(check("let c = '\"'; #[link(name = \"Foo\")] extern \"C\" {}").is_err());
        }
        for path in ["mac_gui/scenarios.rs", "mac_common/session.rs"] {
            assert!(
                super::check_gui(
                    path,
                    &format!("{prefix}#[path = \"escape.rs\"] mod outside;")
                )
                .is_err()
            );
        }
    }
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
        assert!(super::check("mac_resources/native.rs", &native).is_ok());
        assert!(super::check("runner.rs", &native).is_err());
        assert!(
            super::check(
                "mac_resources/native.rs",
                &format!("{prefix}unsafe {{ call() }}")
            )
            .is_err()
        );
        assert!(
            super::check(
                "mac_resources/native.rs",
                &format!("{prefix}#[link(name = \"AppKit\")]")
            )
            .is_err()
        );
    }
}
