//! Typed window-preference coordination.
//!
//! plan_ref: docs/plan/03_system_architecture.md#flow-coordination

use crate::config::{ConfigCoordinator, ConfigRevisionExhausted, ContentZoomPercent, ThemeMode};
use crate::instruction::WindowPreferenceIntent;

/// Capability request produced after the sole config authority accepts an intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowPreferenceEffect {
    NoOp,
    ApplyTheme(ThemeMode),
    ApplyOpacity { opacity: u8, persist: bool },
    ApplyAlwaysOnTop(bool),
    ApplyContentZoom(ContentZoomPercent),
}

/// Applies one typed preference instruction to `ConfigCoordinator`.
pub fn coordinate_window_preference(
    config: &mut ConfigCoordinator,
    intent: WindowPreferenceIntent,
) -> Result<WindowPreferenceEffect, ConfigRevisionExhausted> {
    match intent {
        WindowPreferenceIntent::SetTheme(theme) => {
            if config.update(|candidate| candidate.theme = theme)? {
                Ok(WindowPreferenceEffect::ApplyTheme(theme))
            } else {
                Ok(WindowPreferenceEffect::NoOp)
            }
        }
        WindowPreferenceIntent::PreviewOpacity(opacity) => {
            Ok(WindowPreferenceEffect::ApplyOpacity {
                opacity: opacity.clamp(40, 100),
                persist: false,
            })
        }
        WindowPreferenceIntent::CommitOpacity(opacity) => {
            let opacity = opacity.clamp(40, 100);
            let persist = config.update(|candidate| candidate.opacity = opacity)?;
            Ok(WindowPreferenceEffect::ApplyOpacity { opacity, persist })
        }
        WindowPreferenceIntent::SetAlwaysOnTop(topmost) => {
            if config.update(|candidate| candidate.always_on_top = topmost)? {
                Ok(WindowPreferenceEffect::ApplyAlwaysOnTop(topmost))
            } else {
                Ok(WindowPreferenceEffect::NoOp)
            }
        }
        WindowPreferenceIntent::SetContentZoom(zoom) => {
            if config.update(|candidate| candidate.content_zoom_percent = zoom)? {
                Ok(WindowPreferenceEffect::ApplyContentZoom(zoom))
            } else {
                Ok(WindowPreferenceEffect::NoOp)
            }
        }
    }
}

#[cfg(test)]
mod phase8_preference_tests {
    use super::*;
    use crate::config::RuntimeConfig;
    use stickymd_core::{DocumentState, LineEnding};

    #[test]
    fn phase8_preview_opacity_is_clamped_without_mutating_config() {
        let mut config = ConfigCoordinator::loaded(RuntimeConfig::default());
        assert_eq!(
            coordinate_window_preference(&mut config, WindowPreferenceIntent::PreviewOpacity(12),)
                .unwrap(),
            WindowPreferenceEffect::ApplyOpacity {
                opacity: 40,
                persist: false,
            }
        );
        assert_eq!(config.current().opacity, 96);
        assert!(!config.is_dirty());
    }

    #[test]
    fn phase10_opacity_clamps_to_the_new_forty_percent_floor() {
        let mut config = ConfigCoordinator::loaded(RuntimeConfig::default());
        assert_eq!(
            coordinate_window_preference(&mut config, WindowPreferenceIntent::CommitOpacity(39),)
                .unwrap(),
            WindowPreferenceEffect::ApplyOpacity {
                opacity: 40,
                persist: true,
            }
        );
        assert_eq!(config.current().opacity, 40);
    }

    #[test]
    fn phase8_committed_preferences_have_one_config_authority() {
        let document = DocumentState::loaded("canonical", LineEnding::Lf, None);
        let document_generation = document.generation();
        let mut config = ConfigCoordinator::loaded(RuntimeConfig::default());
        assert_eq!(
            coordinate_window_preference(
                &mut config,
                WindowPreferenceIntent::SetTheme(ThemeMode::Dark),
            )
            .unwrap(),
            WindowPreferenceEffect::ApplyTheme(ThemeMode::Dark)
        );
        assert_eq!(config.current().theme, ThemeMode::Dark);
        assert!(config.is_dirty());

        assert_eq!(
            coordinate_window_preference(
                &mut config,
                WindowPreferenceIntent::SetTheme(ThemeMode::Dark),
            )
            .unwrap(),
            WindowPreferenceEffect::NoOp
        );
        assert_eq!(document.generation(), document_generation);
        assert_eq!(document.text(), "canonical");
    }

    #[test]
    fn phase10_content_zoom_uses_the_config_authority() {
        let document = DocumentState::loaded("canonical", LineEnding::Lf, None);
        let generation = document.generation();
        let saved_generation = document.saved_generation();
        let dirty = document.is_dirty();
        let can_undo = document.can_undo();
        let mut config = ConfigCoordinator::loaded(RuntimeConfig::default());
        let zoom = ContentZoomPercent::new_clamped(130);
        assert_eq!(
            coordinate_window_preference(
                &mut config,
                WindowPreferenceIntent::SetContentZoom(zoom),
            )
            .unwrap(),
            WindowPreferenceEffect::ApplyContentZoom(zoom)
        );
        assert_eq!(config.current().content_zoom_percent, zoom);
        assert_eq!(document.generation(), generation);
        assert_eq!(document.saved_generation(), saved_generation);
        assert_eq!(document.is_dirty(), dirty);
        assert_eq!(document.can_undo(), can_undo);
    }
}
