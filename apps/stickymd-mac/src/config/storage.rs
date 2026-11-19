//! Durable TOML loading, evidence preservation, and atomic config publication.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::platform::macos::atomic_file::{AtomicPublishError, atomic_publish_in};
use crate::platform::macos::file::DirectoryHandle;
use stickymd_app::config::{
    ConfigLoadOutcome, ConfigSerializeError, RuntimeConfig, config_text, load_config_from,
    preserve_with,
};

/// Loads `config.toml` from the Note Directory the program opened at start, wherever it is now: a
/// link or non-file there reads as a failure, and a corrupt file is renamed aside inside the same
/// directory. Preserved names are reported under `shown`, where the directory was at start.
pub fn load_config_in(
    directory: &DirectoryHandle,
    shown: &Path,
) -> Result<ConfigLoadOutcome, ConfigStorageError> {
    let read = directory.open_read(CONFIG_NAME).and_then(|mut file| {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut file, &mut text).map(|_| text)
    });
    Ok(load_config_from(read, || {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |duration| duration.as_secs());
        preserve_in(directory, shown, stamp)
    }))
}

/// Renames a corrupt `config.toml` aside in `directory`, never over an existing name.
fn preserve_in(directory: &DirectoryHandle, shown: &Path, stamp: u64) -> Option<PathBuf> {
    preserve_with(Path::new(CONFIG_NAME), stamp, |from, to| {
        directory.rename_no_replace(file_name(from)?, file_name(to)?)
    })
    .ok()
    .map(|preserved| shown.join(preserved))
}

const CONFIG_NAME: &str = "config.toml";
const CONFIG_TEMPORARY_NAME: &str = "config.toml.tmp";

fn file_name(path: &Path) -> std::io::Result<&str> {
    path.to_str()
        .ok_or_else(|| std::io::Error::other("the preserved name is not UTF-8"))
}

/// Publishes inside the Note Directory the program is working in, wherever it is now.
pub fn save_config_in(
    directory: &DirectoryHandle,
    config: &RuntimeConfig,
) -> Result<(), ConfigStorageError> {
    let source = config_text(config).map_err(ConfigStorageError::Serialize)?;
    atomic_publish_in(
        directory,
        CONFIG_NAME,
        CONFIG_TEMPORARY_NAME,
        source.as_bytes(),
    )
    .map_err(ConfigStorageError::Publish)
}

#[derive(Debug, Error)]
pub enum ConfigStorageError {
    #[error("cannot serialize config.toml: {0}")]
    Serialize(ConfigSerializeError),
    #[error("cannot atomically publish config.toml: {0}")]
    Publish(AtomicPublishError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::unique_temp_path;
    use stickymd_desktop::testing::contracts::{self, ConfigStorageUnderTest};

    /// The contracts name `<root>/config.toml`; the host's functions take the directory.
    fn directory_of(path: &Path) -> (DirectoryHandle, &Path) {
        assert_eq!(path.file_name().unwrap(), CONFIG_NAME);
        let directory = path.parent().unwrap();
        (DirectoryHandle::open(directory).unwrap(), directory)
    }

    fn host() -> ConfigStorageUnderTest {
        ConfigStorageUnderTest {
            load: |path| {
                let (directory, shown) = directory_of(path);
                load_config_in(&directory, shown).unwrap()
            },
            save: |path, temporary, config| {
                assert_eq!(temporary.file_name().unwrap(), CONFIG_TEMPORARY_NAME);
                save_config_in(&directory_of(path).0, config).unwrap();
            },
            preserve_at: |path, stamp| {
                let (directory, shown) = directory_of(path);
                ConfigLoadOutcome::after_preservation(preserve_in(&directory, shown, stamp))
            },
            temp_path: |label| unique_temp_path(&format!("config-{label}")),
        }
    }

    #[test]
    fn missing_and_partial_config_use_defaults() {
        contracts::missing_and_partial_config_use_defaults(host());
    }

    #[test]
    fn unknown_fields_are_ignored_but_invalid_enum_is_preserved_as_corrupt() {
        contracts::unknown_fields_are_ignored_but_invalid_enum_is_preserved_as_corrupt(host());
    }

    #[test]
    fn corrupt_config_name_collision_preserves_both_evidence_files() {
        contracts::corrupt_config_name_collision_preserves_both_evidence_files(host());
    }

    #[test]
    fn newer_version_is_preserved_and_not_overwritten() {
        contracts::newer_version_is_preserved_and_not_overwritten(host());
    }

    #[test]
    fn invalid_v1_values_and_older_versions_are_preserved_without_blocking_note_startup() {
        contracts::invalid_v1_values_and_older_versions_are_preserved_without_blocking_note_startup(
            host(),
        );
    }

    #[test]
    fn config_bytes_are_fixed_and_survive_a_load_and_save() {
        contracts::config_bytes_are_fixed_and_survive_a_load_and_save(host());
    }

    #[test]
    fn config_save_is_atomic_and_roundtrips() {
        contracts::config_save_is_atomic_and_roundtrips(host());
    }

    #[test]
    fn zoom_and_split_sync_roundtrip_and_missing_zoom_defaults() {
        contracts::zoom_and_split_sync_roundtrip_and_missing_zoom_defaults(host());
    }
}
