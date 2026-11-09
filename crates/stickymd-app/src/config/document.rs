//! The text of `config.toml` and what to do about it: parsing, the version and validity verdict,
//! serialization, the name a preserved file takes and the outcome a host reports. Reading the file
//! and moving it aside stay with the host.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

use std::path::PathBuf;

use super::{CONFIG_VERSION, RuntimeConfig};

pub type ConfigSerializeError = toml::ser::Error;

/// How many numbered names a preserved file tries after its plain one.
pub const PRESERVED_CONFIG_SUFFIXES: u16 = 1_024;

/// The name a corrupt `config.toml` is moved to. `sequence` 0 is the plain name; the host tries
/// each with a move that never replaces, which is what decides.
pub fn preserved_config_name(stamp: u64, sequence: u16) -> String {
    if sequence == 0 {
        format!("config.invalid-{stamp}.toml")
    } else {
        format!("config.invalid-{stamp}-{sequence}.toml")
    }
}

pub fn config_text(config: &RuntimeConfig) -> Result<String, ConfigSerializeError> {
    toml::to_string_pretty(config)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfigWarning {
    CorruptPreserved(PathBuf),
    CorruptCouldNotBePreserved,
    UnsupportedNewerVersion(u32),
    ReadFailed(String),
}

impl ConfigWarning {
    /// What the status line says.
    pub fn message(&self) -> String {
        match self {
            Self::CorruptPreserved(_) => {
                "config.toml 已损坏；原文件已保留，当前使用默认配置。".into()
            }
            Self::CorruptCouldNotBePreserved => {
                "config.toml 已损坏且无法改名保留；不会覆盖它，当前使用默认配置。".into()
            }
            Self::UnsupportedNewerVersion(version) => {
                format!("config.toml 版本 {version} 高于当前程序；文件已保留，当前使用默认配置。")
            }
            Self::ReadFailed(error) => {
                format!("config.toml 无法读取；不会覆盖它，当前使用默认配置：{error}")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ConfigLoadOutcome {
    pub config: RuntimeConfig,
    pub warning: Option<ConfigWarning>,
    pub should_create_default: bool,
    /// False when an unreadable, unsupported, or unpreserved file still owns the canonical config
    /// path and must not be overwritten on close.
    pub persistence_allowed: bool,
}

impl ConfigLoadOutcome {
    /// There is no file yet.
    pub fn absent() -> Self {
        Self {
            config: RuntimeConfig::default(),
            warning: None,
            should_create_default: true,
            persistence_allowed: true,
        }
    }

    /// The file exists and cannot be read, so it must not be replaced.
    pub fn read_failed(reason: String) -> Self {
        Self {
            config: RuntimeConfig::default(),
            warning: Some(ConfigWarning::ReadFailed(reason)),
            should_create_default: false,
            persistence_allowed: false,
        }
    }

    /// The verdict on the file's text; `None` is a corrupt file the host must move aside.
    pub fn from_text(source: &str) -> Option<Self> {
        let parsed: RuntimeConfig = toml::from_str(source).ok()?;
        if parsed.version > CONFIG_VERSION {
            return Some(Self {
                config: RuntimeConfig::default(),
                warning: Some(ConfigWarning::UnsupportedNewerVersion(parsed.version)),
                should_create_default: false,
                persistence_allowed: false,
            });
        }
        if parsed.version != CONFIG_VERSION || !parsed.is_semantically_valid() {
            return None;
        }
        Some(Self {
            config: parsed,
            warning: None,
            should_create_default: false,
            persistence_allowed: true,
        })
    }

    /// A corrupt file was moved to `preserved`, or could not be (`None`), in which case it still
    /// owns the path.
    pub fn after_preservation(preserved: Option<PathBuf>) -> Self {
        let warning = preserved.map_or(
            ConfigWarning::CorruptCouldNotBePreserved,
            ConfigWarning::CorruptPreserved,
        );
        let moved_aside = matches!(warning, ConfigWarning::CorruptPreserved(_));
        Self {
            config: RuntimeConfig::default(),
            warning: Some(warning),
            should_create_default: moved_aside,
            persistence_allowed: moved_aside,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_with_version(version: u32) -> String {
        config_text(&RuntimeConfig {
            version,
            ..RuntimeConfig::default()
        })
        .unwrap()
    }

    #[test]
    fn a_missing_file_is_created_and_an_unreadable_one_is_left_alone() {
        let absent = ConfigLoadOutcome::absent();
        assert!(absent.should_create_default && absent.persistence_allowed);
        assert_eq!(absent.warning, None);

        let unreadable = ConfigLoadOutcome::read_failed("denied".into());
        assert!(!unreadable.should_create_default && !unreadable.persistence_allowed);
        assert_eq!(
            unreadable.warning,
            Some(ConfigWarning::ReadFailed("denied".into()))
        );
    }

    #[test]
    fn the_current_version_is_used_as_written() {
        let outcome = ConfigLoadOutcome::from_text(&text_with_version(CONFIG_VERSION)).unwrap();
        assert_eq!(outcome.config, RuntimeConfig::default());
        assert_eq!(outcome.warning, None);
        assert!(outcome.persistence_allowed && !outcome.should_create_default);
    }

    #[test]
    fn a_newer_version_keeps_its_file_and_runs_on_defaults() {
        let newer = CONFIG_VERSION + 1;
        let outcome = ConfigLoadOutcome::from_text(&text_with_version(newer)).unwrap();
        assert_eq!(
            outcome.warning,
            Some(ConfigWarning::UnsupportedNewerVersion(newer))
        );
        assert!(!outcome.persistence_allowed && !outcome.should_create_default);
    }

    #[test]
    fn text_that_is_not_a_current_configuration_is_corrupt() {
        assert_eq!(ConfigLoadOutcome::from_text("not = [valid"), None);
        assert_eq!(
            ConfigLoadOutcome::from_text(&text_with_version(CONFIG_VERSION - 1)),
            None
        );
    }

    #[test]
    fn only_a_file_moved_aside_may_be_replaced() {
        let moved = ConfigLoadOutcome::after_preservation(Some("kept.toml".into()));
        assert!(moved.should_create_default && moved.persistence_allowed);
        assert_eq!(
            moved.warning,
            Some(ConfigWarning::CorruptPreserved("kept.toml".into()))
        );

        let stuck = ConfigLoadOutcome::after_preservation(None);
        assert!(!stuck.should_create_default && !stuck.persistence_allowed);
        assert_eq!(
            stuck.warning,
            Some(ConfigWarning::CorruptCouldNotBePreserved)
        );
    }

    #[test]
    fn preserved_names_are_numbered_after_the_plain_one() {
        assert_eq!(preserved_config_name(7, 0), "config.invalid-7.toml");
        assert_eq!(preserved_config_name(7, 3), "config.invalid-7-3.toml");
    }

    #[test]
    fn every_warning_says_what_happened_to_the_file() {
        for (warning, expected) in [
            (
                ConfigWarning::CorruptPreserved("x".into()),
                "config.toml 已损坏；原文件已保留，当前使用默认配置。",
            ),
            (
                ConfigWarning::CorruptCouldNotBePreserved,
                "config.toml 已损坏且无法改名保留；不会覆盖它，当前使用默认配置。",
            ),
            (
                ConfigWarning::UnsupportedNewerVersion(9),
                "config.toml 版本 9 高于当前程序；文件已保留，当前使用默认配置。",
            ),
            (
                ConfigWarning::ReadFailed("denied".into()),
                "config.toml 无法读取；不会覆盖它，当前使用默认配置：denied",
            ),
        ] {
            assert_eq!(warning.message(), expected);
        }
    }
}
