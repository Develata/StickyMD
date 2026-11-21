//! Which tracked files are inputs of the macOS app `build-app.sh` builds (plan 11
//! #phase-16-macos-host). A file is an input unless it is listed here as unable to reach that
//! app, so an unlisted new file can only cause a rerun, never a false reuse.
//!
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host

/// Directories nothing of which is compiled, linked or copied into the macOS app.
const OUTSIDE: &[&str] = &[
    // The Windows host. Cargo resolves `-p stickymd-mac` without it; a dependency change it
    // causes still reaches `Cargo.lock`, which is an input.
    "apps/stickymd-win/",
    // Verification tooling; the mac-gui driver is bound by its own hash in the ledger.
    "tools/",
    "docs/",
    ".github/",
    "experiments/",
    // Root fixtures, read only by tests and the Windows release baselines.
    "tests/",
    // License texts and images for the Windows package and the repository pages.
    "assets/",
    "dist/",
];

/// Repository documents and the dependency policy at the root.
const OUTSIDE_FILES: &[&str] = &[
    "AGENTS.md",
    "CHANGELOG.md",
    "CONTRIBUTING.md",
    "LICENSE",
    "README.en.md",
    "README.md",
    "README.zh-CN.md",
    "SECURITY.md",
    "THIRD_PARTY_NOTICES.md",
    "deny.toml",
];

pub fn is_input(path: &str) -> bool {
    !OUTSIDE.iter().any(|directory| path.starts_with(directory)) && !OUTSIDE_FILES.contains(&path)
}
