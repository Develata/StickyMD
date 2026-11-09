//! Projects monitor facts and the end of a window move into window-domain geometry.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-purpose

use super::DesktopShell;
use crate::host::Host;
use stickymd_app::flow::window::geometry::PhysicalRect;
use stickymd_app::flow::window::state::WindowIntent;
use stickymd_app::interaction::rules::window_display;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;

/// A native event that moves the window or changes what it is measured against.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum GeometryEvent {
    Moved,
    Resized(PhysicalSize<u32>),
    ScaleChanged,
}

/// The geometry part of a window event, and the only place these events are told apart.
pub(super) fn geometry_event(event: &WindowEvent) -> Option<GeometryEvent> {
    match event {
        WindowEvent::Moved(_) => Some(GeometryEvent::Moved),
        WindowEvent::Resized(size) => Some(GeometryEvent::Resized(*size)),
        WindowEvent::ScaleFactorChanged { .. } => Some(GeometryEvent::ScaleChanged),
        _ => None,
    }
}

impl<H: Host> DesktopShell<H> {
    pub(super) fn on_window_geometry(&mut self, event: GeometryEvent) {
        match event {
            GeometryEvent::Moved => self.observe_pending_placement(),
            GeometryEvent::Resized(size) => {
                self.resize(size);
                self.observe_pending_placement();
            }
            GeometryEvent::ScaleChanged => {
                if let Some(window) = &self.window {
                    self.resize(window.inner_size());
                }
                // winit raises this from inside WM_DPICHANGED and applies its own SetWindowPos
                // after the callback returns, so a frame read now can be moved right after it
                // is confirmed. Recovery and observation wait for the next event turn.
                self.display_recovery_due = true;
            }
        }
    }

    /// The display change that `GeometryEvent::ScaleChanged` put off, if there is one.
    pub(super) fn run_deferred_display_recovery(&mut self) {
        if std::mem::take(&mut self.display_recovery_due) {
            self.recover_display_topology();
            self.observe_pending_placement();
        }
    }

    pub(super) fn complete_window_drag(&mut self) {
        if !self.move_resize_active {
            return;
        }
        self.move_resize_active = false;
        self.dispatch_window_intent(WindowIntent::MoveResizeEnded);
        let completed = self.window.as_ref().and_then(|window| {
            let position = window.outer_position().ok()?;
            let size = window.outer_size();
            let frame = PhysicalRect::new(position.x, position.y, size.width, size.height);
            let snapshot = self.host.snapshot().ok()?;
            let monitors = snapshot.displays;
            let monitor = snapshot
                .current
                .and_then(|identity| {
                    monitors
                        .iter()
                        .find(|candidate| candidate.identity == identity)
                        .cloned()
                })
                .or_else(|| window_display(&monitors, ""))?;
            Some((frame, monitor))
        });
        if let Some((frame, monitor)) = completed {
            self.dispatch_window_intent(WindowIntent::DragEnded {
                frame,
                monitor,
                now_ms: self.timestamp_ms(),
            });
        }
        // Native geometry queries may transiently fail during topology churn.
        // The drag guard is a separate fact and must always be released.
        self.refresh_window_guards();
    }

    pub(super) fn recover_display_topology(&mut self) {
        let Ok(snapshot) = self.host.snapshot() else {
            return;
        };
        self.dispatch_window_intent(WindowIntent::DisplayTopologyChanged {
            monitors: snapshot.displays,
            now_ms: self.timestamp_ms(),
        });
    }

    pub(super) fn window_accepts_editor_mutation(&self) -> bool {
        self.shell_input_enabled && self.app.window_accepts_mutation()
    }
}
