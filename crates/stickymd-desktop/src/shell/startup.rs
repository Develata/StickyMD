//! The window's startup once the window and its surface exist, and the one way a startup ends
//! early. What each failure means is `stickymd_app::flow::startup`.
//!
//! plan_ref: docs/plan/09_windows_shell.md#windows-shell-failure-paths

use winit::window::Window;

use super::DesktopShell;
use crate::host::{Host, StartupDetail};
use stickymd_app::config::ViewMode;
use stickymd_app::flow::startup::{StartupDegraded, StartupStop};
use stickymd_app::flow::window::state::{WindowIntent, effective_topmost};

impl<H: Host> DesktopShell<H> {
    /// Ends a startup that cannot go on. The tray goes before the notice, so that no icon offers
    /// commands while the notice waits for the user.
    pub(super) fn abort_startup(&mut self, stop: &StartupStop, exit: impl FnOnce()) {
        let message = stop.to_string();
        eprintln!("{message}");
        self.host.destroy();
        self.host.show_error("StickyMD", &message);
        self.startup_failed = true;
        exit();
    }

    /// Goes on without something the note can do without. A diagnostic already there stays: it
    /// comes from loading the note, which matters more than how the window looks.
    fn startup_degraded(&mut self, degraded: StartupDegraded) {
        let message = degraded.to_string();
        if degraded.needs_dialog() {
            self.host.show_error("StickyMD", &message);
        }
        self.diagnostic.get_or_insert(message);
    }

    /// `window` is passed in because `self` is borrowed mutably throughout; the two calls on it
    /// are what the windowless tests cannot make.
    pub(super) fn initialize_window_shell(&mut self, window: &Window) -> Result<(), StartupStop> {
        self.prepare_window_shell()?;
        self.host.detail(StartupDetail::WindowShowBegin);
        window.set_visible(true);
        self.host.detail(StartupDetail::WindowShowEnd);
        self.settle_shown_window()?;
        window.focus_window();
        self.host.milestone("focus_ready");
        self.refresh_window_guards();
        self.host.milestone("guards_ready");
        self.host.milestone("shell_ready");
        Ok(())
    }

    /// Everything before the window is shown.
    pub(super) fn prepare_window_shell(&mut self) -> Result<(), StartupStop> {
        let monitors = self
            .host
            .snapshot()
            .map(|snapshot| snapshot.displays)
            .unwrap_or_default();
        let Some(monitor) = self.app.startup_display(&monitors) else {
            return Err(StartupStop::NoDisplay);
        };
        self.host.milestone("monitor_ready");
        let tray_available = match self.host.create(self.tray_view, self.sink.clone()) {
            Ok(()) => true,
            Err(error) => {
                self.startup_degraded(StartupDegraded::Tray(error));
                false
            }
        };
        if tray_available {
            self.host
                .reassert_identity()
                .map_err(|cause| StartupStop::ToolIdentity {
                    shown: false,
                    cause,
                })?;
        } else {
            // A tool-window-only lifecycle without a tray recovery path is
            // forbidden. Restore ordinary taskbar reachability before show.
            self.host.restore_ordinary_entry();
        }
        let frame = self.app.start_window_flow(monitor, tray_available);
        let _ = self.host.apply_frame(frame);
        self.host.milestone("tray_ready");
        self.host.detail(StartupDetail::SplitModeBegin);
        self.dispatch_window_intent(WindowIntent::SplitModeChanged {
            split: self.app.config().current().view_mode == ViewMode::Split,
        });
        self.host.detail(StartupDetail::SplitModeEnd);
        Ok(())
    }

    /// Everything after the window is shown that can still fail.
    pub(super) fn settle_shown_window(&mut self) -> Result<(), StartupStop> {
        self.host.detail(StartupDetail::ToolIdentityBegin);
        self.host
            .reassert_identity()
            .map_err(|cause| StartupStop::ToolIdentity { shown: true, cause })?;
        self.host.detail(StartupDetail::ToolIdentityEnd);
        self.host.milestone("window_visible");
        // Winit may refresh native extended styles while making the window
        // visible. Apply layered alpha and z-order after that transition so
        // the configured projection is the final native fact.
        if let Err(error) = self.host.set_opacity(self.app.config().current().opacity) {
            self.startup_degraded(StartupDegraded::Opacity(error));
        }
        self.host.milestone("opacity_ready");
        self.apply_startup_topmost();
        self.host.milestone("topmost_ready");
        Ok(())
    }

    /// Applies the configured value with no receipt: a read-back of any kind is not a failure.
    pub(super) fn apply_startup_topmost(&mut self) {
        let effective = effective_topmost(
            self.app.config().current().always_on_top,
            self.app
                .window_flow()
                .as_ref()
                .is_some_and(|flow| flow.state().temporary_sensor_topmost()),
        );
        if let Err(error) = self.host.set_topmost(effective) {
            self.app.startup_topmost_failed();
            self.startup_degraded(StartupDegraded::Topmost(error));
        }
    }
}
