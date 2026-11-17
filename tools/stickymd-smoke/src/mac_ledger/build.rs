//! The build a product input fingerprint stands for (plan 11 #phase-16-macos-host): the variables
//! that would change what `build-app.sh` builds without being recorded are refused, and what is
//! recorded besides the tracked files (toolchain, Xcode tools, Cargo configuration outside the
//! repository) is serialized here.
//! plan_ref: docs/plan/11_testing_and_release.md#phase-16-macos-host
use std::path::{Path, PathBuf};

/// Variables that change what Cargo, rustc, the linker or a dependency's build script produce.
const REFUSED: &[&str] = &[
    "AR",
    "CARGO_ENCODED_RUSTFLAGS",
    "CARGO_INCREMENTAL",
    "CC",
    "CFLAGS",
    "CXX",
    "CXXFLAGS",
    "LDFLAGS",
    "MACOSX_DEPLOYMENT_TARGET",
    "RUSTC",
    "RUSTC_BOOTSTRAP",
    "RUSTC_WORKSPACE_WRAPPER",
    "RUSTC_WRAPPER",
    "RUSTFLAGS",
    "SDKROOT",
];

/// Families of the same: Cargo's profile, build and per-target configuration (which includes
/// `CARGO_TARGET_DIR`), unstable flags, and the C toolchain variables a build script reads per
/// target.
const REFUSED_PREFIXES: &[&str] = &[
    "AR_",
    "CARGO_BUILD_",
    "CARGO_PROFILE_",
    "CARGO_TARGET_",
    "CARGO_UNSTABLE_",
    "CC_",
    "CFLAGS_",
    "CXXFLAGS_",
    "CXX_",
    "LDFLAGS_",
];

/// The names among `variables` that a build to be fingerprinted must not have set, sorted.
pub(super) fn refused(variables: impl IntoIterator<Item = (String, String)>) -> Vec<String> {
    let mut names: Vec<String> = variables
        .into_iter()
        .map(|(name, _)| name)
        .filter(|name| {
            REFUSED.contains(&name.as_str())
                || REFUSED_PREFIXES
                    .iter()
                    .any(|prefix| name.starts_with(prefix))
        })
        .collect();
    names.sort();
    names
}

/// The Cargo configuration files Cargo merges into a build run in `root` that are not tracked
/// there: those of `root`'s ancestors and of `cargo_home`, in the order Cargo reads them.
pub(super) fn outside_cargo_configs(root: &Path, cargo_home: &Path) -> Vec<PathBuf> {
    let mut directories: Vec<PathBuf> = root
        .ancestors()
        .skip(1)
        .map(|ancestor| ancestor.join(".cargo"))
        .collect();
    directories.push(cargo_home.to_path_buf());
    let mut files = Vec::new();
    for directory in directories {
        for name in ["config", "config.toml"] {
            let file = directory.join(name);
            if file.is_file() && !files.contains(&file) {
                files.push(file);
            }
        }
    }
    files
}

/// The bytes appended to the tracked inputs: each recorded tool output and configuration file as
/// a length-prefixed label and value, after the fixed target and profile.
pub(super) fn identity(tools: &[(&str, String)], configs: &[(PathBuf, Vec<u8>)]) -> Vec<u8> {
    let mut bytes = Vec::new();
    let mut field = |label: &[u8], value: &[u8]| {
        bytes.extend_from_slice(&(label.len() as u64).to_le_bytes());
        bytes.extend_from_slice(label);
        bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(value);
    };
    field(b"target", b"aarch64-apple-darwin");
    field(b"profile", b"release --locked");
    for (label, value) in tools {
        field(label.as_bytes(), value.as_bytes());
    }
    for (path, content) in configs {
        field(path.to_string_lossy().as_bytes(), content);
    }
    bytes
}

#[cfg(test)]
mod tests {
    use super::{identity, outside_cargo_configs, refused};
    use std::path::PathBuf;

    #[test]
    fn variables_that_change_the_build_are_refused_and_others_pass() {
        let variables = [
            "RUSTFLAGS",
            "CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS",
            "CARGO_TARGET_DIR",
            "CARGO_BUILD_TARGET_DIR",
            "CC_aarch64-apple-darwin",
            "MACOSX_DEPLOYMENT_TARGET",
            "CARGO_HOME",
            "CARGO_TERM_COLOR",
            "CARGO_PKG_NAME",
            "CARGO_MANIFEST_DIR",
            "RUSTUP_TOOLCHAIN",
            "HOME",
            "PATH",
        ]
        .map(|name| (name.to_owned(), String::new()));
        assert_eq!(
            refused(variables),
            [
                "CARGO_BUILD_TARGET_DIR",
                "CARGO_PROFILE_RELEASE_DEBUG_ASSERTIONS",
                "CARGO_TARGET_DIR",
                "CC_aarch64-apple-darwin",
                "MACOSX_DEPLOYMENT_TARGET",
                "RUSTFLAGS",
            ]
        );
    }

    #[test]
    fn cargo_configurations_outside_the_repository_are_found_in_reading_order() {
        let base = std::env::temp_dir().join(format!(
            "stickymd-cargo-configs-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = base.join("work/repo");
        let home = base.join("home/.cargo");
        for directory in [root.join(".cargo"), base.join("work/.cargo"), home.clone()] {
            std::fs::create_dir_all(directory).unwrap();
        }
        std::fs::write(root.join(".cargo/config.toml"), "tracked").unwrap();
        std::fs::write(base.join("work/.cargo/config.toml"), "parent").unwrap();
        std::fs::write(home.join("config.toml"), "home").unwrap();
        assert_eq!(
            outside_cargo_configs(&root, &home),
            [
                base.join("work/.cargo/config.toml"),
                home.join("config.toml")
            ]
        );
        std::fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn every_recorded_tool_and_configuration_byte_changes_the_identity() {
        let tools = |ld: &str| {
            vec![
                ("rustc -vV", "rustc 1.97.1".to_owned()),
                ("ld -v", ld.to_owned()),
            ]
        };
        let configs =
            |text: &[u8]| vec![(PathBuf::from("/home/.cargo/config.toml"), text.to_vec())];
        let base = identity(&tools("ld-1"), &configs(b"[build]"));
        assert_eq!(base, identity(&tools("ld-1"), &configs(b"[build]")));
        assert_ne!(base, identity(&tools("ld-2"), &configs(b"[build]")));
        assert_ne!(
            base,
            identity(&tools("ld-1"), &configs(b"[build]\njobs = 1"))
        );
        assert_ne!(base, identity(&tools("ld-1"), &[]));
    }
}
