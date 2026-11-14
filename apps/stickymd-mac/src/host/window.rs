//! The native window: visibility, z-order, opacity and frame.
//!
//! plan_ref: docs/plan/09_windows_shell.md#macos-adapter-mapping

use std::sync::Arc;

use stickymd_app::flow::window::geometry::PhysicalRect;
use stickymd_app::ports::{
    FailureKind, FrameFact, HostFailure, TopmostFact, Visibility, WindowHost,
};
use winit::window::Window;

use super::{MacHost, failed};
use crate::platform::macos::window::{
    ABOVE_MENU_BAR_LEVEL, FLOATING_LEVEL, NORMAL_LEVEL, level, set_alpha, set_frame, set_level,
    use_regular_activation_policy,
};

fn visibility_of(window: &Window) -> Visibility {
    Visibility {
        visible: window.is_visible(),
        minimized: window.is_minimized(),
    }
}

impl MacHost {
    fn existing_window(&self) -> Result<&Arc<Window>, FailureKind> {
        self.window.as_ref().ok_or(FailureKind::Unavailable)
    }

    /// Whether the frame rises into the bar at the top of its display, where it has to be above
    /// the menu bar to be seen: a top-docked strip and the animation toward it.
    fn over_the_menu_bar(&self, window: &Window, frame: PhysicalRect) -> bool {
        let Some(monitor) = window
            .current_monitor()
            .or_else(|| window.primary_monitor())
        else {
            return false;
        };
        let Some(insets) = crate::platform::macos::monitor::display_insets(&monitor) else {
            return false;
        };
        let work_top = f64::from(monitor.position().y) + insets.top * monitor.scale_factor();
        f64::from(frame.y) < work_top - 0.5
    }

    fn apply_level(&self, window: &Window, frame: PhysicalRect) {
        let wanted = if self.over_the_menu_bar(window, frame) {
            ABOVE_MENU_BAR_LEVEL
        } else if self.wants_topmost {
            FLOATING_LEVEL
        } else {
            NORMAL_LEVEL
        };
        if level(window) != Some(wanted) {
            set_level(window, wanted);
        }
    }

    fn current_frame(window: &Window) -> Option<PhysicalRect> {
        let position = window.outer_position().ok()?;
        let size = window.outer_size();
        Some(PhysicalRect::new(
            position.x,
            position.y,
            size.width,
            size.height,
        ))
    }
}

impl WindowHost for MacHost {
    fn show(&mut self) -> Result<Visibility, FailureKind> {
        let window = self.existing_window()?;
        window.set_visible(true);
        window.set_minimized(false);
        Ok(visibility_of(window))
    }

    fn hide(&mut self) -> Result<(), FailureKind> {
        let window = self.existing_window()?;
        window.set_visible(false);
        match window.is_visible() {
            Some(false) => Ok(()),
            Some(true) => Err(FailureKind::Failed),
            None => Err(FailureKind::Unsupported),
        }
    }

    fn focus(&mut self) -> Result<Visibility, FailureKind> {
        let window = self.existing_window()?;
        window.set_minimized(false);
        window.focus_window();
        Ok(visibility_of(window))
    }

    fn visibility(&self) -> Result<Visibility, FailureKind> {
        self.existing_window().map(|window| visibility_of(window))
    }

    fn set_topmost(&mut self, on: bool) -> Result<TopmostFact, HostFailure> {
        self.wants_topmost = on;
        let Some(window) = self.window.clone() else {
            return Ok(TopmostFact { topmost: None });
        };
        let Some(frame) = Self::current_frame(&window) else {
            return Err(failed("the window has no position"));
        };
        self.apply_level(&window, frame);
        // Over the menu bar the level says nothing about the choice, which still stands.
        let topmost = if self.over_the_menu_bar(&window, frame) {
            on
        } else {
            level(&window).is_some_and(|level| level != NORMAL_LEVEL)
        };
        Ok(TopmostFact {
            topmost: Some(topmost),
        })
    }

    fn set_opacity(&mut self, percent: u8) -> Result<(), HostFailure> {
        if let Some(window) = &self.window
            && !set_alpha(window, percent)
        {
            return Err(failed("the window has no native handle"));
        }
        Ok(())
    }

    fn apply_frame(&mut self, target: PhysicalRect) -> Result<(), FailureKind> {
        if let Some(window) = self.window.clone() {
            self.apply_level(&window, target);
            if !set_frame(&window, target) {
                return Err(FailureKind::Unavailable);
            }
        }
        Ok(())
    }

    fn observe_frame(&self) -> Result<FrameFact, FailureKind> {
        let window = self.existing_window()?;
        Self::current_frame(window)
            .map(|outer| FrameFact { outer })
            .ok_or(FailureKind::Unsupported)
    }

    fn restore_ordinary_entry(&mut self) {
        use_regular_activation_policy();
    }
}
