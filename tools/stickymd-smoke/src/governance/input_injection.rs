//! Physical input reaches the desktop only through `input_injection` (plan 11 physical input
//! injection): the Windows APIs that inject input or place the cursor may be named nowhere else
//! in the smoke CLI's Rust sources or in any PowerShell script under `tools/`. It is a lexical
//! regression check over those files (comments included, which only makes it stricter): it does
//! not follow build scripts, other crates or names assembled at run time.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#physical-input-injection

use std::path::Path;

/// Paths, relative to the repository and with `/` separators, that may name the APIs.
const ALLOWED: &str = "tools/stickymd-smoke/src/input_injection";

/// The API names, split so that this file does not name them.
fn apis() -> [String; 4] {
    [
        ["Send", "Input"].concat(),
        ["keybd", "_event"].concat(),
        ["mouse", "_event"].concat(),
        ["SetCursor", "Pos"].concat(),
    ]
}

/// The first injection API `content` names, unless `relative` lies inside the module.
pub(super) fn misplaced_api(relative: &str, content: &str) -> Option<String> {
    let inside =
        relative == format!("{ALLOWED}.rs") || relative.starts_with(&format!("{ALLOWED}/"));
    if inside {
        return None;
    }
    apis()
        .into_iter()
        .find(|api| content.contains(api.as_str()))
}

pub(super) fn verify(root: &Path) -> Result<(), String> {
    let mut files = Vec::new();
    super::collect_files(&root.join("tools/stickymd-smoke/src"), "rs", &mut files)?;
    super::collect_files(&root.join("tools"), "ps1", &mut files)?;
    for path in files {
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let content = super::read_text(&path)?;
        if let Some(api) = misplaced_api(&relative, &content) {
            return Err(format!(
                "{relative} names `{api}`; physical input is injected only through `{ALLOWED}`"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_module_may_name_an_injection_api() {
        for api in apis() {
            let content = format!("unsafe {{ {api}(1, &input, 40) }}");
            for allowed in [
                "tools/stickymd-smoke/src/input_injection.rs",
                "tools/stickymd-smoke/src/input_injection/win32.rs",
            ] {
                assert_eq!(misplaced_api(allowed, &content), None, "{allowed}");
            }
            for elsewhere in [
                "tools/stickymd-smoke/src/window_control.rs",
                "tools/stickymd-smoke/src/input_injection_extra.rs",
                "tools/stickymd-smoke/helpers/windows-uia.ps1",
                "tools/smoke/phase-08.ps1",
            ] {
                assert_eq!(
                    misplaced_api(elsewhere, &content),
                    Some(api.clone()),
                    "{elsewhere}"
                );
            }
            // A comment counts as well.
            assert!(misplaced_api("tools/smoke/x.ps1", &format!("# uses {api}")).is_some());
        }
        assert_eq!(
            misplaced_api("tools/smoke/x.ps1", "GetCursorPos(&point)"),
            None
        );
    }

    /// Scripts are found whatever the case of their extension: Windows file names ignore it.
    #[test]
    fn a_script_with_an_upper_case_extension_is_scanned() {
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "stickymd-injection-scan-{}-{nonce}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("tools")).unwrap();
        std::fs::write(root.join("tools/probe.PS1"), format!("# {}", apis()[0])).unwrap();
        let scanned = verify(&root);
        std::fs::remove_dir_all(&root).unwrap();
        let error = scanned.unwrap_err();
        assert!(error.contains("tools/probe.PS1"), "{error}");
    }

    #[test]
    fn the_repository_injects_only_through_the_module() {
        let root =
            crate::governance::find_repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
        verify(&root).unwrap();
    }
}
