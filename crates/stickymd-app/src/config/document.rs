//! The text of `config.toml` and what to do about it: parsing, the version and validity verdict,
//! serialization, the name a preserved file takes and the outcome a host reports. Reading the file
//! and moving it aside stay with the host.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

use std::io;
use std::path::{Path, PathBuf};

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

/// Decides what a host's read means. Only corrupt text invokes preservation; missing,
/// unreadable and newer-version files keep their existing outcomes without a move attempt.
pub fn load_config_from(
    read: io::Result<String>,
    preserve: impl FnOnce() -> Option<PathBuf>,
) -> ConfigLoadOutcome {
    let source = match read {
        Ok(source) => source,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return ConfigLoadOutcome::absent();
        }
        Err(error) => return ConfigLoadOutcome::read_failed(error.to_string()),
    };
    ConfigLoadOutcome::from_text(&source)
        .unwrap_or_else(|| ConfigLoadOutcome::after_preservation(preserve()))
}

/// Tries bounded preservation names using the host's move that never replaces a destination.
pub fn preserve_with(
    path: &Path,
    stamp: u64,
    mut move_no_replace: impl FnMut(&Path, &Path) -> io::Result<()>,
) -> io::Result<PathBuf> {
    // The no-replace move is the authority gate. A preliminary exists check
    // would be racy and could overwrite older evidence on Windows.
    for sequence in 0..=PRESERVED_CONFIG_SUFFIXES {
        let candidate = path.with_file_name(preserved_config_name(stamp, sequence));
        match move_no_replace(path, &candidate) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "all corrupt config preservation names are occupied",
    ))
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
    fn load_preserves_neither_missing_unreadable_valid_nor_newer_files() {
        let partial = "version = 1\nopacity = 85\n";
        let newer = text_with_version(CONFIG_VERSION + 1);
        for (read, expected) in [
            (
                Err(io::Error::from(io::ErrorKind::NotFound)),
                ConfigLoadOutcome::absent(),
            ),
            (
                Err(io::Error::new(io::ErrorKind::PermissionDenied, "denied")),
                ConfigLoadOutcome::read_failed("denied".into()),
            ),
            (
                Ok(partial.into()),
                ConfigLoadOutcome::from_text(partial).unwrap(),
            ),
            (
                Ok(newer.clone()),
                ConfigLoadOutcome::from_text(&newer).unwrap(),
            ),
        ] {
            assert_eq!(
                load_config_from(read, || panic!("this file must not be moved")),
                expected
            );
        }
    }

    #[test]
    fn load_preserves_corrupt_text_once_and_reports_whether_it_moved() {
        for source in [
            "not = [valid",
            "version = 0\n",
            "version = 1\ntheme = \"purple\"\n",
            "version = 1\nopacity = 1\n",
        ] {
            for preserved in [Some(PathBuf::from("kept.toml")), None] {
                let expected = ConfigLoadOutcome::after_preservation(preserved.clone());
                let mut calls = 0;
                let outcome = load_config_from(Ok(source.into()), || {
                    calls += 1;
                    preserved
                });
                assert_eq!(calls, 1);
                assert_eq!(outcome, expected);
            }
        }
    }

    #[test]
    fn preservation_tries_the_plain_name_first_and_stops_on_success() {
        let path = Path::new("note/config.toml");
        for collisions in [0, 2, PRESERVED_CONFIG_SUFFIXES] {
            let mut calls = 0;
            let preserved = preserve_with(path, 42, |source, candidate| {
                assert_eq!(source, path);
                assert_eq!(
                    candidate,
                    path.with_file_name(preserved_config_name(42, calls))
                );
                let sequence = calls;
                calls += 1;
                if sequence < collisions {
                    Err(io::ErrorKind::AlreadyExists.into())
                } else {
                    Ok(())
                }
            })
            .unwrap();
            assert_eq!(calls, collisions + 1);
            assert_eq!(
                preserved,
                path.with_file_name(preserved_config_name(42, collisions))
            );
        }
    }

    #[test]
    fn preservation_stops_on_other_errors_and_keeps_the_original_error() {
        let mut calls = 0;
        let error = preserve_with(Path::new("config.toml"), 42, |_, _| {
            calls += 1;
            Err(io::Error::from_raw_os_error(13))
        })
        .unwrap_err();
        assert_eq!(calls, 1);
        assert_eq!(error.raw_os_error(), Some(13));
    }

    #[test]
    fn preservation_exhaustion_is_bounded_and_keeps_the_error_text() {
        let path = Path::new("note/config.toml");
        let mut calls = 0;
        let error = preserve_with(path, 42, |source, candidate| {
            assert_eq!(source, path);
            assert_eq!(
                candidate,
                path.with_file_name(preserved_config_name(42, calls))
            );
            calls += 1;
            Err(io::ErrorKind::AlreadyExists.into())
        })
        .unwrap_err();
        assert_eq!(calls, PRESERVED_CONFIG_SUFFIXES + 1);
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert_eq!(
            error.to_string(),
            "all corrupt config preservation names are occupied"
        );
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
