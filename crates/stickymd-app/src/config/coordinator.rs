//! Single-owner runtime configuration coordination.
//!
//! plan_ref: docs/plan/05_document_persistence.md#config-persistence

use super::RuntimeConfig;

/// Monotonic identity for a committed runtime configuration projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ConfigRevision(u64);

impl ConfigRevision {
    pub const fn initial() -> Self {
        Self(0)
    }

    fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

/// Immutable request consumed by the single I/O worker.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigPersistRequest {
    pub revision: ConfigRevision,
    pub config: RuntimeConfig,
}

/// Owns the only mutable `RuntimeConfig` and serializes durable acknowledgements.
///
/// Window/UI adapters receive projections and typed mutations. They never own a
/// second writable copy of the configured preferences.
#[derive(Debug)]
pub struct ConfigCoordinator {
    current: RuntimeConfig,
    revision: ConfigRevision,
    persisted_revision: ConfigRevision,
    in_flight: Option<ConfigRevision>,
}

impl ConfigCoordinator {
    pub fn loaded(current: RuntimeConfig) -> Self {
        Self {
            current,
            revision: ConfigRevision::initial(),
            persisted_revision: ConfigRevision::initial(),
            in_flight: None,
        }
    }

    pub const fn current(&self) -> &RuntimeConfig {
        &self.current
    }

    #[cfg(test)]
    pub const fn revision(&self) -> ConfigRevision {
        self.revision
    }

    pub const fn is_dirty(&self) -> bool {
        self.revision.0 != self.persisted_revision.0
    }

    pub const fn is_saving(&self) -> bool {
        self.in_flight.is_some()
    }

    /// Commits a logical preference change. Equal projections are no-ops.
    pub fn update(
        &mut self,
        update: impl FnOnce(&mut RuntimeConfig),
    ) -> Result<bool, ConfigRevisionExhausted> {
        let mut candidate = self.current.clone();
        update(&mut candidate);
        if candidate == self.current {
            return Ok(false);
        }
        let next = self.revision.next().ok_or(ConfigRevisionExhausted)?;
        self.current = candidate;
        self.revision = next;
        Ok(true)
    }

    /// Starts at most one durable write. Changes made while it is in flight
    /// remain dirty and are submitted only after this revision is acknowledged.
    pub fn begin_persist(&mut self) -> Option<ConfigPersistRequest> {
        if !self.is_dirty() || self.in_flight.is_some() {
            return None;
        }
        self.in_flight = Some(self.revision);
        Some(ConfigPersistRequest {
            revision: self.revision,
            config: self.current.clone(),
        })
    }

    /// Applies only the matching in-flight receipt. A stale receipt cannot make
    /// a newer projection clean.
    pub fn acknowledge(&mut self, revision: ConfigRevision, succeeded: bool) -> ConfigAck {
        if self.in_flight != Some(revision) {
            return ConfigAck::Stale;
        }
        self.in_flight = None;
        if succeeded && revision > self.persisted_revision {
            self.persisted_revision = revision;
        }
        ConfigAck::Applied {
            needs_follow_up: self.is_dirty(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigAck {
    Applied { needs_follow_up: bool },
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConfigRevisionExhausted;

impl std::fmt::Display for ConfigRevisionExhausted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("configuration revision exhausted")
    }
}

impl std::error::Error for ConfigRevisionExhausted {}

#[cfg(test)]
mod phase8_config_tests {
    use super::*;
    use stickymd_core::{DocumentState, LineEnding};

    #[test]
    fn phase8_config_equal_update_is_a_no_op() {
        let mut coordinator = ConfigCoordinator::loaded(RuntimeConfig::default());
        assert!(!coordinator.update(|_| {}).unwrap());
        assert_eq!(coordinator.revision(), ConfigRevision::initial());
        assert!(!coordinator.is_dirty());
    }

    #[test]
    fn phase8_config_in_flight_write_coalesces_to_latest_follow_up() {
        let mut coordinator = ConfigCoordinator::loaded(RuntimeConfig::default());
        coordinator.update(|config| config.opacity = 80).unwrap();
        let first = coordinator.begin_persist().unwrap();
        coordinator.update(|config| config.opacity = 85).unwrap();
        assert!(coordinator.begin_persist().is_none());
        assert_eq!(
            coordinator.acknowledge(first.revision, true),
            ConfigAck::Applied {
                needs_follow_up: true
            }
        );
        let latest = coordinator.begin_persist().unwrap();
        assert_eq!(latest.config.opacity, 85);
        assert!(latest.revision > first.revision);
    }

    #[test]
    fn phase8_config_failed_or_stale_ack_never_clears_dirty() {
        let mut coordinator = ConfigCoordinator::loaded(RuntimeConfig::default());
        coordinator
            .update(|config| config.always_on_top = true)
            .unwrap();
        let request = coordinator.begin_persist().unwrap();
        assert_eq!(
            coordinator.acknowledge(ConfigRevision::initial(), true),
            ConfigAck::Stale
        );
        assert!(coordinator.is_saving());
        assert_eq!(
            coordinator.acknowledge(request.revision, false),
            ConfigAck::Applied {
                needs_follow_up: true
            }
        );
        assert!(coordinator.is_dirty());
    }

    #[test]
    fn phase8_preference_updates_cannot_change_document_generation() {
        let document = DocumentState::loaded("canonical", LineEnding::Lf, None);
        let generation = document.generation();
        let mut coordinator = ConfigCoordinator::loaded(RuntimeConfig::default());
        coordinator
            .update(|config| {
                config.theme = crate::config::ThemeMode::Dark;
                config.opacity = 70;
                config.always_on_top = true;
            })
            .unwrap();
        assert_eq!(document.generation(), generation);
        assert_eq!(document.text(), "canonical");
    }
}
