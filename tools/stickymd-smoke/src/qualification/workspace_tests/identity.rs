//! Conservative execution identity; unfamiliar overrides run normally without shared success.
//! plan_ref: docs/plan/11_testing_and_release.md#shared-headless-prerequisite

use super::Identity;
use crate::qualification::{module_ledger::fingerprint, receipt, source_freeze};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

pub(super) fn capture(root: &Path) -> Result<Identity, String> {
    let freeze = source_freeze::read(root)?;
    source_freeze::validate_against_repository(root, &freeze)?;
    let cargo = receipt::command_text(root, "cargo", &["--version", "--verbose"])?;
    let executable =
        std::env::current_exe().map_err(|e| format!("cannot identify smoke executable: {e}"))?;
    let mut material = Vec::new();
    for value in [
        "shared workspace prerequisite v1",
        super::COMMAND,
        &freeze.source_commit,
        &freeze.rustc,
        &freeze.target,
        &cargo,
        &receipt::sha256(&executable)?,
        &root
            .canonicalize()
            .map_err(|e| e.to_string())?
            .to_string_lossy(),
        std::env::consts::OS,
        std::env::consts::ARCH,
    ] {
        part(&mut material, value.as_bytes());
    }
    // Only execution settings are read. No registry credentials or arbitrary environment values enter receipts.
    let settings = [
        "PATH",
        "PATHEXT",
        "CARGO",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "RUSTUP_TOOLCHAIN_SOURCE",
        "RUST_RECURSION_COUNT",
        "RUST_LOG",
        "COMPUTERNAME",
        "HOSTNAME",
        "OS",
        "USERPROFILE",
        "HOME",
        "TEMP",
        "TMP",
        "SYSTEMROOT",
        "WINDIR",
        "PROCESSOR_ARCHITECTURE",
        "PROCESSOR_IDENTIFIER",
        "NUMBER_OF_PROCESSORS",
        "VCINSTALLDIR",
        "VCTOOLSINSTALLDIR",
        "WINDOWSSDKDIR",
        "WINDOWSSDKVERSION",
        "UCRTVERSION",
        "LIB",
        "LIBPATH",
        "INCLUDE",
        "CC",
        "CXX",
        "AR",
    ];
    let mut environment = BTreeMap::new();
    let mut reusable = true;
    for (key, value) in std::env::vars_os() {
        let Some(key) = key.to_str() else {
            reusable = false;
            continue;
        };
        let key = key.to_ascii_uppercase();
        if unsupported_override(&key) {
            reusable = false;
        }
        if settings.contains(&key.as_str()) {
            environment.insert(key, value);
        }
    }
    for key in settings {
        part(&mut material, key.as_bytes());
        let value = environment.get(key);
        match value {
            Some(value) => {
                part(&mut material, b"present");
                part(&mut material, value.as_encoded_bytes());
            }
            None => part(&mut material, b"absent"),
        }
    }
    let mut configs = BTreeSet::new();
    for directory in root.ancestors() {
        configs.insert(directory.join(".cargo/config"));
        configs.insert(directory.join(".cargo/config.toml"));
    }
    let cargo_home = std::env::var_os("CARGO_HOME")
        .filter(|value| !value.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .filter(|value| !value.is_empty())
                .map(|p| Path::new(&p).join(".cargo"))
        });
    if let Some(directory) = cargo_home {
        // Cargo runs from root, even when the smoke CLI was launched from a subdirectory.
        let directory = root.join(directory);
        if directory.is_absolute() {
            configs.insert(directory.join("config"));
            configs.insert(directory.join("config.toml"));
        } else {
            // A Windows drive-relative path such as C:cache depends on per-drive cwd.
            // Do not guess a different process's resolution or share its test success.
            reusable = false;
        }
    } else {
        reusable = false;
    }
    for config in configs {
        part(&mut material, config.as_os_str().as_encoded_bytes());
        match fs::read(&config) {
            Ok(bytes) => {
                reusable &= supported_config(&bytes);
                // Hash existing settings without persisting or printing their raw contents.
                part(&mut material, receipt::sha256(&config)?.as_bytes());
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => part(&mut material, b"absent"),
            Err(_) => {
                return Err("cannot inspect Cargo configuration for workspace identity".into());
            }
        }
    }
    let fingerprint = fingerprint::workspace_inputs(root, &material)?;
    // Catch source edits during input collection as well as across the actual test run.
    source_freeze::validate_against_repository(root, &freeze)?;
    Ok(Identity {
        source: freeze.source_commit,
        fingerprint,
        reusable,
    })
}

fn part(output: &mut Vec<u8>, bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    output.extend_from_slice(bytes);
}

fn unsupported_override(name: &str) -> bool {
    if name.starts_with("STICKYMD_") {
        return true;
    }
    if name.starts_with("CARGO_") {
        return !matches!(
            name,
            "CARGO_HOME" | "CARGO_MANIFEST_DIR" | "CARGO_MANIFEST_PATH"
        ) && !name.starts_with("CARGO_PKG_");
    }
    name.starts_with("RUST")
        && !matches!(
            name,
            "RUSTUP_HOME"
                | "RUSTUP_TOOLCHAIN"
                | "RUSTUP_TOOLCHAIN_SOURCE"
                | "RUST_RECURSION_COUNT"
                | "RUST_LOG"
        )
}

fn supported_config(bytes: &[u8]) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    let lines: Vec<_> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();
    lines.is_empty()
        || lines
            == [
                "[target.x86_64-pc-windows-msvc]",
                "rustflags = [\"-C\", \"target-feature=+crt-static\"]",
            ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unfamiliar_build_and_test_overrides_disable_sharing() {
        for name in [
            "RUSTFLAGS",
            "RUSTC_WRAPPER",
            "RUSTDOCFLAGS",
            "RUST_TEST_THREADS",
            "CARGO_BUILD_TARGET",
            "CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUNNER",
            "STICKYMD_SMOKE_RESOURCE_CASE",
        ] {
            assert!(unsupported_override(name), "{name}");
        }
        for name in [
            "CARGO_HOME",
            "CARGO_PKG_VERSION",
            "RUSTUP_HOME",
            "RUSTUP_TOOLCHAIN",
            "RUSTUP_TOOLCHAIN_SOURCE",
            "RUST_RECURSION_COUNT",
            "RUST_LOG",
        ] {
            assert!(!unsupported_override(name), "{name}");
        }
    }
    #[test]
    fn cargo_config_support_is_explicit_instead_of_approximating_toml() {
        assert!(supported_config(include_bytes!(
            "../../../../../.cargo/config.toml"
        )));
        for text in [
            "[env]\nRUST_TEST_THREADS = '1'",
            "include = ['outside.toml']",
            "[build]\nrustc-wrapper = 'wrapper'",
            "[alias]\ntest = 'check'",
        ] {
            assert!(!supported_config(text.as_bytes()));
        }
    }
}
