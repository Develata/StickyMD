//! The macOS host (`apps/stickymd-mac`) confines `unsafe` and the native macOS bindings to its
//! `platform/` directory, keeps `#![deny(unsafe_op_in_unsafe_fn)]` on every crate root, gives
//! every unsafe block a `SAFETY` comment, and gives every non-test source a `plan_ref`
//! (plan 11 Phase 16, plan 03). It is the macOS counterpart of the Windows host's checks and
//! scans nothing of the Windows host.
//!
//! The checks are lexical: they read each line without its `//` comment, split it into
//! identifiers and look for whole words, so `unsafe_op_in_unsafe_fn` is not `unsafe`. They do
//! not follow macros, `build.rs` or names assembled at run time.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

use std::path::Path;

/// The crate, relative to the repository.
const CRATE: &str = "apps/stickymd-mac";
/// The one directory under `src/` that may hold `unsafe` and native bindings.
const PLATFORM: &str = "platform/";
/// Crate roots: each is a separate crate and needs the lint itself.
const ROOTS: [&str; 2] = ["main.rs", "lib.rs"];
const DENY: &str = "#![deny(unsafe_op_in_unsafe_fn)]";

/// Identifiers of `line` outside a `//` comment.
fn identifiers(line: &str) -> impl Iterator<Item = &str> {
    let code = line.split("//").next().unwrap_or_default();
    code.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|word| !word.is_empty())
}

/// Whether `word` names a native macOS binding crate.
fn is_native_binding(word: &str) -> bool {
    word == "libc" || word == "block2" || word.starts_with("objc2")
}

/// Test sources are exempt from `plan_ref`: they are named by their subject's module.
fn is_test_source(relative: &str) -> bool {
    let file = relative.rsplit('/').next().unwrap_or(relative);
    file == "tests.rs"
        || file == "test_support.rs"
        || file.ends_with("_tests.rs")
        || relative.split('/').any(|part| part == "tests")
}

/// The first violation of the host rules in one source, `relative` to the crate's `src/`.
fn violation(relative: &str, content: &str) -> Option<String> {
    let in_platform = relative.starts_with(PLATFORM);
    if !in_platform {
        for (index, line) in content.lines().enumerate() {
            for word in identifiers(line) {
                if word == "unsafe" || is_native_binding(word) {
                    return Some(format!(
                        "{CRATE}/src/{relative}:{} names `{word}` outside src/{PLATFORM}",
                        index + 1
                    ));
                }
            }
        }
    } else if let Some(line) = super::unsafe_block_without_safety(content) {
        return Some(format!(
            "{CRATE}/src/{relative}:{line} unsafe block has no adjacent SAFETY invariant"
        ));
    }
    if ROOTS.contains(&relative) && !content.contains(DENY) {
        return Some(format!("{CRATE}/src/{relative} must retain {DENY}"));
    }
    if !is_test_source(relative)
        && !content
            .lines()
            .any(|line| line.trim().starts_with("//! plan_ref: "))
    {
        return Some(format!(
            "{CRATE}/src/{relative} declares no `//! plan_ref:` (test sources are exempt)"
        ));
    }
    None
}

pub(super) fn verify(root: &Path) -> Result<(), String> {
    let source = root.join(CRATE).join("src");
    let mut files = Vec::new();
    super::collect_files(&source, "rs", &mut files)?;
    // A scan of nothing proves nothing.
    if files.is_empty() {
        return Err(format!("{CRATE}/src holds no Rust source to check"));
    }
    let mut roots = 0;
    for path in files {
        let relative = path
            .strip_prefix(&source)
            .map_err(|_| format!("{} lies outside {CRATE}/src", path.display()))?
            .to_string_lossy()
            .replace('\\', "/");
        roots += usize::from(ROOTS.contains(&relative.as_str()));
        if let Some(message) = violation(&relative, &super::read_text(&path)?) {
            return Err(message);
        }
    }
    if roots == 0 {
        return Err(format!("{CRATE}/src has no crate root (main.rs or lib.rs)"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAFE: &str = "//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host\n";

    /// `unsafe` and the native bindings are refused outside `platform/` as whole words, in code
    /// and not in comments; inside it they are allowed, with the SAFETY rule.
    #[test]
    fn unsafe_and_native_bindings_are_confined_to_the_platform_directory() {
        let clean = format!("{SAFE}#![deny(unsafe_op_in_unsafe_fn)]\nfn main() {{}}\n");
        assert_eq!(violation("main.rs", &clean), None);
        // Words that merely contain the forbidden ones, and comments, are not violations.
        let words = format!(
            "{SAFE}#![deny(unsafe_op_in_unsafe_fn)]\n// unsafe libc objc2\nlet unsafe_code = 1; // block2\nlet libcx = 2;\n"
        );
        assert_eq!(violation("main.rs", &words), None);
        for bad in [
            "unsafe { f() }",
            "unsafe fn f() {}",
            "use libc::flock;",
            "use block2::RcBlock;",
            "use objc2::rc::Retained;",
            "use objc2_app_kit::NSWindow;",
            "let x = objc2_foundation::NSString::new();",
        ] {
            for relative in ["main.rs", "host/window.rs", "storage.rs"] {
                let source = format!("{SAFE}#![deny(unsafe_op_in_unsafe_fn)]\n{bad}\n");
                let message = violation(relative, &source)
                    .unwrap_or_else(|| panic!("{relative} accepted `{bad}`"));
                assert!(message.contains("outside src/platform/"), "{message}");
            }
        }
        // The same lines are fine under platform/, apart from the SAFETY rule.
        for allowed in [
            "use libc::flock;",
            "use block2::RcBlock;",
            "use objc2_app_kit::NSWindow;",
            "unsafe fn f() {}",
            "// SAFETY: the pointer is valid for the call.\nunsafe { f() }",
        ] {
            let source = format!("{SAFE}{allowed}\n");
            assert_eq!(
                violation("platform/pasteboard.rs", &source),
                None,
                "{allowed}"
            );
        }
        // A directory merely named like it is not it.
        assert!(violation("platform_x/a.rs", &format!("{SAFE}unsafe fn f() {{}}\n")).is_some());
        assert!(violation("host/platform/a.rs", &format!("{SAFE}use libc::c_int;\n")).is_some());
    }

    #[test]
    fn every_unsafe_block_needs_a_safety_comment_within_six_lines() {
        let block = |comment: &str, padding: usize| {
            format!(
                "{SAFE}{comment}{}unsafe {{ call() }}\n",
                "\n".repeat(padding)
            )
        };
        assert_eq!(
            violation("platform/a.rs", &block("// SAFETY: ok\n", 0)),
            None
        );
        assert_eq!(
            violation("platform/a.rs", &block("// SAFETY: ok\n", 5)),
            None
        );
        for source in [
            block("", 0),
            block("// no invariant\n", 0),
            block("// SAFETY: ok\n", 7),
        ] {
            let message = violation("platform/a.rs", &source).expect("missing SAFETY");
            assert!(
                message.contains("no adjacent SAFETY invariant"),
                "{message}"
            );
        }
    }

    #[test]
    fn crate_roots_keep_the_deny_lint_and_business_sources_declare_a_plan_ref() {
        for root in ["main.rs", "lib.rs"] {
            let message = violation(root, SAFE).expect("root without the lint");
            assert!(message.contains(DENY), "{message}");
            assert_eq!(violation(root, &format!("{SAFE}{DENY}\n")), None);
        }
        let message = violation("storage.rs", "fn f() {}\n").expect("no plan_ref");
        assert!(message.contains("plan_ref"), "{message}");
        assert_eq!(violation("storage.rs", SAFE), None);
        for exempt in [
            "tests.rs",
            "storage/tests.rs",
            "shell_tests.rs",
            "tests/pasteboard.rs",
            "test_support.rs",
        ] {
            assert_eq!(violation(exempt, "fn f() {}\n"), None, "{exempt}");
        }
    }

    /// The real crate passes; a missing source tree or root fails rather than passing on nothing.
    #[test]
    fn the_macos_host_passes_and_an_empty_tree_does_not() {
        let repository = crate::governance::find_repository_root(std::path::Path::new(env!(
            "CARGO_MANIFEST_DIR"
        )))
        .unwrap();
        verify(&repository).unwrap();

        let root =
            std::env::temp_dir().join(format!("stickymd-governance-mac-{}", std::process::id()));
        let source = root.join("apps/stickymd-mac/src");
        std::fs::create_dir_all(&source).unwrap();
        assert!(verify(&root).is_err());
        std::fs::write(source.join("storage.rs"), SAFE).unwrap();
        let message = verify(&root).unwrap_err();
        assert!(message.contains("no crate root"), "{message}");
        std::fs::write(source.join("main.rs"), format!("{SAFE}{DENY}\n")).unwrap();
        verify(&root).unwrap();
        std::fs::create_dir_all(source.join("platform")).unwrap();
        std::fs::write(
            source.join("platform/mod.rs"),
            format!("{SAFE}unsafe {{ f() }}\n"),
        )
        .unwrap();
        let message = verify(&root).unwrap_err();
        assert!(message.contains("platform/mod.rs:2"), "{message}");
        std::fs::remove_dir_all(root).unwrap();
    }
}
