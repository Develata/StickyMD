//! The native window: visibility, z-order, opacity, frame and the tool-window identity.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-adapter-mapping
//! plan_ref: docs/plan/09_windows_shell.md#tool-window-identity

use std::sync::Arc;

use stickymd_app::flow::window::geometry::PhysicalRect;
use stickymd_app::ports::{
    FailureKind, FrameFact, HostFailure, TopmostFact, Visibility, WindowHost,
};
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::platform::windows::WindowExtWindows;
use winit::window::Window;

use super::{WinHost, failed};
use crate::platform::windows::tool_window::apply_tool_window_identity;
use crate::platform::windows::window_opacity::set_window_opacity;
use crate::platform::windows::window_topmost::set_window_topmost_no_activate;

impl WinHost {
    fn existing_window(&self) -> Result<&Arc<Window>, FailureKind> {
        self.window.as_ref().ok_or(FailureKind::Unavailable)
    }
}

fn visibility_of(window: &Window) -> Visibility {
    Visibility {
        visible: window.is_visible(),
        minimized: window.is_minimized(),
    }
}

impl WindowHost for WinHost {
    fn show(&mut self) -> Result<Visibility, FailureKind> {
        let window = self.existing_window()?;
        window.set_visible(true);
        window.set_minimized(false);
        Ok(visibility_of(window))
    }

    /// winit hides synchronously on the event loop thread and gives no result, so the window is
    /// asked afterwards.
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
        let Some(window) = &self.window else {
            return Ok(TopmostFact { topmost: None });
        };
        let topmost = set_window_topmost_no_activate(window.as_ref(), on).map_err(failed)?;
        Ok(TopmostFact { topmost })
    }

    fn set_opacity(&mut self, percent: u8) -> Result<(), HostFailure> {
        if let Some(window) = &self.window {
            set_window_opacity(window.as_ref(), percent).map_err(failed)?;
        }
        Ok(())
    }

    fn apply_frame(&mut self, target: PhysicalRect) -> Result<(), FailureKind> {
        if let Some(window) = &self.window {
            window.set_outer_position(PhysicalPosition::new(target.x, target.y));
            let _ = window.request_inner_size(PhysicalSize::new(target.width, target.height));
        }
        Ok(())
    }

    fn observe_frame(&self) -> Result<FrameFact, FailureKind> {
        let window = self.existing_window()?;
        let position = window
            .outer_position()
            .map_err(|_| FailureKind::Unsupported)?;
        let size = window.outer_size();
        Ok(FrameFact {
            outer: PhysicalRect::new(position.x, position.y, size.width, size.height),
        })
    }

    /// Only a tray-owned window has the identity: without the tray it keeps its taskbar entry.
    fn reassert_identity(&mut self) -> Result<(), HostFailure> {
        match (&self.tray, &self.window) {
            (Some(_), Some(window)) => apply_tool_window_identity(window.as_ref()).map_err(failed),
            _ => Ok(()),
        }
    }

    fn restore_ordinary_entry(&mut self) {
        if let Some(window) = &self.window {
            window.set_skip_taskbar(false);
        }
    }
}
