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
#[cfg(test)]
use stickymd_app::config::ConfigWarning;
use stickymd_app::config::{
    ConfigLoadOutcome, ConfigSerializeError, PRESERVED_CONFIG_SUFFIXES, RuntimeConfig, config_text,
    preserved_config_name,
};

pub fn load_config(path: &Path) -> Result<ConfigLoadOutcome, ConfigStorageError> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ConfigLoadOutcome::absent());
        }
        Err(error) => return Ok(ConfigLoadOutcome::read_failed(error.to_string())),
    };
    Ok(ConfigLoadOutcome::from_text(&source).unwrap_or_else(|| preserve_corrupt_config(path)))
}

fn preserve_corrupt_config(path: &Path) -> ConfigLoadOutcome {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    preserve_corrupt_config_at(path, stamp)
}

fn preserve_corrupt_config_at(path: &Path, stamp: u64) -> ConfigLoadOutcome {
    ConfigLoadOutcome::after_preservation(rename_corrupt_config(path, stamp).ok())
}

fn rename_corrupt_config(path: &Path, stamp: u64) -> std::io::Result<PathBuf> {
    // The no-replace move is the authority gate. A preliminary exists check
    // would be racy and could overwrite older evidence on Windows.
    for sequence in 0..=PRESERVED_CONFIG_SUFFIXES {
        let candidate = path.with_file_name(preserved_config_name(stamp, sequence));
        match move_file_no_replace(path, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "all corrupt config preservation names are occupied",
    ))
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
    use stickymd_app::config::{ContentZoomPercent, DockEdge, ThemeMode, ViewMode};

    fn unique_dir(label: &str) -> PathBuf {
        unique_temp_path(&format!("config-{label}"))
    }

    #[test]
    fn missing_and_partial_config_use_defaults() {
        let root = unique_dir("defaults");
        fs::create_dir(&root).unwrap();
        let path = root.join("config.toml");
        let missing = load_config(&path).unwrap();
        assert!(missing.should_create_default);
        assert!(missing.persistence_allowed);
        assert_eq!(missing.config, RuntimeConfig::default());
        fs::write(&path, "version = 1\nopacity = 85\n").unwrap();
        let partial = load_config(&path).unwrap();
        assert_eq!(partial.config.opacity, 85);
        assert_eq!(partial.config.theme, ThemeMode::Light);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn unknown_fields_are_ignored_but_invalid_enum_is_preserved_as_corrupt() {
        let root = unique_dir("unknown");
        fs::create_dir(&root).unwrap();
        let path = root.join("config.toml");
        fs::write(&path, "version = 1\nfuture = true\n").unwrap();
        assert!(load_config(&path).unwrap().warning.is_none());
        fs::write(&path, "version = 1\ntheme = \"purple\"\n").unwrap();
        let invalid = load_config(&path).unwrap();
        assert!(matches!(
            invalid.warning,
            Some(ConfigWarning::CorruptPreserved(_))
        ));
        assert!(!path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_config_name_collision_preserves_both_evidence_files() {
        let root = unique_dir("collision");
        fs::create_dir(&root).unwrap();
        let path = root.join("config.toml");
        let occupied = root.join("config.invalid-42.toml");
        fs::write(&occupied, "older evidence").unwrap();
        fs::write(&path, "version = invalid").unwrap();

        let outcome = preserve_corrupt_config_at(&path, 42);
        let Some(ConfigWarning::CorruptPreserved(preserved)) = outcome.warning else {
            panic!("corrupt config should be preserved");
        };
        assert_eq!(preserved, root.join("config.invalid-42-1.toml"));
        assert_eq!(fs::read_to_string(occupied).unwrap(), "older evidence");
        assert_eq!(fs::read_to_string(preserved).unwrap(), "version = invalid");
        assert!(outcome.persistence_allowed);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn newer_version_is_preserved_and_not_overwritten() {
        let root = unique_dir("newer");
        fs::create_dir(&root).unwrap();
        let path = root.join("config.toml");
        fs::write(&path, "version = 2\n").unwrap();
        let outcome = load_config(&path).unwrap();
        assert_eq!(
            outcome.warning,
            Some(ConfigWarning::UnsupportedNewerVersion(2))
        );
        assert!(!outcome.should_create_default);
        assert!(!outcome.persistence_allowed);
        assert!(path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_v1_values_and_older_versions_are_preserved_without_blocking_note_startup() {
        for source in [
            "version = 1\nopacity = 1\n",
            "version = 1\n[window]\nwidth_dip = 1\n",
            "version = 1\n[window]\ndock_offset_ratio = nan\n",
            "version = 1\n[window]\nfloating_x_ratio = inf\n",
            "version = 0\n",
        ] {
            let root = unique_dir("semantic-invalid");
            fs::create_dir(&root).unwrap();
            let path = root.join("config.toml");
            fs::write(&path, source).unwrap();
            let outcome = load_config(&path).unwrap();
            assert_eq!(outcome.config, RuntimeConfig::default());
            assert!(matches!(
                outcome.warning,
                Some(ConfigWarning::CorruptPreserved(_))
            ));
            assert!(outcome.persistence_allowed);
            fs::remove_dir_all(root).unwrap();
        }
    }

    /// The durable bytes of `RuntimeConfig::default()`, fixed before the config model left this
    /// crate (Phase 15): a change here changes `config.toml` on disk.
    const DEFAULT_CONFIG_BYTES: &str = "version = 1
theme = \"light\"
opacity = 96
content_zoom_percent = 100
split_scroll_sync = true
always_on_top = false
view_mode = \"source\"

[window]
width_dip = 520
height_dip = 680
monitor_id = \"\"
dock_edge = \"none\"
dock_offset_ratio = 0.5
floating_x_ratio = 0.5
floating_y_ratio = 0.25
";

    /// A config with every field off its default, including a non-ASCII monitor id that needs
    /// quoting and ratios whose decimal text depends on the f32 formatting.
    const POPULATED_CONFIG_BYTES: &str = "version = 1
theme = \"dark\"
opacity = 83
content_zoom_percent = 175
split_scroll_sync = false
always_on_top = true
view_mode = \"split\"

[window]
width_dip = 640
height_dip = 480
monitor_id = 'monitor-中\"1'
dock_edge = \"right\"
dock_offset_ratio = 0.25
floating_x_ratio = 0.33333334
floating_y_ratio = 0.1
";

    fn populated_config() -> RuntimeConfig {
        let mut config = RuntimeConfig {
            theme: ThemeMode::Dark,
            opacity: 83,
            content_zoom_percent: ContentZoomPercent::new_clamped(175),
            split_scroll_sync: false,
            always_on_top: true,
            view_mode: ViewMode::Split,
            ..RuntimeConfig::default()
        };
        config.window.width_dip = 640;
        config.window.height_dip = 480;
        config.window.monitor_id = "monitor-中\"1".to_owned();
        config.window.dock_edge = DockEdge::Right;
        config.window.dock_offset_ratio = 0.25;
        config.window.floating_x_ratio = 1.0 / 3.0;
        config.window.floating_y_ratio = 0.1;
        config
    }

    /// Phase 15 byte compatibility: the serialized bytes are fixed, and a load followed by a save
    /// reproduces them, in this build and in any checkpoint a rollback returns to.
    #[test]
    fn phase15_config_bytes_are_fixed_and_survive_a_load_and_save() {
        let root = unique_dir("bytes");
        fs::create_dir(&root).unwrap();
        let path = root.join("config.toml");
        let temporary = root.join("config.toml.tmp");
        for (config, bytes) in [
            (RuntimeConfig::default(), DEFAULT_CONFIG_BYTES),
            (populated_config(), POPULATED_CONFIG_BYTES),
        ] {
            save_config(&path, &temporary, &config).unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
            fs::write(&path, bytes).unwrap();
            let loaded = load_config(&path).unwrap();
            assert!(loaded.warning.is_none());
            assert_eq!(loaded.config, config);
            save_config(&path, &temporary, &loaded.config).unwrap();
            assert_eq!(fs::read_to_string(&path).unwrap(), bytes);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn config_save_is_atomic_and_roundtrips() {
        let root = unique_dir("save");
        fs::create_dir(&root).unwrap();
        let path = root.join("config.toml");
        let temporary = root.join("config.toml.tmp");
        let config = RuntimeConfig {
            opacity: 83,
            ..RuntimeConfig::default()
        };
        save_config(&path, &temporary, &config).unwrap();
        assert_eq!(load_config(&path).unwrap().config, config);
        assert!(!temporary.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn zoom_and_split_sync_roundtrip_and_missing_zoom_defaults() {
        let root = unique_dir("preferences");
        fs::create_dir(&root).unwrap();
        let path = root.join("config.toml");
        let temporary = root.join("config.toml.tmp");
        let config = RuntimeConfig {
            content_zoom_percent: ContentZoomPercent::new_clamped(175),
            split_scroll_sync: false,
            opacity: 40,
            ..RuntimeConfig::default()
        };
        save_config(&path, &temporary, &config).unwrap();
        assert_eq!(load_config(&path).unwrap().config, config);

        fs::write(&path, "version = 1\nopacity = 40\n").unwrap();
        let migrated = load_config(&path).unwrap();
        assert_eq!(migrated.config.content_zoom_percent.value(), 100);
        assert!(migrated.config.split_scroll_sync);
        fs::remove_dir_all(root).unwrap();
    }
}
