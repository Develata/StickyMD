//! Versioned v1 portable configuration.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::platform::windows::atomic_file::{AtomicPublishError, atomic_publish};

pub const CONFIG_VERSION: u32 = 1;
const MAX_WINDOW_DIP: u32 = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    #[default]
    Light,
    System,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ViewMode {
    #[default]
    Source,
    Split,
    Preview,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum DockEdge {
    #[default]
    None,
    Left,
    Right,
    Top,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct WindowConfig {
    pub width_dip: u32,
    pub height_dip: u32,
    pub monitor_id: String,
    pub dock_edge: DockEdge,
    pub dock_offset_ratio: f32,
    pub floating_x_ratio: f32,
    pub floating_y_ratio: f32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            width_dip: 520,
            height_dip: 680,
            monitor_id: String::new(),
            dock_edge: DockEdge::None,
            dock_offset_ratio: 0.5,
            floating_x_ratio: 0.5,
            floating_y_ratio: 0.25,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RuntimeConfig {
    pub version: u32,
    pub theme: ThemeMode,
    pub opacity: u8,
    pub always_on_top: bool,
    pub view_mode: ViewMode,
    pub window: WindowConfig,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            theme: ThemeMode::Light,
            opacity: 96,
            always_on_top: false,
            view_mode: ViewMode::Source,
            window: WindowConfig::default(),
        }
    }
}

impl RuntimeConfig {
    fn is_semantically_valid(&self) -> bool {
        (70..=100).contains(&self.opacity)
            && (360..=MAX_WINDOW_DIP).contains(&self.window.width_dip)
            && (240..=MAX_WINDOW_DIP).contains(&self.window.height_dip)
            && valid_ratio(self.window.dock_offset_ratio)
            && valid_ratio(self.window.floating_x_ratio)
            && valid_ratio(self.window.floating_y_ratio)
    }
}

fn valid_ratio(value: f32) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigWarning {
    CorruptPreserved(PathBuf),
    CorruptCouldNotBePreserved,
    UnsupportedNewerVersion(u32),
    ReadFailed(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigLoadOutcome {
    pub config: RuntimeConfig,
    pub warning: Option<ConfigWarning>,
    pub should_create_default: bool,
    /// False when an unreadable, unsupported, or unpreserved file still owns
    /// the canonical config path and must not be overwritten on close.
    pub persistence_allowed: bool,
}

pub fn load_config(path: &Path) -> Result<ConfigLoadOutcome, ConfigStorageError> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ConfigLoadOutcome {
                config: RuntimeConfig::default(),
                warning: None,
                should_create_default: true,
                persistence_allowed: true,
            });
        }
        Err(error) => {
            return Ok(ConfigLoadOutcome {
                config: RuntimeConfig::default(),
                warning: Some(ConfigWarning::ReadFailed(error.to_string())),
                should_create_default: false,
                persistence_allowed: false,
            });
        }
    };

    let parsed: RuntimeConfig = match toml::from_str(&source) {
        Ok(parsed) => parsed,
        Err(_) => return Ok(preserve_corrupt_config(path)),
    };

    if parsed.version > CONFIG_VERSION {
        return Ok(ConfigLoadOutcome {
            config: RuntimeConfig::default(),
            warning: Some(ConfigWarning::UnsupportedNewerVersion(parsed.version)),
            should_create_default: false,
            persistence_allowed: false,
        });
    }
    if parsed.version != CONFIG_VERSION {
        return Ok(preserve_corrupt_config(path));
    }
    if !parsed.is_semantically_valid() {
        return Ok(preserve_corrupt_config(path));
    }

    Ok(ConfigLoadOutcome {
        config: parsed,
        warning: None,
        should_create_default: false,
        persistence_allowed: true,
    })
}

fn preserve_corrupt_config(path: &Path) -> ConfigLoadOutcome {
    let preserved = corrupt_path(path);
    let warning = match fs::rename(path, &preserved) {
        Ok(()) => ConfigWarning::CorruptPreserved(preserved),
        Err(_) => ConfigWarning::CorruptCouldNotBePreserved,
    };
    let should_create_default = matches!(warning, ConfigWarning::CorruptPreserved(_));
    let persistence_allowed = should_create_default;
    ConfigLoadOutcome {
        config: RuntimeConfig::default(),
        warning: Some(warning),
        should_create_default,
        persistence_allowed,
    }
}

pub fn save_config(
    target: &Path,
    temporary: &Path,
    config: &RuntimeConfig,
) -> Result<(), ConfigStorageError> {
    let source = toml::to_string_pretty(config).map_err(ConfigStorageError::Serialize)?;
    atomic_publish(target, temporary, source.as_bytes()).map_err(ConfigStorageError::Publish)
}

fn corrupt_path(path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs());
    path.with_file_name(format!("config.invalid-{stamp}.toml"))
}

#[derive(Debug, Error)]
pub enum ConfigStorageError {
    #[error("cannot serialize config.toml: {0}")]
    Serialize(toml::ser::Error),
    #[error("cannot atomically publish config.toml: {0}")]
    Publish(AtomicPublishError),
}

#[cfg(test)]
mod phase8_config_runtime_tests {
    use super::*;

    fn unique_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "stickymd-config-{label}-{}-{nonce}",
            std::process::id()
        ))
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
}
