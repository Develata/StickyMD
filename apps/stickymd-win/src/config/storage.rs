//! Durable TOML loading, evidence preservation, and atomic config publication.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use thiserror::Error;

use crate::platform::windows::atomic_file::{
    AtomicPublishError, atomic_publish, move_file_no_replace,
};
use stickymd_app::config::{
    ConfigLoadOutcome, ConfigSerializeError, RuntimeConfig, config_text, load_config_from,
    preserve_with,
};

pub fn load_config(path: &Path) -> Result<ConfigLoadOutcome, ConfigStorageError> {
    Ok(load_config_from(fs::read_to_string(path), || {
        preserve_corrupt_config(path)
    }))
}

fn preserve_corrupt_config(path: &Path) -> Option<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    preserve_with(path, stamp, move_file_no_replace).ok()
}

#[cfg(test)]
fn preserve_corrupt_config_at(path: &Path, stamp: u64) -> ConfigLoadOutcome {
    ConfigLoadOutcome::after_preservation(preserve_with(path, stamp, move_file_no_replace).ok())
}

pub fn save_config(
    target: &Path,
    temporary: &Path,
    config: &RuntimeConfig,
) -> Result<(), ConfigStorageError> {
    let source = config_text(config).map_err(ConfigStorageError::Serialize)?;
    atomic_publish(target, temporary, source.as_bytes()).map_err(ConfigStorageError::Publish)
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

    fn host() -> ConfigStorageUnderTest {
        ConfigStorageUnderTest {
            load: |path| load_config(path).unwrap(),
            save: |path, temporary, config| save_config(path, temporary, config).unwrap(),
            preserve_at: preserve_corrupt_config_at,
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
    fn phase15_config_bytes_are_fixed_and_survive_a_load_and_save() {
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
